use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    dao::{group::PgGroupDao, location::PgLocationDao, producer::PgProducerDao},
    helpers::http_app_state::AppState,
    interface::{
        http::HttpApiError,
        types::{
            group::Group,
            producer::{NewProducer, Producer, UpdateProducer},
        },
    },
    service::{auth::JWT, producer::ProducerService},
};

/// Response for listing producers grouped by group.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct GetProducersResponse {
    pairs: Vec<(Group, Vec<Producer>)>,
}

impl From<Vec<(Group, Vec<Producer>)>> for GetProducersResponse {
    fn from(pairs: Vec<(Group, Vec<Producer>)>) -> Self {
        Self { pairs }
    }
}

/// Response for a single producer lookup.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct GetProducerResponse {
    producer: Producer,
}

impl From<Producer> for GetProducerResponse {
    fn from(value: Producer) -> Self {
        Self { producer: value }
    }
}

/// Request body for creating a producer.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct PostProducerBody {
    producer: NewProducer,
}

/// Response for a successful producer creation.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct PostProducerResponse {
    producer: Producer,
}

/// Request body for updating a producer.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub enum PatchProducerBody {
    UpdateMetadata(UpdateProducer),
    Erase,
}

/// Response for a successful producer update.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct PatchProducerResponse {
    producer: Producer,
}

impl From<Producer> for PatchProducerResponse {
    fn from(value: Producer) -> Self {
        Self { producer: value }
    }
}

impl From<Producer> for PostProducerResponse {
    fn from(value: Producer) -> Self {
        Self { producer: value }
    }
}

/// Get all owned producers
///
/// Fetches all producers for the currently authenticated user.
#[utoipa::path(
    get,
    path = "/producers",
    tag = "producers",
    responses((status = 200, description = "Got producers", body = GetProducersResponse)),
    security(("bearer_auth" = []))
)]
pub async fn get_producers(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
) -> Result<Json<GetProducersResponse>, HttpApiError> {
    let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
    let producers = p_service.get_owned_producers(jwt.user_id)?;
    Ok(Json(GetProducersResponse::from(producers)))
}

/// Get producer by ID
///
/// Retrieves the details of a specific producer.
#[utoipa::path(
    get,
    path = "/producers/{id}",
    tag = "producers",
    params(
        ("id" = Uuid, Path, description = "The unique ID of the producer")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Found producer details", body = GetProducerResponse),
    )
)]
pub async fn get_producer_by_id(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GetProducerResponse>, HttpApiError> {
    let user_id = jwt.user_id;
    let producer_id = id;
    let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = pool.into();
    let producer = p_service.get_producer(user_id, producer_id)?;
    Ok(Json(GetProducerResponse::from(producer)))
}

/// Create a new producer
///
/// Creates a new producer and associates it with a specific group.
#[utoipa::path(
    post,
    path = "/producers",
    tag = "producers",
    security(("bearer_auth" = [])),
    request_body = PostProducerBody, // Tells Swagger to show a JSON editor for this struct
    responses(
        (status = 201, description = "Producer created successfully", body = PostProducerResponse),
    )
)]
pub async fn post_producer(
    jwt: JWT,
    State(state): State<AppState>,
    Json(body): Json<PostProducerBody>,
) -> Result<(StatusCode, Json<PostProducerResponse>), HttpApiError> {
    body.producer.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    let new_producer = body.producer;
    let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = state.pool.into();
    let res = p_service.create_producer(jwt.user_id, new_producer)?;

    if let Some(forwarder_id) = res.forwarder_id {
        let _ = state.mqtt_peers_tx.send(forwarder_id);
    }

    Ok((StatusCode::CREATED, Json(res.into())))
}

