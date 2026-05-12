mod jwt;

use chrono::{Duration, Utc};
pub use jwt::{JWT, JWTKind};
use lettre::{AsyncTransport, Tokio1Executor};

use crate::interface::http::auth_controller::{LoginPayload, RegisterPayload, RegisterResponse};
use crate::interface::types::auth::{AuthMethod, EmailPass};
use crate::model::models::{
    NewEmailAuthMethod, NewEmailVerification, NewUser, PasswordReset, User,
};
use crate::service::ServiceError;
use crate::{dao::identity::IdentityDao, model::models::NewRefreshToken};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct PasswordResetInput {
    code: String,
    email: String,
    new_password: String,
}

impl PasswordResetInput {
    pub fn new(email: String, code: String, new_password: String) -> Self {
        Self {
            code,
            email,
            new_password,
        }
    }
}

pub struct LoginInput {
    auth_method: AuthMethod,
}

impl From<LoginPayload> for LoginInput {
    fn from(value: LoginPayload) -> Self {
        Self {
            auth_method: value.auth_method,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RegisterInput {
    login: String,
    auth_method: AuthMethod,
}

pub struct ChangePasswordInput {
    user_id: Uuid,
    current_password: String,
    new_password: String,
}

pub struct VerifyEmailOut {
    pub verified: bool,
}

impl ChangePasswordInput {
    pub fn new(user_id: Uuid, current_password: String, new_password: String) -> Self {
        Self {
            user_id,
            current_password,
            new_password,
        }
    }
}

impl From<RegisterPayload> for RegisterInput {
    fn from(value: RegisterPayload) -> Self {
        Self {
            login: value.login,
            auth_method: value.auth_method,
        }
    }
}

pub struct TokenPair {
    pub session_token: String,
    pub refresh_token: String,
}

pub struct AuthService<D: IdentityDao> {
    dao: D,
}

#[derive(Serialize)]
pub struct RefreshSessionOut {
    pub session_token: String,
    pub refresh_token: String,
}

impl<D: IdentityDao> AuthService<D> {
    pub fn new(dao: D) -> Self {
        Self { dao }
    }

    async fn send_mail(user_mail: &str, subject: &str, body: String) -> Result<(), ServiceError> {
        let gmail_user = crate::config::get().get_gmail_user();
        let gmail_app_password = crate::config::get().get_gmail_pass();

        let email = lettre::Message::builder()
            .from(format!("TFG Backend <{}>", gmail_user).parse().unwrap())
            .to(format!("User <{user_mail}>").parse().unwrap())
            .subject(subject)
            .body(body)
            .unwrap();

        let creds = lettre::transport::smtp::authentication::Credentials::new(
            gmail_user.to_owned(),
            gmail_app_password.to_owned(),
        );

        let mailer = lettre::AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com")
            .inspect_err(|e| {
                log::error!("[send_verification_mail] SmtpTransport after relay: {e:?}")
            })
            .map_err(|_| ServiceError::InternalError)?
            .credentials(creds)
            .build();

        mailer
            .send(email)
            .await
            .map_err(|e| log::error!("[send_verification_mail] send error: {e:?}"))
            .and_then(|r| {
                if r.is_positive() {
                    Ok(())
                } else {
                    log::error!("[send_verification_mail] response was not positive: {r:?}");
                    Err(())
                }
            })
            .map_err(|_| ServiceError::InternalError)
    }

    /// authenticate_user
    ///
    /// Given a [`LoginInput`], returns a [`JWT`] token pair that will be used for further
    /// authentication by the user on other endpoints
    ///
    pub fn authenticate_user(&self, input: LoginInput) -> Result<Option<TokenPair>, ServiceError> {
        match input.auth_method {
            AuthMethod::EmailPass(email_pass) => {
                let EmailPass::Fields(email_pass) = email_pass;

                let (user, stored_hash) =
                    if let Some(res) = self.dao.find_by_email(&email_pass.email)? {
                        res
                    } else {
                        log::info!("[authenticate_user] no user found");
                        return Ok(None);
                    };

                let auth_method = self
                    .dao
                    .find_email_auth_by_user_id(&user.id)?
                    .ok_or(ServiceError::Forbidden)?;

                if !auth_method.is_verified {
                    log::info!("[authenticate_user] email not verified");
                    return Ok(None);
                }

                if !crate::helpers::crypto_password::verify_password(&stored_hash, &email_pass.pass)
                {
                    log::info!("[authenticate_user] password mismatch");
                    return Ok(None);
                }

                let jwt_kind = JWTKind::Refresh;
                let (iat, exp) = jwt_kind.get_iat_exp();
                let new_token = NewRefreshToken {
                    user_id: user.id,
                    expires_at: exp,
                };
                let version_n = self.dao.rotate_refresh_token(new_token)?;

                let refresh_token = JWT::new(
                    version_n,
                    user.login.clone(),
                    user.id,
                    jwt_kind,
                    iat.timestamp(),
                    exp.timestamp(),
                );

                let jwt_kind = JWTKind::Session;
                let (iat, exp) = jwt_kind.get_iat_exp();
                let access_token = JWT::new(
                    version_n,
                    user.login,
                    user.id,
                    jwt_kind,
                    iat.timestamp(),
                    exp.timestamp(),
                );

                let key = JWT::get_enc_key();
                let refresh_token = refresh_token.encode(key)?;
                let access_token = access_token.encode(key)?;

                Ok(Some(TokenPair {
                    session_token: access_token,
                    refresh_token,
                }))
            }
        }
    }

    /// request_password_reset
    ///
    /// # Description
    /// Given a email, generate a password reset for it and send a mail with the code
    ///
    /// # Use
    /// Reset password with just email access
    pub async fn request_password_reset(&self, email: &str) -> Result<(), ServiceError> {
        let (user, _) = match self.dao.find_by_email(email)? {
            Some(user) => user,
            None => {
                log::info!("[request_password_reset] no user found for {email}");
                return Ok(());
            }
        };

        let code = rand::random_range(0..=999_999);
        let code = format!("{:06}", code);
        let npr = crate::model::models::NewPasswordReset {
            user_id: user.id,
            email,
            code: &code,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
        };
        let pr = self.dao.create_password_reset(npr)?;

        match Self::send_mail(
            email,
            "Password Reset Code",
            format!("Your one-time code is: {code}"),
        )
        .await
        {
            Ok(()) => (),
            Err(e) => {
                log::error!("[request_password_reset] Error sending verification code: {e:?}");
                self.dao.consume_password_reset(&pr.id)?;
                Err(ServiceError::InternalError)?
            }
        };

        log::info!("[request_password_reset] Sent email to {email}, with code: {code}");

        Ok(())
    }

    /// reset_password
    ///
    /// # Description
    /// Given a code, an email and a new password, ensure a valid PasswordReset exists for it and reset the password
    /// # Use
    /// After requesting the password reset, use it
    pub fn reset_password(
        &self,
        PasswordResetInput {
            code,
            email,
            new_password,
        }: PasswordResetInput,
    ) -> Result<(), ServiceError> {
        let (user, _) = self
            .dao
            .find_by_email(&email)?
            .ok_or(ServiceError::NotFound)?;

        let pr = match self.reset_password_verify(&code, &user) {
            Ok(pr) => pr,
            Err(e) => {
                log::warn!("[reset_password] Verification failed, adding attempt: {e:?}");
                self.dao.add_attempt_user_verifications(&user.id)?;
                Err(ServiceError::NotFound)?
            }
        };

        // Reset password
        let hashed_pass =
            crate::helpers::crypto_password::hash_password(&new_password).map_err(|e| {
                ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                    "Bcrypt hash error: {e}"
                )))
            })?;

        self.dao
            .update_email_password(&user.id, &hashed_pass)
            .map_err(ServiceError::from)?;

        self.dao.consume_password_reset(&pr.id)?;

        Ok(())
    }

