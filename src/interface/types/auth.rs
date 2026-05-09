//! User related authentication methods type definitions

use crate::helpers::validation::{PASSWORD_MAX_LEN, PASSWORD_MIN_LEN};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Fields introduced by the user on a [`EmailPass`] login
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema, Validate)]
pub struct EmailPassFields {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = PASSWORD_MIN_LEN, max = PASSWORD_MAX_LEN))]
    pub pass: String,
}

/// Holds both kind of [`EmailPass`] possibilities
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub enum EmailPass {
    Fields(EmailPassFields),
}

/// Is intended to be expanded when new [`AuthMethod`]s are used (OAuth, ...)
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum AuthMethod {
    EmailPass(EmailPass),
}

impl AuthMethod {
    pub fn validate(&self) -> Result<(), validator::ValidationErrors> {
        match self {
            AuthMethod::EmailPass(email_pass) => match email_pass {
                EmailPass::Fields(fields) => fields.validate(),
            },
        }
    }
}
