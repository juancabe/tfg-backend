use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    dao::{group::PgGroupDao, producer::PgProducerDao, template_data::PgTemplateDataDao},
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::template_data::{PacketTemplateView, ProducerDataQuery, ProducerDataView},
    },
    service::{auth::JWT, template_data::TemplateDataService},
};

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct GetTemplatesResponse {
    pub templates: Vec<PacketTemplateView>,
}

#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct GetProducerDataResponse {
    pub data: ProducerDataView,
}

#[utoipa::path(
    get,
    path = "/templates",
    tag = "templates",
    responses((status = 200, description = "List available packet templates", body = GetTemplatesResponse)),
    security(("bearer_auth" = []))
)]
pub async fn get_templates(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
) -> Result<Json<GetTemplatesResponse>, HttpApiError> {
    let service = TemplateDataService::new(
        PgTemplateDataDao::new(pool.clone()),
        PgProducerDao::new(pool.clone()),
        PgGroupDao::new(pool),
    );

    let templates = service.list_templates(jwt.user_id)?;
    Ok(Json(GetTemplatesResponse { templates }))
}

#[utoipa::path(
    get,
    path = "/producers/{id}/data",
    tag = "templates",
    params(
        ("id" = Uuid, Path, description = "Producer ID"),
        ("template_id" = Option<Uuid>, Query, description = "Optional data template ID"),
        ("from" = Option<String>, Query, description = "RFC3339 lower bound"),
        ("to" = Option<String>, Query, description = "RFC3339 upper bound"),
        ("limit" = Option<u32>, Query, description = "Max samples, bounded to 5000")
    ),
    responses((status = 200, description = "Producer data samples with templates", body = GetProducerDataResponse)),
    security(("bearer_auth" = []))
)]
pub async fn get_producer_data(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ProducerDataQuery>,
) -> Result<Json<GetProducerDataResponse>, HttpApiError> {
    let service = TemplateDataService::new(
        PgTemplateDataDao::new(pool.clone()),
        PgProducerDao::new(pool.clone()),
        PgGroupDao::new(pool),
    );

    let data = service.get_producer_data(jwt.user_id, id, query)?;
    Ok(Json(GetProducerDataResponse { data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        service::auth::JWTKind,
        test_utils::{
            create_test_group, create_test_location, create_test_producer, create_test_user, setup,
        },
    };
    use axum::extract::{Path, Query, State};
    use diesel::prelude::*;

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
    async fn test_get_templates_includes_randomfloat() {
        let ctx = setup();
        let user = create_test_user(&ctx.pool, "tmpl_list");

        let res = get_templates(
            mock_jwt(user.id, user.login),
            State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
            }),
        )
        .await
        .expect("get templates");

        assert!(res.0.templates.iter().any(|t| t.name == "randomfloat"));
    }

    #[tokio::test]
    async fn test_get_producer_data_owner_only() {
        let ctx = setup();
        let owner = create_test_user(&ctx.pool, "data_owner");
        let other = create_test_user(&ctx.pool, "data_other");
        let location = create_test_location(&ctx.pool, "data_loc");
        let group = create_test_group(&ctx.pool, owner.id, location.id, "data_group");
        let producer = create_test_producer(&ctx.pool, group.id, location.id, "data_prod");

        let mut conn = ctx.pool.get().expect("db conn");
        let template_id = Uuid::parse_str(crate::helpers::data_templates::RANDOMFLOAT_TEMPLATE_ID)
            .expect("valid template id");
        diesel::insert_into(crate::model::schema::data::table)
            .values(crate::model::models::NewData {
                producer_id: producer.id,
                template_id,
                payload: serde_json::json!({"value": 0.42}),
            })
            .execute(&mut conn)
            .expect("insert data sample");
        drop(conn);

        let owner_res = get_producer_data(
            mock_jwt(owner.id, owner.login),
            State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
            }),
            Path(producer.id),
            Query(ProducerDataQuery {
                template_id: None,
                from: None,
                to: None,
                limit: Some(100),
            }),
        )
        .await
        .expect("owner can read data");

        assert!(!owner_res.0.data.samples.is_empty());

        let other_res = get_producer_data(
            mock_jwt(other.id, other.login),
            State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
            }),
            Path(producer.id),
            Query(ProducerDataQuery {
                template_id: None,
                from: None,
                to: None,
                limit: Some(100),
            }),
        )
        .await;

        assert!(other_res.is_err());
    }
}
