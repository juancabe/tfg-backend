//! User related authentication methods type definitions

use serde::{Deserialize, Serialize};
/// Fields introduced by the user on a [`EmailPass`] login
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct EmailPassFields {
    pub email: String,
    pub hashed_pass: String,
}

/// Confirmation of the email, this is currently a
/// TODO: implement correct email confirmation, checking expiration
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub struct EmailPassConfirmation {
    pub email: String,
}

/// Holds both kind of [`EmailPass`] possibilities
#[derive(Serialize, Deserialize, Debug, utoipa::ToSchema)]
pub enum EmailPass {
    Fields(EmailPassFields),
    Confirmation(EmailPassConfirmation),
}

/// Is intended to be expanded when new [`AuthMethod`]s are used (OAuth, ...)
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum AuthMethod {
    EmailPass(EmailPass),
}
