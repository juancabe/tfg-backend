use ntex_mqtt::v5::{self, codec::PublishAckReason};

use crate::dao::DbPool;

use super::auth::{MqttAuthService, ProducerIdentity};

pub const MAX_PAYLOAD_BYTES: usize = 4096;

/// Ingest service for accepted MQTT publishes.
///
/// This type is the integration point where publish payloads can later be
/// routed into domain services (persistence, parsing, command fan-out, etc.).
#[derive(Clone)]
pub struct MqttIngestService {
    pool: DbPool,
}

impl MqttIngestService {
    /// Creates a new ingest service with shared DB access.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Handles one MQTT v5 publish packet.
    ///
    /// Steps:
    /// - Authorize topic for the authenticated producer identity.
    /// - Read full payload from stream.
    /// - Return MQTT v5 ack, including negative ack on authorization/format
    ///   failure.
    pub async fn handle_publish(
        &self,
        identity: &ProducerIdentity,
        publish: v5::Publish,
    ) -> v5::PublishAck {
        let publish_topic = publish.publish_topic();

        if !MqttAuthService::authorize_publish(identity, publish_topic) {
            return publish.ack().reason_code(PublishAckReason::NotAuthorized);
        }

        if publish.payload_size() > MAX_PAYLOAD_BYTES {
            log::warn!(
                "MQTT payload too large producer_id={}, topic={}",
                identity.producer_id,
                publish_topic
            );
            return publish
                .ack()
                .reason_code(PublishAckReason::PayloadFormatInvalid);
        }

        match publish.read_all().await {
            Ok(payload) => {
                log::info!(
                    "MQTT publish accepted producer_id={} topic={} payload_size={}",
                    identity.producer_id,
                    publish_topic,
                    payload.len()
                );

                let _pool = &self.pool;
                publish.ack()
            }
            Err(err) => {
                log::warn!(
                    "MQTT payload read failed producer_id={} topic={} error={err:?}",
                    identity.producer_id,
                    publish_topic
                );
                publish
                    .ack()
                    .reason_code(PublishAckReason::PayloadFormatInvalid)
            }
        }
    }
}
