use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    helpers::http_app_state::AppState, interface::http::ApiError, model::models, service::auth::JWT,
};

pub struct ProducerController {}

// TODO: Don't wrap it, complete new type
#[derive(Serialize, Deserialize, Debug)]
pub struct Producer(models::Producer);

// TODO: Don't wrap it, complete new type
#[derive(Serialize, Deserialize, Debug)]
pub struct Group(models::Group);

#[derive(Deserialize, Serialize, Debug)]
pub struct GetProducersResponse {
    producers: Vec<(Producer, Vec<Uuid>)>,
    groups: Vec<Group>,
}

impl From<Vec<(Producer, Vec<Group>)>> for GetProducersResponse {
    fn from(_pairs: Vec<(Producer, Vec<Group>)>) -> Self {
        todo!()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GetProducerResponse {
    producer: Producer,
}

impl From<Producer> for GetProducerResponse {
    fn from(_value: Producer) -> Self {
        todo!()
    }
}

// TODO: Create type
#[derive(Deserialize, Serialize, Debug)]
pub struct NewProducer {}

#[derive(Deserialize, Serialize, Debug)]
pub struct PostProducerBody {
    producer: NewProducer,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PostProducerResponse {
    producer: Producer,
}

impl ProducerController {
    pub fn get_producers(
        jwt: JWT,
        State(AppState { pool: _ }): State<AppState>,
    ) -> Result<Json<GetProducersResponse>, ApiError> {
        let _user_id = jwt.user_id;
        todo!()
        // let p_service: ProducerService<PgProducerDao, PgGroupDao, PgLocationDao> = pool.into();
        // let producers = p_service.get_visible_producers(user_id)?;
        // Ok(Json(GetProducersResponse::from(producers)))
    }

    pub fn get_producer_by_id(
        jwt: JWT,
        State(AppState { pool: _ }): State<AppState>,
        Path(id): Path<Uuid>,
    ) -> Result<Json<GetProducerResponse>, ApiError> {
        let _user_id = jwt.user_id;
        let _producer_id = id;
        todo!()
        // let p_service: ProducerService<PgProducerDao, PgGroupDao, PgLocationDao> = pool.into();
        // let producer = p_service.get_producer(user_id, producer_id)?;
        // Ok(Json(GetProducerResponse::from(producer)))
    }

    pub fn post_producer(
        _jwt: JWT,
        State(AppState { pool: _ }): State<AppState>,
        Json(body): Json<PostProducerBody>,
    ) -> Result<Json<PostProducerResponse>, ApiError> {
        let _new_producer = body.producer;
        todo!()
        // let p_service: ProducerService<PgProducerDao, PgGroupDao, PgLocationDao> = pool.into();
        // let res = p_service.create_producer(jwt.user_id, new_producer)?;
        // Ok(Json(res.into()))
    }
}
