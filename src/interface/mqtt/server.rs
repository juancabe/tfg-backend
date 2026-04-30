use std::{env, io, str};

use ntex::{
    ServiceFactory,
    service::{chain_factory, fn_factory_with_config, fn_service},
    util::Bytes,
};
use ntex_mqtt::{
    MqttError as NtexMqttError,
    error::SendPacketError,
    v5::{self, codec::ConnectAckReason},
};
use openssl::ssl::{self, SslFiletype, SslMethod};
use thiserror::Error;

use crate::{dao::DbPool, service::mqtt::MqttForwarderPeersService};

use super::{
    auth::{AuthDecision, MqttAuthService, ProducerIdentity},
    ingest::MqttIngestService,
};
use crate::dao::mqtt::PgMqttDao;

#[cfg(test)]
const MAX_FORWARDER_PEERS: usize = 20;

#[derive(serde::Serialize)]
struct RecvPeersPayload(Vec<[u8; 32]>);

fn is_forwarder_peers_topic(topic: &str) -> bool {
    let mut it = topic.split('/').filter(|s| !s.is_empty());

    matches!(it.next(), Some("v1"))
        && matches!(it.next(), Some("users"))
        && it.next().is_some()
        && matches!(it.next(), Some("groups"))
        && it.next().is_some()
        && matches!(it.next(), Some("forwarder_peers"))
        && it.next().is_some()
        && it.next().is_none()
}

fn load_forwarder_peers(
    forwarder_peers_service: &MqttForwarderPeersService,
    forwarder_id: uuid::Uuid,
) -> Result<Vec<[u8; 32]>, String> {
    forwarder_peers_service
        .load_forwarder_peers(forwarder_id)
        .map_err(|err| format!("query peers failed: {err}"))
}

