pub mod auth_controller;

use std::{env, net::SocketAddr, sync::Arc};

use axum::{
    Json, extract::rejection::JsonRejection, response::IntoResponse,
    routing::MethodRouter,
};
use diesel::{PgConnection, r2d2::ConnectionManager};
use dotenvy::dotenv;
use serde::Serialize;
use thiserror::Error;
use tokio::io;

use crate::MAX_DB_POOL_SIZE;

#[derive(Debug, Error)]
pub enum InterfaceError {
    #[error("Initialization error occured")]
    InitError(String),
    #[error("Tokio IO error")]
    TokioIOError(#[from] io::Error),
    #[error("DbPool creation error")]
    DbPoolError(#[from] r2d2::Error),
}

pub struct Interface {}

impl Interface {
    pub async fn run_http_server(addr: SocketAddr) -> Result<(), InterfaceError> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = r2d2::Pool::builder()
            .max_size(MAX_DB_POOL_SIZE)
            .build(manager)?;

        let router = MethodRouter::new().with_state(pool);

        axum::serve(listener, router).await.map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("JSON payload rejected")]
    JsonRejection(#[from] JsonRejection),
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
        };

        let mut response = (status, Json(ErrorResponse::from(message))).into_response();
        if let Some(err) = err {
            // For use of middleware, won't be sent to client
            response.extensions_mut().insert(Arc::new(err));
        }
        response
    }
}
