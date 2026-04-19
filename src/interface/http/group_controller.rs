use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dao::{group::PgGroupDao, location::PgLocationDao},
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::group::{Group, NewGroup, UpdateGroup},
    },
    service::{auth::JWT, group::GroupService},
};

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct GetGroupsResponse {
    pub groups: Vec<Group>,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct GetGroupResponse {
    pub group: Group,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct PostGroupBody {
    pub group: NewGroup,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct PostGroupResponse {
    pub group: Group,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct PatchGroupBody {
    pub group: UpdateGroup,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct PatchGroupResponse {
    pub group: Group,
}

#[utoipa::path(
    get,
    path = "/groups",
    tag = "groups",
    responses(
        (status = 200, description = "List groups", body = GetGroupsResponse),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_groups(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
) -> Result<Json<GetGroupsResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let groups = g_service.get_groups(jwt.user_id)?;
    Ok(Json(GetGroupsResponse { groups }))
}

#[utoipa::path(
    get,
    path = "/groups/{id}",
    tag = "groups",
    params(
        ("id" = Uuid, Path, description = "Group ID")
    ),
    responses(
        (status = 200, description = "Group details", body = GetGroupResponse),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_group_by_id(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GetGroupResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let group = g_service.get_group(jwt.user_id, id)?;
    Ok(Json(GetGroupResponse { group }))
}

#[utoipa::path(
    post,
    path = "/groups",
    tag = "groups",
    request_body = PostGroupBody,
    responses(
        (status = 201, description = "Group created", body = PostGroupResponse),
    ),
    security(("bearer_auth" = []))
)]
pub async fn post_group(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Json(body): Json<PostGroupBody>,
) -> Result<(StatusCode, Json<PostGroupResponse>), HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let group = g_service.create_group(jwt.user_id, body.group)?;
    Ok((StatusCode::CREATED, Json(PostGroupResponse { group })))
}

#[utoipa::path(
    patch,
    path = "/groups/{id}",
    tag = "groups",
    params(("id" = Uuid, Path, description = "Group ID")),
    request_body = PatchGroupBody,
    responses((status = 200, description = "Group updated", body = PatchGroupResponse)),
    security(("bearer_auth" = []))
)]
pub async fn patch_group_by_id(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchGroupBody>,
) -> Result<Json<PatchGroupResponse>, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    let group = g_service.update_group(jwt.user_id, id, body.group)?;
    Ok(Json(PatchGroupResponse { group }))
}

#[utoipa::path(
    delete,
    path = "/groups/{id}",
    tag = "groups",
    params(("id" = Uuid, Path, description = "Group ID")),
    responses((status = 204, description = "Group deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_group_by_id(
    jwt: JWT,
    State(AppState { pool }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpApiError> {
    let g_service: GroupService<PgGroupDao, PgLocationDao> = pool.into();
    g_service.delete_group(jwt.user_id, id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::auth::JWTKind;
    use crate::test_utils::{create_test_group, create_test_location, create_test_user, setup};
    use axum::extract::{Path, State};

    fn mock_jwt(user_id: Uuid, login: String) -> JWT {
        let (iat, exp): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
            JWTKind::Session.get_iat_exp();
        JWT::new(
            1,
            login,
            user_id,
            JWTKind::Session,
            iat.timestamp(),
            exp.timestamp(),
        )
    }

    #[tokio::test]
    async fn test_patch_group_success() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "patch_group_owner");
        let location = create_test_location(&ctx.pool, "patch_group_loc");
        let group = create_test_group(&ctx.pool, user.id, location.id, "patch_group");

        let res = patch_group_by_id(
            mock_jwt(user.id, user.login),
            State(AppState {
                pool: ctx.pool.clone(),
            }),
            Path(group.id),
            Json(PatchGroupBody {
                group: UpdateGroup {
                    name: Some("new_group_name".to_string()),
                    description: Some("new desc".to_string()),
                    is_public: Some(true),
                },
            }),
        )
        .await
        .unwrap();

        assert_eq!(res.0.group.name, "new_group_name");
        assert!(res.0.group.is_public);
    }

    #[tokio::test]
    async fn test_patch_group_forbidden() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "patch_group_owner2");
        let other = create_test_user(&ctx.pool, "patch_group_other2");
        let location = create_test_location(&ctx.pool, "patch_group_loc2");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "patch_group2");

        let res = patch_group_by_id(
            mock_jwt(other.id, other.login),
            State(AppState {
                pool: ctx.pool.clone(),
            }),
            Path(group.id),
            Json(PatchGroupBody {
                group: UpdateGroup {
                    name: Some("new_group_name".to_string()),
                    description: None,
                    is_public: None,
                },
            }),
        )
        .await;

        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_delete_group_success() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "delete_group_owner");
        let location = create_test_location(&ctx.pool, "delete_group_loc");
        let group = create_test_group(&ctx.pool, user.id, location.id, "delete_group");

        let status = delete_group_by_id(
            mock_jwt(user.id, user.login),
            State(AppState {
                pool: ctx.pool.clone(),
            }),
            Path(group.id),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_group_forbidden() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "delete_group_owner2");
        let other = create_test_user(&ctx.pool, "delete_group_other2");
        let location = create_test_location(&ctx.pool, "delete_group_loc2");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "delete_group2");

        let res = delete_group_by_id(
            mock_jwt(other.id, other.login),
            State(AppState {
                pool: ctx.pool.clone(),
            }),
            Path(group.id),
        )
        .await;

        assert!(res.is_err());
    }
}
