//! Device provisioning service layer.

use chrono::{Duration, Utc};
use diesel::Connection;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::dsl::exists;
use diesel::select;
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
        AvailableForwarderView, CancelPendingProvisioningResponse,
        ConfirmPendingProvisioningRequest, ConfirmPendingProvisioningResponse,
        CreatePendingProvisioningRequest, CreatePendingProvisioningResponse,
        ListAvailableForwardersResponse, ListPendingProvisioningResponse, PendingProvisioningView,
        ProvisioningWifiBundle,
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

/// A producer candidate eligible to act as a LoRa forwarder.
#[derive(Debug, Clone)]
pub struct ForwarderCandidate {
    /// Producer ID.
    pub id: Uuid,
    /// Group ID the producer belongs to.
    pub group_id: Uuid,
    /// Producer name.
    pub name: String,
    /// BLE device name.
    pub ble_name: String,
    /// Producer description.
    pub description: String,
    /// LoRa address.
    pub lora_addr: i32,
    /// Whether the producer forwards LoRa traffic.
    pub forward_lora: bool,
    /// X25519 public key in base64.
    pub x25519_pub_key_bytes_base64: String,
}

/// Service for device provisioning operations.
pub struct ProvisioningService<Pd, Gd, Ld, Prd, Id>
where
    Pd: PendingRegistrationDao,
    Gd: GroupDao,
    Ld: LocationDao,
    Prd: ProducerDao,
    Id: crate::dao::identity::IdentityDao,
{
    pending_dao: Pd,
    group_dao: Gd,
    location_dao: Ld,
    producer_dao: Prd,
    identity_dao: Id,
}

