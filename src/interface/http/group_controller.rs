use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dao::{group::PgGroupDao, location::PgLocationDao},
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::group::{Group, NewGroup},
    },
    service::{auth::JWT, group::GroupService},
};

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct GetGroupsResponse {
    pub groups: Vec<Group>,
}

pub struct GetGroupResponse {
    pub group: Group,
}

pub struct PostGroupBody {
    pub group: NewGroup,
}

pub struct PostGroupResponse {
    pub group: Group,
}

pub async fn get_groups(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
) -> Result<Json<GetGroupsResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let groups = g_service.get_groups(jwt.user_id)?;
    Ok(Json(GetGroupsResponse { groups }))
}

pub async fn get_group_by_id(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GetGroupResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let group = g_service.get_group(jwt.user_id, id)?;
    Ok(Json(GetGroupResponse { group }))
}

pub async fn post_group(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Json(body): Json<PostGroupBody>,
) -> Result<Json<PostGroupResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let group = g_service.create_group(jwt.user_id, body.group)?;
    Ok(Json(PostGroupResponse { group }))
}
