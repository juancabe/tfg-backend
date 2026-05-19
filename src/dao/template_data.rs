//! Template and data query DAO operations.

use crate::dao::{DaoError, DbPool};
use crate::model::models::{Data, PacketTemplate, TemplateField};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

/// Filter parameters for querying producer data.
#[derive(Debug, Clone)]
pub struct DataQueryFilter {
    /// Optional template ID to filter by.
    pub template_id: Option<Uuid>,
    /// Optional start time for the data range.
    pub from: Option<DateTime<Utc>>,
    /// Optional end time for the data range.
    pub to: Option<DateTime<Utc>>,
    /// Maximum number of records to return.
    pub limit: i64,
}

/// Trait defining template and data query database operations.
pub trait TemplateDataDao {
    /// Lists all packet templates with their fields, optionally filtered by creator.
    fn list_templates_with_fields(
        &self,
        creator_user_id_filter: Option<Uuid>,
    ) -> Result<Vec<(PacketTemplate, Vec<TemplateField>)>, DaoError>;

    /// Lists data records for a producer, applying the given filter.
    fn list_data_for_producer(
        &self,
        producer_id_in: Uuid,
        filter: DataQueryFilter,
    ) -> Result<Vec<Data>, DaoError>;

    /// Lists packet templates by their IDs along with their fields.
    fn list_templates_by_ids_with_fields(
        &self,
        template_ids: &[Uuid],
    ) -> Result<Vec<(PacketTemplate, Vec<TemplateField>)>, DaoError>;
}

/// PostgreSQL implementation of [`TemplateDataDao`].
#[derive(Clone)]
pub struct PgTemplateDataDao {
    pool: DbPool,
}

impl From<DbPool> for PgTemplateDataDao {
    fn from(value: DbPool) -> Self {
        Self::new(value)
    }
}

impl PgTemplateDataDao {
    /// Creates a new [`PgTemplateDataDao`] with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl TemplateDataDao for PgTemplateDataDao {
    fn list_templates_with_fields(
        &self,
        creator_user_id_filter: Option<Uuid>,
    ) -> Result<Vec<(PacketTemplate, Vec<TemplateField>)>, DaoError> {
        use crate::model::schema::packet_templates::dsl as pt;
        use crate::model::schema::template_fields::dsl as tf;

        let conn = &mut self.pool.get()?;

        let templates: Vec<PacketTemplate> = match creator_user_id_filter {
            Some(user_id) => pt::packet_templates
                .filter(
                    pt::creator_user_id
                        .eq(user_id)
                        .or(pt::creator_user_id.is_null()),
                )
                .order_by(pt::name.asc())
                .select(PacketTemplate::as_select())
                .load(conn)?,
            None => pt::packet_templates
                .order_by(pt::name.asc())
                .select(PacketTemplate::as_select())
                .load(conn)?,
        };

        let all_fields: Vec<TemplateField> = tf::template_fields
            .select(TemplateField::as_select())
            .load(conn)?;

        Ok(templates
            .into_iter()
            .map(|t| {
                let t_fields = all_fields
                    .iter()
                    .filter(|f| f.template_id == t.id)
                    .cloned()
                    .collect();
                (t, t_fields)
            })
            .collect())
    }

    fn list_data_for_producer(
        &self,
        producer_id_in: Uuid,
        filter: DataQueryFilter,
    ) -> Result<Vec<Data>, DaoError> {
        use crate::model::schema::data::dsl::*;

        let conn = &mut self.pool.get()?;

        let mut query = data.into_boxed().filter(producer_id.eq(producer_id_in));

        if let Some(template_id_filter) = filter.template_id {
            query = query.filter(template_id.eq(template_id_filter));
        }
        if let Some(from_filter) = filter.from {
            query = query.filter(recv_at.ge(from_filter));
        }
        if let Some(to_filter) = filter.to {
            query = query.filter(recv_at.le(to_filter));
        }

        query
            .order_by(recv_at.desc())
            .limit(filter.limit)
            .select(Data::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn list_templates_by_ids_with_fields(
        &self,
        template_ids: &[Uuid],
    ) -> Result<Vec<(PacketTemplate, Vec<TemplateField>)>, DaoError> {
        use crate::model::schema::packet_templates::dsl as pt;
        use crate::model::schema::template_fields::dsl as tf;

        if template_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = &mut self.pool.get()?;

        let templates: Vec<PacketTemplate> = pt::packet_templates
            .filter(pt::id.eq_any(template_ids))
            .order_by(pt::name.asc())
            .select(PacketTemplate::as_select())
            .load(conn)?;

        let fields: Vec<TemplateField> = tf::template_fields
            .filter(tf::template_id.eq_any(template_ids))
            .select(TemplateField::as_select())
            .load(conn)?;

        Ok(templates
            .into_iter()
            .map(|t| {
                let t_fields = fields
                    .iter()
                    .filter(|f| f.template_id == t.id)
                    .cloned()
                    .collect();
                (t, t_fields)
            })
            .collect())
    }
}
