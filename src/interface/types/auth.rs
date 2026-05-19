//! User related authentication methods type definitions

use crate::helpers::validation::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Fields introduced by the user on a [`EmailPass`] login
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema, Validate)]
pub struct EmailPassFields {
    /// The user's email address.
    #[validate(email)]
    pub email: String,
    /// The user's password.
    #[validate(length(min = PASSWORD_MIN_LEN, max = PASSWORD_MAX_LEN))]
    pub pass: String,
}

/// Holds both kind of [`EmailPass`] possibilities
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub enum EmailPass {
    /// Login with email and password fields.
    Fields(EmailPassFields),
}

/// Is intended to be expanded when new [`AuthMethod`]s are used (OAuth, ...)
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum AuthMethod {
    /// Email and password authentication.
    EmailPass(EmailPass),
}

impl AuthMethod {
    /// Validates the inner auth method fields.
    pub fn validate(&self) -> Result<(), validator::ValidationErrors> {
        match self {
            AuthMethod::EmailPass(email_pass) => match email_pass {
                EmailPass::Fields(fields) => fields.validate(),
            },
        }
    }
}
