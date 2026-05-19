//! http
//!
//! Implementation of the HTTP server using ['axum'] and ['tokio']

use axum::{
    Json, Router,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::io;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    dao::DbPool, helpers::http_app_state::AppState, interface::http::docs::ApiDoc,
    service::ServiceError,
};

// Simple HTTP logging middleware: method, route (or path), status and latency
async fn log_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let route = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|m| m.as_str().to_string())
        .unwrap_or(path.clone());
    let start = std::time::Instant::now();
    let resp = next.run(req).await;
    let status = resp.status();
    let elapsed_ms = start.elapsed().as_millis();
    if status.is_server_error() || elapsed_ms > 1000 {
        log::warn!(
            "{} {} -> {} ({}ms)",
            method,
            route,
            status.as_u16(),
            elapsed_ms
        );
    } else {
        log::info!(
            "{} {} -> {} ({}ms)",
            method,
            route,
            status.as_u16(),
            elapsed_ms
        );
    }
    resp
}

/// Errors that can occur during HTTP server initialization.
#[derive(Debug, Error)]
pub enum HttpError {
    /// A server initialization error occurred.
    #[error("Initialization error occurred")]
    InitError(String),
    /// A Tokio IO error occurred.
    #[error("Tokio IO error")]
    TokioIOError(#[from] io::Error),
    /// A database pool creation error occurred.
    #[error("DbPool creation error")]
    DbPoolError(#[from] r2d2::Error),
}

/// Authentication endpoint handlers.
pub mod auth_controller;
/// OpenAPI/Swagger documentation generation.
pub mod docs;
/// JWT token extractor for axum.
pub mod extractor;
/// Group management endpoint handlers.
pub mod group_controller;
/// Producer management endpoint handlers.
pub mod producer_controller;
/// Device provisioning endpoint handlers.
pub mod provisioning_controller;
/// Template data endpoint handlers.
pub mod template_data_controller;

/// HTTP server entry point.
pub struct Http {}

impl Http {
    /// Starts the HTTP server on the given address with the provided database pool and MQTT peer channel.
    pub async fn run_http_server(
        addr: SocketAddr,
        pool: DbPool,
        mqtt_peers_tx: tokio::sync::broadcast::Sender<uuid::Uuid>,
    ) -> Result<(), HttpError> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        log::info!("Created TcpListener on {:?}", addr);

        let state = AppState {
            pool,
            mqtt_peers_tx,
        };

        let openapi_doc = ApiDoc::openapi();

        let app = Router::new()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi_doc))
            .route("/auth/login", post(auth_controller::post_login))
            .route("/auth/register", post(auth_controller::post_register))
            .route("/auth/refresh", post(auth_controller::post_refresh))
            .route(
                "/auth/change-password",
                post(auth_controller::post_change_password),
            )
            .route(
                "/auth/password-reset-request",
                post(auth_controller::post_password_reset_request),
            )
            .route(
                "/auth/password-reset",
                post(auth_controller::post_password_reset),
            )
            .route("/auth/verify", get(auth_controller::get_verify_email))
            .route(
                "/producers",
                get(producer_controller::get_producers).post(producer_controller::post_producer),
            )
            .route(
                "/producers/{id}",
                get(producer_controller::get_producer_by_id)
                    .patch(producer_controller::patch_producer_by_id)
                    .delete(producer_controller::delete_producer_by_id),
            )
            .route(
                "/producers/{id}/data",
                get(template_data_controller::get_producer_data),
            )
            .route(
                "/groups",
                get(group_controller::get_groups).post(group_controller::post_group),
            )
            .route("/templates", get(template_data_controller::get_templates))
            .route(
                "/groups/{id}",
                get(group_controller::get_group_by_id)
                    .patch(group_controller::patch_group_by_id)
                    .delete(group_controller::delete_group_by_id),
            )
            .route(
                "/provisioning/pending",
                get(provisioning_controller::get_pending)
                    .post(provisioning_controller::post_pending),
            )
            .route(
                "/provisioning/groups/{group_id}/available-forwarders",
                get(provisioning_controller::get_available_forwarders),
            )
            .route(
                "/provisioning/pending/{id}/confirm",
                post(provisioning_controller::post_pending_confirm),
            )
            .route(
                "/provisioning/pending/{id}/cancel",
                post(provisioning_controller::post_pending_cancel),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(log_middleware));

        log::info!("Created HTTP Router");
        log::info!("Swagger UI available at http://{:?}/swagger-ui", addr);

        axum::serve(listener, app).await.map_err(Into::into)
    }
}

/// Errors that can occur during HTTP request handling.
#[derive(Debug, Error)]
pub enum HttpApiError {
    /// The JSON payload was rejected.
    #[error("JSON payload rejected")]
    JsonRejection(#[from] JsonRejection),
    /// A service-layer error occurred.
    #[error("Service error: {0}")]
    ServiceError(#[from] ServiceError),
}

/// How error responses will be serialized
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    message: String,
}

impl From<String> for ErrorResponse {
    fn from(val: String) -> Self {
        ErrorResponse { message: val }
    }
}

impl IntoResponse for HttpApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message, err) = match &self {
            HttpApiError::JsonRejection(json_rejection) => (
                json_rejection.status(),
                json_rejection.body_text(),
                Some(self),
            ),
            HttpApiError::ServiceError(service_err) => {
                let status = match service_err {
                    ServiceError::Forbidden => StatusCode::FORBIDDEN,
                    ServiceError::ForbiddenJWT(_error) => StatusCode::FORBIDDEN,
                    ServiceError::NotFound => StatusCode::NOT_FOUND,
                    ServiceError::DbError(dao_error) => match dao_error {
                        crate::dao::DaoError::ConnectionError(error) => {
                            log::warn!("{error:?}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        crate::dao::DaoError::DieselError(error) => {
                            log::warn!("{error:?}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        crate::dao::DaoError::NotFound => StatusCode::NOT_FOUND,
                        crate::dao::DaoError::UniqueViolation(_)
                        | crate::dao::DaoError::ForeignKeyViolation(_) => StatusCode::CONFLICT,
                        crate::dao::DaoError::OtherRare(e) => {
                            log::warn!("{e:?}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                    },
                    ServiceError::TypeError(_) => StatusCode::BAD_REQUEST,
                    ServiceError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, service_err.to_string(), Some(self))
            }
        };

        let mut response = (status, Json(ErrorResponse::from(message))).into_response();
        if let Some(err) = err {
            // For use of middleware, won't be sent to client
            response.extensions_mut().insert(Arc::new(err));
        }
        response
    }
}
