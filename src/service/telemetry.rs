use crate::dao::group::GroupDao;
use crate::dao::producer::ProducerDao;
use crate::dao::telemetry::TelemetryDao;
use crate::model::models::Data;
use crate::service::ServiceError;
use crate::service::producer_access::ProducerAccessService;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct TelemetryService<T: TelemetryDao, P: ProducerDao, G: GroupDao> {
    _dao: T,
    _access_service: ProducerAccessService<P, G>,
}

impl<T: TelemetryDao, P: ProducerDao, G: GroupDao> TelemetryService<T, P, G> {
    pub fn new(dao: T, producer_dao: P, group_dao: G) -> Self {
        Self {
            _dao: dao,
            _access_service: ProducerAccessService::new(producer_dao, group_dao),
        }
    }

    pub fn get_time_series(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<Data>, ServiceError> {
        todo!()
    }

    pub fn get_latest(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<Option<Data>, ServiceError> {
        todo!()
    }
}
