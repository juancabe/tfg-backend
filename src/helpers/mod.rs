//! Contains definitions of helper types and implementations that intend to "extend" std for the current crate

/// Password hashing and verification utilities.
pub mod crypto_password;
/// Default data template constants.
pub mod data_templates;
/// HTTP application state shared across request handlers.
pub mod http_app_state;
/// Input validation helpers and length constants.
pub mod validation;
