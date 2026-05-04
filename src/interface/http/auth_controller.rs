//! AuthController
//!
//! Defines handlers for "/login", "/register", "/refresh" and "/change-password" endpoints

use crate::{
    dao::identity::PgIdentityDao,
    helpers::{http_app_state::AppState, users_auth::AuthMethod},
    service::auth::{AuthService, ChangePasswordInput, RefreshSessionOut, TokenPair, JWT},
};

use super::HttpApiError;

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use axum::extract::Query;
use axum::response::Html;
use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
/// Holds a JWT refresh token hash
pub struct RefreshPayload {
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

#[derive(Deserialize, utoipa::ToSchema)]
/// Accepts a unique `login` and [`AuthMethod`]
pub struct RegisterPayload {
    pub login: String,
    pub auth_method: AuthMethod,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
/// Returns the used `login`
pub enum RegisterResponse {
    Success,
    NeedsVerification,
}

#[derive(Deserialize, utoipa::ToSchema, Debug)]
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

#[derive(Deserialize, Serialize, Debug, utoipa::ToSchema)]
pub struct CorrectLoginResponse {
    pub session_token: String,
    pub refresh_token: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
/// Accepts current and new password for change
pub struct ChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ChangePasswordResponse {
    pub session_token: String,
    pub refresh_token: String,
}

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
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body(content = RegisterPayload, description = "Login and auth method"),
    responses((status = 200, description = "Successfully registered, may need verification", body = RegisterResponse))
)]

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
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body(content = LoginPayload, description = "Auth method"),
    responses((status = 200, description = "Tried to log in, may have provided incorrect credentials", body = LoginResponse))
)]
pub async fn post_login(
    State(AppState { pool }): State<AppState>,
    payload: Result<Json<LoginPayload>, JsonRejection>,
) -> Result<LoginResponse, HttpApiError> {
    let payload = payload?.0;
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
    State(AppState { pool }): State<AppState>,
    payload: Result<Json<ChangePasswordPayload>, JsonRejection>,
) -> Result<Json<ChangePasswordResponse>, HttpApiError> {
    let payload = payload?.0;
    let dao = PgIdentityDao::new(pool);
    let service = AuthService::new(dao);
    let input = ChangePasswordInput::new(
        jwt.user_id,
        payload.current_password,
        payload.new_password,
    );
    let tokens = service.change_password(input)?;
    Ok(Json(ChangePasswordResponse {
        session_token: tokens.session_token,
        refresh_token: tokens.refresh_token,
    }))
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
    State(AppState { pool }): State<AppState>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::users_auth::{EmailPass, EmailPassFields};
    use crate::test_utils::setup;
    use axum::extract::State;

    mod unit_test {
        // No unit tests for AuthController at the moment.
        // Unit tests would typically mock dependencies like AuthService and PgIdentityDao.
    }

    mod integration_test {
        use crate::interface::http::auth_controller;
        use crate::dao::identity::IdentityDao;

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
            let state = State(AppState { pool: ctx.pool });
            let payload = Ok(Json(RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
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
            });
            let payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
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
                    hashed_pass: "password".to_string(),
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
            });

            // Register first
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
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
                    hashed_pass: "password".to_string(),
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
            let state = State(AppState { pool: ctx.pool });

            let login_payload = Ok(Json(LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "nonexistent@example.com".to_string(),
                    hashed_pass: "wrong".to_string(),
                })),
            }));

            let res = auth_controller::post_login(state, login_payload)
                .await
                .unwrap();
            assert!(matches!(res, LoginResponse::InvalidCredentials));
        }

        #[tokio::test]
        async fn test_post_login_wrong_password_for_existing_user() {
            let ctx = setup();
            let state = State(AppState {
                pool: ctx.pool.clone(),
            });

            // Register
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    hashed_pass: "password".to_string(),
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
                    hashed_pass: "wrong_password".to_string(),
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
            });

            // Register and login to get refresh token
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
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
            let state = State(AppState { pool: ctx.pool });

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
            });

            // Register and login to get tokens
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    hashed_pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email,
                    hashed_pass: "password".to_string(),
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
            });

            // Register and login to get refresh token
            let email = "test@example.com".to_string();
            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    hashed_pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email,
                    hashed_pass: "password".to_string(),
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
            });

            let register_payload = RegisterPayload {
                login: "test_user".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
                })),
            };
            let _ = auth_controller::post_register(state.clone(), Ok(Json(register_payload)))
                .await
                .unwrap();

            verify_user(&ctx.pool, "test_user", "test@example.com");

            let login_payload = LoginPayload {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test@example.com".to_string(),
                    hashed_pass: "password".to_string(),
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
    }
}
