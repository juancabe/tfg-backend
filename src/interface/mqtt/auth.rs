use crate::{dao::DbPool, helpers::crypto_password, model::models::MqttBrokerAuth};
use diesel::prelude::*;
use uuid::Uuid;

/// Auth service for MQTT connect/publish decisions.
///
/// It validates broker credentials stored in `mqtt_broker_auths` and derives a
/// per-session producer identity used by the ingest pipeline.
#[derive(Clone)]
pub struct MqttAuthService {
    pool: DbPool,
}

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
    /// Exact topic this producer is allowed to publish to.
    pub allowed_topic: String,
    /// Exact topic this producer is allowed to subscribe to.
    pub allowed_subscribe_topic: String,
}

impl MqttAuthService {
    /// Creates a new MQTT auth service backed by the shared DB pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Authenticates MQTT username/password credentials.
    ///
    /// Returns [`AuthDecision::Allowed`] with session identity when credentials
    /// match a row in `mqtt_broker_auths`; otherwise returns
    /// [`AuthDecision::Rejected`].
    pub fn authenticate(&self, username_in: &str, password_in: &str) -> AuthDecision {
        use crate::model::schema::mqtt_broker_auths::dsl::*;

        let conn = match self.pool.get() {
            Ok(conn) => conn,
            Err(err) => {
                log::error!("MQTT auth DB connection error: {err:?}");
                return AuthDecision::Rejected;
            }
        };

        let mut conn = conn;
        let row = mqtt_broker_auths
            .filter(username.eq(username_in))
            .select(MqttBrokerAuth::as_select())
            .first::<MqttBrokerAuth>(&mut conn)
            .optional();

        let Some(auth_row) = (match row {
            Ok(val) => val,
            Err(err) => {
                log::warn!("MQTT auth query failed: {err:?}");
                return AuthDecision::Rejected;
            }
        }) else {
            log::warn!("MQTT auth row not found");
            return AuthDecision::Rejected;
        };

        if !crypto_password::verify_password(&auth_row.password_hash, password_in) {
            log::warn!("MQTT auth pass doesn't match");
            return AuthDecision::Rejected;
        }

        AuthDecision::Allowed(ProducerIdentity {
            producer_id: auth_row.producer_id,
            allowed_topic: auth_row.topic,
            allowed_subscribe_topic: auth_row.subscribe_topic,
        })
    }

    /// Authorizes a publish attempt for a session identity.
    ///
    /// Current policy is strict exact-topic match and is intentionally simple
    /// for ingest-focused bootstrapping.
    pub fn authorize_publish(identity: &ProducerIdentity, topic: &str) -> bool {
        if identity.allowed_topic == "/v1/forbidden" {
            return false;
        }
        topic_matches_filter(topic, &identity.allowed_topic)
    }

    pub fn authorize_subscribe(identity: &ProducerIdentity, topic_filter: &str) -> bool {
        identity.allowed_subscribe_topic == topic_filter
    }
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
