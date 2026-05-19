//! Shared type definitions for API requests and responses.

use thiserror::Error;

/// Authentication-related types.
pub mod auth;
/// Group-related types.
pub mod group;
/// Location-related types.
pub mod location;
/// Producer-related types.
pub mod producer;
/// Provisioning-related types.
pub mod provisioning;
/// Template data-related types.
pub mod template_data;

/// Type conversion errors.
#[derive(Error, Debug)]
pub enum TypeError {
    /// A location-related error occurred.
    #[error("Location error")]
    LocationError(#[from] location::Error),
    /// A validation error with a description.
    #[error("Validation error: {0}")]
    Validation(String),
}
