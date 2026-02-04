use crate::dao::group::GroupDao;
use crate::model::models::{Group, GroupJoinRequest, GroupUpdate, NewGroup};
use crate::service::ServiceError;
use crate::service::group_access::GroupAccessService;
use uuid::Uuid;

// Placeholder types
pub struct GroupDetails {}

pub struct GroupService<D: GroupDao> {
    _dao: D,
    _raccess_service: GroupAccessService<D>,
}

impl<D: GroupDao + Clone> GroupService<D> {
    pub fn new(dao: D) -> Self {
        Self {
            _raccess_service: GroupAccessService::new(dao.clone()),
            _dao: dao,
        }
    }

    pub fn create_group(
        &self,
        _user_id: Uuid,
        _new_group: NewGroup,
    ) -> Result<Group, ServiceError> {
        todo!()
    }

    pub fn request_access(
        &self,
        _user_id: Uuid,
        _group_id: Uuid,
    ) -> Result<GroupJoinRequest, ServiceError> {
        todo!()
    }

    pub fn resolve_request(
        &self,
        _user_id: Uuid,
        _req_id: Uuid,
        _decision: bool,
    ) -> Result<(), ServiceError> {
        todo!()
    }

    pub fn get_group_details(
        &self,
        _user_id: Uuid,
        _group_id: Uuid,
    ) -> Result<GroupDetails, ServiceError> {
        todo!()
    }

    pub fn update_group(
        &self,
        _user_id: Uuid,
        _group_id: Uuid,
        _update: GroupUpdate,
    ) -> Result<GroupDetails, ServiceError> {
        todo!()
    }

    pub fn toggle_favorite(&self, _user_id: Uuid, _group_id: Uuid) -> Result<(), ServiceError> {
        todo!()
    }
}
