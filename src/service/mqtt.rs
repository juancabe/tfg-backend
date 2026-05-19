//! MQTT authentication, authorization, and data ingestion services.

use openssl::base64;
use uuid::Uuid;

use crate::{
    dao::{
        mqtt::{MqttDao, PgMqttDao},
        producer::{PgProducerDao, ProducerDao},
    },
    helpers::crypto_password,
};

use super::ServiceError;

const MAX_FORWARDER_PEERS: usize = 20;

/// Result of an authentication attempt.
pub enum AuthDecision {
    /// Credentials are valid and a producer identity is attached to the session.
    Allowed(ProducerIdentity),
    /// Credentials are invalid or could not be validated.
    Rejected,
}

/// Producer identity materialized from broker auth records.
///
/// This identity is placed into MQTT session state after a successful CONNECT
/// handshake and later reused for publish authorization.
#[derive(Clone)]
pub struct ProducerIdentity {
    /// Producer database identifier.
    pub producer_id: Uuid,
    /// MQTT username used to authenticate this session.
    pub mqtt_username: String,
    /// Exact topic this producer is allowed to publish to.
    pub allowed_topic: String,
    /// Exact topic this producer is allowed to subscribe to.
    pub allowed_subscribe_topic: String,
}

/// Target information for a publish authorization.
#[derive(Debug, Clone)]
pub struct PublishTarget {
    /// The producer that owns this target.
    pub producer_id: Uuid,
    /// The data template ID extracted from the topic, if present.
    pub template_id: Option<Uuid>,
}

/// The type of publish authorization granted.
#[derive(Debug, Clone)]
pub enum PublishAuthorization {
    /// Direct publish to the producer's own topic.
    Direct(PublishTarget),
    /// Delegated publish through a forwarder.
    Delegated(PublishTarget),
}

/// Service for MQTT auth and topic-level authorization decisions.
#[derive(Clone)]
pub struct MqttAuthService<M: MqttDao = PgMqttDao> {
    mqtt_dao: M,
}

impl<M: MqttDao> MqttAuthService<M> {
    /// Creates a new [`MqttAuthService`] with the given DAO.
    pub fn new(mqtt_dao: M) -> Self {
        Self { mqtt_dao }
    }

    /// Authenticates MQTT username/password credentials.
    ///
    /// Returns [`AuthDecision::Allowed`] with session identity when credentials
    /// match a row in `mqtt_broker_auths`; otherwise returns
    /// [`AuthDecision::Rejected`].
    pub fn authenticate(&self, username_in: &str, password_in: &str) -> AuthDecision {
        let row = match self.mqtt_dao.find_broker_auth_by_username(username_in) {
            Ok(row) => row,
            Err(err) => {
                log::warn!("MQTT auth query failed: {err:?}");
                return AuthDecision::Rejected;
            }
        };

        let Some(auth_row) = row else {
            log::warn!("MQTT auth row not found");
            return AuthDecision::Rejected;
        };

        if !crypto_password::verify_password(&auth_row.password_hash, password_in) {
            log::warn!("MQTT auth pass doesn't match");
            return AuthDecision::Rejected;
        }

        AuthDecision::Allowed(ProducerIdentity {
            producer_id: auth_row.producer_id,
            mqtt_username: auth_row.username,
            allowed_topic: auth_row.topic,
            allowed_subscribe_topic: auth_row.subscribe_topic,
        })
    }

    /// Checks if a topic filter matches the identity's allowed subscribe topic.
    pub fn authorize_subscribe(identity: &ProducerIdentity, topic_filter: &str) -> bool {
        identity.allowed_subscribe_topic == topic_filter
    }

    /// Authorizes a publish to a topic, checking both direct and delegated access.
    pub fn authorize_publish(
        &self,
        identity: &ProducerIdentity,
        topic: &str,
    ) -> Option<PublishAuthorization> {
        if identity.allowed_topic != "/v1/forbidden"
            && topic_matches_filter(topic, &identity.allowed_topic)
        {
            return Some(PublishAuthorization::Direct(PublishTarget {
                producer_id: identity.producer_id,
                template_id: extract_template_id(topic),
            }));
        }

        self.authorize_delegated_publish(identity, topic)
    }

