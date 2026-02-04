use crate::dao::{DaoError, DbPool};
use crate::model::models::{Group, GroupJoinRequest, GroupUpdate, NewGroup, NewGroupJoinRequest};
use uuid::Uuid;

// Helper Enum for Permissions
#[derive(Debug, PartialEq)]
pub enum GroupRole {
    Owner,
    Member,
    None,
}

pub trait GroupDao {
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError>;
    fn update_group(&self, group_id: Uuid, update: GroupUpdate) -> Result<Group, DaoError>;
    fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>, DaoError>;

    fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError>;
    fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError>;

    fn check_permission(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, DaoError>;

    fn create_join_request(&self, req: NewGroupJoinRequest) -> Result<GroupJoinRequest, DaoError>;
    // Decision is true (accept) or false (decline)
    fn resolve_join_request(&self, req_id: Uuid, decision: bool) -> Result<(), DaoError>;

    fn toggle_favorite_group(&self, user_id: Uuid, group_id: Uuid) -> Result<bool, DaoError>;
}

pub struct PgGroupDao {
    pool: DbPool,
}

impl PgGroupDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl GroupDao for PgGroupDao {
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError> {
        todo!()
    }

    fn update_group(&self, group_id: Uuid, update: GroupUpdate) -> Result<Group, DaoError> {
        todo!()
    }

    fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>, DaoError> {
        todo!()
    }

    fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError> {
        todo!()
    }

    fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError> {
        todo!()
    }

    fn check_permission(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, DaoError> {
        todo!()
    }

    fn create_join_request(&self, req: NewGroupJoinRequest) -> Result<GroupJoinRequest, DaoError> {
        todo!()
    }

    fn resolve_join_request(&self, req_id: Uuid, decision: bool) -> Result<(), DaoError> {
        todo!()
    }

    fn toggle_favorite_group(&self, user_id: Uuid, group_id: Uuid) -> Result<bool, DaoError> {
        todo!()
    }
}
