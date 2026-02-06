use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
};
use serde::{Deserialize, Serialize};

use crate::{
    dao::identity::PgIdentityDao,
    helpers::auth::AuthMethod,
    interface::{ApiError, DbPool},
    service::auth::{AuthService, RefreshSessionOut, TokenPair},
};

pub struct AuthController {}

#[derive(Deserialize)]
pub struct RefreshPayload {
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize)]
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
pub struct RegisterPayload {
    pub login: String,
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize)]
pub struct RegisterResponse {
    pub login: String,
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum LoginResponse {
    Correct(CorrectLoginResponse),
    InvalidCredentials,
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
    pub async fn post_refresh(
        State(pool): State<DbPool>,
        payload: Result<Json<RefreshPayload>, JsonRejection>,
    ) -> Result<Json<RefreshSessionResponse>, ApiError> {
        let payload_refresh = &payload?.refresh_token;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let output = service.refresh_session(payload_refresh)?;
        Ok(Json(output.into()))
    }

    pub async fn register(
        State(pool): State<DbPool>,
        payload: Result<Json<RegisterPayload>, JsonRejection>,
    ) -> Result<RegisterResponse, ApiError> {
        let payload = payload?.0;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let created = service.register_user(payload.into())?;
        Ok(RegisterResponse {
            login: created.login,
        })
    }

    pub async fn login(
        State(pool): State<DbPool>,
        payload: Result<Json<LoginPayload>, JsonRejection>,
    ) -> Result<LoginResponse, ApiError> {
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
