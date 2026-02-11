use crate::{
    dao::identity::PgIdentityDao,
    helpers::{auth::AuthMethod, state::AppState},
    interface::ApiError,
    service::auth::{AuthService, RefreshSessionOut, TokenPair},
};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

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
    pub async fn post_refresh(
        State(AppState { pool }): State<AppState>,
        payload: Result<Json<RefreshPayload>, JsonRejection>,
    ) -> Result<Json<RefreshSessionResponse>, ApiError> {
        let payload_refresh = &payload?.refresh_token;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let output = service.refresh_session(payload_refresh)?;
        Ok(Json(output.into()))
    }

    pub async fn post_register(
        State(AppState { pool }): State<AppState>,
        payload: Result<Json<RegisterPayload>, JsonRejection>,
    ) -> Result<Json<RegisterResponse>, ApiError> {
        let payload = payload?.0;
        let dao = PgIdentityDao::new(pool);
        let service = AuthService::new(dao);
        let created = service.register_user(payload.into())?;
        Ok(Json(RegisterResponse {
            login: created.login,
        }))
    }

    pub async fn post_login(
        State(AppState { pool }): State<AppState>,
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
