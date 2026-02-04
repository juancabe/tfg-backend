use crate::dao::{DaoError, DbPool};
use crate::model::models::{NewPacketTemplate, NewTemplateField, PacketTemplate, TemplateField};
use uuid::Uuid;

// Struct to hold a full template definition (template + fields)
pub struct FullTemplate {
    pub template: PacketTemplate,
    pub fields: Vec<TemplateField>,
}

pub trait TemplateDao {
    fn create_template(&self, new_template: NewPacketTemplate) -> Result<PacketTemplate, DaoError>;
    fn add_field(&self, field: NewTemplateField) -> Result<TemplateField, DaoError>;
    fn get_template_structure(&self, template_id: Uuid) -> Result<FullTemplate, DaoError>;
    fn list_templates_by_creator(&self, user_id: Uuid) -> Result<Vec<PacketTemplate>, DaoError>;
}

pub struct PgTemplateDao {
    pool: DbPool,
}

impl PgTemplateDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl TemplateDao for PgTemplateDao {
    fn create_template(&self, new_template: NewPacketTemplate) -> Result<PacketTemplate, DaoError> {
        todo!()
    }

    fn add_field(&self, field: NewTemplateField) -> Result<TemplateField, DaoError> {
        todo!()
    }

    fn get_template_structure(&self, template_id: Uuid) -> Result<FullTemplate, DaoError> {
        todo!()
    }

    fn list_templates_by_creator(&self, user_id: Uuid) -> Result<Vec<PacketTemplate>, DaoError> {
        todo!()
    }
}
