use crate::dao::group::GroupDao;
use crate::dao::template::{FullTemplate, TemplateDao};
use crate::model::models::{NewPacketTemplate, PacketTemplate};
use crate::service::ServiceError;
use crate::service::group_access::GroupAccessService;
use uuid::Uuid;

pub struct TemplateService<T: TemplateDao, G: GroupDao> {
    _dao: T,
    _group_access: GroupAccessService<G>,
}

impl<T: TemplateDao, G: GroupDao> TemplateService<T, G> {
    pub fn new(dao: T, group_dao: G) -> Self {
        Self {
            _dao: dao,
            _group_access: GroupAccessService::new(group_dao),
        }
    }

    pub fn create_template(
        &self,
        _user_id: Uuid,
        _new_template: NewPacketTemplate,
    ) -> Result<PacketTemplate, ServiceError> {
        todo!()
    }

    pub fn get_template(
        &self,
        _user_id: Uuid,
        _template_id: Uuid,
    ) -> Result<FullTemplate, ServiceError> {
        todo!()
    }

    pub fn list_templates(&self, _user_id: Uuid) -> Result<Vec<PacketTemplate>, ServiceError> {
        todo!()
    }
}
