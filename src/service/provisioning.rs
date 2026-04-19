use std::env;

use chrono::{Duration, Utc};
use diesel::Connection;
use uuid::Uuid;

use crate::{
    dao::{
        DbPool,
        group::{GroupDao, PgGroupDao},
        location::{LocationDao, PgLocationDao},
        pending_registration::{PendingRegistrationDao, PgPendingRegistrationDao},
        producer::{PgProducerDao, ProducerDao},
    },
    helpers::crypto_password,
    interface::types::provisioning::{
        CancelPendingProvisioningResponse, ConfirmPendingProvisioningRequest,
        ConfirmPendingProvisioningResponse, CreatePendingProvisioningRequest,
        CreatePendingProvisioningResponse, ListPendingProvisioningResponse,
        PendingProvisioningView, ProvisioningWifiBundle,
    },
    model::models::{
        NewMqttBrokerAuth, NewPendingProducerRegistration, NewProducer,
        PendingProducerRegistration, PendingProducerRegistrationStatus,
    },
    service::ServiceError,
};

const DEFAULT_PENDING_TTL_MINUTES: i64 = 15;
// TODO: consider extracting MAX_FIRMWARE_NAME_LEN and contains_mqtt_topic_meta into
// a shared helper module so provisioning and group services reuse the same
// validation logic and messages.
const MAX_FIRMWARE_NAME_LEN: usize = 32;

pub struct ProvisioningService<Pd, Gd, Ld, Prd>
where
    Pd: PendingRegistrationDao,
    Gd: GroupDao,
    Ld: LocationDao,
    Prd: ProducerDao,
{
    pending_dao: Pd,
    group_dao: Gd,
    location_dao: Ld,
    producer_dao: Prd,
}

