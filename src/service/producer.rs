use crate::dao::group::GroupDao;
use crate::dao::producer::{ProducerAuthCredentials, ProducerDao};
use crate::model::models::{NewProducer, Producer};
use crate::service::ServiceError;
use crate::service::group_access::GroupAccessService;
use crate::service::producer_access::ProducerAccessService;
use uuid::Uuid;

pub struct ProducerService<P: ProducerDao, G: GroupDao> {
    _producer_dao: P,
    _group_dao: G,
    _access_service: ProducerAccessService<P, G>,
    _group_access_service: GroupAccessService<G>,
}

impl<P: ProducerDao + Clone, G: GroupDao + Clone> ProducerService<P, G> {
    pub fn new(producer_dao: P, group_dao: G) -> Self {
        Self {
            _access_service: ProducerAccessService::new(producer_dao.clone(), group_dao.clone()),
            _group_access_service: GroupAccessService::new(group_dao.clone()),
            _producer_dao: producer_dao,
            _group_dao: group_dao,
        }
    }

    pub fn create_producer(
        &self,
        _user_id: Uuid,
        _new_producer: NewProducer,
    ) -> Result<Producer, ServiceError> {
        todo!()
    }

    pub fn get_producer(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<Producer, ServiceError> {
        todo!()
    }

    pub fn delete_producer(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<Producer, ServiceError> {
        todo!()
    }

    pub fn share_with_group(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
        _group_id: Uuid,
    ) -> Result<(), ServiceError> {
        todo!()
    }

    pub fn rotate_credentials(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<ProducerAuthCredentials, ServiceError> {
        todo!()
    }
}
