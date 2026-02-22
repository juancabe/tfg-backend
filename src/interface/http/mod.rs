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
use diesel::{PgConnection, r2d2::ConnectionManager};
use dotenvy::dotenv;
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::io;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    dao::{DbPool, MAX_DB_POOL_SIZE},
    helpers::http_app_state::AppState,
    interface::http::docs::ApiDoc,
    service::ServiceError,
};

pub const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("Initialization error occured")]
    InitError(String),
    #[error("Tokio IO error")]
    TokioIOError(#[from] io::Error),
    #[error("DbPool creation error")]
    DbPoolError(#[from] r2d2::Error),
}

pub mod auth_controller;
pub mod docs;
pub mod extractor;
pub mod group_controller;
pub mod producer_controller;

pub struct Http {}

impl Http {
    pub async fn run_http_server(addr: SocketAddr) -> Result<(), HttpError> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        log::info!("Created TcpListener on {:?}", addr);

        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(database_url.clone());
        let pool: DbPool = r2d2::Pool::builder()
            .max_size(MAX_DB_POOL_SIZE) // Ensure this constant is in scope
            .build(manager)?;
        log::info!("Created DbPool on {}", database_url);

        let state = AppState { pool };

        let openapi_doc = ApiDoc::openapi();

        let app = Router::new()
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi_doc))
            .route("/auth/login", post(auth_controller::post_login))
            .route("/auth/register", post(auth_controller::post_register))
            .route("/auth/refresh", post(auth_controller::post_refresh))
            .route(
                "/producers",
                get(producer_controller::get_producers).post(producer_controller::post_producer),
            )
            .route(
                "/producers/{id}",
                get(producer_controller::get_producer_by_id),
            )
            .with_state(state);

        log::info!("Created HTTP Router");
        log::info!("Swagger UI available at http://{:?}/swagger-ui", addr);

        axum::serve(listener, app).await.map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum HttpApiError {
    #[error("JSON payload rejected")]
    JsonRejection(#[from] JsonRejection),
    #[error("Service error: {0}")]
    ServiceError(#[from] ServiceError),
}

/// How error responses will be serialized
/// TODO: utoipa for this
#[derive(Serialize)]
struct ErrorResponse {
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
