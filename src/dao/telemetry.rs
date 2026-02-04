use crate::dao::{DaoError, DbPool};
use crate::model::models::{Data, NewData};
use chrono::{DateTime, Utc};
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
    fn ingest_packet(&self, data: NewData) -> Result<Uuid, DaoError> {
        todo!()
    }

    fn batch_ingest(&self, data_items: Vec<NewData>) -> Result<usize, DaoError> {
        todo!()
    }

    fn fetch_time_series(
        &self,
        producer_id: Uuid,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Data>, DaoError> {
        todo!()
    }

    fn get_latest_packet(&self, producer_id: Uuid) -> Result<Option<Data>, DaoError> {
        todo!()
    }
}
