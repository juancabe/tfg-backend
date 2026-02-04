use crate::dao::group::GroupDao;
use crate::dao::producer::ProducerDao;
use crate::model::models::Producer;
use crate::service::ServiceError;
use uuid::Uuid;

pub enum AccessLevel {
    Read,
    Write,
    Admin,
}

pub struct ProducerAccessService<P: ProducerDao, G: GroupDao> {
    _producer_dao: P,
    _group_dao: G,
}

impl<P: ProducerDao, G: GroupDao> ProducerAccessService<P, G> {
    pub fn new(producer_dao: P, group_dao: G) -> Self {
        Self {
            _producer_dao: producer_dao,
            _group_dao: group_dao,
        }
    }

    pub fn require_access(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<AccessLevel, ServiceError> {
        todo!()
    }

    pub fn list_accessible_producers(&self, _user_id: Uuid) -> Result<Vec<Producer>, ServiceError> {
        todo!()
    }
}