impl<Pd, Gd, Ld, Prd> ProvisioningService<Pd, Gd, Ld, Prd>
where
    Pd: PendingRegistrationDao,
    Gd: GroupDao,
    Ld: LocationDao,
    Prd: ProducerDao,
{
    pub fn new(pending_dao: Pd, group_dao: Gd, location_dao: Ld, producer_dao: Prd) -> Self {
        Self {
            pending_dao,
            group_dao,
            location_dao,
            producer_dao,
        }
    }

    pub fn create_pending(
        &self,
        owner_user_id: Uuid,
        req: CreatePendingProvisioningRequest,
    ) -> Result<CreatePendingProvisioningResponse, ServiceError> {
        validate_name_sizes(&req)?;

        let group = self.group_dao.get_group(req.group_id)?;
        if group.owner_user_id != owner_user_id {
            return Err(ServiceError::Forbidden);
        }
        if group.name.is_empty() || group.name.len() > MAX_FIRMWARE_NAME_LEN {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(format!(
                    "group name must be 1..={MAX_FIRMWARE_NAME_LEN} characters for firmware compatibility"
                )),
            ));
        }
        if contains_mqtt_topic_meta(&group.name) {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(
                    "group name cannot include '/', '+' or '#'".to_string(),
                ),
            ));
        }

        let broker_host = env::var("MQTT_PROVISION_HOST")
            .or_else(|_| env::var("MQTT_HOST"))
            .unwrap_or_else(|_| "localhost".to_string());
        let broker_port: i16 = env::var("MQTT_PROVISION_PORT")
            .or_else(|_| env::var("MQTT_PORT"))
            .ok()
            .and_then(|v| v.parse::<i16>().ok())
            .unwrap_or(1883);

        let mqtt_username = format!(
            "dev-{}",
            crypto_password::random_token_hex(8).map_err(|e| {
                ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                    "OpenSSL token generation error: {e}"
                )))
            })?
        );
        let mqtt_password_plain = crypto_password::random_token_hex(16).map_err(|e| {
            ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                "OpenSSL token generation error: {e}"
            )))
        })?;
        let mqtt_password_hash =
            crypto_password::hash_password(&mqtt_password_plain).map_err(|e| {
                ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                    "OpenSSL password hash error: {e}"
                )))
            })?;

        let group_name = group.name.clone();
        let publish_topic = req.data_template_id.map(|template_id| {
            format!(
                "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
                mqtt_username, group_name, req.producer_name, template_id
            )
        });
        let mqtt_topic = publish_topic
            .clone()
            .unwrap_or_else(|| "/v1/forbidden".to_string());

        let mqtt_subscribe_topic = format!(
            "/v1/users/{}/groups/{}/forwarder_peers/{}",
            mqtt_username, group_name, req.producer_name
        );

        let now = Utc::now();
        let expires_at = now + Duration::minutes(DEFAULT_PENDING_TTL_MINUTES);

        let new_pending = NewPendingProducerRegistration {
            owner_user_id,
            group_id: req.group_id,
            producer_id: None,
            status: PendingProducerRegistrationStatus::Pending as i16,
            expires_at,
            confirmed_at: None,
            cancelled_at: None,
            producer_name: req.producer_name,
            ble_name: req.ble_name,
            description: req.description,
            location_name: req.location.name,
            location_geo_lat: req.location.geo_lat,
            location_geo_long: req.location.geo_long,
            forwarder_id: req.forwarder_id,
            data_template_id: req.data_template_id,
            mqtt_username: mqtt_username.clone(),
            mqtt_password_hash,
            mqtt_topic: mqtt_topic.clone(),
            mqtt_subscribe_topic: mqtt_subscribe_topic.clone(),
            broker_host: broker_host.clone(),
            broker_port,
            x25519_pub_key_bytes_base64: None,
            lora_addr: None,
        };

        let created = self.pending_dao.create(new_pending)?;
        let pending = pending_view_from_model(&created);

        Ok(CreatePendingProvisioningResponse {
            pending,
            wifi_bundle: ProvisioningWifiBundle {
                broker_host,
                broker_port,
                mqtt_username,
                mqtt_password: mqtt_password_plain,
                publish_topic,
                subscribe_topic: mqtt_subscribe_topic,
            },
        })
    }

    pub fn list_pending_for_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<ListPendingProvisioningResponse, ServiceError> {
        let pending = self.pending_dao.get_pending_for_owner(owner_user_id)?;
        Ok(ListPendingProvisioningResponse {
            pendings: pending.iter().map(pending_view_from_model).collect(),
        })
    }

    pub fn confirm_pending(
        &self,
        owner_user_id: Uuid,
        pending_id: Uuid,
        req: ConfirmPendingProvisioningRequest,
    ) -> Result<ConfirmPendingProvisioningResponse, ServiceError> {
        let pending = self
            .pending_dao
            .get_by_id(pending_id)?
            .ok_or(ServiceError::NotFound)?;

        validate_pending_owner_and_status(owner_user_id, &pending)?;

        if pending.expires_at < Utc::now() {
            let _ = self.pending_dao.mark_expired_before(Utc::now())?;
            return Err(ServiceError::Forbidden);
        }

        let pool = self.producer_dao.pool_clone();
        let mut conn = pool.get().map_err(crate::dao::DaoError::from)?;
        let producer = conn.transaction(|conn| {
            let loc = self.location_dao.create_location_with_conn(
                conn,
                crate::model::models::NewLocation {
                    name: pending.location_name.clone(),
                    geo_lat: pending.location_geo_lat,
                    geo_long: pending.location_geo_long,
                },
            )?;

            let producer = self.producer_dao.create_producer_with_conn(
                conn,
                NewProducer {
                    location_id: loc.id,
                    group_id: pending.group_id,
                    name: pending.producer_name.clone(),
                    ble_name: pending.ble_name.clone(),
                    description: pending.description.clone(),
                    forwarder_id: pending.forwarder_id,
                },
            )?;

            if let Some(pub_key) = req.x25519_pub_key_bytes_base64.clone() {
                self.producer_dao
                    .set_internet_auth_with_conn(conn, producer.id, pub_key)?;
            }

            self.producer_dao.set_mqtt_auth_with_conn(
                conn,
                producer.id,
                NewMqttBrokerAuth {
                    producer_id: producer.id,
                    broker_host: pending.broker_host.clone(),
                    broker_port: pending.broker_port,
                    username: pending.mqtt_username.clone(),
                    password_hash: pending.mqtt_password_hash.clone(),
                    topic: pending.mqtt_topic.clone(),
                    subscribe_topic: pending.mqtt_subscribe_topic.clone(),
                },
            )?;

            self.pending_dao.mark_confirmed_with_conn(
                conn,
                pending.id,
                producer.id,
                req.x25519_pub_key_bytes_base64.clone(),
                req.lora_addr,
            )?;

            Ok::<_, crate::dao::DaoError>(producer)
        })?;

        Ok(ConfirmPendingProvisioningResponse {
            pending_id: pending.id,
            producer_id: producer.id,
            status: PendingProducerRegistrationStatus::Confirmed as i16,
        })
    }

    pub fn cancel_pending(
        &self,
        owner_user_id: Uuid,
        pending_id: Uuid,
    ) -> Result<CancelPendingProvisioningResponse, ServiceError> {
        let pending = self
            .pending_dao
            .get_by_id(pending_id)?
            .ok_or(ServiceError::NotFound)?;

        validate_pending_owner_and_status(owner_user_id, &pending)?;

        let cancelled = self.pending_dao.mark_cancelled(pending_id)?;
        Ok(CancelPendingProvisioningResponse {
            pending_id: cancelled.id,
            status: cancelled.status,
        })
    }
}