/// Update producer by ID
///
/// Updates the metadata of a specific producer.
#[utoipa::path(
    patch,
    path = "/producers/{id}",
    tag = "producers",
    params(("id" = Uuid, Path, description = "The unique ID of the producer")),
    security(("bearer_auth" = [])),
    request_body = PatchProducerBody,
    responses((status = 200, description = "Operation applied", body = PatchProducerResponse))
)]
pub async fn patch_producer_by_id(
    jwt: JWT,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchProducerBody>,
) -> Result<Json<PatchProducerResponse>, HttpApiError> {
    let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = state.pool.into();
    let old_producer = p_service.get_producer(jwt.user_id, id)?;

    let PatchProducerBody::UpdateMetadata(update_body) = body else {
        p_service.erase_producer(jwt.user_id, id, state.mqtt_erase_tx)?;
        return Ok(Json(old_producer.into()));
    };

    update_body.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;

    let producer = p_service.update_producer(jwt.user_id, id, update_body)?;

    // Send event for the new forwarder_id
    if let Some(forwarder_id) = producer.forwarder_id {
        let _ = state.mqtt_peers_tx.send(forwarder_id);
    }

    // Send event for the old forwarder_id if it's different
    if let Some(old_forwarder_id) = old_producer.forwarder_id
        && producer.forwarder_id != Some(old_forwarder_id)
        && let Err(e) = state.mqtt_peers_tx.send(old_forwarder_id)
    {
        log::warn!("Failed to broadcast old forwarder update: {}", e);
    }

    Ok(Json(producer.into()))
}