impl<Pd, Gd, Ld, Prd, Id> ProvisioningService<Pd, Gd, Ld, Prd, Id>
where
    Pd: PendingRegistrationDao,
    Gd: GroupDao,
    Ld: LocationDao,
    Prd: ProducerDao,
    Id: crate::dao::identity::IdentityDao,
{
    /// Creates a new [`ProvisioningService`] with the given DAOs.
    pub fn new(
        pending_dao: Pd,
        group_dao: Gd,
        location_dao: Ld,
        producer_dao: Prd,
        identity_dao: Id,
    ) -> Self {
        Self {
            pending_dao,
            group_dao,
            location_dao,
            producer_dao,
            identity_dao,
        }
    }

    /// Creates a pending provisioning request for a new device.
    pub fn create_pending(
        &self,
        owner_user_id: Uuid,
        req: CreatePendingProvisioningRequest,
    ) -> Result<CreatePendingProvisioningResponse, ServiceError> {
        let forward_lora = req.forward_lora.unwrap_or(false);
        if forward_lora && req.forwarder_id.is_some() {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(
                    "forwarder_id must be empty when forward_lora is true".to_string(),
                ),
            ));
        }

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

        if let Some(forwarder_id) = req.forwarder_id {
            self.validate_forwarder_selection(owner_user_id, req.group_id, forwarder_id)?;
        }

        if let Some(template_id) = req.data_template_id {
            use crate::model::schema::packet_templates::dsl::*;
            let mut conn = self
                .producer_dao
                .pool_clone()
                .get()
                .map_err(crate::dao::DaoError::from)?;
            let template_exists = select(exists(packet_templates.filter(id.eq(template_id))))
                .get_result::<bool>(&mut conn)
                .map_err(crate::dao::DaoError::from)?;

            if !template_exists {
                return Err(ServiceError::TypeError(
                    crate::interface::types::TypeError::Validation(
                        "data_template_id does not exist".to_string(),
                    ),
                ));
            }
        }

        let owner = self
            .identity_dao
            .find_by_id(owner_user_id)?
            .ok_or(ServiceError::NotFound)?;
        let user_login = owner.login;

        let broker_host = crate::config::get().get_mqtt_provision_host().to_string();
        let broker_port: i16 = crate::config::get().get_mqtt_provision_port();

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
                user_login, group_name, req.producer_name, template_id
            )
        });
        let mqtt_topic = publish_topic
            .clone()
            .unwrap_or_else(|| "/v1/forbidden".to_string());

        let mqtt_subscribe_topic = format!(
            "/v1/users/{}/groups/{}/forwarder_peers/{}",
            user_login, group_name, req.producer_name
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
            forward_lora,
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
                user_login,
                mqtt_username,
                mqtt_password: mqtt_password_plain,
                publish_topic,
                subscribe_topic: mqtt_subscribe_topic,
            },
        })
    }

    /// Lists all pending provisioning requests for an owner.
    pub fn list_pending_for_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<ListPendingProvisioningResponse, ServiceError> {
        let pending = self.pending_dao.get_pending_for_owner(owner_user_id)?;
        Ok(ListPendingProvisioningResponse {
            pendings: pending.iter().map(pending_view_from_model).collect(),
        })
    }

    /// Lists available forwarder candidates for a group.
    pub fn list_available_forwarders(
        &self,
        owner_user_id: Uuid,
        group_id: Uuid,
    ) -> Result<ListAvailableForwardersResponse, ServiceError> {
        let group = self.group_dao.get_group(group_id)?;
        if group.owner_user_id != owner_user_id {
            return Err(ServiceError::Forbidden);
        }

        let forwarders = self
            .producer_dao
            .list_available_forwarders(owner_user_id, group_id)?
            .into_iter()
            .map(|f| AvailableForwarderView {
                id: f.id,
                name: f.name,
                ble_name: f.ble_name,
                description: f.description,
                lora_addr: f.lora_addr,
                x25519_pub_key_bytes_base64: f.x25519_pub_key_bytes_base64,
            })
            .collect();

        Ok(ListAvailableForwardersResponse { forwarders })
    }

    /// Confirms a pending provisioning request, creating the producer and auth records.
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

        if pending.forward_lora && pending.forwarder_id.is_some() {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(
                    "invalid pending provisioning: forward_lora true cannot have forwarder_id"
                        .to_string(),
                ),
            ));
        }

        if let Some(forwarder_id) = pending.forwarder_id {
            self.validate_forwarder_selection(owner_user_id, pending.group_id, forwarder_id)?;
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
                    lora_addr: req.lora_addr,
                    forward_lora: pending.forward_lora,
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
            forwarder_id: pending.forwarder_id,
        })
    }

    /// Cancels a pending provisioning request.
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

    fn validate_forwarder_selection(
        &self,
        owner_user_id: Uuid,
        group_id: Uuid,
        forwarder_id: Uuid,
    ) -> Result<(), ServiceError> {
        let candidate = self
            .producer_dao
            .get_forwarder_candidate(forwarder_id)?
            .ok_or_else(|| {
                ServiceError::TypeError(crate::interface::types::TypeError::Validation(
                    "forwarder_id does not reference an available forwarder".to_string(),
                ))
            })?;

        if candidate.group_id != group_id {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(
                    "forwarder_id must belong to the same group".to_string(),
                ),
            ));
        }

        let group = self.group_dao.get_group(candidate.group_id)?;
        if group.owner_user_id != owner_user_id {
            return Err(ServiceError::Forbidden);
        }

        if !candidate.forward_lora {
            return Err(ServiceError::TypeError(
                crate::interface::types::TypeError::Validation(
                    "forwarder_id must reference a producer with forward_lora=true".to_string(),
                ),
            ));
        }

        Ok(())
    }
}

impl From<DbPool>
    for ProvisioningService<
        PgPendingRegistrationDao,
        PgGroupDao,
        PgLocationDao,
        PgProducerDao,
        crate::dao::identity::PgIdentityDao,
    >
{
    fn from(pool: DbPool) -> Self {
        Self::new(
            PgPendingRegistrationDao::new(pool.clone()),
            PgGroupDao::new(pool.clone()),
            PgLocationDao::new(pool.clone()),
            PgProducerDao::new(pool.clone()),
            crate::dao::identity::PgIdentityDao::new(pool),
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
        forward_lora: model.forward_lora,
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
