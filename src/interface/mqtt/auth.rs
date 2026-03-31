use crate::{dao::DbPool, model::models::MqttBrokerAuth};
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
            return AuthDecision::Rejected;
        };

        if auth_row.password != password_in {
            return AuthDecision::Rejected;
        }

        AuthDecision::Allowed(ProducerIdentity {
            producer_id: auth_row.producer_id,
            allowed_topic: auth_row.topic,
        })
    }

    /// Authorizes a publish attempt for a session identity.
    ///
    /// Current policy is strict exact-topic match and is intentionally simple
    /// for ingest-focused bootstrapping.
    pub fn authorize_publish(identity: &ProducerIdentity, topic: &str) -> bool {
        identity.allowed_topic == topic
    }
}
