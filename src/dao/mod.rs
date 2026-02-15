//! Persistence access implementation (_Data Access Object_)

pub mod identity;

use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use thiserror::Error;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

pub const MAX_DB_POOL_SIZE: u32 = 10;

#[derive(Error, Debug)]
pub enum DaoError {
    #[error("Database connection error")]
    ConnectionError(#[from] r2d2::Error),

    #[error("Database query error: {0}")]
    DieselError(diesel::result::Error),

    #[error("Entity not found")]
    NotFound,

    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Unexpected error: {0}")]
    OtherRare(String),
}

impl DaoError {
    pub fn update_expect_eq<T: Eq + std::fmt::Debug>(
        at_row: &T,
        update: &T,
        at_dao_fn: &str,
    ) -> Result<(), Self> {
        if update.eq(at_row) {
            Ok(())
        } else {
            Err(Self::OtherRare(format!(
                "[{at_dao_fn}] expected updated row to contain new value ({update:?}) but contains ({at_row:?})"
            )))
        }
    }
}

impl From<diesel::result::Error> for DaoError {
    fn from(err: diesel::result::Error) -> Self {
        match err {
            diesel::result::Error::NotFound => DaoError::NotFound,
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                info,
            ) => DaoError::UniqueViolation(info.message().to_string()),
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                info,
            ) => DaoError::ForeignKeyViolation(info.message().to_string()),
            _ => DaoError::DieselError(err),
        }
    }
}
