//! http
//!
//! Implementation of the HTTP server using ['axum'] and ['tokio']

use axum::{
    Json, Router, extract::rejection::JsonRejection, http::StatusCode, response::IntoResponse,
};
use diesel::{PgConnection, r2d2::ConnectionManager};
use dotenvy::dotenv;
use serde::Serialize;
use std::{env, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::io;

use crate::{
    dao::{DbPool, MAX_DB_POOL_SIZE},
    helpers::http_app_state::AppState,
    service::ServiceError,
};

#[derive(Debug, Error)]
pub enum InterfaceError {
    #[error("Initialization error occured")]
    InitError(String),
    #[error("Tokio IO error")]
    TokioIOError(#[from] io::Error),
    #[error("DbPool creation error")]
    DbPoolError(#[from] r2d2::Error),
}

pub mod auth_controller;

pub struct Interface {}

impl Interface {
    pub async fn run_http_server(addr: SocketAddr) -> Result<(), InterfaceError> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool: DbPool = r2d2::Pool::builder()
            .max_size(MAX_DB_POOL_SIZE)
            .build(manager)?;

        let state = AppState { pool };

        let app = Router::new()
            // AuthController
            .with_state(state);

        axum::serve(listener, app).await.map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
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

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message, err) = match &self {
            ApiError::JsonRejection(json_rejection) => (
                json_rejection.status(),
                json_rejection.body_text(),
                Some(self),
            ),
            ApiError::ServiceError(service_err) => {
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