    fn reset_password_verify(
        &self,
        code: &str,
        user: &User,
    ) -> Result<PasswordReset, ServiceError> {
        code.parse::<i32>()
            .map_err(|e| log::warn!("[reset_password] received invalid i32 as code: {e:?}"))
            .and_then(|num| {
                if !(0..=999_999).contains(&num) {
                    Err(())
                } else {
                    Ok(1)
                }
            })
            .map_err(|()| {
                ServiceError::TypeError(crate::interface::types::TypeError::Validation(
                    "Invalid code format, expected 6 numeric digit".to_string(),
                ))
            })?;

        let pr = self
            .dao
            .find_password_reset(code, &user.id)?
            .ok_or(ServiceError::NotFound)?;

        const MAX_RESET_ATTEMPTS: i32 = 10;

        if pr.attempts > MAX_RESET_ATTEMPTS
            || pr.expires_at < Utc::now()
            || pr.consumed_at.is_some()
        {
            Err(ServiceError::NotFound)?
        }

        Ok(pr)
    }

    /// validate_session
    ///
    /// # Description
    /// Given a session token, validates it and returns the associated `User`.
    ///
    /// # Use
    /// This is intended for use in an auth middleware.
    /// TODO: this may use a "Redis" db
    pub fn validate_session(&self, session_token: &str) -> Result<JWT, ServiceError> {
        let decoded = JWT::decode(session_token, JWT::get_dec_key())?.claims;

        if decoded.is_refresh {
            return Err(ServiceError::Forbidden);
        }

        let user = self
            .dao
            .find_by_login(&decoded.login)?
            .ok_or(ServiceError::Forbidden)?;

        // Verify that the token belongs to the current, active refresh token family.
        if !self.dao.verify_token(&user.id, decoded.version_n)? {
            return Err(ServiceError::Forbidden);
        }

        Ok(decoded)
    }

