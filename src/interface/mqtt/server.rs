use std::{env, io, str};

use ntex::service::{fn_factory_with_config, fn_service};
use ntex_mqtt::v5::{self, codec::ConnectAckReason};
use thiserror::Error;

use crate::dao::DbPool;

use super::{
    auth::{AuthDecision, MqttAuthService, ProducerIdentity},
    ingest::MqttIngestService,
};

/// MQTT interface error type.
///
/// This is currently minimal boilerplate and can be expanded with structured
/// startup/runtime variants as broker responsibilities grow.
#[derive(Debug, Error)]
pub enum MqttError {
    #[error("MQTT initialization error")]
    Init,
}

impl From<()> for MqttError {
    fn from(_: ()) -> Self {
        Self::Init
    }
}

impl TryFrom<MqttError> for v5::PublishAck {
    type Error = MqttError;

    fn try_from(err: MqttError) -> Result<Self, Self::Error> {
        Err(err)
    }
}

/// Runtime configuration for the embedded MQTT broker.
#[derive(Clone, Debug)]
pub struct MqttBrokerConfig {
    /// Network interface/address where the broker listens.
    pub bind_host: String,
    /// TCP port where the broker listens.
    pub bind_port: u16,
    /// Number of ntex workers for MQTT server processing.
    pub workers: usize,
}

impl MqttBrokerConfig {
    /// Reads broker configuration from environment variables.
    ///
    /// Supported keys:
    /// - `MQTT_BIND_HOST` (default `0.0.0.0`)
    /// - `MQTT_BIND_PORT` (fallback `MQTT_PORT`, default `1884`)
    /// - `MQTT_WORKERS` (default `1`)
    pub fn from_env() -> Self {
        let bind_host = env::var("MQTT_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let bind_port = env::var("MQTT_BIND_PORT")
            .or_else(|_| env::var("MQTT_PORT"))
            .unwrap_or_else(|_| "1884".to_string())
            .parse::<u16>()
            .expect("MQTT_BIND_PORT/MQTT_PORT must be a number");
        let workers = env::var("MQTT_WORKERS")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<usize>()
            .expect("MQTT_WORKERS must be a number");

        Self {
            bind_host,
            bind_port,
            workers,
        }
    }

    /// Returns `host:port` for `ntex::server::build().bind(...)`.
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind_host, self.bind_port)
    }
}

/// Embedded MQTT v5 ingest broker.
///
/// This broker is intentionally ingest-focused: authenticate producer clients,
/// validate publish permissions, and pass payload handling to `MqttIngestService`.
pub struct MqttBroker;

impl MqttBroker {
    /// Runs the MQTT server until shutdown/error.
    ///
    /// Handshake flow:
    /// - Extract username/password from CONNECT.
    /// - Authenticate against DB-backed auth service.
    /// - Store `ProducerIdentity` in MQTT session state.
    ///
    /// Publish flow:
    /// - Resolve session identity.
    /// - Delegate to ingest service for authorization and ack decision.
    pub async fn run(config: MqttBrokerConfig, pool: DbPool) -> io::Result<()> {
        let auth = MqttAuthService::new(pool.clone());
        let ingest = MqttIngestService::new(pool);
        let bind_addr = config.bind_addr();

        log::info!(
            "Starting MQTT ingest broker on {} (workers={})",
            bind_addr,
            config.workers
        );

        ntex::server::build()
            .bind("mqtt-v5", bind_addr, move |_| {
                let auth = auth.clone();
                let ingest = ingest.clone();

                async move {
                    v5::MqttServer::new(move |handshake: v5::Handshake| {
                        let auth = auth.clone();

                        async move {
                            let packet = handshake.packet();
                            let username = packet.username.as_deref().unwrap_or_default();
                            let password_bytes = packet.password.as_deref().unwrap_or_default();

                            let password = match str::from_utf8(password_bytes) {
                                Ok(password) => password,
                                Err(_) => {
                                    return Ok::<_, MqttError>(
                                        handshake.failed(ConnectAckReason::BadUserNameOrPassword),
                                    );
                                }
                            };

                            match auth.authenticate(username, password) {
                                AuthDecision::Allowed(identity) => Ok(handshake.ack(identity)),
                                AuthDecision::Rejected => {
                                    Ok(handshake.failed(ConnectAckReason::NotAuthorized))
                                }
                            }
                        }
                    })
                    .publish(fn_factory_with_config(
                        move |session: v5::Session<ProducerIdentity>| {
                            let ingest = ingest.clone();

                            async move {
                                Ok::<_, MqttError>(fn_service(move |publish: v5::Publish| {
                                    let ingest = ingest.clone();
                                    let session = session.clone();

                                    async move {
                                        let ack =
                                            ingest.handle_publish(session.state(), publish).await;
                                        Ok::<_, MqttError>(ack)
                                    }
                                }))
                            }
                        },
                    ))
                }
            })?
            .workers(config.workers)
            .run()
            .await
    }
}
