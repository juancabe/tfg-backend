use crate::dao::DbPool;
use crate::dao::location::LocationDao;
use crate::dao::producer::{ProducerAuthCredentials, ProducerDao};
use crate::interface;
use crate::model::models::{self, Group, Producer};
use crate::service::ServiceError;
use uuid::Uuid;

pub struct ProducerService<P: ProducerDao, L: LocationDao> {
    _producer_dao: P,
    _location_dao: L,
}

// P and G must implement From<DbPool>
impl<P, L> From<DbPool> for ProducerService<P, L>
where
    P: ProducerDao + Clone + From<DbPool>,
    L: LocationDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        // We create P and G generically
        let p_dao = P::from(value.clone());
        let l_dao = L::from(value);

        Self::new(p_dao, l_dao)
    }
}

impl<P: ProducerDao + Clone, L: LocationDao + Clone> ProducerService<P, L> {
    pub fn new(producer_dao: P, location_dao: L) -> Self {
        Self {
            _producer_dao: producer_dao,
            _location_dao: location_dao,
        }
    }

    pub fn create_producer(
        &self,
        user_id: Uuid,
        new_producer: interface::types::producer::NewProducer,
    ) -> Result<Producer, ServiceError> {
        // Check if the user is owner of new_producer.group_id

        let loc = self
            ._location_dao
            .create_location(new_producer.location.try_into()?)?;

        let model_new_producer = models::NewProducer {
            location_id: loc.id,
            group_id: new_producer.group_id,
            name: new_producer.name,
            description: new_producer.description,
            forwarder_id: new_producer.forwarder_id,
        };

        self._producer_dao
            .create_producer(model_new_producer)
            .map_err(Into::into)
    }

    /// Only returns the producer when the user has visibility permissions via any of the groups
    pub fn get_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<Producer, ServiceError> {
        todo!();
        // self._access_service.require_access(user_id, producer_id)?;
        // self._producer_dao
        //     .get_by_id(producer_id)?
        //     .ok_or(ServiceError::NotFound)
    }

    pub fn get_visible_producers(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Producer, Vec<Group>)>, ServiceError> {
        todo!()
        // let producers = self._access_service.list_accessible_producers(user_id)?;
        // let mut results = Vec::new();
        //
        // for p in producers {
        //     let groups = self._group_dao.get_producer_groups(p.id, p.main_group_id)?;
        //     results.push((p, groups));
        // }
        //
        // Ok(results)
    }

    pub fn delete_producer(
        &self,
        user_id: Uuid,
        producer_id: Uuid,
    ) -> Result<Producer, ServiceError> {
        todo!()
        // let level = self._access_service.require_access(user_id, producer_id)?;
        // if level < AccessLevel::Admin {
        //     return Err(ServiceError::Forbidden);
        // }
        //
        // let producer = self
        //     ._producer_dao
        //     .get_by_id(producer_id)?
        //     .ok_or(ServiceError::NotFound)?;
        //
        // self._producer_dao.delete(producer_id)?;
        // Ok(producer)
    }

    pub fn share_with_group(
        &self,
        user_id: Uuid,
        producer_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), ServiceError> {
        todo!()
        // let level = self._access_service.require_access(user_id, producer_id)?;
        // if level < AccessLevel::Write {
        //     return Err(ServiceError::Forbidden);
        // }
        // self._group_access_service
        //     .require_owner(user_id, group_id)?;
        //
        // self._producer_dao
        //     .share_with_group(producer_id, group_id)
        //     .map_err(Into::into)
    }

    pub fn rotate_credentials(
        &self,
        user_id: Uuid,
        producer_id: Uuid,
    ) -> Result<ProducerAuthCredentials, ServiceError> {
        // let level = self._access_service.require_access(user_id, producer_id)?;
        // if level < AccessLevel::Write {
        //     return Err(ServiceError::Forbidden);
        // }
        todo!("Implement credential rotation in DAO")
    }
}

