use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dao::{group::PgGroupDao, location::PgLocationDao, producer::PgProducerDao},
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::{
            group::Group,
            producer::{NewProducer, Producer},
        },
    },
    service::{auth::JWT, producer::ProducerService},
};

pub struct ProducerController {}

#[derive(Deserialize, Serialize, Debug)]
pub struct GetProducersResponse {
    pairs: Vec<(Group, Vec<Producer>)>,
}

impl From<Vec<(Group, Vec<Producer>)>> for GetProducersResponse {
    fn from(pairs: Vec<(Group, Vec<Producer>)>) -> Self {
        Self { pairs }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GetProducerResponse {
    producer: Producer,
}

impl From<Producer> for GetProducerResponse {
    fn from(value: Producer) -> Self {
        Self { producer: value }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PostProducerBody {
    producer: NewProducer,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PostProducerResponse {
    producer: Producer,
}

impl From<Producer> for PostProducerResponse {
    fn from(value: Producer) -> Self {
        Self { producer: value }
    }
}

// TODO: Docs
impl ProducerController {
    pub fn get_producers(
        jwt: JWT,
        State(AppState { pool }): State<AppState>,
    ) -> Result<Json<GetProducersResponse>, HttpApiError> {
        let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
        let producers = p_service.get_owned_producers(jwt.user_id)?;
        Ok(Json(GetProducersResponse::from(producers)))
    }

    pub fn get_producer_by_id(
        jwt: JWT,
        State(AppState { pool }): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<Json<GetProducerResponse>, HttpApiError> {
        let user_id = jwt.user_id;
        let producer_id = id;
        let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
        let producer = p_service.get_producer(user_id, producer_id)?;
        Ok(Json(GetProducerResponse::from(producer)))
    }

    pub fn post_producer(
        jwt: JWT,
        State(AppState { pool }): State<AppState>,
        Json(body): Json<PostProducerBody>,
    ) -> Result<Json<PostProducerResponse>, HttpApiError> {
        let new_producer = body.producer;
        let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
        let res = p_service.create_producer(jwt.user_id, new_producer)?;
        Ok(Json(res.into()))
    }
}
