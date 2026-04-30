use crate::{
    dao::{DaoError, DbPool},
    model::models::{MqttBrokerAuth, NewData},
};
use diesel::prelude::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DelegatedPublishTargetRow {
    pub producer_id: Uuid,
    pub topic: String,
}

pub trait MqttDao {
    fn find_broker_auth_by_username(
        &self,
        username_in: &str,
    ) -> Result<Option<MqttBrokerAuth>, DaoError>;

    fn find_delegated_publish_target(
        &self,
        forwarder_id: Uuid,
        mqtt_username_in: &str,
        group_name_in: &str,
        producer_name_in: &str,
    ) -> Result<Option<DelegatedPublishTargetRow>, DaoError>;

    fn insert_data(&self, new_data: NewData) -> Result<(), DaoError>;

    fn list_forwarder_peer_keys(&self, forwarder_id: Uuid)
    -> Result<Vec<(Uuid, String)>, DaoError>;
}

#[derive(Clone)]
pub struct PgMqttDao {
    pool: DbPool,
}

impl From<DbPool> for PgMqttDao {
    fn from(value: DbPool) -> Self {
        Self::new(value)
    }
}

impl PgMqttDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl MqttDao for PgMqttDao {
    fn find_broker_auth_by_username(
        &self,
        username_in: &str,
    ) -> Result<Option<MqttBrokerAuth>, DaoError> {
        use crate::model::schema::mqtt_broker_auths::dsl::*;

        let conn = &mut self.pool.get()?;

        mqtt_broker_auths
            .filter(username.eq(username_in))
            .select(MqttBrokerAuth::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn find_delegated_publish_target(
        &self,
        forwarder_id_in: Uuid,
        mqtt_username_in: &str,
        group_name_in: &str,
        producer_name_in: &str,
    ) -> Result<Option<DelegatedPublishTargetRow>, DaoError> {
        use crate::model::schema::{groups, mqtt_broker_auths, producers};

        let conn = &mut self.pool.get()?;

        producers::table
            .inner_join(groups::table.on(groups::id.eq(producers::group_id)))
            .inner_join(
                mqtt_broker_auths::table.on(mqtt_broker_auths::producer_id.eq(producers::id)),
            )
            .filter(producers::forwarder_id.eq(Some(forwarder_id_in)))
            .filter(producers::name.eq(producer_name_in))
            .filter(groups::name.eq(group_name_in))
            .filter(mqtt_broker_auths::username.eq(mqtt_username_in))
            .select((producers::id, mqtt_broker_auths::topic))
            .first::<(Uuid, String)>(conn)
            .optional()
            .map(|row| {
                row.map(|(producer_id, topic)| DelegatedPublishTargetRow { producer_id, topic })
            })
            .map_err(Into::into)
    }

    fn insert_data(&self, new_data: NewData) -> Result<(), DaoError> {
        use crate::model::schema::data::dsl::*;

        let conn = &mut self.pool.get()?;

        diesel::insert_into(data)
            .values(&new_data)
            .execute(conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn list_forwarder_peer_keys(
        &self,
        forwarder_id_in: Uuid,
    ) -> Result<Vec<(Uuid, String)>, DaoError> {
        use crate::model::schema::{internet_auths, producers};

        let conn = &mut self.pool.get()?;

        producers::table
            .inner_join(internet_auths::table.on(internet_auths::producer_id.eq(producers::id)))
            .filter(producers::forwarder_id.eq(Some(forwarder_id_in)))
            .select((producers::id, internet_auths::x25519_pub_key_bytes_base64))
            .load::<(Uuid, String)>(conn)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        helpers::data_templates::RANDOMFLOAT_TEMPLATE_ID,
        model::{
            models::{NewInternetAuth, NewMqttBrokerAuth},
            schema::{data, internet_auths, mqtt_broker_auths, producers},
        },
        test_utils::{
            create_test_group, create_test_location, create_test_producer, create_test_user, setup,
        },
    };

    mod unit_test {
        use super::*;

        #[test]
        fn test_find_broker_auth_by_username_found() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_lookup_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_lookup_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_lookup_group");
            let producer = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_lookup_prod");

            let mut conn = ctx.pool.get().expect("db conn");
            diesel::insert_into(mqtt_broker_auths::table)
                .values(NewMqttBrokerAuth {
                    producer_id: producer.id,
                    broker_host: "localhost".to_string(),
                    broker_port: 1883,
                    username: "lookup-user".to_string(),
                    password_hash: "hashed-pass".to_string(),
                    topic: "/v1/topic".to_string(),
                    subscribe_topic: "/v1/sub".to_string(),
                })
                .execute(&mut conn)
                .expect("insert mqtt auth");
            drop(conn);

            let row = dao
                .find_broker_auth_by_username("lookup-user")
                .expect("find by username");

            let row = row.expect("expected mqtt auth row");
            assert_eq!(row.producer_id, producer.id);
            assert_eq!(row.username, "lookup-user");
            assert_eq!(row.topic, "/v1/topic");
        }

        #[test]
        fn test_find_broker_auth_by_username_not_found() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let row = dao
                .find_broker_auth_by_username("missing-user")
                .expect("find by username");

            assert!(row.is_none());
        }

        #[test]
        fn test_find_delegated_publish_target_found() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_delegate_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_delegate_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_delegate_group");

            let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_forwarder");
            let delegated = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_delegated");

            let topic = format!(
                "/v1/users/{}/groups/{}/producers/{}/data_templates/{}",
                "stored-user", group.name, delegated.name, RANDOMFLOAT_TEMPLATE_ID
            );

            let mut conn = ctx.pool.get().expect("db conn");
            diesel::update(producers::table.find(delegated.id))
                .set(producers::forwarder_id.eq(Some(forwarder.id)))
                .execute(&mut conn)
                .expect("set delegated forwarder");

            diesel::insert_into(mqtt_broker_auths::table)
                .values(NewMqttBrokerAuth {
                    producer_id: delegated.id,
                    broker_host: "localhost".to_string(),
                    broker_port: 1883,
                    username: "stored-user".to_string(),
                    password_hash: "unused".to_string(),
                    topic: topic.clone(),
                    subscribe_topic: "/v1/forbidden".to_string(),
                })
                .execute(&mut conn)
                .expect("insert delegated mqtt auth");
            drop(conn);

            let row = dao
                .find_delegated_publish_target(
                    forwarder.id,
                    "stored-user",
                    group.name.as_str(),
                    delegated.name.as_str(),
                )
                .expect("find delegated target");

            let row = row.expect("expected delegated target");
            assert_eq!(row.producer_id, delegated.id);
            assert_eq!(row.topic, topic);
        }

        #[test]
        fn test_find_delegated_publish_target_mismatch_returns_none() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_delegate_mismatch_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_delegate_mismatch_loc");
            let group =
                create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_delegate_mismatch_group");

            let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_forwarder");
            let delegated = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_delegated");

            let mut conn = ctx.pool.get().expect("db conn");
            diesel::update(producers::table.find(delegated.id))
                .set(producers::forwarder_id.eq(Some(forwarder.id)))
                .execute(&mut conn)
                .expect("set delegated forwarder");

            diesel::insert_into(mqtt_broker_auths::table)
                .values(NewMqttBrokerAuth {
                    producer_id: delegated.id,
                    broker_host: "localhost".to_string(),
                    broker_port: 1883,
                    username: "stored-user".to_string(),
                    password_hash: "unused".to_string(),
                    topic: "/v1/topic".to_string(),
                    subscribe_topic: "/v1/forbidden".to_string(),
                })
                .execute(&mut conn)
                .expect("insert delegated mqtt auth");
            drop(conn);

            let row = dao
                .find_delegated_publish_target(
                    forwarder.id,
                    "other-user",
                    group.name.as_str(),
                    delegated.name.as_str(),
                )
                .expect("find delegated target");

            assert!(row.is_none());
        }

        #[test]
        fn test_insert_data_success() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_insert_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_insert_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_insert_group");
            let producer = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_insert_prod");

            let template_id = Uuid::parse_str(RANDOMFLOAT_TEMPLATE_ID).expect("valid template id");

            dao.insert_data(NewData {
                producer_id: producer.id,
                template_id,
                payload: serde_json::json!({"value": 12.34}),
            })
            .expect("insert data");

            let mut conn = ctx.pool.get().expect("db conn");
            let count = data::table
                .filter(data::producer_id.eq(producer.id))
                .filter(data::template_id.eq(template_id))
                .count()
                .get_result::<i64>(&mut conn)
                .expect("count data rows");

            assert_eq!(count, 1);
        }

