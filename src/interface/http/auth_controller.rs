//! AuthController
//!
//! Defines handlers for "/login", "/register", "/refresh", "/change-password",
//! "/password-reset-request" and "/password-reset" endpoints
#![allow(missing_docs)]

use crate::helpers::validation::{
    LOGIN_MAX_LEN, LOGIN_MIN_LEN, PASSWORD_MAX_LEN, PASSWORD_MIN_LEN,
};
use crate::interface::types::auth::AuthMethod;
use crate::{
    dao::identity::{IdentityDao, PgIdentityDao},
    helpers::http_app_state::AppState,
    service::auth::{
        AuthService, ChangePasswordInput, JWT, PasswordResetInput, RefreshSessionOut, TokenPair,
    },
};

use super::HttpApiError;

use axum::extract::Query;
use axum::response::Html;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, utoipa::ToSchema, Validate)]
/// Holds a JWT refresh token hash
pub struct RefreshPayload {
    #[validate(length(min = 8, max = 2048))]
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Deserialize, utoipa::ToSchema, Validate)]
/// Accepts a unique `login` and [`AuthMethod`]
pub struct RegisterPayload {
    #[validate(length(min = LOGIN_MIN_LEN, max = LOGIN_MAX_LEN))]
    pub login: String,
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
/// Returns the used `login`
pub enum RegisterResponse {
    Success,
    NeedsVerification,
}

#[derive(Deserialize, utoipa::ToSchema, Debug, Validate)]
/// Accepts the [`AuthMethod`] that'll be used to login
pub struct LoginPayload {
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
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

/// Response returned on successful login with JWT tokens.
#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct CorrectLoginResponse {
    pub session_token: String,
    pub refresh_token: String,
}

#[derive(Deserialize, utoipa::ToSchema, Validate)]
/// Accepts current and new password for change
pub struct ChangePasswordPayload {
    #[validate(length(min = PASSWORD_MIN_LEN, max = PASSWORD_MAX_LEN))]
    pub current_password: String,
    #[validate(length(min = PASSWORD_MIN_LEN, max = PASSWORD_MAX_LEN))]
    pub new_password: String,
}

/// Response returned after a successful password change with new tokens.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ChangePasswordResponse {
    pub session_token: String,
    pub refresh_token: String,
}

/// Request payload for initiating a password reset via email.
#[derive(Deserialize, utoipa::ToSchema, Validate)]
pub struct PasswordResetRequestPayload {
    #[validate(email)]
    pub email: String,
}

/// Request payload for completing a password reset with a verification code.
#[derive(Deserialize, utoipa::ToSchema, Validate)]
pub struct PasswordResetPayload {
    #[validate(email)]
    pub email: String,
    #[validate(custom(function = "validate_reset_code"))]
    pub code: String,
    #[validate(length(min = PASSWORD_MIN_LEN, max = PASSWORD_MAX_LEN))]
    pub new_password: String,
}

/// Query parameters for the email verification endpoint.
#[derive(Deserialize)]
pub struct VerifyEmailQuery {
    pub token: String,
}

impl From<TokenPair> for CorrectLoginResponse {
    fn from(value: TokenPair) -> Self {
        Self {
            session_token: value.session_token,
            refresh_token: value.refresh_token,
        }
    }
}

fn validate_reset_code(value: &str) -> Result<(), validator::ValidationError> {
    if value.len() == 6 && value.chars().all(|c| c.is_ascii_digit()) {
        Ok(())
    } else {
        let mut error = validator::ValidationError::new("invalid_code");
        error.message = Some("Invalid code format, expected 6 numeric digit".into());
        Err(error)
    }
}

/// `post_refresh`
///
/// Use of `refresh` token to create new `refresh` and `session` token
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "auth",
    request_body(content = RefreshPayload, description = "Refresh token"),
    responses((status = 200, description = "Successfully refreshed session", body = RefreshSessionResponse))
)]
pub async fn post_refresh(
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<RefreshPayload>, JsonRejection>,
) -> Result<Json<RefreshSessionResponse>, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    let payload_refresh = &payload.refresh_token;
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    let output = service.refresh_session(payload_refresh)?;
    Ok(Json(output.into()))
}

/// `post_register`
///
/// Use of [`RegisterPayload`] to create a new `User` on the system
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body(content = RegisterPayload, description = "Login and auth method"),
    responses((status = 200, description = "Successfully registered, may need verification", body = RegisterResponse))
)]

