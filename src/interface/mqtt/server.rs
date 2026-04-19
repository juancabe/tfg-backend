use std::{env, io, str};

use ntex::{
    ServiceFactory,
    service::{chain_factory, fn_factory_with_config, fn_service},
};
use ntex_mqtt::{
    MqttError as NtexMqttError,
    v5::{self, codec::ConnectAckReason},
};
use openssl::ssl::{self, SslFiletype, SslMethod};
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
    #[error("MQTT TLS initialization error: {0}")]
    TlsInit(String),
    #[error("MQTT TLS accept error: {0}")]
    TlsAccept(String),
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
    /// PEM chain certificate path for MQTT TLS listener.
    pub tls_cert_path: String,
    /// PEM private key path for MQTT TLS listener.
    pub tls_key_path: String,
    /// Number of ntex workers for MQTT server processing.
    pub workers: usize,
}

impl MqttBrokerConfig {
    /// Reads broker configuration from environment variables.
    ///
    /// Supported keys:
    /// - `MQTT_BIND_HOST` (default `0.0.0.0`)
    /// - `MQTT_BIND_PORT` (fallback `MQTT_PORT`, default `8883`)
    /// - `MQTT_TLS_CERT_PATH` (default `mqtt-server-cert.pem`)
    /// - `MQTT_TLS_KEY_PATH` (default `mqtt-server-key.pem`)
    /// - `MQTT_WORKERS` (default `1`)
    pub fn from_env() -> Self {
        let bind_host = env::var("MQTT_BIND_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let bind_port = env::var("MQTT_BIND_PORT")
            .or_else(|_| env::var("MQTT_PORT"))
            .unwrap_or_else(|_| "8883".to_string())
            .parse::<u16>()
            .expect("MQTT_BIND_PORT/MQTT_PORT must be a number");
        let tls_cert_path =
            env::var("MQTT_TLS_CERT_PATH").unwrap_or_else(|_| "mqtt-server-cert.pem".to_string());
        let tls_key_path =
            env::var("MQTT_TLS_KEY_PATH").unwrap_or_else(|_| "mqtt-server-key.pem".to_string());
        let workers = env::var("MQTT_WORKERS")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<usize>()
            .expect("MQTT_WORKERS must be a number");

        Self {
            bind_host,
            bind_port,
            tls_cert_path,
            tls_key_path,
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
    fn control_service_factory() -> impl ServiceFactory<
        v5::Control<MqttError>,
        v5::Session<ProducerIdentity>,
        Response = v5::ControlAck,
        Error = MqttError,
        InitError = MqttError,
    > {
        fn_factory_with_config(|session: v5::Session<ProducerIdentity>| async move {
            Ok::<_, MqttError>(fn_service(move |control: v5::Control<MqttError>| {
                let session = session.clone();

                async move {
                    match control {
                        v5::Control::Protocol(v5::CtlFrame::Subscribe(mut subscribe)) => {
                            for mut subscription in subscribe.iter_mut() {
                                let requested_qos = subscription.options().qos;
                                let topic = subscription.topic().as_ref();
                                if !MqttAuthService::authorize_subscribe(session.state(), topic) {
                                    log::warn!(
                                        "MQTT subscribe rejected producer_id={} topic={} qos={requested_qos:?}",
                                        session.state().producer_id,
                                        topic,
                                    );
                                    subscription.fail(v5::codec::SubscribeAckReason::NotAuthorized);
                                    continue;
                                }

                                log::info!(
                                    "MQTT subscribe accepted producer_id={} topic={:?} qos={requested_qos:?}",
                                    session.state().producer_id,
                                    subscription.topic(),
                                );
                                subscription.confirm(requested_qos);
                            }

                            Ok::<_, MqttError>(subscribe.ack())
                        }
                        v5::Control::Protocol(v5::CtlFrame::Unsubscribe(unsubscribe)) => {
                            for topic in unsubscribe.iter() {
                                log::info!(
                                    "MQTT unsubscribe accepted producer_id={} topic={topic:?}",
                                    session.state().producer_id,
                                );
                            }

                            Ok(unsubscribe.ack())
                        }
                        _ => Ok(control.ack()),
                    }
                }
            }))
        })
    }

    fn build_tls_acceptor(config: &MqttBrokerConfig) -> io::Result<ssl::SslAcceptor> {
        let mut builder = ssl::SslAcceptor::mozilla_intermediate(SslMethod::tls())
            .map_err(|err| MqttError::TlsInit(err.to_string()))
            .map_err(io::Error::other)?;
        builder
            .set_private_key_file(&config.tls_key_path, SslFiletype::PEM)
            .map_err(|err| MqttError::TlsInit(err.to_string()))
            .map_err(io::Error::other)?;
        builder
            .set_certificate_chain_file(&config.tls_cert_path)
            .map_err(|err| MqttError::TlsInit(err.to_string()))
            .map_err(io::Error::other)?;
        builder
            .check_private_key()
            .map_err(|err| MqttError::TlsInit(err.to_string()))
            .map_err(io::Error::other)?;

        Ok(builder.build())
    }

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
        let tls_acceptor = Self::build_tls_acceptor(&config)?;

        log::info!(
            "Starting MQTT ingest broker with TLS on {} (workers={}, cert_path={}, key_path={})",
            bind_addr,
            config.workers,
            config.tls_cert_path,
            config.tls_key_path,
        );

        ntex::server::build()
            .bind("mqtt-v5-tls", bind_addr, move |_| {
                let auth = auth.clone();
                let ingest = ingest.clone();
                let tls_acceptor = tls_acceptor.clone();

                async move {
                    chain_factory(ntex::server::openssl::SslAcceptor::new(
                        tls_acceptor.clone(),
                    ))
                    .map_err(|err| NtexMqttError::Service(MqttError::TlsAccept(err.to_string())))
                    .and_then(
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
                                            handshake
                                                .failed(ConnectAckReason::BadUserNameOrPassword),
                                        );
                                    }
                                };

                                match auth.authenticate(username, password) {
                                    AuthDecision::Allowed(identity) => {
                                        log::info!(
                                            "Mqtt connect allowed for {username}:{password}"
                                        );
                                        Ok(handshake.ack(identity))
                                    }
                                    AuthDecision::Rejected => {
                                        log::warn!(
                                            "Mqtt connect Rejected for {username}:{password}"
                                        );
                                        Ok(handshake.failed(ConnectAckReason::NotAuthorized))
                                    }
                                }
                            }
                        })
                        .control(Self::control_service_factory())
                        .publish(fn_factory_with_config(
                            move |session: v5::Session<ProducerIdentity>| {
                                let ingest = ingest.clone();

                                async move {
                                    Ok::<_, MqttError>(fn_service(move |publish: v5::Publish| {
                                        let ingest = ingest.clone();
                                        let session = session.clone();

                                        async move {
                                            let ack = ingest
                                                .handle_publish(session.state(), publish)
                                                .await;
                                            Ok::<_, MqttError>(ack)
                                        }
                                    }))
                                }
                            },
                        )),
                    )
                }
            })?
            .workers(config.workers)
            .run()
            .await
    }
}
