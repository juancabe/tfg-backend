use std::time::Duration;

use chrono::{DateTime, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub enum JWTKind {
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
    pub version_n: i64,
    pub login: String,
    pub user_id: Uuid,
    pub iat: i64,
    pub exp: i64,
    pub is_refresh: bool,
}

lazy_static::lazy_static! {
    static ref enc: EncodingKey = EncodingKey::from_base64_secret(crate::config::get().get_jwt_key_base64()).expect("JWT key should be valid base64");
    static ref dec: DecodingKey = DecodingKey::from_base64_secret(crate::config::get().get_jwt_key_base64()).expect("JWT key should be valid base64");
}

impl JWT {
    const ALGORITHM: Algorithm = Algorithm::HS256;

    pub fn get_dec_key() -> &'static DecodingKey {
        &dec
    }

    pub fn get_enc_key() -> &'static EncodingKey {
        &enc
    }

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
