mod jwt;
pub use jwt::JWT;

use crate::helpers::users_auth::{AuthMethod, EmailPass};
use crate::interface::http::auth_controller::{LoginPayload, RegisterPayload, RegisterResponse};
use crate::model::models::{NewEmailAuthMethod, NewUser};
use crate::service::ServiceError;
use crate::service::auth::jwt::JWTKind;
use crate::{dao::identity::IdentityDao, model::models::NewRefreshToken};
use serde::{Deserialize, Serialize};

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
    _dao: D,
}

#[derive(Serialize)]
pub struct RefreshSessionOut {
    pub session_token: String,
    pub refresh_token: String,
}

impl<D: IdentityDao> AuthService<D> {
    pub fn new(dao: D) -> Self {
        Self { _dao: dao }
    }

    /// authenticate_user
    ///
    /// Given a [`LoginInput`], returns a [`JWT`] token pair that will be used for further
    /// authentication by the user on other endpoints
    ///
    /// # TODO
    /// TODO: Right now, for [`EmailPass`] _auth_ it doesn't check wether the email is veified
    /// TODO: Test
    pub fn authenticate_user(&self, input: LoginInput) -> Result<Option<TokenPair>, ServiceError> {
        match input.auth_method {
            AuthMethod::EmailPass(email_pass) => {
                let email_pass = match email_pass {
                    EmailPass::Fields(email_pass_fields) => email_pass_fields,
                    EmailPass::Confirmation(_) => Err(ServiceError::Forbidden)?,
                };

                // TODO: Check verified email
                let user = if let Some(user) = self._dao.find_by_email_pass(email_pass)? {
                    user
                } else {
                    return Ok(None);
                };

                let jwt_kind = JWTKind::Refresh;
                let (iat, exp) = jwt_kind.get_iat_exp();
                let new_token = NewRefreshToken {
                    user_id: user.id,
                    expires_at: exp,
                };
                let version_n = self._dao.rotate_refresh_token(new_token)?;

                // TODO: Get rid of clone
                let refresh_token = JWT::new(
                    version_n,
                    user.login.clone(),
                    user.id,
                    jwt_kind,
                    iat.timestamp(),
                    exp.timestamp(),
                );

                let jwt_kind = JWTKind::Refresh;
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
            ._dao
            .find_by_login(&decoded.login)?
            .ok_or(ServiceError::Forbidden)?;

        // Verify that the token belongs to the current, active refresh token family.
        if !self._dao.verify_token(&user.id, decoded.version_n)? {
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
    /// TODO: Test
    pub fn refresh_session(&self, refresh_token: &str) -> Result<RefreshSessionOut, ServiceError> {
        let decoded = JWT::decode(refresh_token, JWT::get_dec_key())?.claims;

        if !decoded.is_refresh {
            return Err(ServiceError::Forbidden);
        }

        let user_id = if let Some(user) = self._dao.find_by_login(&decoded.login)? {
            user.id
        } else {
            return Err(ServiceError::Forbidden);
        };

        if !self._dao.verify_token(&user_id, decoded.version_n)? {
            return Err(ServiceError::Forbidden);
        }

        let jwt_kind = JWTKind::Refresh;
        let (iat, exp) = jwt_kind.get_iat_exp();

        let new_refresh = NewRefreshToken {
            user_id,
            expires_at: exp,
        };

        let new_version_n = self._dao.rotate_refresh_token(new_refresh)?;
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
    /// TODO: Test
    pub fn register_user(&self, input: RegisterInput) -> Result<RegisterResponse, ServiceError> {
        // The user insert operation needs to be atomic on the service level, if auth method
        // creation fails user needs to be deleted from DB
        let new_user = NewUser {
            login: &input.login,
        };

        let user = self._dao.create_user(new_user)?;

        if let Err(e) = match input.auth_method {
            AuthMethod::EmailPass(email_pass) => match email_pass {
                crate::helpers::users_auth::EmailPass::Fields(email_pass_fields) => {
                    let auth = NewEmailAuthMethod {
                        user_id: user.id,
                        email: &email_pass_fields.email,
                        hashed_pass: &email_pass_fields.hashed_pass,
                    };
                    self._dao.add_email_auth_method(auth).map(|_| ())
                    // TODO: Send email with verification challenge
                }
                crate::helpers::users_auth::EmailPass::Confirmation(email_pass_confirmation) => {
                    // TODO: Change auth status to verified
                    todo!("email_pass_confirmation not implemented: {email_pass_confirmation:?}")
                }
            },
        } {
            log::warn!("Error adding auth method: {e:?}");
            todo!("Delete user")
        };

        // TODO: Auth methods that don't require a confirmation's Session Tokens may be returned here
        Ok(RegisterResponse::Success)
    }
}
