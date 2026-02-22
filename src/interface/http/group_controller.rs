use axum::{
    Json,
    extract::{Path, State},
};
use diesel::sql_types::Uuid;

use crate::{helpers::http_app_state::AppState, interface::http::HttpApiError, service::auth::JWT};

pub struct GetGroupsResponse {}
pub struct GetGroupResponse {}
pub struct PostGroupBody {}

pub async fn get_groups(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
) -> Result<Json<GetGroupsResponse>, HttpApiError> {
    // let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
    // let producers = p_service.get_owned_producers(jwt.user_id)?;
    // Ok(Json(GetProducersResponse::from(producers)))
    todo!()
}

pub async fn get_group_by_id(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GetGroupResponse>, HttpApiError> {
    // let user_id = jwt.user_id;
    // let producer_id = id;
    // let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
    // let producer = p_service.get_producer(user_id, producer_id)?;
    // Ok(Json(GetProducerResponse::from(producer)))
    todo!()
}

pub async fn post_group(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Json(body): Json<PostGroupBody>,
) -> Result<Json<PostGroupBody>, HttpApiError> {
    // let new_producer = body.producer;
    // let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
    // let res = p_service.create_producer(jwt.user_id, new_producer)?;
    // Ok(Json(res.into()))
    todo!()
}