pub async fn post_register(
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<RegisterPayload>, JsonRejection>,
) -> Result<Json<RegisterResponse>, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    payload.auth_method.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    let created = service.register_user(payload.into()).await?;
    Ok(Json(created))
}

/// `post_login`
///
/// Use of [`LoginPayload`] to login to the system
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body(content = LoginPayload, description = "Auth method"),
    responses((status = 200, description = "Tried to log in, may have provided incorrect credentials", body = LoginResponse))
)]
pub async fn post_login(
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<LoginPayload>, JsonRejection>,
) -> Result<LoginResponse, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    payload.auth_method.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    log::info!("[post_login] with:  {:?}", payload);
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    if let Some(tokens) = service.authenticate_user(payload.into())? {
        Ok(LoginResponse::Correct(tokens.into()))
    } else {
        Ok(LoginResponse::InvalidCredentials)
    }
}

/// `post_change_password`
///
/// Changes the current user's password using the session token
#[utoipa::path(
    post,
    path = "/auth/change-password",
    tag = "auth",
    request_body(content = ChangePasswordPayload, description = "Current and new password"),
    responses((status = 200, description = "Password changed", body = ChangePasswordResponse)),
    security(("bearer_auth" = []))
)]
pub async fn post_change_password(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<ChangePasswordPayload>, JsonRejection>,
) -> Result<Json<ChangePasswordResponse>, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    let input =
        ChangePasswordInput::new(jwt.user_id, payload.current_password, payload.new_password);
    let tokens = service.change_password(input)?;
    Ok(Json(ChangePasswordResponse {
        session_token: tokens.session_token,
        refresh_token: tokens.refresh_token,
    }))
}

/// `post_password_reset_request`
///
/// Requests a password reset email
#[utoipa::path(
    post,
    path = "/auth/password-reset-request",
    tag = "auth",
    request_body(content = PasswordResetRequestPayload, description = "Password reset request"),
    responses((status = 204, description = "Password reset code sent"))
)]
pub async fn post_password_reset_request(
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<PasswordResetRequestPayload>, JsonRejection>,
) -> Result<StatusCode, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;

    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    service.request_password_reset(&payload.email).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `post_password_reset`
///
/// Resets a password using an email code
#[utoipa::path(
    post,
    path = "/auth/password-reset",
    tag = "auth",
    request_body(content = PasswordResetPayload, description = "Password reset payload"),
    responses((status = 204, description = "Password reset"))
)]
pub async fn post_password_reset(
    State(AppState { pool, .. }): State<AppState>,
    payload: Result<Json<PasswordResetPayload>, JsonRejection>,
) -> Result<StatusCode, HttpApiError> {
    let payload = payload?.0;
    payload.validate().map_err(|e| {
        HttpApiError::ServiceError(crate::service::ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(e.to_string()),
        ))
    })?;

    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    let input = PasswordResetInput::new(payload.email, payload.code, payload.new_password);
    service.reset_password(input)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `get_verify_email`
///
/// Verifies email using token from query
#[utoipa::path(
    get,
    path = "/auth/verify",
    tag = "auth",
    params(
        ("token" = String, Query, description = "Verification token")
    ),
    responses(
        (status = 200, description = "Verification result", body = String)
    )
)]
pub async fn get_verify_email(
    State(AppState { pool, .. }): State<AppState>,
    Query(query): Query<VerifyEmailQuery>,
) -> Result<Html<String>, HttpApiError> {
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);

    let token = match Uuid::parse_str(&query.token) {
        Ok(token) => token,
        Err(_) => {
            let body = "<html><body><h1>Verification failed</h1><p>Invalid or expired token.</p></body></html>";
            return Ok(Html(body.to_string()));
        }
    };
    let result = match service.verify_email(token) {
        Ok(result) => result,
        Err(_) => {
            let body = "<html><body><h1>Verification failed</h1><p>Invalid or expired token.</p></body></html>";
            return Ok(Html(body.to_string()));
        }
    };

    let body = if result.verified {
        "<html><body><h1>Email verified</h1><p>Your email has been verified.</p></body></html>"
    } else {
        "<html><body><h1>Verification failed</h1><p>Invalid or expired token.</p></body></html>"
    };
    Ok(Html(body.to_string()))
}

