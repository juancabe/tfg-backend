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
    pub async fn get_producers(
        jwt: JWT,
        State(AppState { pool }): State<AppState>,
    ) -> Result<Json<GetProducersResponse>, HttpApiError> {
        let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
        let producers = p_service.get_owned_producers(jwt.user_id)?;
        Ok(Json(GetProducersResponse::from(producers)))
    }

    pub async fn get_producer_by_id(
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

    pub async fn post_producer(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::types::location::NewLocation;
    use crate::service::auth::JWTKind;
    use crate::test_utils::{
        create_test_group, create_test_location, create_test_producer, create_test_user, setup,
    };
    use axum::extract::{Path, State};

    mod unit_test {
        // No unit tests for ProducerController at the moment.
        // Unit tests would typically mock dependencies like ProducerService, PgProducerDao, etc.
    }

    mod integration_test {
        use super::*;

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
        async fn test_get_producers_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "group");
            create_test_producer(&ctx.pool, group.id, location.id, "prod");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState { pool: ctx.pool });

            let res = ProducerController::get_producers(jwt, state).await.unwrap();
            assert_eq!(res.0.pairs.len(), 1);
            assert_eq!(res.0.pairs[0].1.len(), 1);
        }

        #[tokio::test]
        async fn test_get_producer_by_id_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "group");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "prod");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState { pool: ctx.pool });

            let res = ProducerController::get_producer_by_id(jwt, state, Path(producer.id))
                .await
                .unwrap();
            assert_eq!(res.0.producer.id, producer.id);
        }

        #[tokio::test]
        async fn test_get_producer_by_id_not_found() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState { pool: ctx.pool });

            let res =
                ProducerController::get_producer_by_id(jwt, state, Path(Uuid::new_v4())).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_producer_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "group");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState { pool: ctx.pool });

            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: Some("Prod Loc".to_string()),
                        geo_lat: Some(1.0),
                        geo_long: Some(2.0),
                    },
                    group_id: group.id,
                    name: "New Prod".to_string(),
                    description: "Desc".to_string(),
                    forwarder_id: None,
                },
            });

            let res = ProducerController::post_producer(jwt, state, body)
                .await
                .unwrap();
            assert_eq!(res.0.producer.name, "New Prod");
        }

        #[tokio::test]
        async fn test_post_producer_forbidden() {
            let ctx = setup();
            let owner = create_test_user(&ctx.pool, "owner");
            let other = create_test_user(&ctx.pool, "other");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "group");

            let jwt = mock_jwt(other.id, other.login);
            let state = State(AppState { pool: ctx.pool });

            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: Some("Prod Loc".to_string()),
                        geo_lat: None,
                        geo_long: None,
                    },
                    group_id: group.id,
                    name: "New Prod".to_string(),
                    description: "Desc".to_string(),
                    forwarder_id: None,
                },
            });

            let res = ProducerController::post_producer(jwt, state, body).await;
            assert!(res.is_err());
        }
    }
}
