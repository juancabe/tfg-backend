use crate::dao::DbPool;
use crate::dao::group::GroupDao;
use crate::dao::location::LocationDao;
use crate::interface;
use crate::interface::types::group::Group;
use crate::model::models::{self};
use crate::service::ServiceError;
use uuid::Uuid;

// TODO: unit and integration tests
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
        new_group: interface::types::group::NewGroup,
    ) -> Result<Group, ServiceError> {
        let interface::types::group::NewGroup {
            owner_user_id,
            name,
            description,
            is_public,
            location,
        } = new_group;

        let interface::types::location::NewLocation {
            name: loc_name,
            geo_lat,
            geo_long,
        } = location;
        let location = models::NewLocation {
            name: loc_name,
            geo_lat,
            geo_long,
        };

        let location = self.location_dao.create_location(location)?;

        let new_group = models::NewGroup {
            owner_user_id,
            location_id: location.id,
            name,
            description,
            is_public,
        };

        if user_id != owner_user_id {
            Err(ServiceError::Forbidden)?
        }

        self.group_dao
            .create_group(new_group)
            .map_err(Into::into)
            .map(Into::into)
    }

    /// Only returns the group when the user has visibility permissions
    pub fn get_group(&self, user_id: Uuid, group_id: Uuid) -> Result<Group, ServiceError> {
        let group = self.group_dao.get_group(group_id)?;
        if !group.is_public && (group.owner_user_id != user_id) {
            Err(ServiceError::Forbidden)?
        }

        Ok(group.into())
    }

    pub fn get_groups(&self, user_id: Uuid) -> Result<Vec<Group>, ServiceError> {
        Ok(self
            .group_dao
            .get_user_groups(user_id)?
            .into_iter()
            .map(|g| g.into())
            .collect())
    }

    pub fn delete_group(&self, user_id: Uuid, group_id: Uuid) -> Result<(), ServiceError> {
        let group = self.group_dao.get_group(group_id)?;
        if group.owner_user_id != user_id {
            Err(ServiceError::Forbidden)?
        }

        self.group_dao.delete_group(group_id).map_err(Into::into)
    }
}
