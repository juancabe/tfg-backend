use std::{env, time::Duration};

use crate::model::models::User;
use crate::service::ServiceError;
use crate::{dao::identity::IdentityDao, model::models::NewRefreshToken};
use chrono::{DateTime, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

// Placeholder Inputs
pub struct LoginInput {}
pub struct RegisterInput {}
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

pub struct AuthService<D: IdentityDao> {
    _dao: D,
}

pub struct RefreshSessionOut {
    pub session_token: String,
    pub refresh_token: Option<String>,
}

impl<D: IdentityDao> AuthService<D> {
    pub fn new(dao: D) -> Self {
        Self { _dao: dao }
    }

    pub fn authenticate_user(&self, _input: LoginInput) -> Result<TokenPair, ServiceError> {
        todo!()
    }

    /// refresh_session
    ///
    /// Given a to-refresh `refresh_token`, claim it (if it kind is Refresh) and get a new fresh one
    pub fn refresh_session(
        &self,
        refresh_token: String,
    ) -> Result<RefreshSessionOut, ServiceError> {
        let decoded = JWT::decode(refresh_token, JWT::get_dec_key())?.claims;
        let user_id = if let Some(user) = self._dao.find_by_login(&decoded.login)? {
            user.id
        } else {
            Err(ServiceError::Forbidden)?
        };
        if !self._dao.verify_token(&user_id, decoded.version_n)? {
            Err(ServiceError::Forbidden)?
        }

        if decoded.is_refresh {
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
                refresh_token: Some(refresh),
            })
        } else {
            let jwt_kind = JWTKind::Session;
            let (iat, exp) = jwt_kind.get_iat_exp();
            let new_session = JWT::new(
                decoded.version_n,
                decoded.login,
                jwt_kind,
                iat.timestamp(),
                exp.timestamp(),
            );
            let session_token = new_session.encode(JWT::get_enc_key())?;
            Ok(RefreshSessionOut {
                session_token,
                refresh_token: None,
            })
        }
    }

    pub fn register_user(&self, _input: RegisterInput) -> Result<User, ServiceError> {
        todo!()
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
