use crate::dao::{DaoError, DbPool};
use crate::model::models::{InternetAuth, MqttBrokerAuth, NewInternetAuth, NewProducer, Producer};
use chrono::{SubsecRound, Utc};
use diesel::prelude::*;
use uuid::Uuid;

#[derive(Debug)]
pub enum ProducerAuthCredentials {
    Internet(InternetAuth),
    Mqtt(MqttBrokerAuth),
    None,
}

pub trait ProducerDao {
    // TODO: Document these
    fn create_producer(&self, new_producer: NewProducer) -> Result<Producer, DaoError>;
    fn get_by_id(&self, id: Uuid) -> Result<Option<Producer>, DaoError>;
    fn get_by_group_id(&self, id: Uuid) -> Result<Vec<Producer>, DaoError>;
    fn update_last_seen(&self, id: Uuid) -> Result<(), DaoError>;

    fn update_group(&self, producer_id_in: Uuid, group_id_in: Uuid) -> Result<(), DaoError>;

    fn set_forwarder(&self, producer_id: Uuid, forwarder_id: Uuid) -> Result<(), DaoError>;

    fn set_internet_auth(&self, producer_id: Uuid, key_b64: String) -> Result<(), DaoError>;
    // Using a tuple or a struct for creds here for simplicity
    fn set_mqtt_auth(
        &self,
        producer_id: Uuid,
        creds: (String, i16, String, String, String),
    ) -> Result<(), DaoError>;

    fn get_auth_credentials(&self, producer_id: Uuid) -> Result<ProducerAuthCredentials, DaoError>;
    fn delete(&self, id: Uuid) -> Result<(), DaoError>;
}

#[derive(Clone)]
pub struct PgProducerDao {
    pool: DbPool,
}

impl From<DbPool> for PgProducerDao {
    fn from(value: DbPool) -> Self {
        Self::new(value)
    }
}

