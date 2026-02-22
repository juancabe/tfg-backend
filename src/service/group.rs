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

pub struct GroupService<G: GroupDao, L: LocationDao> {
    group_dao: G,
    location_dao: L,
}

impl<G, L> From<DbPool> for GroupService<G, L>
where
    G: GroupDao + Clone + From<DbPool>,
    L: LocationDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        let l_dao = L::from(value.clone());
        let g_dao = G::from(value);

        Self::new(g_dao, l_dao)
    }
}

impl<G: GroupDao + Clone, L: LocationDao + Clone> GroupService<G, L> {
    pub fn new(group_dao: G, location_dao: L) -> Self {
        Self {
            group_dao,
            location_dao,
        }
    }

    pub fn create_group(
        &self,
        user_id: Uuid,
        new_producer: interface::types::group::NewGroup,
    ) -> Result<Group, ServiceError> {
        todo!()
        // {
        //     let group = self.group_dao.get_group(new_producer.group_id)?;
        //     if group.owner_user_id != user_id {
        //         return Err(ServiceError::Forbidden);
        //     }
        // }
        //
        // let loc = self
        //     .location_dao
        //     .create_location(new_producer.location.try_into()?)?;
        //
        // let model_new_producer = models::NewProducer {
        //     location_id: loc.id,
        //     group_id: new_producer.group_id,
        //     name: new_producer.name,
        //     description: new_producer.description,
        //     forwarder_id: new_producer.forwarder_id,
        // };
        //
        // self.producer_dao
        //     .create_producer(model_new_producer)
        //     .map_err(Into::into)
        //     .map(|p| p.into())
    }

    /// Only returns the producer when the user has visibility permissions
    pub fn get_group(&self, user_id: Uuid, group_id: Uuid) -> Result<Group, ServiceError> {
        todo!()
        // let producer = self
        //     .producer_dao
        //     .get_by_id(producer_id)?
        //     .ok_or(ServiceError::NotFound)?;
        //
        // let group = self.group_dao.get_group(producer.group_id)?;
        //
        // if group.owner_user_id != user_id && !group.is_public {
        //     Err(ServiceError::Forbidden)
        // } else {
        //     Ok(producer.into())
        // }
    }

    pub fn get_groups(&self, user_id: Uuid) -> Result<Vec<(Group, Vec<Producer>)>, ServiceError> {
        todo!()
        // self.group_dao
        //     .get_user_groups(user_id)?
        //     .into_iter()
        //     .map(|g| {
        //         let producers = self
        //             .producer_dao
        //             .get_by_group_id(g.id)
        //             .map(|v| v.into_iter().map(Producer::from).collect())
        //             .map_err(ServiceError::from)?;
        //         Ok((Group::from(g), producers))
        //     })
        //     .collect()
    }

    pub fn delete_group(&self, user_id: Uuid, group_id: Uuid) -> Result<(), ServiceError> {
        todo!()
        // let producer = self
        //     .producer_dao
        //     .get_by_id(producer_id)?
        //     .ok_or(ServiceError::NotFound)?;
        // let group = self.group_dao.get_group(producer.group_id)?;
        // if user_id != group.owner_user_id {
        //     Err(ServiceError::Forbidden)?;
        // }
        //
        // self.producer_dao.delete(producer_id)?;
        // Ok(())
    }
}