impl From<DbPool>
    for ProvisioningService<PgPendingRegistrationDao, PgGroupDao, PgLocationDao, PgProducerDao>
{
    fn from(pool: DbPool) -> Self {
        Self::new(
            PgPendingRegistrationDao::new(pool.clone()),
            PgGroupDao::new(pool.clone()),
            PgLocationDao::new(pool.clone()),
            PgProducerDao::new(pool),
        )
    }
}

fn pending_view_from_model(model: &PendingProducerRegistration) -> PendingProvisioningView {
    PendingProvisioningView {
        id: model.id,
        group_id: model.group_id,
        producer_name: model.producer_name.clone(),
        ble_name: model.ble_name.clone(),
        description: model.description.clone(),
        data_template_id: model.data_template_id,
        forwarder_id: model.forwarder_id,
        status: model.status,
        expires_at: model.expires_at,
        created_at: model.created_at,
    }
}

fn validate_pending_owner_and_status(
    owner_user_id: Uuid,
    pending: &PendingProducerRegistration,
) -> Result<(), ServiceError> {
    if pending.owner_user_id != owner_user_id {
        return Err(ServiceError::Forbidden);
    }

    if pending.status != PendingProducerRegistrationStatus::Pending as i16 {
        return Err(ServiceError::Forbidden);
    }

    Ok(())
}

fn validate_name_sizes(req: &CreatePendingProvisioningRequest) -> Result<(), ServiceError> {
    if req.producer_name.is_empty() || req.producer_name.len() > MAX_FIRMWARE_NAME_LEN {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(format!(
                "producer_name must be 1..={MAX_FIRMWARE_NAME_LEN} characters"
            )),
        ));
    }
    if contains_mqtt_topic_meta(&req.producer_name) {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(
                "producer_name cannot include '/', '+' or '#'".to_string(),
            ),
        ));
    }

    if req.ble_name.is_empty() || req.ble_name.len() > MAX_FIRMWARE_NAME_LEN {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(format!(
                "ble_name must be 1..={MAX_FIRMWARE_NAME_LEN} characters"
            )),
        ));
    }
    if contains_mqtt_topic_meta(&req.ble_name) {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(
                "ble_name cannot include '/', '+' or '#'".to_string(),
            ),
        ));
    }

    if req.group_id.is_nil() {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation("group_id cannot be nil".to_string()),
        ));
    }

    Ok(())
}

fn contains_mqtt_topic_meta(v: &str) -> bool {
    v.contains('/') || v.contains('+') || v.contains('#')
}