    /// refresh_session
    ///
    /// # Description
    /// Given a to-refresh `refresh_token`, claims it and returns a new pair of
    /// session and refresh tokens ([`RefreshSessionOut`]).
    ///
    /// # Use
    /// Intended for use by a dedicated endpoint
    ///
    pub fn refresh_session(&self, refresh_token: &str) -> Result<RefreshSessionOut, ServiceError> {
        let decoded = JWT::decode(refresh_token, JWT::get_dec_key())?.claims;

        if !decoded.is_refresh {
            return Err(ServiceError::Forbidden);
        }

        let user_id = if let Some(user) = self.dao.find_by_login(&decoded.login)? {
            user.id
        } else {
            return Err(ServiceError::Forbidden);
        };

        if !self.dao.verify_token(&user_id, decoded.version_n)? {
            return Err(ServiceError::Forbidden);
        }

        let jwt_kind = JWTKind::Refresh;
        let (iat, exp) = jwt_kind.get_iat_exp();

        let new_refresh = NewRefreshToken {
            user_id,
            expires_at: exp,
        };

        let new_version_n = self.dao.rotate_refresh_token(new_refresh)?;
        let new_refresh = JWT::new(
            new_version_n,
            decoded.login.clone(),
            decoded.user_id,
            jwt_kind,
            iat.timestamp(),
            exp.timestamp(),
        );
        let refresh = new_refresh.encode(JWT::get_enc_key())?;

        let new_session = JWT::new(
            new_version_n,
            decoded.login,
            decoded.user_id,
            JWTKind::Session,
            iat.timestamp(),
            exp.timestamp(),
        );
        let session = new_session.encode(JWT::get_enc_key())?;
        Ok(RefreshSessionOut {
            session_token: session,
            refresh_token: refresh,
        })
    }