/// `delete_account`
///
/// # Description
/// Deletes the currently authenticated user's account.
#[utoipa::path(
    delete,
    path = "/auth/account",
    tag = "auth",
    responses((status = 200, description = "Account deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_account(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
) -> Result<StatusCode, HttpApiError> {
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);

    service.delete_user(&jwt.user_id)?;

    Ok(StatusCode::OK)
}

/// `get_user`
///
/// # Description
/// Returns the currently authenticated user's API-visible info.
#[utoipa::path(
    get,
    path = "/auth/user",
    tag = "auth",
    responses(
        (status = 200, description = "Authenticated user info", body = crate::interface::types::auth::User),
        (status = 404, description = "User not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_user(
    jwt: JWT,
    State(AppState { pool, .. }): State<AppState>,
) -> Result<Json<crate::interface::types::auth::User>, HttpApiError> {
    let dao = PgIdentityDao::new(pool);
    let user = dao
        .find_by_id(jwt.user_id)
        .map_err(crate::service::ServiceError::from)?
        .ok_or(HttpApiError::ServiceError(
            crate::service::ServiceError::NotFound,
        ))?;

    Ok(Json(user.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup;
    use axum::extract::State;

    mod unit_test {
        // No unit tests for AuthController at the moment.
        // Unit tests would typically mock dependencies like AuthService and PgIdentityDao.
    }

    mod integration_test {
        use crate::dao::identity::IdentityDao;
        use crate::interface::http::auth_controller;
        use crate::interface::types::auth::{AuthMethod, EmailPass, EmailPassFields};
        use crate::model::models::NewPasswordReset;

        use super::*;

        fn verify_user(pool: &crate::dao::DbPool, login: &str, email: &str) {
            let dao = PgIdentityDao::new(pool.clone());
            let user_id = dao.find_by_login(login).unwrap().unwrap().id;
            let verification = dao
                .create_email_verification(crate::model::models::NewEmailVerification {
                    user_id,
                    email,
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                })
                .unwrap();
            dao.mark_email_verified(&verification.user_id).unwrap();
        }

        #[tokio::test]
        async fn test_post_register_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });
            let payload = Ok(Json(RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            }));

            let res = auth_controller::post_register(state, payload)
                .await
                .unwrap();
            assert!(matches!(res.0, RegisterResponse::NeedsVerification));
        }

        #[tokio::test]
        async fn test_post_register_duplicate() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });
            let payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };

            let _ = auth_controller::post_register(state.clone(), Ok(Json(payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            // Try again with same login
            let payload_dup = Ok(Json(RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test2@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            }));

            let res = auth_controller::post_register(state, payload_dup).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_login_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register first
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            // Login
            let login_payload = Ok(Json(LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            }));

            let res = auth_controller::post_login(state, login_payload)
                .await
                .unwrap();
            assert!(matches!(res, LoginResponse::Correct(_)));
        }

        #[tokio::test]
        async fn test_post_login_invalid_credentials() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let login_payload = Ok(Json(LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "nonexistent@example.com".to_string(),
                    pass: "wrongpass".to_string(),
                })),
            }));

            let res = auth_controller::post_login(state, login_payload)
                .await
                .unwrap();
            assert!(matches!(res, LoginResponse::InvalidCredentials));
        }

        #[tokio::test]
        async fn test_post_register_invalid_login() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });
            let payload = Ok(Json(RegisterPayload {
                login: "ab".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            }));

            let res = auth_controller::post_register(state, payload).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_register_invalid_email() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });
            let payload = Ok(Json(RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "invalid-email".to_string(),
                    pass: "password".to_string(),
                })),
            }));

            let res = auth_controller::post_register(state, payload).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_change_password_invalid_length() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state.clone(), Ok(Json(login_payload)))
                .await
                .unwrap();
            let session_token = if let LoginResponse::Correct(tokens) = login_res {
                tokens.session_token
            } else {
                panic!("Login failed");
            };

            let service = AuthService::new(PgIdentityDao::new(ctx.pool.clone()));
            let jwt = service.validate_session(&session_token).unwrap();

            let payload = Ok(Json(ChangePasswordPayload {
                current_password: "password".to_string(),
                new_password: "short".to_string(),
            }));

            let res = auth_controller::post_change_password(jwt, state, payload).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_login_wrong_password_for_existing_user() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            // Login with wrong password
            let login_payload = Ok(Json(LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email,
                    pass: "wrong_password".to_string(),
                })),
            }));

            let res = auth_controller::post_login(state, login_payload)
                .await
                .unwrap();
            assert!(matches!(res, LoginResponse::InvalidCredentials));
        }

        #[tokio::test]
        async fn test_post_refresh_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register and login to get refresh token
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state.clone(), Ok(Json(login_payload)))
                .await
                .unwrap();

            let refresh_token = if let LoginResponse::Correct(tokens) = login_res {
                tokens.refresh_token
            } else {
                panic!("Login failed");
            };

            // Refresh
            let refresh_payload = Ok(Json(RefreshPayload { refresh_token }));
            let res = auth_controller::post_refresh(state, refresh_payload)
                .await
                .unwrap();

            assert!(!res.0.session_token.is_empty());
            assert!(!res.0.refresh_token.is_empty());
        }

        #[tokio::test]
        async fn test_post_refresh_invalid_token() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let refresh_payload = Ok(Json(RefreshPayload {
                refresh_token: "invalid_token".to_string(),
            }));

            let res = auth_controller::post_refresh(state, refresh_payload).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_refresh_with_session_token() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register and login to get tokens
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email,
                    pass: "password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state.clone(), Ok(Json(login_payload)))
                .await
                .unwrap();

            let session_token = if let LoginResponse::Correct(tokens) = login_res {
                tokens.session_token
            } else {
                panic!("Login failed");
            };

            // Try to refresh with session token instead of refresh token
            let refresh_payload = Ok(Json(RefreshPayload {
                refresh_token: session_token,
            }));
            let res = auth_controller::post_refresh(state, refresh_payload).await;

            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_post_refresh_twice_fails() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register and login to get refresh token
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email,
                    pass: "password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state.clone(), Ok(Json(login_payload)))
                .await
                .unwrap();

            let refresh_token = if let LoginResponse::Correct(tokens) = login_res {
                tokens.refresh_token
            } else {
                panic!("Login failed");
            };

            // First refresh
            let res1 = auth_controller::post_refresh(
                state.clone(),
                Ok(Json(RefreshPayload {
                    refresh_token: refresh_token.clone(),
                })),
            )
            .await;
            assert!(res1.is_ok());

            // Second refresh with SAME token should fail
            let res2 =
                auth_controller::post_refresh(state, Ok(Json(RefreshPayload { refresh_token })))
                    .await;
            assert!(res2.is_err());
        }

        #[tokio::test]
        async fn test_post_change_password_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state.clone(), Ok(Json(login_payload)))
                .await
                .unwrap();
            let session_token = if let LoginResponse::Correct(tokens) = login_res {
                tokens.session_token
            } else {
                panic!("Login failed");
            };

            let service = AuthService::new(PgIdentityDao::new(ctx.pool.clone()));
            let jwt = service.validate_session(&session_token).unwrap();

            let payload = Ok(Json(ChangePasswordPayload {
                current_password: "password".to_string(),
                new_password: "new_password".to_string(),
            }));

            let res = auth_controller::post_change_password(jwt, state.clone(), payload)
                .await
                .unwrap();
            assert!(!res.0.session_token.is_empty());
            assert!(!res.0.refresh_token.is_empty());

            let service = AuthService::new(PgIdentityDao::new(ctx.pool.clone()));
            let old_session_valid = service.validate_session(&session_token).is_ok();
            assert!(!old_session_valid);

            let new_session_valid = service.validate_session(&res.0.session_token).is_ok();
            assert!(new_session_valid);
        }

        #[tokio::test]
        async fn test_post_password_reset_request_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let payload = Ok(Json(PasswordResetRequestPayload {
                email: "test@example.com".to_string(),
            }));

            let res = auth_controller::post_password_reset_request(state.clone(), payload).await;
            assert!(matches!(res, Ok(StatusCode::NO_CONTENT)));
        }

        #[tokio::test]
        async fn test_post_password_reset_request_unknown_email_no_error() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let payload = Ok(Json(PasswordResetRequestPayload {
                email: "missing@example.com".to_string(),
            }));

            let res = auth_controller::post_password_reset_request(state, payload).await;
            assert!(matches!(res, Ok(StatusCode::NO_CONTENT)));
        }

        #[tokio::test]
        async fn test_post_password_reset_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user_id = dao.find_by_login("test_user").unwrap().unwrap().id;
            let code = "123456".to_string();
            dao.create_password_reset(NewPasswordReset {
                user_id,
                email: "test@example.com",
                code: &code,
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            })
            .unwrap();

            let payload = Ok(Json(PasswordResetPayload {
                email: "test@example.com".to_string(),
                code: code.clone(),
                new_password: "new_password".to_string(),
            }));

            let res = auth_controller::post_password_reset(state.clone(), payload).await;
            assert!(matches!(res, Ok(StatusCode::NO_CONTENT)));

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    pass: "new_password".to_string(),
                })),
            };
            let login_res = auth_controller::post_login(state, Ok(Json(login_payload)))
                .await
                .unwrap();
            assert!(matches!(login_res, LoginResponse::Correct(_)));
        }

        #[tokio::test]
        async fn test_post_password_reset_invalid_code_format() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool,
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            let payload = Ok(Json(PasswordResetPayload {
                email: "test@example.com".to_string(),
                code: "abc".to_string(),
                new_password: "new_password".to_string(),
            }));

            let res = auth_controller::post_password_reset(state, payload).await;
            assert!(res.is_err());
        }

        #[tokio::test]
        async fn test_delete_account() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register and get token
            let payload = RegisterPayload {
                login: "delete_me".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "delete_me@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let res = auth_controller::post_register(state.clone(), Ok(Json(payload))).await;
            assert!(res.is_ok());
            let (user_id, session_token) = {
                let dao = PgIdentityDao::new(ctx.pool.clone());
                let user = dao.find_by_login("delete_me").unwrap().unwrap();
                // verify email
                let service = AuthService::new(dao);
                let verification = PgIdentityDao::new(ctx.pool.clone())
                    .create_email_verification(crate::model::models::NewEmailVerification {
                        user_id: user.id,
                        email: "delete_me@example.com",
                        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                    })
                    .unwrap();
                service.verify_email(verification.id).unwrap();

                let login_payload = LoginPayload {
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: "delete_me@example.com".to_string(),
                        pass: "password".to_string(),
                    })),
                };
                let tokens = service
                    .authenticate_user(login_payload.into())
                    .unwrap()
                    .unwrap();
                (user.id, tokens.session_token)
            };

            let service = AuthService::new(PgIdentityDao::new(ctx.pool.clone()));
            let jwt = service.validate_session(&session_token).unwrap();

            // Perform deletion
            let res = auth_controller::delete_account(jwt, state).await;
            assert!(matches!(res, Ok(StatusCode::OK)));

            // Verify user is gone
            let dao = PgIdentityDao::new(ctx.pool.clone());
            assert!(dao.find_by_id(user_id).unwrap().is_none());
        }

        #[tokio::test]
        async fn test_get_user_success() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
                mqtt_peers_tx: tokio::sync::broadcast::channel(10).0,
                mqtt_erase_tx: tokio::sync::broadcast::channel(10).0,
            });

            // Register and get token
            let payload = RegisterPayload {
                login: "get_user_test".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "get_user@example.com".to_string(),
                    pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(payload)))
                .await
                .unwrap();

            let session_token = {
                let dao = PgIdentityDao::new(ctx.pool.clone());
                let user = dao.find_by_login("get_user_test").unwrap().unwrap();
                // verify email
                let service = AuthService::new(dao);
                let verification = PgIdentityDao::new(ctx.pool.clone())
                    .create_email_verification(crate::model::models::NewEmailVerification {
                        user_id: user.id,
                        email: "get_user@example.com",
                        expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                    })
                    .unwrap();
                service.verify_email(verification.id).unwrap();

                let login_payload = LoginPayload {
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: "get_user@example.com".to_string(),
                        pass: "password".to_string(),
                    })),
                };
                let tokens = service
                    .authenticate_user(login_payload.into())
                    .unwrap()
                    .unwrap();
                tokens.session_token
            };

            let service = AuthService::new(PgIdentityDao::new(ctx.pool.clone()));
            let jwt = service.validate_session(&session_token).unwrap();

            // Hit the endpoint
            let res = auth_controller::get_user(jwt, state).await.unwrap();

            assert_eq!(res.0.login, "get_user_test");
        }
    }
}
