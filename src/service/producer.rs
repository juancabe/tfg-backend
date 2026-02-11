use crate::dao::DbPool;
use crate::dao::group::GroupDao;
use crate::dao::producer::{ProducerAuthCredentials, ProducerDao};
use crate::model::models::{Group, GroupVisibility, NewProducer, Producer, User};
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

// P and G must implement From<DbPool>
impl<P, G> From<DbPool> for ProducerService<P, G>
where
    P: ProducerDao + Clone + From<DbPool>,
    G: GroupDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        // We create P and G generically
        let p_dao = P::from(value.clone());
        let g_dao = G::from(value);

        Self::new(p_dao, g_dao)
    }
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

    /// Only returns the producer when the user has visibility permissions via any of the groups
    pub fn get_producer(&self, user_id: Uuid, producer_id: Uuid) -> Result<Producer, ServiceError> {
        // TODO: Write tests
        let producer = self
            ._producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;
        let p_groups: Vec<Group> = self
            ._group_dao
            .get_producer_groups(producer_id, producer.main_group_id)?;
        let u_groups = self._group_dao.get_user_groups(user_id)?;

        for pgroup in p_groups {
            // Check if user is group owner
            if pgroup.owner_user_id == producer.id {
                return Ok(producer);
            }

            // Check if group is public
            if matches!(pgroup.visibility, GroupVisibility::AllCanView) {
                return Ok(producer);
            }

            // Then we have to check if user group status has visibility
            if u_groups.iter().any(|ug| {
                ug.visibility
                    .is_satisfied_by(user_id, ug.owner_user_id, ug.id == pgroup.id)
            }) {
                return Ok(producer);
            }
        }

        return Err(ServiceError::Forbidden);
    }

    pub fn get_visible_producers(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Producer, Vec<Group>)>, ServiceError> {
        // TODO: Write tests
        let groups = self._group_dao.get_user_groups(user_id)?;
        groups
            .into_iter()
            .map(|g| {
                // If the group is not visible for user, return it empty
                if !g.visibility.is_satisfied_by(user_id, g.owner_user_id, true) {
                    return Ok((g, vec![]));
                }

                match self._producer_dao.get_by_group_id(g.id) {
                    Ok(producers) => Ok((g, producers)),
                    Err(e) => Err(ServiceError::from(e)),
                }
            })
            .collect()
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
