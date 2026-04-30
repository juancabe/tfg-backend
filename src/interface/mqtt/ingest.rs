use ntex_mqtt::v5::{self, codec::PublishAckReason};
#[cfg(test)]
use uuid::Uuid;

use crate::{dao::DbPool, service::mqtt::MqttIngestService as DomainMqttIngestService};

use super::auth::{ProducerIdentity, PublishAuthorization};

pub const MAX_PAYLOAD_BYTES: usize = 4096;

/// Ingest service for accepted MQTT publishes.
///
/// This type is the integration point where publish payloads can later be
/// routed into domain services (persistence, parsing, command fan-out, etc.).
#[derive(Clone)]
pub struct MqttIngestService {
    ingest_service: DomainMqttIngestService,
}

impl MqttIngestService {
    /// Creates a new ingest service with shared DB access.
    pub fn new(pool: DbPool) -> Self {
        let ingest_service = DomainMqttIngestService::from(pool);
        Self { ingest_service }
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

        let authorization = match self
            .ingest_service
            .authorize_publish(identity, publish_topic)
        {
            Some(auth) => auth,
            None => {
                return publish.ack().reason_code(PublishAckReason::NotAuthorized);
            }
        };

        let ingest_target = match authorization {
            PublishAuthorization::Direct(target) | PublishAuthorization::Delegated(target) => {
                target
            }
        };

        if ingest_target.template_id.is_none() {
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
                if let Some(template_id) = ingest_target.template_id {
                    if let Err(err) = self
                        .ingest_service
                        .persist_payload(&ingest_target, payload.as_ref())
                    {
                        log::warn!(
                            "MQTT ingest persist failed producer_id={} topic={} template_id={} error={err:?}",
                            ingest_target.producer_id,
                            publish_topic,
                            template_id,
                        );
                        return publish
                            .ack()
                            .reason_code(PublishAckReason::UnspecifiedError);
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

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::data_templates::RANDOMFLOAT_TEMPLATE_ID;

    #[test]
    fn test_extract_template_id_success() {
        let topic = format!(
            "/v1/users/user-a/groups/group-a/producers/prod-a/data_templates/{}",
            RANDOMFLOAT_TEMPLATE_ID
        );
        let extracted = extract_template_id(&topic);
        assert_eq!(
            extracted,
            Some(Uuid::parse_str(RANDOMFLOAT_TEMPLATE_ID).expect("valid uuid"))
        );
    }

    #[test]
    fn test_extract_template_id_wrong_section() {
        let topic = format!(
            "/v1/users/user-a/groups/group-a/producers/prod-a/other/{}",
            RANDOMFLOAT_TEMPLATE_ID
        );
        assert_eq!(extract_template_id(&topic), None);
    }
}
