//! Persistence access implementation (_Data Access Object_)

/// Group DAO operations for creating, updating, and managing groups.
pub mod group;
/// User identity and authentication DAO operations.
pub mod identity;
/// Location DAO operations for managing geographic locations.
pub mod location;
/// MQTT broker DAO operations for auth, data ingestion, and forwarder peers.
pub mod mqtt;
/// Pending producer registration DAO operations.
pub mod pending_registration;
/// Producer DAO operations for managing producers and their credentials.
pub mod producer;
/// Template and data query DAO operations.
pub mod template_data;

use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use thiserror::Error;

/// Type alias for the PostgreSQL connection pool.
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;
/// Type alias for a pooled PostgreSQL connection.
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<PgConnection>>;

/// Maximum number of connections in the database pool.
pub const MAX_DB_POOL_SIZE: u32 = 10;

/// Errors that can occur during database operations.
#[derive(Error, Debug)]
pub enum DaoError {
    /// Failed to acquire a connection from the pool.
    #[error("Database connection error")]
    ConnectionError(#[from] r2d2::Error),

    /// A Diesel query error occurred.
    #[error("Database query error: {0}")]
    DieselError(diesel::result::Error),

    /// The requested entity was not found.
    #[error("Entity not found")]
    NotFound,

    /// A unique constraint was violated.
    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    /// A foreign key constraint was violated.
    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    /// An unexpected error occurred.
    #[error("Unexpected error: {0}")]
    OtherRare(String),
}

impl DaoError {
    /// Verifies that an updated row matches the expected value.
    /// Returns `Ok(())` if they are equal, otherwise `OtherRare`.
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
        log::warn!("Diesel error encountered: {err:?}");
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
