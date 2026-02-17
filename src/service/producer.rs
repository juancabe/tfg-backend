use crate::dao::DbPool;
use crate::dao::group::GroupDao;
use crate::dao::location::LocationDao;
use crate::dao::producer::{ProducerAuthCredentials, ProducerDao};
use crate::interface;
use crate::interface::types::group::Group;
use crate::interface::types::producer::Producer;
use crate::model::models::{self};
use crate::service::ServiceError;
use uuid::Uuid;

// TODO: Integration test
pub struct ProducerService<P: ProducerDao, L: LocationDao, G: GroupDao> {
    producer_dao: P,
    location_dao: L,
    group_dao: G,
}

impl<P, L, G> From<DbPool> for ProducerService<P, L, G>
where
    P: ProducerDao + Clone + From<DbPool>,
    L: LocationDao + Clone + From<DbPool>,
    G: GroupDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        // We create P and G generically
        let p_dao = P::from(value.clone());
        let l_dao = L::from(value.clone());
        let g_dao = G::from(value);

        Self::new(p_dao, l_dao, g_dao)
    }
}

impl<P: ProducerDao + Clone, L: LocationDao + Clone, G: GroupDao + Clone> ProducerService<P, L, G> {
    // TODO: Unit test
    pub fn new(producer_dao: P, location_dao: L, group_dao: G) -> Self {
        Self {
            producer_dao,
            location_dao,
            group_dao,
        }
    }

    // TODO: Unit test
    pub fn create_producer(
        &self,
        user_id: Uuid,
        new_producer: interface::types::producer::NewProducer,
    ) -> Result<Producer, ServiceError> {
        {
            let group = self.group_dao.get_group(new_producer.group_id)?;
            if group.owner_user_id != user_id {
                return Err(ServiceError::Forbidden);
            }
        }

        let loc = self
            .location_dao
            .create_location(new_producer.location.try_into()?)?;

        let model_new_producer = models::NewProducer {
            location_id: loc.id,
            group_id: new_producer.group_id,
            name: new_producer.name,
            description: new_producer.description,
            forwarder_id: new_producer.forwarder_id,
        };

        self.producer_dao
            .create_producer(model_new_producer)
            .map_err(Into::into)
            .map(|p| p.into())
    }

    /// Only returns the producer when the user has visibility permissions
    // TODO: Unit test
    pub fn get_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<Producer, ServiceError> {
        let producer = self
            .producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;

        let group = self.group_dao.get_group(producer.group_id)?;

        if group.owner_user_id != user_id && !group.is_public {
            Err(ServiceError::Forbidden)
        } else {
            Ok(producer.into())
        }
    }

    // TODO: Unit test
    pub fn get_owned_producers(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Group, Vec<Producer>)>, ServiceError> {
        self.group_dao
            .get_user_groups(user_id)?
            .into_iter()
            .map(|g| {
                let producers = self
                    .producer_dao
                    .get_by_group_id(g.id)
                    .map(|v| v.into_iter().map(Producer::from).collect())
                    .map_err(ServiceError::from)?;
                Ok((Group::from(g), producers))
            })
            .collect()
    }

    // TODO: Unit test
    pub fn delete_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<(), ServiceError> {
        let producer = self
            .producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;
        let group = self.group_dao.get_group(producer.group_id)?;
        if user_id != group.owner_user_id {
            Err(ServiceError::Forbidden)?;
        }

        self.producer_dao.delete(producer_id)?;
        Ok(())
    }

    // TODO: Unit test (it should assert it panics)
    pub fn rotate_credentials(
        &self,
        _user_id: Uuid,
        _producer_id: Uuid,
    ) -> Result<ProducerAuthCredentials, ServiceError> {
        todo!("Implement credential rotation in DAO")
    }
}
