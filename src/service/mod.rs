pub mod auth;
pub mod producer;

use thiserror::Error;

use crate::dao::DaoError;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Forbidden access")]
    Forbidden,
    #[error("Forbidden access due to JWT: {0}")]
    ForbiddenJWT(#[from] jsonwebtoken::errors::Error),
    #[error("Resource not found")]
    NotFound,
    #[error("Database error: {0}")]
    DbError(#[from] DaoError),
}