impl PgProducerDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProducerDao for PgProducerDao {
    fn create_producer(&self, new_producer: NewProducer) -> Result<Producer, DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(producers)
            .values(&new_producer)
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_by_id(&self, search_id: Uuid) -> Result<Option<Producer>, DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;
        producers
            .filter(id.eq(search_id))
            .select(Producer::as_select())
            .load(conn)
            .map(|r| r.into_iter().next())
            .map_err(Into::into)
    }

    fn get_by_group_id(&self, group_id_in: Uuid) -> Result<Vec<Producer>, DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        producers
            .filter(group_id.eq(group_id_in))
            .select(Producer::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn update_last_seen(&self, search_id: Uuid) -> Result<(), DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        let now = Utc::now().round_subsecs(6);

        diesel::update(producers.find(search_id))
            .set(last_seen.eq(now))
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
            .and_then(|r| DaoError::update_expect_eq(&r.last_seen, &now, "update_last_seen"))
    }

    fn update_group(&self, producer_id_in: Uuid, group_id_in: Uuid) -> Result<(), DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        diesel::update(producers.find(producer_id_in))
            .set(group_id.eq(group_id_in))
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
            .and_then(|r| DaoError::update_expect_eq(&r.group_id, &group_id_in, "set_group_id"))
    }

    fn set_forwarder(&self, producer_id: Uuid, forwarder_id_in: Uuid) -> Result<(), DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        diesel::update(producers.find(producer_id))
            .set(forwarder_id.eq(forwarder_id_in))
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
            .and_then(|r| {
                DaoError::update_expect_eq(&r.forwarder_id, &Some(forwarder_id_in), "set_forwarder")
            })
    }

    fn set_internet_auth(&self, producer_id_in: Uuid, key_b64: String) -> Result<(), DaoError> {
        use crate::model::schema::internet_auths::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(internet_auths)
            .values(&NewInternetAuth {
                producer_id: producer_id_in,
                x25519_pub_key_bytes_base64: key_b64,
            })
            .execute(conn)
            .map_err(Into::into)
            .map(|_| ())
    }

    fn set_mqtt_auth(
        &self,
        _producer_id_in: Uuid,
        _creds: (String, i16, String, String, String),
    ) -> Result<(), DaoError> {
        todo!()
        // use crate::schema::mqtt_broker_auths::dsl::*;
        // let conn = &mut self.pool.get()?;
        // diesel::insert_into(mqtt_broker_auths)
        //     .values(&NewMqttBrokerAuth {
        //         producer_id: producer_id_in,
        //         broker_host: todo!(),
        //         broker_port: todo!(),
        //         username: todo!(),
        //         password: todo!(),
        //         topic: todo!(),
        //     })
        //     .execute(conn)
        //     .map_err(Into::into)
        //     .map(|_| ())
    }

    fn get_auth_credentials(
        &self,
        producer_id_in: Uuid,
    ) -> Result<ProducerAuthCredentials, DaoError> {
        use crate::model::schema::{internet_auths, mqtt_broker_auths, producers};
        let conn = &mut self.pool.get()?;

        // Perform two Left Joins:
        // producers <- left join -> internet_auths
        // producers <- left join -> mqtt_broker_auths
        let result = producers::table
            .find(producer_id_in)
            .left_join(internet_auths::table)
            .left_join(mqtt_broker_auths::table)
            .select((
                internet_auths::all_columns.nullable(),
                mqtt_broker_auths::all_columns.nullable(),
            ))
            .first::<(
                Option<crate::model::models::InternetAuth>,
                Option<crate::model::models::MqttBrokerAuth>,
            )>(conn)
            .optional()?;

        match result {
            Some((Some(internet), None)) => Ok(ProducerAuthCredentials::Internet(internet)),
            Some((None, Some(mqtt))) => Ok(ProducerAuthCredentials::Mqtt(mqtt)),
            Some((None, None)) => Ok(ProducerAuthCredentials::None), // Producer exists, but no auth set
            Some((Some(a), Some(b))) => Err(DaoError::OtherRare(format!(
                "Found a producer with two auths: {a:?}, {b:?}"
            ))),
            None => Err(DaoError::NotFound),
        }
    }

    fn delete(&self, producer_id: Uuid) -> Result<(), DaoError> {
        use crate::model::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;
        match diesel::delete(producers.find(producer_id)).execute(conn)? {
            1 => Ok(()),
            0 => Err(DaoError::NotFound),
            rows => Err(DaoError::OtherRare(format!(
                "group_id_in delete affected rows: {rows}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::models::NewProducer;
    use crate::test_utils::{
        create_test_group, create_test_location, create_test_producer, create_test_user, setup,
    };
    use uuid::Uuid;

    mod unit_test {
        use super::*;

        #[test]
        fn test_create_producer() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "prod_creator");
            let location = create_test_location(&ctx.pool, "prod_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "prod_group");

            let new_producer = NewProducer {
                location_id: location.id,
                group_id: group.id,
                name: "test_producer".to_string(),
                ble_name: "ble_test_producer".to_string(),
                description: "A test producer".to_string(),
                forwarder_id: None,
            };

            let result = dao
                .create_producer(new_producer)
                .expect("Failed to create producer");

            assert_eq!(result.name, "test_producer");
            assert_eq!(result.group_id, group.id);
        }

        #[test]
        fn test_create_producer_invalid_group() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let location = create_test_location(&ctx.pool, "prod_loc_inv");

            let new_producer = NewProducer {
                location_id: location.id,
                group_id: Uuid::new_v4(), // Invalid group ID
                name: "test_producer_invalid".to_string(),
                ble_name: "ble_test_producer_invalid".to_string(),
                description: "Invalid group".to_string(),
                forwarder_id: None,
            };

            let result = dao.create_producer(new_producer);
            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }

        #[test]
        fn test_get_by_id() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "get_id_user");
            let location = create_test_location(&ctx.pool, "get_id_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "get_id_group");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "get_id_prod");

            let found = dao.get_by_id(producer.id).expect("Failed to get producer");
            assert!(found.is_some());
            assert_eq!(found.unwrap().id, producer.id);
        }

        #[test]
        fn test_get_by_id_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let found = dao
                .get_by_id(Uuid::new_v4())
                .expect("Failed to get producer");
            assert!(found.is_none());
        }

        #[test]
        fn test_get_by_group_id() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "get_grp_user");
            let location = create_test_location(&ctx.pool, "get_grp_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "get_grp_group");
            let p1 = create_test_producer(&ctx.pool, group.id, location.id, "p1");
            let p2 = create_test_producer(&ctx.pool, group.id, location.id, "p2");

            let producers = dao
                .get_by_group_id(group.id)
                .expect("Failed to get producers");
            assert_eq!(producers.len(), 2);
            assert!(producers.iter().any(|p| p.id == p1.id));
            assert!(producers.iter().any(|p| p.id == p2.id));
        }

        #[test]
        fn test_update_last_seen() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "last_seen_user");
            let location = create_test_location(&ctx.pool, "last_seen_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "last_seen_grp");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "last_seen_prod");

            let old_last_seen = producer.last_seen;

            // Ensure some time passes or just rely on clock
            // For robustness in tests where execution is fast, we might want to check for inequality primarily
            // but since resolution is microsecond, it should be fine.

            dao.update_last_seen(producer.id)
                .expect("Failed to update last seen");

            let updated = dao.get_by_id(producer.id).unwrap().unwrap();
            // Just check it changed.
            assert_ne!(updated.last_seen, old_last_seen);
        }

        #[test]
        fn test_update_group() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "upd_grp_user");
            let location = create_test_location(&ctx.pool, "upd_grp_loc");
            let group1 = create_test_group(&ctx.pool, user.id, location.id, "grp1");
            let group2 = create_test_group(&ctx.pool, user.id, location.id, "grp2");
            let producer = create_test_producer(&ctx.pool, group1.id, location.id, "upd_grp_prod");

            dao.update_group(producer.id, group2.id)
                .expect("Failed to update group");

            let updated = dao.get_by_id(producer.id).unwrap().unwrap();
            assert_eq!(updated.group_id, group2.id);
        }

        #[test]
        fn test_set_forwarder() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "fwd_user");
            let location = create_test_location(&ctx.pool, "fwd_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "fwd_grp");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");
            let forwarder = create_test_producer(&ctx.pool, group.id, location.id, "fwd");

            dao.set_forwarder(producer.id, forwarder.id)
                .expect("Failed to set forwarder");

            let updated = dao.get_by_id(producer.id).unwrap().unwrap();
            assert_eq!(updated.forwarder_id, Some(forwarder.id));
        }

        #[test]
        fn test_set_internet_auth() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "auth_user");
            let location = create_test_location(&ctx.pool, "auth_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "auth_grp");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "auth_prod");
            let key = "some_base64_key".to_string();

            dao.set_internet_auth(producer.id, key.clone())
                .expect("Failed to set auth");

            let creds = dao
                .get_auth_credentials(producer.id)
                .expect("Failed to get creds");
            match creds {
                ProducerAuthCredentials::Internet(auth) => {
                    assert_eq!(auth.x25519_pub_key_bytes_base64, key);
                }
                _ => panic!("Expected Internet auth"),
            }
        }

        #[test]
        fn test_get_auth_credentials_none() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "no_auth_user");
            let location = create_test_location(&ctx.pool, "no_auth_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "no_auth_grp");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "no_auth_prod");

            let creds = dao
                .get_auth_credentials(producer.id)
                .expect("Failed to get creds");
            match creds {
                ProducerAuthCredentials::None => {}
                _ => panic!("Expected None auth"),
            }
        }

        #[test]
        fn test_delete() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "del_user");
            let location = create_test_location(&ctx.pool, "del_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "del_grp");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "del_prod");

            dao.delete(producer.id).expect("Failed to delete producer");

            let found = dao.get_by_id(producer.id).expect("Failed to check");
            assert!(found.is_none());
        }

        #[test]
        fn test_update_last_seen_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            let result = dao.update_last_seen(Uuid::new_v4());

            // update_last_seen uses update_expect_eq which uses returning, if no row found, it's NotFound
            // Diesel returns NotFound if 0 rows updated when using get_result
            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_update_group_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "ugnf_user");
            let location = create_test_location(&ctx.pool, "ugnf_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "ugnf_grp");

            let result = dao.update_group(Uuid::new_v4(), group.id);

            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_set_forwarder_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            let result = dao.set_forwarder(Uuid::new_v4(), Uuid::new_v4());

            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_set_internet_auth_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            // This is an INSERT. If producer_id doesn't exist, it should be a ForeignKeyViolation
            let result = dao.set_internet_auth(Uuid::new_v4(), "key".to_string());

            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }

        #[test]
        fn test_delete_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            let result = dao.delete(Uuid::new_v4());

            assert!(matches!(result, Err(DaoError::NotFound)));
        }
    }

    mod integration_test {
        use super::*;

        #[test]
        fn test_producer_lifecycle_flow() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            // 1. Setup environment
            let user = create_test_user(&ctx.pool, "lifecycle");
            let location = create_test_location(&ctx.pool, "lifecycle");
            let group1 = create_test_group(&ctx.pool, user.id, location.id, "lifecycle1");
            let group2 = create_test_group(&ctx.pool, user.id, location.id, "lifecycle2");

            // 2. Create producer
            let producer = dao
                .create_producer(NewProducer {
                    location_id: location.id,
                    group_id: group1.id,
                    name: "lifecycle_prod".to_string(),
                    ble_name: "ble_lifecycle_prod".to_string(),
                    description: "desc".to_string(),
                    forwarder_id: None,
                })
                .expect("Create failed");

            // 3. Update group
            dao.update_group(producer.id, group2.id)
                .expect("Update group failed");

            // 4. Set auth
            let key = "base64key".to_string();
            dao.set_internet_auth(producer.id, key.clone())
                .expect("Set auth failed");

            // 5. Verify credentials and state
            let creds = dao
                .get_auth_credentials(producer.id)
                .expect("Get creds failed");
            match creds {
                ProducerAuthCredentials::Internet(auth) => {
                    assert_eq!(auth.x25519_pub_key_bytes_base64, key);
                }
                _ => panic!("Expected Internet auth"),
            }

            let updated = dao
                .get_by_id(producer.id)
                .expect("Get producer failed")
                .unwrap();
            assert_eq!(updated.group_id, group2.id);

            // 6. Delete
            dao.delete(producer.id).expect("Delete failed");
            let found = dao.get_by_id(producer.id).expect("Final check failed");
            assert!(found.is_none());
        }

        #[test]
        fn test_get_auth_credentials_not_found() {
            let ctx = setup();
            let dao = PgProducerDao::new(ctx.pool.clone());

            let result = dao.get_auth_credentials(Uuid::new_v4());
            assert!(matches!(result, Err(DaoError::NotFound)));
        }
    }
}
