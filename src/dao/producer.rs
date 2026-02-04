use crate::dao::{DaoError, DbPool};
use crate::model::models::{
    InternetAuth, MqttBrokerAuth, NewInternetAuth, NewProducer, NewProducerForGroup, Producer,
};
use chrono::Utc;
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub enum ProducerAuthCredentials {
    Internet(InternetAuth),
    Mqtt(MqttBrokerAuth),
    None,
}

pub trait ProducerDao {
    fn create_producer(&self, new_producer: NewProducer) -> Result<Producer, DaoError>;
    fn get_by_id(&self, id: Uuid) -> Result<Option<Producer>, DaoError>;
    fn update_last_seen(&self, id: Uuid) -> Result<(), DaoError>;

    fn share_with_group(&self, producer_id: Uuid, group_id: Uuid) -> Result<(), DaoError>;
    fn set_forwarder(&self, producer_id: Uuid, forwarder_id: Uuid) -> Result<(), DaoError>;

    fn set_internet_auth(&self, producer_id: Uuid, key_b64: String) -> Result<(), DaoError>;
    // Using a tuple or a struct for creds here for simplicity
    fn set_mqtt_auth(
        &self,
        producer_id: Uuid,
        creds: (String, i16, String, String, String),
    ) -> Result<(), DaoError>;

    fn get_auth_credentials(&self, producer_id: Uuid) -> Result<ProducerAuthCredentials, DaoError>;
}

pub struct PgProducerDao {
    pool: DbPool,
}

impl PgProducerDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ProducerDao for PgProducerDao {
    fn create_producer(&self, new_producer: NewProducer) -> Result<Producer, DaoError> {
        use crate::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(producers)
            .values(&new_producer)
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_by_id(&self, search_id: Uuid) -> Result<Option<Producer>, DaoError> {
        use crate::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;
        producers
            .filter(id.eq(search_id))
            .select(Producer::as_select())
            .load(conn)
            .map(|r| r.into_iter().next())
            .map_err(Into::into)
    }

    fn update_last_seen(&self, search_id: Uuid) -> Result<(), DaoError> {
        use crate::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        let now = Utc::now();

        diesel::update(producers.find(search_id))
            .set(last_seen.eq(now))
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
            .and_then(|r| DaoError::update_expect_eq(&r.last_seen, &now, "update_last_seen"))
    }

    fn share_with_group(&self, producer_id_in: Uuid, group_id_in: Uuid) -> Result<(), DaoError> {
        use crate::schema::producers_for_groups::dsl::*;
        let conn = &mut self.pool.get()?;

        diesel::insert_into(producers_for_groups)
            .values(&NewProducerForGroup {
                producer_id: producer_id_in,
                group_id: group_id_in,
            })
            .execute(conn)
            .map_err(Into::into)
            .map(|_| ())
    }

    fn set_forwarder(&self, producer_id: Uuid, forwarder_id_in: Uuid) -> Result<(), DaoError> {
        use crate::schema::producers::dsl::*;
        let conn = &mut self.pool.get()?;

        diesel::update(producers.find(producer_id))
            .set(forwarder_id.eq(forwarder_id))
            .returning(Producer::as_returning())
            .get_result(conn)
            .map_err(Into::into)
            .and_then(|r| {
                DaoError::update_expect_eq(&r.forwarder_id, &Some(forwarder_id_in), "set_forwarder")
            })
    }

    fn set_internet_auth(&self, producer_id_in: Uuid, key_b64: String) -> Result<(), DaoError> {
        use crate::schema::internet_auths::dsl::*;
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
        use crate::schema::{internet_auths, mqtt_broker_auths, producers};
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
            Some((Some(a), Some(b))) => Err(DaoError::Other(format!(
                "Found a producer with two auths: {a:?}, {b:?}"
            ))),
            None => Err(DaoError::NotFound),
        }
    }
}