    /// register_user
    ///
    /// # Description
    /// Given a [`RegisterInput`], tries to create a new `User` in the system
    ///
    /// # Use
    /// Intended for use by a register endpoint
    ///
    pub async fn register_user(
        &self,
        input: RegisterInput,
    ) -> Result<RegisterResponse, ServiceError> {
        // The user insert operation needs to be atomic on the service level, if auth method
        // creation fails user needs to be deleted from DB
        let new_user = NewUser {
            login: &input.login,
        };

        let user = self.dao.create_user(new_user)?;

        if let Err(e) = match input.auth_method {
            AuthMethod::EmailPass(email_pass) => match email_pass {
                EmailPass::Fields(email_pass_fields) => {
                    let hashed_pass =
                        crate::helpers::crypto_password::hash_password(&email_pass_fields.pass)
                            .map_err(|e| {
                                ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                                    "Bcrypt hash error: {e}"
                                )))
                            })?;

                    let auth = NewEmailAuthMethod {
                        user_id: user.id,
                        email: &email_pass_fields.email,
                        hashed_pass: &hashed_pass,
                    };
                    self.dao.add_email_auth_method(auth).map(|_| ())?;

                    let expires_at = Utc::now() + Duration::hours(24);
                    let verification =
                        self.dao.create_email_verification(NewEmailVerification {
                            user_id: user.id,
                            email: &email_pass_fields.email,
                            expires_at,
                        })?;

                    let verify_url = format!(
                        "http://{}:{}/auth/verify?token={}",
                        crate::config::get().get_server_name(),
                        crate::config::get().get_http_port(),
                        verification.id
                    );
                    Self::send_mail(
                        &email_pass_fields.email,
                        "Email Verification link",
                        format!("Click this link to verify your account: {}", verify_url),
                    )
                    .await?;

                    log::info!("[register_user] verification url: {verify_url}");
                    Ok::<(), ServiceError>(())
                }
            },
        } {
            log::warn!("Error adding auth method: {e:?}");
            if let Err(delete_err) = self.dao.delete_user(&user.id) {
                log::error!("Failed to clean up user after auth method failure: {delete_err:?}");
            }
            return Err(ServiceError::DbError(crate::dao::DaoError::OtherRare(
                "Failed to create auth method".to_string(),
            )));
        };

        // TODO: Auth methods that don't require a confirmation's Session Tokens may be returned here
        Ok(RegisterResponse::NeedsVerification)
    }

    /// verify_email
    ///
    /// # Description
    /// Validates an email verification token and marks the email as verified.
    pub fn verify_email(&self, token: Uuid) -> Result<VerifyEmailOut, ServiceError> {
        let verification = self
            .dao
            .find_email_verification(&token)?
            .ok_or(ServiceError::NotFound)?;

        if verification.consumed_at.is_some() {
            return Ok(VerifyEmailOut { verified: false });
        }

        if verification.expires_at < Utc::now() {
            return Ok(VerifyEmailOut { verified: false });
        }

        self.dao.mark_email_verified(&verification.user_id)?;
        self.dao.consume_email_verification(&token)?;

        Ok(VerifyEmailOut { verified: true })
    }

    /// change_password
    ///
    /// # Description
    /// Given a [`ChangePasswordInput`], validates the current password, updates it,
    /// and returns a fresh session/refresh token pair.
    pub fn change_password(&self, input: ChangePasswordInput) -> Result<TokenPair, ServiceError> {
        let auth = self
            .dao
            .find_email_auth_by_user_id(&input.user_id)?
            .ok_or(ServiceError::Forbidden)?;

        if !crate::helpers::crypto_password::verify_password(
            &auth.hashed_pass,
            &input.current_password,
        ) {
            return Err(ServiceError::Forbidden);
        }

        let hashed_pass = crate::helpers::crypto_password::hash_password(&input.new_password)
            .map_err(|e| {
                ServiceError::DbError(crate::dao::DaoError::OtherRare(format!(
                    "Bcrypt hash error: {e}"
                )))
            })?;

        self.dao
            .update_email_password(&input.user_id, &hashed_pass)
            .map_err(ServiceError::from)?;

        let user = self
            .dao
            .find_by_id(input.user_id)?
            .ok_or(ServiceError::Forbidden)?;

        let jwt_kind = JWTKind::Refresh;
        let (iat, exp) = jwt_kind.get_iat_exp();
        let new_token = NewRefreshToken {
            user_id: user.id,
            expires_at: exp,
        };
        let version_n = self.dao.rotate_refresh_token(new_token)?;

        let refresh_token = JWT::new(
            version_n,
            user.login.clone(),
            user.id,
            jwt_kind,
            iat.timestamp(),
            exp.timestamp(),
        );

        let jwt_kind = JWTKind::Session;
        let (iat, exp) = jwt_kind.get_iat_exp();
        let access_token = JWT::new(
            version_n,
            user.login,
            user.id,
            jwt_kind,
            iat.timestamp(),
            exp.timestamp(),
        );

        let key = JWT::get_enc_key();
        let refresh_token = refresh_token.encode(key)?;
        let access_token = access_token.encode(key)?;

        Ok(TokenPair {
            session_token: access_token,
            refresh_token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dao::DaoError;
    use crate::dao::identity::PgIdentityDao;
    use crate::test_utils::setup;

    mod unit_test {
        // No unit tests for AuthService at the moment.
    }

    mod integration_test {
        use super::*;
        use crate::helpers::crypto_password::verify_password;
        use crate::interface::types::auth::EmailPassFields;
        use crate::model::models::NewPasswordReset;

        #[tokio::test]
        async fn test_register_and_authenticate_integration() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            let login = "test_user".to_string();
            let email = "test@example.com".to_string();
            let pass = "password".to_string();

            let register_input = RegisterInput {
                login: login.clone(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: pass.clone(),
                })),
            };

            // Act - Register
            let register_res = service
                .register_user(register_input)
                .await
                .expect("Failed to register user");
            assert!(matches!(register_res, RegisterResponse::NeedsVerification));

            // Act - Authenticate
            let login_input = LoginInput {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: pass.clone(),
                })),
            };

            let token_pair = service
                .authenticate_user(login_input)
                .expect("Failed to authenticate");
            assert!(token_pair.is_none());

            let user_id = service.dao.find_by_login(&login).unwrap().unwrap().id;

            let verification = service
                .dao
                .create_email_verification(NewEmailVerification {
                    user_id,
                    email: &email,
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                })
                .expect("Failed to create verification");

            let verify_res = service
                .verify_email(verification.id)
                .expect("Failed to verify email");
            assert!(verify_res.verified);

            let login_input = LoginInput {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: email.clone(),
                    pass: pass.clone(),
                })),
            };

            let token_pair = service
                .authenticate_user(login_input)
                .expect("Failed to authenticate")
                .expect("User should be found");

            assert!(!token_pair.session_token.is_empty());
            assert!(!token_pair.refresh_token.is_empty());

            // Act - Validate Session
            let decoded = service
                .validate_session(&token_pair.session_token)
                .expect("Failed to validate session");
            assert_eq!(decoded.login, login);
            assert!(!decoded.is_refresh);

            // Act - Refresh Session
            let refresh_out = service
                .refresh_session(&token_pair.refresh_token)
                .expect("Failed to refresh session");
            assert!(!refresh_out.session_token.is_empty());
            assert!(!refresh_out.refresh_token.is_empty());

            // Validate new session
            let decoded_new = service
                .validate_session(&refresh_out.session_token)
                .expect("Failed to validate new session");
            assert_eq!(decoded_new.login, login);

            // Old refresh token should be invalid now
            let old_refresh_res = service.refresh_session(&token_pair.refresh_token);
            assert!(old_refresh_res.is_err());
        }

        #[test]
        fn test_authenticate_invalid_credentials() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            let login_input = LoginInput {
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "nonexistent@example.com".to_string(),
                    pass: "wrong_password".to_string(),
                })),
            };

            let result = service
                .authenticate_user(login_input)
                .expect("Should not return error for invalid credentials");
            assert!(result.is_none());
        }

        #[test]
        fn test_validate_invalid_token() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            let result = service.validate_session("invalid.token.here");
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_register_user_duplicate_login() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            let input = RegisterInput {
                login: "duplicate".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test1@example.com".to_string(),
                    pass: "pass".to_string(),
                })),
            };

            service
                .register_user(input)
                .await
                .expect("First registration should succeed");

            let input_duplicate = RegisterInput {
                login: "duplicate".to_string(),
                auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                    email: "test2@example.com".to_string(),
                    pass: "pass".to_string(),
                })),
            };

            let result = service.register_user(input_duplicate).await;
            assert!(matches!(
                result,
                Err(ServiceError::DbError(DaoError::UniqueViolation(_)))
            ));
        }

        #[tokio::test]
        async fn test_validate_session_with_refresh_token() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            // Register and get tokens
            let email = "test@example.com".to_string();
            service
                .register_user(RegisterInput {
                    login: "user".to_string(),
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: email.clone(),
                        pass: "pass".to_string(),
                    })),
                })
                .await
                .unwrap();

            let verification = service
                .dao
                .create_email_verification(NewEmailVerification {
                    user_id: service.dao.find_by_login("user").unwrap().unwrap().id,
                    email: &email,
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                })
                .unwrap();
            service
                .dao
                .mark_email_verified(&verification.user_id)
                .unwrap();

            let tokens = service
                .authenticate_user(LoginInput {
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email,
                        pass: "pass".to_string(),
                    })),
                })
                .unwrap()
                .unwrap();

            // Try to validate session using refresh token
            let result = service.validate_session(&tokens.refresh_token);
            assert!(matches!(result, Err(ServiceError::Forbidden)));
        }

        #[tokio::test]
        async fn test_refresh_session_with_session_token() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            // Register and get tokens
            let email = "test@example.com".to_string();
            service
                .register_user(RegisterInput {
                    login: "user".to_string(),
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: email.clone(),
                        pass: "pass".to_string(),
                    })),
                })
                .await
                .unwrap();

            let verification = service
                .dao
                .create_email_verification(NewEmailVerification {
                    user_id: service.dao.find_by_login("user").unwrap().unwrap().id,
                    email: &email,
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                })
                .unwrap();
            service
                .dao
                .mark_email_verified(&verification.user_id)
                .unwrap();

            let tokens = service
                .authenticate_user(LoginInput {
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email,
                        pass: "pass".to_string(),
                    })),
                })
                .unwrap()
                .unwrap();

            // Try to refresh session using session token
            let result = service.refresh_session(&tokens.session_token);
            assert!(matches!(result, Err(ServiceError::Forbidden)));
        }

        #[tokio::test]
        async fn test_request_password_reset_missing_user_no_error() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            let result = service.request_password_reset("missing@example.com").await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_reset_password_success() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            service
                .register_user(RegisterInput {
                    login: "user".to_string(),
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: "test@example.com".to_string(),
                        pass: "old_password".to_string(),
                    })),
                })
                .await
                .unwrap();

            let verification = service
                .dao
                .create_email_verification(NewEmailVerification {
                    user_id: service.dao.find_by_login("user").unwrap().unwrap().id,
                    email: "test@example.com",
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(24),
                })
                .unwrap();
            service
                .dao
                .mark_email_verified(&verification.user_id)
                .unwrap();

            let user_id = service.dao.find_by_login("user").unwrap().unwrap().id;
            let code = "123456".to_string();
            service
                .dao
                .create_password_reset(NewPasswordReset {
                    user_id,
                    email: "test@example.com",
                    code: &code,
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                })
                .unwrap();

            service
                .reset_password(PasswordResetInput::new(
                    "test@example.com".to_string(),
                    code.clone(),
                    "new_password".to_string(),
                ))
                .unwrap();

            let auth = service
                .dao
                .find_email_auth_by_user_id(&user_id)
                .unwrap()
                .unwrap();
            assert!(verify_password(&auth.hashed_pass, "new_password"));

            let pr = service
                .dao
                .find_password_reset(&code, &user_id)
                .unwrap()
                .unwrap();
            assert!(pr.consumed_at.is_some());
        }

        #[tokio::test]
        async fn test_reset_password_invalid_code_increments_attempts() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let service = AuthService::new(dao);

            service
                .register_user(RegisterInput {
                    login: "user".to_string(),
                    auth_method: AuthMethod::EmailPass(EmailPass::Fields(EmailPassFields {
                        email: "test@example.com".to_string(),
                        pass: "old_password".to_string(),
                    })),
                })
                .await
                .unwrap();

            let user_id = service.dao.find_by_login("user").unwrap().unwrap().id;
            service
                .dao
                .create_password_reset(NewPasswordReset {
                    user_id,
                    email: "test@example.com",
                    code: "123456",
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(2),
                })
                .unwrap();

            let result = service.reset_password(PasswordResetInput::new(
                "test@example.com".to_string(),
                "654321".to_string(),
                "new_password".to_string(),
            ));
            assert!(matches!(result, Err(ServiceError::NotFound)));

            let pr = service
                .dao
                .find_password_reset("123456", &user_id)
                .unwrap()
                .unwrap();
            assert_eq!(pr.attempts, 1);
        }
    }
}
