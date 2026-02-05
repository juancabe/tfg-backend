use crate::dao::group::{GroupDao, GroupRole};
use crate::service::ServiceError;
use uuid::Uuid;

pub struct GroupAccessService<D: GroupDao> {
    dao: D,
}

impl<D: GroupDao> GroupAccessService<D> {
    pub fn new(dao: D) -> Self {
        Self { dao }
    }

    pub fn require_member(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, ServiceError> {
        let role = self.dao.check_permission(user_id, group_id)?;
        match role {
            GroupRole::None => Err(ServiceError::Forbidden),
            _ => Ok(role),
        }
    }

    pub fn require_admin(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, ServiceError> {
        // Assuming Owner is the only admin role for now
        self.require_owner(user_id, group_id)
    }

    pub fn require_owner(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, ServiceError> {
        let role = self.dao.check_permission(user_id, group_id)?;
        match role {
            GroupRole::Owner => Ok(role),
            _ => Err(ServiceError::Forbidden),
        }
    }

    pub fn get_role(
        &self,
        user_id: Uuid,
        group_id: Uuid,
    ) -> Result<Option<GroupRole>, ServiceError> {
        Ok(Some(self.dao.check_permission(user_id, group_id)?))
    }
}
