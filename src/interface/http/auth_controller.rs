//! AuthController
//!
//! Defines handlers for "/login", "/register" and "/refresh" endpoints

use crate::{
    dao::identity::PgIdentityDao,
    helpers::{http_app_state::AppState, users_auth::AuthMethod},
    service::auth::{AuthService, RefreshSessionOut, TokenPair},
};

use super::HttpApiError;

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

pub struct AuthController {}

#[derive(Deserialize)]
/// Holds a JWT refresh token hash
pub struct RefreshPayload {
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize)]
/// Returns corresponding JWT hashes
pub struct RefreshSessionResponse {
    pub session_token: String,
    pub refresh_token: String,
}

impl From<RefreshSessionOut> for RefreshSessionResponse {
    fn from(value: RefreshSessionOut) -> Self {
        Self {
            session_token: value.session_token,
            refresh_token: value.refresh_token,
        }
    }
}

#[derive(Deserialize)]
/// Accepts a unique `login` and [`AuthMethod`]
pub struct RegisterPayload {
    pub login: String,
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize)]
/// Returns the used `login`
pub enum RegisterResponse {
    Success,
    NeedsVerification,
}

#[derive(Deserialize)]
/// Accepts the [`AuthMethod`] that'll be used to login
pub struct LoginPayload {
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize, Debug)]
/// Returns whether login was valid and the corresponding response data
pub enum LoginResponse {
    Correct(CorrectLoginResponse),
    InvalidCredentials,
}

impl IntoResponse for LoginResponse {
    fn into_response(self) -> axum::response::Response {
        match &self {
            LoginResponse::Correct(_) => (StatusCode::OK, Json(self)),
            LoginResponse::InvalidCredentials => (StatusCode::UNAUTHORIZED, Json(self)),
        }
        .into_response()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CorrectLoginResponse {
    pub session_token: String,
    pub refresh_token: String,
}

impl From<TokenPair> for CorrectLoginResponse {
    fn from(value: TokenPair) -> Self {
        Self {
            session_token: value.session_token,
            refresh_token: value.refresh_token,
        }
    }
}

impl AuthController {
    /// `post_refresh`
    ///
    /// Use of `refresh` token to create new `refresh` and `session` token
    pub async fn post_refresh(
        State(AppState { pool }): State<AppState>,
        payload: Result<Json<RefreshPayload>, JsonRejection>,
    ) -> Result<Json<RefreshSessionResponse>, HttpApiError> {
        let payload_refresh = &payload?.refresh_token;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let output = service.refresh_session(payload_refresh)?;
        Ok(Json(output.into()))
    }

    /// `post_register`
    ///
    /// Use of [`RegisterPayload`] to create a new `User` on the system
    pub async fn post_register(
        State(AppState { pool }): State<AppState>,
        payload: Result<Json<RegisterPayload>, JsonRejection>,
    ) -> Result<Json<RegisterResponse>, HttpApiError> {
        let payload = payload?.0;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let created = service.register_user(payload.into())?;
        Ok(Json(created))
    }

    /// `post_login`
    ///
    /// Use of [`LoginPayload`] to create a new `User` on the system
    pub async fn post_login(
        State(AppState { pool }): State<AppState>,
        payload: Result<Json<LoginPayload>, JsonRejection>,
    ) -> Result<LoginResponse, HttpApiError> {
        let payload = payload?.0;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        if let Some(tokens) = service.authenticate_user(payload.into())? {
            Ok(LoginResponse::Correct(tokens.into()))
        } else {
            Ok(LoginResponse::InvalidCredentials)
        }
    }
}
