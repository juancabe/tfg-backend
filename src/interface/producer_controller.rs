use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::{
    dao::{group::PgGroupDao, producer::PgProducerDao},
    helpers::state::AppState,
    interface::ApiError,
    model::models,
    service::{auth::JWT, producer::ProducerService},
};

pub struct ProducerController {}

pub struct Producer(models::Producer);

impl From<models::Producer> for Producer {
    fn from(value: models::Producer) -> Self {
        Self(value)
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GetProducersResponse {
    producers: Vec<Producer>,
}

impl ProducerController {
    pub fn get_producers(
        jwt: JWT,
        State(AppState { pool }): State<AppState>,
    ) -> Result<Json<GetProducersResponse>, ApiError> {
        let user_id = jwt.user_id;
        let p_service: ProducerService<PgProducerDao, PgGroupDao> = pool.into();
        let producers = p_service.get_visible_producers(user_id)?;
        Ok(Json(GetProducersResponse {
            producers: producers.into_iter().map(|p| p.into()).collect(),
        }))
    }
}