        #[test]
        fn test_insert_data_invalid_template_fails() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_insert_fail_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_insert_fail_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_insert_fail_group");
            let producer = create_test_producer(&ctx.pool, group.id, loc.id, "mqtt_insert_fail");

            let result = dao.insert_data(NewData {
                producer_id: producer.id,
                template_id: Uuid::new_v4(),
                payload: serde_json::json!({"value": 0}),
            });

            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }

        #[test]
        fn test_list_forwarder_peer_keys_filters_by_forwarder() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_peers_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_peers_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_peers_group");

            let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder");
            let peer_a = create_test_producer(&ctx.pool, group.id, loc.id, "peer_a");
            let peer_b = create_test_producer(&ctx.pool, group.id, loc.id, "peer_b");
            let unrelated = create_test_producer(&ctx.pool, group.id, loc.id, "unrelated");

            let mut conn = ctx.pool.get().expect("db conn");
            diesel::update(producers::table.find(peer_a.id))
                .set(producers::forwarder_id.eq(Some(forwarder.id)))
                .execute(&mut conn)
                .expect("set peer_a forwarder");
            diesel::update(producers::table.find(peer_b.id))
                .set(producers::forwarder_id.eq(Some(forwarder.id)))
                .execute(&mut conn)
                .expect("set peer_b forwarder");

            diesel::insert_into(internet_auths::table)
                .values(NewInternetAuth {
                    producer_id: peer_a.id,
                    x25519_pub_key_bytes_base64: "key_a".to_string(),
                })
                .execute(&mut conn)
                .expect("insert peer_a key");
            diesel::insert_into(internet_auths::table)
                .values(NewInternetAuth {
                    producer_id: peer_b.id,
                    x25519_pub_key_bytes_base64: "key_b".to_string(),
                })
                .execute(&mut conn)
                .expect("insert peer_b key");
            diesel::insert_into(internet_auths::table)
                .values(NewInternetAuth {
                    producer_id: unrelated.id,
                    x25519_pub_key_bytes_base64: "key_unrelated".to_string(),
                })
                .execute(&mut conn)
                .expect("insert unrelated key");
            drop(conn);

            let mut rows = dao
                .list_forwarder_peer_keys(forwarder.id)
                .expect("list forwarder peer keys");
            rows.sort_by_key(|(id, _)| *id);

            assert_eq!(rows.len(), 2);
            assert!(
                rows.iter()
                    .any(|(id, key)| *id == peer_a.id && key == "key_a")
            );
            assert!(
                rows.iter()
                    .any(|(id, key)| *id == peer_b.id && key == "key_b")
            );
            assert!(!rows.iter().any(|(id, _)| *id == unrelated.id));
        }

        #[test]
        fn test_list_forwarder_peer_keys_empty() {
            let ctx = setup();
            let dao = PgMqttDao::new(ctx.pool.clone());

            let owner = create_test_user(&ctx.pool, "mqtt_peers_empty_owner");
            let loc = create_test_location(&ctx.pool, "mqtt_peers_empty_loc");
            let group = create_test_group(&ctx.pool, owner.id, loc.id, "mqtt_peers_empty_group");
            let forwarder = create_test_producer(&ctx.pool, group.id, loc.id, "forwarder");

            let rows = dao
                .list_forwarder_peer_keys(forwarder.id)
                .expect("list forwarder peer keys");

            assert!(rows.is_empty());
        }
    }
}
