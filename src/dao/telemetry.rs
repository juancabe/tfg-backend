use crate::dao::{DaoError, DbPool};
use crate::model::models::{Data, NewData};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub trait TelemetryDao {
    fn ingest_packet(&self, data: NewData) -> Result<Uuid, DaoError>; // Returns ID
    fn batch_ingest(&self, data_items: Vec<NewData>) -> Result<usize, DaoError>; // Returns count
    fn fetch_time_series(
        &self,
        producer_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Data>, DaoError>;
    fn get_latest_packet(&self, producer_id: Uuid) -> Result<Option<Data>, DaoError>;
}

pub struct PgTelemetryDao {
    pool: DbPool,
}

impl PgTelemetryDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl TelemetryDao for PgTelemetryDao {
    fn ingest_packet(&self, new_data: NewData) -> Result<Uuid, DaoError> {
        use crate::schema::data::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(data)
            .values(&new_data)
            .returning(id)
            .get_result(conn)
            .map_err(Into::into)
    }

    fn batch_ingest(&self, data_items: Vec<NewData>) -> Result<usize, DaoError> {
        use crate::schema::data::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(data)
            .values(&data_items)
            .execute(conn)
            .map_err(Into::into)
    }

    fn fetch_time_series(
        &self,
        producer_id_in: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Data>, DaoError> {
        use crate::schema::data::dsl::*;
        let conn = &mut self.pool.get()?;
        data.filter(producer_id.eq(producer_id_in))
            .filter(recv_at.ge(start))
            .filter(recv_at.le(end))
            .order(recv_at.asc())
            .select(Data::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn get_latest_packet(&self, producer_id_in: Uuid) -> Result<Option<Data>, DaoError> {
        use crate::schema::data::dsl::*;
        let conn = &mut self.pool.get()?;
        data.filter(producer_id.eq(producer_id_in))
            .order(recv_at.desc())
            .select(Data::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }
}
