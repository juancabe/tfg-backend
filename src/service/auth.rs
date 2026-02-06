use std::{env, time::Duration};

use crate::helpers::auth::AuthMethod;
use crate::interface::auth_controller::{LoginPayload, RegisterPayload};
use crate::model::models::{NewEmailAuthMethod, NewUser, User};
use crate::service::ServiceError;
use crate::{dao::identity::IdentityDao, model::models::NewRefreshToken};
use chrono::{DateTime, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

// Placeholder Inputs
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

    pub fn authenticate_user(&self, input: LoginInput) -> Result<Option<TokenPair>, ServiceError> {
        match input.auth_method {
            AuthMethod::EmailPass(email_pass) => {
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
                    jwt_kind,
                    iat.timestamp(),
                    exp.timestamp(),
                );

                let jwt_kind = JWTKind::Refresh;
                let (iat, exp) = jwt_kind.get_iat_exp();
                let access_token = JWT::new(
                    version_n,
                    user.login,
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
    /// Given a session token, validates it and returns the associated User.
    ///
    /// # Use
    /// This is intended for use in an auth middleware.
    /// TODO: this should trigger a "Redis" db
    pub fn validate_session(&self, session_token: &str) -> Result<User, ServiceError> {
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

        Ok(user)
    }

    /// refresh_session
    ///
    /// # Description
    /// Given a to-refresh `refresh_token`, claims it and returns a new pair of
    /// session and refresh tokens.
    ///
    /// # Use
    /// Intended for use by a dedicated endpoint
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
            jwt_kind,
            iat.timestamp(),
            exp.timestamp(),
        );
        let refresh = new_refresh.encode(JWT::get_enc_key())?;

        let new_session = JWT::new(
            new_version_n,
            decoded.login,
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

    pub fn register_user(&self, input: RegisterInput) -> Result<User, ServiceError> {
        // The user insert operation needs to be atomic on the service level, if auth method
        // creation fails user needs to be deleted from DB
        let new_user = NewUser {
            login: &input.login,
        };

        let user = self._dao.create_user(new_user)?;

        if let Err(e) = match input.auth_method {
            AuthMethod::EmailPass(email_pass) => {
                let auth = NewEmailAuthMethod {
                    user_id: user.id,
                    email: &email_pass.email,
                    hashed_pass: &email_pass.hashed_pass,
                };
                self._dao.add_email_auth_method(auth).map(|_| ())
            }
        } {
            log::warn!("Error adding auth method: {e:?}");
            todo!("Delete user")
        };

        // TODO: For auth methods that don't require a confirmation Session Tokens may be returned
        // from here

        Ok(user)
    }
}

enum JWTKind {
    Refresh,
    Session,
}

impl JWTKind {
    fn exp_offset_secs(&self) -> Duration {
        match self {
            JWTKind::Refresh => Duration::from_hours(24 * 14),
            JWTKind::Session => Duration::from_mins(10),
        }
    }

    pub fn get_iat_exp(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let iat = chrono::Utc::now();
        let exp = iat + self.exp_offset_secs();
        (iat, exp)
    }

    pub fn is_refresh(&self) -> bool {
        matches!(self, Self::Refresh)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JWT {
    // claims
    version_n: i64,
    login: String,
    iat: i64,
    exp: i64,
    is_refresh: bool,
}

lazy_static::lazy_static! {
    static ref b64_key: String = env::var("JWT_KEY_BASE64").expect("Should be set/loaded");
    static ref enc: EncodingKey = EncodingKey::from_base64_secret(&b64_key).expect("JWT key should be valid base64");
    static ref dec: DecodingKey = DecodingKey::from_base64_secret(&b64_key).expect("JWT key should be valid base64");
}

impl JWT {
    const ALGORITHM: Algorithm = Algorithm::HS256;

    fn get_dec_key() -> &'static DecodingKey {
        &dec
    }

    fn get_enc_key() -> &'static EncodingKey {
        &enc
    }

    fn new(version_n: i64, login: String, jwt_kind: JWTKind, iat: i64, exp: i64) -> Self {
        Self {
            version_n,
            login,
            iat,
            exp,
            is_refresh: jwt_kind.is_refresh(),
        }
    }

    pub fn encode(&self, key: &EncodingKey) -> Result<String, jsonwebtoken::errors::Error> {
        encode(&Header::new(Self::ALGORITHM), &self, key)
    }

    pub fn decode(
        token: impl AsRef<[u8]>,
        key: &DecodingKey,
    ) -> Result<TokenData<Self>, jsonwebtoken::errors::Error> {
        let validation = Validation::new(Self::ALGORITHM);
        decode(token, key, &validation)
    }
}
