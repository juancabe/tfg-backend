//! JWT token creation, encoding, and decoding.

use std::time::Duration;

use chrono::{DateTime, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of JWT token.
pub enum JWTKind {
    /// Long-lived token used to obtain new session tokens.
    Refresh,
    /// Short-lived token used for API authentication.
    Session,
}

impl JWTKind {
    fn exp_offset_secs(&self) -> Duration {
        match self {
            JWTKind::Refresh => Duration::from_hours(24 * 14),
            JWTKind::Session => Duration::from_mins(10),
        }
    }

    /// Returns the issued-at and expiration timestamps for this token kind.
    pub fn get_iat_exp(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let iat = chrono::Utc::now();
        let exp = iat + self.exp_offset_secs();
        (iat, exp)
    }

    /// Returns true if this kind is a refresh token.
    pub fn is_refresh(&self) -> bool {
        matches!(self, Self::Refresh)
    }
}

/// A JWT token with custom claims.
#[derive(Debug, Serialize, Deserialize)]
pub struct JWT {
    /// Token version number for rotation tracking.
    pub version_n: i64,
    /// The user's login name.
    pub login: String,
    /// The user's database ID.
    pub user_id: Uuid,
    /// Issued-at timestamp (Unix seconds).
    pub iat: i64,
    /// Expiration timestamp (Unix seconds).
    pub exp: i64,
    /// Whether this is a refresh token.
    pub is_refresh: bool,
}

lazy_static::lazy_static! {
    static ref enc: EncodingKey = EncodingKey::from_base64_secret(crate::config::get().get_jwt_key_base64()).expect("JWT key should be valid base64");
    static ref dec: DecodingKey = DecodingKey::from_base64_secret(crate::config::get().get_jwt_key_base64()).expect("JWT key should be valid base64");
}

impl JWT {
    const ALGORITHM: Algorithm = Algorithm::HS256;

    /// Returns the static decoding key.
    pub fn get_dec_key() -> &'static DecodingKey {
        &dec
    }

    /// Returns the static encoding key.
    pub fn get_enc_key() -> &'static EncodingKey {
        &enc
    }

    /// Creates a new [`JWT`] with the given claims.
    pub fn new(
        version_n: i64,
        login: String,
        user_id: Uuid,
        jwt_kind: JWTKind,
        iat: i64,
        exp: i64,
    ) -> Self {
        Self {
            version_n,
            login,
            user_id,
            iat,
            exp,
            is_refresh: jwt_kind.is_refresh(),
        }
    }

    /// Encodes the JWT claims into a signed token string.
    pub fn encode(&self, key: &EncodingKey) -> Result<String, jsonwebtoken::errors::Error> {
        encode(&Header::new(Self::ALGORITHM), &self, key)
    }

    /// Decodes and validates a JWT token string.
    pub fn decode(
        token: impl AsRef<[u8]>,
        key: &DecodingKey,
    ) -> Result<TokenData<Self>, jsonwebtoken::errors::Error> {
        let validation = Validation::new(Self::ALGORITHM);
        decode(token, key, &validation)
    }
}
