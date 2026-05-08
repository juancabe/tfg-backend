pub mod auth;
pub mod group;
pub mod mqtt;
pub mod producer;
pub mod provisioning;
pub mod template_data;

use thiserror::Error;

use crate::{dao::DaoError, interface};

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
    #[error("Type error: {0}")]
    TypeError(#[from] interface::types::TypeError),
    #[error("Internal Error")]
    InternalError,
}
