use std::collections::HashSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    dao::{group::GroupDao, producer::ProducerDao, template_data::TemplateDataDao},
    interface::types::template_data::{
        DataSampleView, PacketTemplateView, ProducerDataQuery, ProducerDataView,
    },
    service::ServiceError,
};

const DEFAULT_LIMIT: u32 = 500;
const MAX_LIMIT: u32 = 5000;

pub struct TemplateDataService<T: TemplateDataDao, P: ProducerDao, G: GroupDao> {
    template_data_dao: T,
    producer_dao: P,
    group_dao: G,
}

impl<T: TemplateDataDao, P: ProducerDao, G: GroupDao> TemplateDataService<T, P, G> {
    pub fn new(template_data_dao: T, producer_dao: P, group_dao: G) -> Self {
        Self {
            template_data_dao,
            producer_dao,
            group_dao,
        }
    }

    pub fn list_templates(&self, user_id: Uuid) -> Result<Vec<PacketTemplateView>, ServiceError> {
        self.template_data_dao
            .list_templates_with_fields(Some(user_id))
            .map(|rows| rows.into_iter().map(PacketTemplateView::from).collect())
            .map_err(Into::into)
    }

    pub fn get_producer_data(
        &self,
        user_id: Uuid,
        producer_id: Uuid,
        query: ProducerDataQuery,
    ) -> Result<ProducerDataView, ServiceError> {
        validate_time_range(query.from, query.to)?;

        let producer = self
            .producer_dao
            .get_by_id(producer_id)?
            .ok_or(ServiceError::NotFound)?;
        let group = self.group_dao.get_group(producer.group_id)?;
        if group.owner_user_id != user_id {
            Err(ServiceError::Forbidden)?;
        }

        let bounded_limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as i64;
        let filter = crate::dao::template_data::DataQueryFilter {
            template_id: query.template_id,
            from: query.from,
            to: query.to,
            limit: bounded_limit,
        };

        let samples = self
            .template_data_dao
            .list_data_for_producer(producer_id, filter)?;

        let template_ids: Vec<Uuid> = samples
            .iter()
            .map(|s| s.template_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let templates = self
            .template_data_dao
            .list_templates_by_ids_with_fields(&template_ids)?;

        Ok(ProducerDataView {
            producer_id,
            samples: samples.into_iter().map(DataSampleView::from).collect(),
            templates: templates
                .into_iter()
                .map(PacketTemplateView::from)
                .collect(),
        })
    }
}

fn validate_time_range(
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<(), ServiceError> {
    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation("'from' must be <= 'to'".to_string()),
        ));
    }
    Ok(())
}
