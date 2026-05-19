//! Business logic service layer.

pub mod auth;
/// Group management service.
pub mod group;
/// MQTT authentication and ingestion services.
pub mod mqtt;
/// Producer management service.
pub mod producer;
/// Device provisioning service.
pub mod provisioning;
/// Template data query service.
pub mod template_data;

use thiserror::Error;

use crate::{dao::DaoError, interface};

/// Errors that can occur in the service layer.
#[derive(Error, Debug)]
pub enum ServiceError {
    /// Access was forbidden.
    #[error("Forbidden access")]
    Forbidden,
    /// Access was forbidden due to a JWT error.
    #[error("Forbidden access due to JWT: {0}")]
    ForbiddenJWT(#[from] jsonwebtoken::errors::Error),
    /// The requested resource was not found.
    #[error("Resource not found")]
    NotFound,
    /// A database error occurred.
    #[error("Database error: {0}")]
    DbError(#[from] DaoError),
    /// A type conversion or validation error occurred.
    #[error("Type error: {0}")]
    TypeError(#[from] interface::types::TypeError),
    /// An internal server error occurred.
    #[error("Internal Error")]
    InternalError,
}
