use diesel::prelude::*;
use ntex_mqtt::v5::{self, codec::PublishAckReason};
use openssl::base64;
use uuid::Uuid;

use crate::{
    dao::{
        DbPool,
        producer::{PgProducerDao, ProducerDao},
    },
    model::models::NewData,
};

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
                let topic_template_id = extract_template_id(publish_topic);

                if let Some(template_id) = topic_template_id {
                    let mut conn = match self.pool.get() {
                        Ok(conn) => conn,
                        Err(err) => {
                            log::error!("MQTT ingest DB connection error: {err:?}");
                            return publish
                                .ack()
                                .reason_code(PublishAckReason::UnspecifiedError);
                        }
                    };

                    let payload_json = serde_json::json!({
                        "raw_bytes_b64": base64::encode_block(payload.as_ref()),
                    });

                    let insert_res = diesel::insert_into(crate::model::schema::data::table)
                        .values(NewData {
                            producer_id: identity.producer_id,
                            template_id,
                            payload: payload_json,
                        })
                        .execute(&mut conn);

                    if let Err(err) = insert_res {
                        log::warn!(
                            "MQTT ingest insert failed producer_id={} topic={} error={err:?}",
                            identity.producer_id,
                            publish_topic
                        );
                        return publish
                            .ack()
                            .reason_code(PublishAckReason::UnspecifiedError);
                    }

                    let producer_dao = PgProducerDao::new(self.pool.clone());
                    if let Err(err) = producer_dao.update_last_seen(identity.producer_id) {
                        log::warn!(
                            "MQTT ingest last_seen update failed producer_id={} error={err:?}",
                            identity.producer_id
                        );
                    }
                }

                log::info!(
                    "MQTT publish accepted producer_id={} topic={} payload_size={}",
                    identity.producer_id,
                    publish_topic,
                    payload.len()
                );
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

fn extract_template_id(topic: &str) -> Option<Uuid> {
    let mut it = topic.split('/').filter(|s| !s.is_empty());
    let v = it.next()?;
    if v != "v1" {
        return None;
    }

    let _users = it.next()?;
    let _username = it.next()?;
    let _groups = it.next()?;
    let _group = it.next()?;
    let _producers = it.next()?;
    let _producer = it.next()?;
    let section = it.next()?;
    if section != "data_templates" {
        return None;
    }

    let template_id = it.next()?;
    Uuid::parse_str(template_id).ok()
}
