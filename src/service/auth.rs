use crate::dao::DaoError;
use crate::dao::identity::IdentityDao;
use crate::model::models::User;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Forbidden access")]
    Forbidden,
    #[error("Resource not found")]
    NotFound,
    #[error("Invalid state")]
    InvalidState,
    #[error("Conflict detected")]
    Conflict,
    #[error("Database error: {0}")]
    DbError(#[from] DaoError),
    #[error("Internal server error")]
    InternalError,
}

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

impl<D: IdentityDao> AuthService<D> {
    pub fn new(dao: D) -> Self {
        Self { _dao: dao }
    }

    pub fn authenticate_user(&self, _input: LoginInput) -> Result<TokenPair, ServiceError> {
        todo!()
    }

    pub fn refresh_session(&self, _refresh_token: String) -> Result<String, ServiceError> {
        todo!()
    }

    pub fn register_user(&self, _input: RegisterInput) -> Result<User, ServiceError> {
        todo!()
    }
}