fn publish_forwarder_peers_snapshot(
    forwarder_peers_service: &MqttForwarderPeersService,
    session: &v5::Session<ProducerIdentity>,
    topic: &str,
) -> Result<(), String> {
    if !is_forwarder_peers_topic(topic) {
        return Ok(());
    }

    let peers = load_forwarder_peers(forwarder_peers_service, session.state().producer_id)?;

    log::info!(
        "Sending peers {peers:?} to: {:?}",
        session.state().producer_id
    );
    let payload = postcard::to_stdvec(&RecvPeersPayload(peers))
        .map_err(|err| format!("postcard encode failed: {err}"))?;

    session
        .sink()
        .publish(topic)
        .send_at_most_once(Bytes::from(payload))
        .map_err(|err: SendPacketError| format!("mqtt publish failed: {err}"))?;

    Ok(())
}

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
    fn control_service_factory(
        forwarder_peers_service: MqttForwarderPeersService,
    ) -> impl ServiceFactory<
        v5::Control<MqttError>,
        v5::Session<ProducerIdentity>,
        Response = v5::ControlAck,
        Error = MqttError,
        InitError = MqttError,
    > {
        fn_factory_with_config(move |session: v5::Session<ProducerIdentity>| {
            let forwarder_peers_service = forwarder_peers_service.clone();
            async move {
                Ok::<_, MqttError>(fn_service(move |control: v5::Control<MqttError>| {
                    let session = session.clone();
                    let forwarder_peers_service = forwarder_peers_service.clone();

                    async move {
                        match control {
                            v5::Control::Protocol(v5::CtlFrame::Subscribe(mut subscribe)) => {
                                for mut subscription in subscribe.iter_mut() {
                                    let requested_qos = subscription.options().qos;
                                    let topic = subscription.topic().to_string();
                                    if !MqttAuthService::<PgMqttDao>::authorize_subscribe(
                                        session.state(),
                                        &topic,
                                    ) {
                                        log::warn!(
                                            "MQTT subscribe rejected producer_id={} topic={} qos={requested_qos:?}",
                                            session.state().producer_id,
                                            topic,
                                        );
                                        subscription
                                            .fail(v5::codec::SubscribeAckReason::NotAuthorized);
                                        continue;
                                    }

                                    log::info!(
                                        "MQTT subscribe accepted producer_id={} topic={:?} qos={requested_qos:?}",
                                        session.state().producer_id,
                                        subscription.topic(),
                                    );
                                    subscription.confirm(requested_qos);

                                    if let Err(err) = publish_forwarder_peers_snapshot(
                                        &forwarder_peers_service,
                                        &session,
                                        &topic,
                                    ) {
                                        log::warn!(
                                            "MQTT forwarder_peers push failed producer_id={} topic={} error={err}",
                                            session.state().producer_id,
                                            topic,
                                        );
                                    }
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
            }
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
        let auth = MqttAuthService::from(pool.clone());
        let ingest = MqttIngestService::new(pool.clone());
        let forwarder_peers_service = MqttForwarderPeersService::from(pool);
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
                let forwarder_peers_service = forwarder_peers_service.clone();

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
                        .control(Self::control_service_factory(
                            forwarder_peers_service.clone(),
                        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::prelude::*;

    use crate::{
        model::{models::NewInternetAuth, schema::internet_auths},
        test_utils::{
            create_test_group, create_test_location, create_test_producer, create_test_user, setup,
        },
    };

    #[test]
    fn test_is_forwarder_peers_topic_shape() {
        assert!(is_forwarder_peers_topic(
            "/v1/users/u/groups/g/forwarder_peers/p"
        ));
        assert!(!is_forwarder_peers_topic(
            "/v1/users/u/groups/g/forwarder_peers"
        ));
        assert!(!is_forwarder_peers_topic(
            "/v1/users/u/groups/g/producers/p"
        ));
    }

    #[test]
    fn test_load_forwarder_peers_skips_invalid_keys() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "mqtt_peers_owner");
        let loc = create_test_location(&ctx.pool, "mqtt_peers_loc");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_peers_group");

        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder");
        let valid_peer = create_test_producer(&ctx.pool, group.id, loc.id, "valid_peer");
        let invalid_b64_peer = create_test_producer(&ctx.pool, group.id, loc.id, "invalid_b64");
        let short_key_peer = create_test_producer(&ctx.pool, group.id, loc.id, "short_key");

        let mut conn = ctx.pool.get().expect("db conn");
        diesel::update(crate::model::schema::producers::table.find(valid_peer.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set valid peer forwarder");
        diesel::update(crate::model::schema::producers::table.find(invalid_b64_peer.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set invalid b64 peer forwarder");
        diesel::update(crate::model::schema::producers::table.find(short_key_peer.id))
            .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
            .execute(&mut conn)
            .expect("set short key peer forwarder");

        let valid_key = [0x2Au8; 32];
        diesel::insert_into(internet_auths::table)
            .values(NewInternetAuth {
                producer_id: valid_peer.id,
                x25519_pub_key_bytes_base64: openssl::base64::encode_block(&valid_key),
            })
            .execute(&mut conn)
            .expect("insert valid peer key");
        diesel::insert_into(internet_auths::table)
            .values(NewInternetAuth {
                producer_id: invalid_b64_peer.id,
                x25519_pub_key_bytes_base64: "not_base64!!".to_string(),
            })
            .execute(&mut conn)
            .expect("insert invalid b64 key");
        diesel::insert_into(internet_auths::table)
            .values(NewInternetAuth {
                producer_id: short_key_peer.id,
                x25519_pub_key_bytes_base64: openssl::base64::encode_block(&[1u8, 2u8, 3u8]),
            })
            .execute(&mut conn)
            .expect("insert short key");

        drop(conn);

        let forwarder_peers_service = MqttForwarderPeersService::from(ctx.pool.clone());
        let peers =
            load_forwarder_peers(&forwarder_peers_service, forwarder.id).expect("load peers");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], valid_key);
    }

    #[test]
    fn test_load_forwarder_peers_caps_to_max() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "mqtt_peers_cap_owner");
        let loc = create_test_location(&ctx.pool, "mqtt_peers_cap_loc");
        let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_peers_cap_group");
        let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder_cap");

        let mut peers = Vec::new();
        for idx in 0..(MAX_FORWARDER_PEERS + 2) {
            let peer = create_test_producer(&ctx.pool, group.id, loc.id, &format!("peer_{idx}"));
            peers.push((peer.id, [idx as u8; 32]));
        }

        let mut conn = ctx.pool.get().expect("db conn");
        for (peer_id, key) in peers {
            diesel::update(crate::model::schema::producers::table.find(peer_id))
                .set(crate::model::schema::producers::forwarder_id.eq(Some(forwarder.id)))
                .execute(&mut conn)
                .expect("set peer forwarder");

            diesel::insert_into(internet_auths::table)
                .values(NewInternetAuth {
                    producer_id: peer_id,
                    x25519_pub_key_bytes_base64: openssl::base64::encode_block(&key),
                })
                .execute(&mut conn)
                .expect("insert peer key");
        }
        drop(conn);

        let forwarder_peers_service = MqttForwarderPeersService::from(ctx.pool.clone());
        let peers = load_forwarder_peers(&forwarder_peers_service, forwarder.id)
            .expect("load capped peers");
        assert_eq!(peers.len(), MAX_FORWARDER_PEERS);
    }
}