    fn authorize_delegated_publish(
        &self,
        identity: &ProducerIdentity,
        topic: &str,
    ) -> Option<PublishAuthorization> {
        let parsed = ParsedDataTopic::parse(topic)?;
        let delegated = self
            .mqtt_dao
            .find_delegated_publish_target_by_producer(
                identity.producer_id,
                parsed.group_name.as_str(),
                parsed.producer_name.as_str(),
            )
            .ok()??;

        let matches_delegated_username = parsed.mqtt_username == delegated.mqtt_username;
        let matches_forwarder_username = parsed.mqtt_username == identity.mqtt_username;
        if !matches_delegated_username && !matches_forwarder_username {
            return None;
        }

        let topic_is_allowed = if matches_delegated_username {
            topic_matches_filter(topic, &delegated.topic)
        } else {
            let forwarder_topic = replace_topic_username(
                &delegated.topic,
                delegated.mqtt_username.as_str(),
                identity.mqtt_username.as_str(),
            )?;
            topic_matches_filter(topic, &forwarder_topic)
        };

        if !topic_is_allowed {
            return None;
        }

        Some(PublishAuthorization::Delegated(PublishTarget {
            producer_id: delegated.producer_id,
            template_id: parsed.template_id,
        }))
    }
}

impl From<crate::dao::DbPool> for MqttAuthService<PgMqttDao> {
    fn from(value: crate::dao::DbPool) -> Self {
        Self::new(PgMqttDao::new(value))
    }
}

/// Service for MQTT ingest persistence and side effects.
#[derive(Clone)]
pub struct MqttIngestService<M: MqttDao = PgMqttDao, P: ProducerDao = PgProducerDao> {
    mqtt_dao: M,
    producer_dao: P,
    auth: MqttAuthService<M>,
}

impl<M: MqttDao + Clone, P: ProducerDao + Clone> MqttIngestService<M, P> {
    /// Creates a new [`MqttIngestService`].
    pub fn new(mqtt_dao: M, producer_dao: P, auth: MqttAuthService<M>) -> Self {
        Self {
            mqtt_dao,
            producer_dao,
            auth,
        }
    }

    /// Delegates publish authorization to the inner auth service.
    pub fn authorize_publish(
        &self,
        identity: &ProducerIdentity,
        topic: &str,
    ) -> Option<PublishAuthorization> {
        self.auth.authorize_publish(identity, topic)
    }

    /// Persists a payload to the database and updates the producer's last_seen.
    pub fn persist_payload(
        &self,
        target: &PublishTarget,
        payload: &[u8],
    ) -> Result<(), ServiceError> {
        let template_id = match target.template_id {
            Some(template_id) => template_id,
            None => {
                return Err(ServiceError::TypeError(
                    crate::interface::types::TypeError::Validation(
                        "template_id is required for ingest".to_string(),
                    ),
                ));
            }
        };

        let payload_json: serde_json::Value =
            serde_json::from_slice(payload).unwrap_or_else(|_| {
                serde_json::json!({
                    "raw_bytes_b64": base64::encode_block(payload),
                })
            });

        self.mqtt_dao.insert_data(crate::model::models::NewData {
            producer_id: target.producer_id,
            template_id,
            payload: payload_json,
        })?;

        if let Err(err) = self.producer_dao.update_last_seen(target.producer_id) {
            log::warn!(
                "MQTT ingest last_seen update failed producer_id={} error={err:?}",
                target.producer_id
            );
        }

        Ok(())
    }
}

impl From<crate::dao::DbPool> for MqttIngestService<PgMqttDao, PgProducerDao> {
    fn from(value: crate::dao::DbPool) -> Self {
        let mqtt_dao = PgMqttDao::new(value.clone());
        let producer_dao = PgProducerDao::new(value);
        let auth = MqttAuthService::new(mqtt_dao.clone());

        Self::new(mqtt_dao, producer_dao, auth)
    }
}

/// Service for forwarder peers snapshot retrieval.
#[derive(Clone)]
pub struct MqttForwarderPeersService<M: MqttDao = PgMqttDao> {
    mqtt_dao: M,
}

impl<M: MqttDao> MqttForwarderPeersService<M> {
    /// Creates a new [`MqttForwarderPeersService`].
    pub fn new(mqtt_dao: M) -> Self {
        Self { mqtt_dao }
    }

    /// Loads and decodes x25519 public keys for a forwarder's peer producers.
    pub fn load_forwarder_peers(&self, forwarder_id: Uuid) -> Result<Vec<[u8; 32]>, ServiceError> {
        let rows = self.mqtt_dao.list_forwarder_peer_keys(forwarder_id)?;

        let mut peers = Vec::new();

        for (producer_id, b64_key) in rows {
            if peers.len() >= MAX_FORWARDER_PEERS {
                log::warn!(
                    "forwarder peers list truncated at {} entries for forwarder_id={}",
                    MAX_FORWARDER_PEERS,
                    forwarder_id
                );
                break;
            }

            let decoded = match openssl::base64::decode_block(&b64_key) {
                Ok(bytes) => bytes,
                Err(err) => {
                    log::warn!(
                        "invalid peer pubkey base64 producer_id={} forwarder_id={} error={err}",
                        producer_id,
                        forwarder_id
                    );
                    continue;
                }
            };

            let Ok(peer_key) = <[u8; 32]>::try_from(decoded.as_slice()) else {
                log::warn!(
                    "invalid peer pubkey length producer_id={} forwarder_id={} len={}",
                    producer_id,
                    forwarder_id,
                    decoded.len()
                );
                continue;
            };

            peers.push(peer_key);
        }

        Ok(peers)
    }
}

