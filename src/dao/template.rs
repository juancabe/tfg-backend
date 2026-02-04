use crate::dao::{DaoError, DbPool};
use crate::model::models::{NewPacketTemplate, NewTemplateField, PacketTemplate, TemplateField};
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
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
        use crate::schema::packet_templates::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(packet_templates)
            .values(&new_template)
            .returning(PacketTemplate::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn add_field(&self, field: NewTemplateField) -> Result<TemplateField, DaoError> {
        use crate::schema::template_fields::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(template_fields)
            .values(&field)
            .returning(TemplateField::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_template_structure(&self, search_id: Uuid) -> Result<FullTemplate, DaoError> {
        use crate::schema::{packet_templates, template_fields};
        let conn = &mut self.pool.get()?;

        let template = packet_templates::table
            .find(search_id)
            .select(PacketTemplate::as_select())
            .first(conn)?;

        let fields = template_fields::table
            .filter(template_fields::template_id.eq(search_id))
            .select(TemplateField::as_select())
            .load(conn)?;

        Ok(FullTemplate { template, fields })
    }

    fn list_templates_by_creator(
        &self,
        search_user_id: Uuid,
    ) -> Result<Vec<PacketTemplate>, DaoError> {
        use crate::schema::packet_templates::dsl::*;
        let conn = &mut self.pool.get()?;
        packet_templates
            .filter(creator_user_id.eq(search_user_id))
            .select(PacketTemplate::as_select())
            .load(conn)
            .map_err(Into::into)
    }
}