/// Delete producer by ID
///
/// Deletes a specific producer.
#[utoipa::path(
    delete,
    path = "/producers/{id}",
    tag = "producers",
    params(("id" = Uuid, Path, description = "The unique ID of the producer")),
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Producer deleted"))
)]
pub async fn delete_producer_by_id(
    jwt: JWT,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, HttpApiError> {
    let p_service: ProducerService<PgProducerDao, PgLocationDao, PgGroupDao> = state.pool.into();

    // Fetch producer to check if it had a forwarder_id
    let producer = p_service.get_producer(jwt.user_id, id).ok();

    p_service.delete_producer(jwt.user_id, id)?;

    if let Some(p) = producer
        && let Some(forwarder_id) = p.forwarder_id
    {
        let _ = state.mqtt_peers_tx.send(forwarder_id);
    }

    Ok(StatusCode::NO_CONTENT)
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
        // No unit tests for producer_controller at the moment.
        // Unit tests would typically mock dependencies like ProducerService, PgProducerDao, etc.
    }

    mod integration_test {
        use crate::interface::http::producer_controller;

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
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let res = producer_controller::get_producers(jwt, state)
                .await
                .unwrap();
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
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let res = producer_controller::get_producer_by_id(jwt, state, Path(producer.id))
                .await
                .unwrap();
            assert_eq!(res.0.producer.id, producer.id);
        }

        #[tokio::test]
        async fn test_get_producer_by_id_not_found() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let res =
                producer_controller::get_producer_by_id(jwt, state, Path(Uuid::new_v4())).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_producer_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "group");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: Some("Prod Loc".to_string()),
                        geo_lat: Some(1.0),
                        geo_long: Some(2.0),
                    },
                    group_id: group.id,
                    name: "New Prod".to_string(),
                    ble_name: "BLE_New Prod".to_string(),
                    description: "Desc".to_string(),
                    forwarder_id: None,
                    lora_addr: None,
                    forward_lora: None,
                },
            });

            let res = producer_controller::post_producer(jwt, state, body)
                .await
                .unwrap();
            assert_eq!(res.0, StatusCode::CREATED);
            assert_eq!(res.1.0.producer.name, "New Prod");
        }

        #[tokio::test]
        async fn test_post_producer_forbidden() {
            let ctx = setup();
            let owner = create_test_user(&ctx.pool, "owner");
            let other = create_test_user(&ctx.pool, "other");
            let location = create_test_location(&ctx.pool, "loc");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "group");

            let jwt = mock_jwt(other.id, other.login);
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: Some("Prod Loc".to_string()),
                        geo_lat: None,
                        geo_long: None,
                    },
                    group_id: group.id,
                    name: "New Prod".to_string(),
                    ble_name: "BLE_New Prod".to_string(),
                    description: "Desc".to_string(),
                    forwarder_id: None,
                    lora_addr: None,
                    forward_lora: None,
                },
            });

            let res = producer_controller::post_producer(jwt, state, body).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_producer_invalid_name_validation() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "owner_invalid_name");
            let location = create_test_location(&ctx.pool, "loc_invalid_name");
            let group = create_test_group(&ctx.pool, user.id, location.id, "group_invalid_name");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: Some("Prod Loc".to_string()),
                        geo_lat: Some(1.0),
                        geo_long: Some(2.0),
                    },
                    group_id: group.id,
                    name: "bad/name".to_string(),
                    ble_name: "BLE_New Prod".to_string(),
                    description: "Desc".to_string(),
                    forwarder_id: None,
                    lora_addr: None,
                    forward_lora: None,
                },
            });

            let res = producer_controller::post_producer(jwt, state, body).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_patch_producer_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "patch_prod_owner");
            let location = create_test_location(&ctx.pool, "patch_prod_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "patch_prod_group");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "patch_prod");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let new_desc = "Updated desc";
            let res = producer_controller::patch_producer_by_id(
                jwt,
                state,
                Path(producer.id),
                Json(PatchProducerBody::UpdateMetadata(UpdateProducer {
                    description: Some(new_desc.to_string()),
                })),
            )
            .await
            .unwrap();

            assert_eq!(res.0.producer.name, producer.name);
            assert_eq!(res.0.producer.description, new_desc);
        }

        #[tokio::test]
        async fn test_patch_producer_forbidden() {
            let ctx = setup();
            let owner = create_test_user(&ctx.pool, "patch_prod_owner2");
            let other = create_test_user(&ctx.pool, "patch_prod_other2");
            let location = create_test_location(&ctx.pool, "patch_prod_loc2");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "patch_prod_group2");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "patch_prod2");

            let jwt = mock_jwt(other.id, other.login);
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let res = producer_controller::patch_producer_by_id(
                jwt,
                state,
                Path(producer.id),
                Json(PatchProducerBody::UpdateMetadata(UpdateProducer {
                    description: None,
                })),
            )
            .await;

            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_delete_producer_success() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "delete_prod_owner");
            let location = create_test_location(&ctx.pool, "delete_prod_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "delete_prod_group");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "delete_prod");

            let jwt = mock_jwt(user.id, user.login);
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let status = producer_controller::delete_producer_by_id(jwt, state, Path(producer.id))
                .await
                .unwrap();

            assert_eq!(status, StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn test_delete_producer_forbidden() {
            let ctx = setup();
            let owner = create_test_user(&ctx.pool, "delete_prod_owner2");
            let other = create_test_user(&ctx.pool, "delete_prod_other2");
            let location = create_test_location(&ctx.pool, "delete_prod_loc2");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "delete_prod_group2");
            let producer = create_test_producer(&ctx.pool, group.id, location.id, "delete_prod2");

            let jwt = mock_jwt(other.id, other.login);
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let res =
                producer_controller::delete_producer_by_id(jwt, state, Path(producer.id)).await;

            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_producer_lifecycle_broadcasts_mqtt_events() {
            let ctx = setup();
            let user = create_test_user(&ctx.pool, "bcast_owner");
            let location = create_test_location(&ctx.pool, "bcast_loc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "bcast_group");

            // Need an existing forwarder
            let forwarder1 = create_test_producer(&ctx.pool, group.id, location.id, "fwd1");

            let _jwt = mock_jwt(user.id, user.login.clone());

            let (tx, mut rx) = tokio::sync::broadcast::channel(10);
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tx,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // 1. Create with forwarder1
            let body = Json(PostProducerBody {
                producer: NewProducer {
                    location: NewLocation {
                        name: None,
                        geo_lat: None,
                        geo_long: None,
                    },
                    group_id: group.id,
                    name: "Bcast Prod".to_string(),
                    ble_name: "BLE_Bcast".to_string(),
                    description: "desc".to_string(),
                    forwarder_id: Some(forwarder1.id),
                    lora_addr: None,
                    forward_lora: None,
                },
            });

            let res = producer_controller::post_producer(
                mock_jwt(user.id, user.login.clone()),
                state.clone(),
                body,
            )
            .await
            .unwrap();
            let producer_id = res.1.0.producer.id;

            let received1 = rx.try_recv().unwrap();
            assert_eq!(received1, forwarder1.id);

            // 3. Delete
            let _ = producer_controller::delete_producer_by_id(
                mock_jwt(user.id, user.login.clone()),
                state.clone(),
                Path(producer_id),
            )
            .await
            .unwrap();

            let received4 = rx.try_recv().unwrap();
            assert_eq!(received4, forwarder1.id);
        }
    }
}