impl From<crate::dao::DbPool> for MqttForwarderPeersService<PgMqttDao> {
    fn from(value: crate::dao::DbPool) -> Self {
        Self::new(PgMqttDao::new(value))
    }
}

#[derive(Debug)]
struct ParsedDataTopic {
    mqtt_username: String,
    group_name: String,
    producer_name: String,
    template_id: Option<Uuid>,
}

impl ParsedDataTopic {
    fn parse(topic: &str) -> Option<Self> {
        let mut it = topic.split('/').filter(|s| !s.is_empty());
        let v = it.next()?;
        if v != "v1" {
            return None;
        }

        let users = it.next()?;
        if users != "users" {
            return None;
        }
        let mqtt_username = it.next()?.to_string();

        let groups = it.next()?;
        if groups != "groups" {
            return None;
        }
        let group_name = it.next()?.to_string();

        let producers = it.next()?;
        if producers != "producers" {
            return None;
        }
        let producer_name = it.next()?.to_string();

        let section = it.next()?;
        if section != "data_templates" {
            return None;
        }

        let template_id = it.next().and_then(|v| Uuid::parse_str(v).ok());

        Some(Self {
            mqtt_username,
            group_name,
            producer_name,
            template_id,
        })
    }
}

fn extract_template_id(topic: &str) -> Option<Uuid> {
    ParsedDataTopic::parse(topic)?.template_id
}

fn topic_matches_filter(topic: &str, filter: &str) -> bool {
    let topic_levels: Vec<&str> = topic.split('/').filter(|s| !s.is_empty()).collect();
    let filter_levels: Vec<&str> = filter.split('/').filter(|s| !s.is_empty()).collect();

    let mut ti = 0usize;
    let mut fi = 0usize;
    while fi < filter_levels.len() {
        let f = filter_levels[fi];
        if f == "#" {
            return true;
        }

        if ti >= topic_levels.len() {
            return false;
        }

        if f != "+" && f != topic_levels[ti] {
            return false;
        }

        ti += 1;
        fi += 1;
    }

    ti == topic_levels.len()
}

fn replace_topic_username(topic: &str, from_username: &str, to_username: &str) -> Option<String> {
    let mut it = topic.split('/').filter(|s| !s.is_empty());
    let v = it.next()?;
    if v != "v1" {
        return None;
    }
    let users = it.next()?;
    if users != "users" {
        return None;
    }
    let username = it.next()?;
    if username != from_username {
        return None;
    }

    let mut rebuilt = String::from("/v1/users/");
    rebuilt.push_str(to_username);
    for part in it {
        rebuilt.push('/');
        rebuilt.push_str(part);
    }

    Some(rebuilt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::prelude::*;

    use crate::{
        model::{models::NewMqttBrokerAuth, schema::mqtt_broker_auths},
        test_utils::{
            create_test_group, create_test_location, create_test_producer, create_test_user, setup,
        },
    };

    fn mk_identity(producer_id: Uuid, allowed_topic: &str) -> ProducerIdentity {
        ProducerIdentity {
            producer_id,
            mqtt_username: "test-user".to_string(),
            allowed_topic: allowed_topic.to_string(),
            allowed_subscribe_topic: "/v1/forbidden".to_string(),
        }
    }

    #[test]
    fn test_authorize_publish_delegated_when_direct_topic_forbidden() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "mqtt_auth_delegate");
        let loc = create_test_location(&ctx.pool, "mqtt_auth_delegate");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_auth_delegate");

        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_auth_forwarder");
        let delegated = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_auth_delegated");

        let mut conn = ctx.pool.get().expect("db conn");
        diesel::update(crate::model::schema::producers::table.find(delegated.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set delegated forwarder");

        let template_id = Uuid::new_v4();
        let delegated_username = "delegated-u".to_string();
        let delegated_topic = format!(
            "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
            delegated_username, group.name, delegated.name, template_id
        );

        diesel::insert_into(mqtt_broker_auths::table)
            .values(NewMqttBrokerAuth {
                producer_id: delegated.id,
                broker_host: "localhost".to_string(),
                broker_port: 1883,
                username: delegated_username,
                password_hash: "unused".to_string(),
                topic: delegated_topic.clone(),
                subscribe_topic: "/v1/forbidden".to_string(),
            })
            .execute(&mut conn)
            .expect("insert delegated mqtt auth");
        drop(conn);

        let svc: MqttAuthService<PgMqttDao> = MqttAuthService::from(ctx.pool.clone());
        let identity = mk_identity(forwarder.id, "/v1/forbidden");

        let authz = svc
            .authorize_publish(&identity, &delegated_topic)
            .expect("delegated publish should be authorized");

        match authz {
            PublishAuthorization::Delegated(target) => {
                assert_eq!(target.producer_id, delegated.id);
                assert_eq!(target.template_id, Some(template_id));
            }
            PublishAuthorization::Direct(_) => panic!("expected delegated authorization"),
        }
    }

    #[test]
    fn test_authorize_publish_delegated_requires_matching_mqtt_username() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "mqtt_auth_delegate_user");
        let loc = create_test_location(&ctx.pool, "mqtt_auth_delegate_user");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_auth_delegate_user");

        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder");
        let delegated = create_test_producer(&ctx.pool, group.id, loc.id, "delegated");

        let mut conn = ctx.pool.get().expect("db conn");
        diesel::update(crate::model::schema::producers::table.find(delegated.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set delegated forwarder");

        let template_id = Uuid::new_v4();
        let expected_topic = format!(
            "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
            "stored-username", group.name, delegated.name, template_id
        );

        diesel::insert_into(mqtt_broker_auths::table)
            .values(NewMqttBrokerAuth {
                producer_id: delegated.id,
                broker_host: "localhost".to_string(),
                broker_port: 1883,
                username: "stored-username".to_string(),
                password_hash: "unused".to_string(),
                topic: expected_topic,
                subscribe_topic: "/v1/forbidden".to_string(),
            })
            .execute(&mut conn)
            .expect("insert delegated mqtt auth");
        drop(conn);

        let attempted_topic = format!(
            "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
            "different-username", group.name, delegated.name, template_id
        );

        let svc: MqttAuthService<PgMqttDao> = MqttAuthService::from(ctx.pool.clone());
        let identity = mk_identity(forwarder.id, "/v1/forbidden");

        assert!(svc.authorize_publish(&identity, &attempted_topic).is_none());
    }

    #[test]
    fn test_authorize_publish_delegated_allows_forwarder_username() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "mqtt_auth_delegate_forwarder_user");
        let loc = create_test_location(&ctx.pool, "mqtt_auth_delegate_forwarder_user");
        let group = create_test_group(
            &ctx.pool,
            owner.id,
            loc.id,
            "mqtt_auth_delegate_forwarder_user",
        );

        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder");
        let delegated = create_test_producer(&ctx.pool, group.id, loc.id, "delegated");

        let mut conn = ctx.pool.get().expect("db conn");
        diesel::update(crate::model::schema::producers::table.find(delegated.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set delegated forwarder");

        let template_id = Uuid::new_v4();
        let delegated_username = "delegated-user".to_string();
        let delegated_topic = format!(
            "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
            delegated_username, group.name, delegated.name, template_id
        );

        diesel::insert_into(crate::model::schema::mqtt_broker_auths::table)
            .values(crate::model::models::NewMqttBrokerAuth {
                producer_id: delegated.id,
                broker_host: "localhost".to_string(),
                broker_port: 1883,
                username: delegated_username,
                password_hash: "unused".to_string(),
                topic: delegated_topic.clone(),
                subscribe_topic: "/v1/forbidden".to_string(),
            })
            .execute(&mut conn)
            .expect("insert delegated mqtt auth");
        drop(conn);

        let forwarder_username = "forwarder-user".to_string();
        let forwarder_topic = format!(
            "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
            forwarder_username, group.name, delegated.name, template_id
        );

        let svc: MqttAuthService<PgMqttDao> = MqttAuthService::from(ctx.pool.clone());
        let identity = ProducerIdentity {
            producer_id: forwarder.id,
            mqtt_username: forwarder_username,
            allowed_topic: "/v1/forbidden".to_string(),
            allowed_subscribe_topic: "/v1/forbidden".to_string(),
        };

        let authz = svc
            .authorize_publish(&identity, &forwarder_topic)
            .expect("forwarder username delegated publish should be authorized");

        match authz {
            PublishAuthorization::Delegated(target) => {
                assert_eq!(target.producer_id, delegated.id);
                assert_eq!(target.template_id, Some(template_id));
            }
            PublishAuthorization::Direct(_) => panic!("expected delegated authorization"),
        }
    }
}
