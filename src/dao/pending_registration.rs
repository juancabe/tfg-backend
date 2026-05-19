//! Pending producer registration DAO operations.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::{
    dao::{DaoError, DbPool},
    model::models::{
        NewPendingProducerRegistration, PendingProducerRegistration,
        PendingProducerRegistrationStatus, PendingProducerRegistrationUpdate,
    },
};

/// Trait defining pending producer registration database operations.
pub trait PendingRegistrationDao {
    /// Creates a new pending registration entry.
    fn create(
        &self,
        new_pending: NewPendingProducerRegistration,
    ) -> Result<PendingProducerRegistration, DaoError>;

    /// Retrieves a pending registration by its ID.
    fn get_by_id(&self, id: Uuid) -> Result<Option<PendingProducerRegistration>, DaoError>;

    /// Returns all pending registrations for a given owner.
    fn get_pending_for_owner(
        &self,
        owner_user_id_in: Uuid,
    ) -> Result<Vec<PendingProducerRegistration>, DaoError>;

    /// Marks a pending registration as confirmed and links it to a producer.
    fn mark_confirmed(
        &self,
        id: Uuid,
        producer_id: Uuid,
        x25519_pub_key_bytes_base64: Option<String>,
        lora_addr: Option<i32>,
    ) -> Result<PendingProducerRegistration, DaoError>;

    /// Marks a pending registration as cancelled.
    fn mark_cancelled(&self, id: Uuid) -> Result<PendingProducerRegistration, DaoError>;

    /// Marks all pending registrations that expired before the given time as expired.
    fn mark_expired_before(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<PendingProducerRegistration>, DaoError>;

    /// Marks a pending registration as confirmed using an existing connection.
    fn mark_confirmed_with_conn(
        &self,
        conn: &mut diesel::PgConnection,
        id: Uuid,
        producer_id: Uuid,
        x25519_pub_key_bytes_base64: Option<String>,
        lora_addr: Option<i32>,
    ) -> Result<PendingProducerRegistration, DaoError>;
}

/// PostgreSQL implementation of [`PendingRegistrationDao`].
#[derive(Clone)]
pub struct PgPendingRegistrationDao {
    pool: DbPool,
}

impl PgPendingRegistrationDao {
    /// Creates a new [`PgPendingRegistrationDao`] with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl From<DbPool> for PgPendingRegistrationDao {
    fn from(pool: DbPool) -> Self {
        Self::new(pool)
    }
}

impl PendingRegistrationDao for PgPendingRegistrationDao {
    fn create(
        &self,
        new_pending: NewPendingProducerRegistration,
    ) -> Result<PendingProducerRegistration, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        diesel::insert_into(pending_producer_registrations)
            .values(&new_pending)
            .returning(PendingProducerRegistration::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_by_id(&self, search_id: Uuid) -> Result<Option<PendingProducerRegistration>, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        pending_producer_registrations
            .filter(id.eq(search_id))
            .select(PendingProducerRegistration::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn get_pending_for_owner(
        &self,
        owner_user_id_in: Uuid,
    ) -> Result<Vec<PendingProducerRegistration>, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        pending_producer_registrations
            .filter(owner_user_id.eq(owner_user_id_in))
            .filter(status.eq(PendingProducerRegistrationStatus::Pending as i16))
            .order(created_at.desc())
            .select(PendingProducerRegistration::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn mark_confirmed(
        &self,
        pending_id: Uuid,
        producer_id_in: Uuid,
        x25519_pub_key_bytes_base64_in: Option<String>,
        lora_addr_in: Option<i32>,
    ) -> Result<PendingProducerRegistration, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        let now = Utc::now();
        let update = PendingProducerRegistrationUpdate {
            producer_id: Some(producer_id_in),
            status: Some(PendingProducerRegistrationStatus::Confirmed as i16),
            confirmed_at: Some(now),
            cancelled_at: None,
            x25519_pub_key_bytes_base64: x25519_pub_key_bytes_base64_in,
            lora_addr: lora_addr_in,
        };

        diesel::update(
            pending_producer_registrations
                .find(pending_id)
                .filter(status.eq(PendingProducerRegistrationStatus::Pending as i16)),
        )
        .set(&update)
        .returning(PendingProducerRegistration::as_returning())
        .get_result(conn)
        .map_err(Into::into)
    }

    fn mark_cancelled(&self, pending_id: Uuid) -> Result<PendingProducerRegistration, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        let now = Utc::now();

        diesel::update(
            pending_producer_registrations
                .find(pending_id)
                .filter(status.eq(PendingProducerRegistrationStatus::Pending as i16)),
        )
        .set((
            status.eq(PendingProducerRegistrationStatus::Cancelled as i16),
            cancelled_at.eq(Some(now)),
        ))
        .returning(PendingProducerRegistration::as_returning())
        .get_result(conn)
        .map_err(Into::into)
    }

    fn mark_expired_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<PendingProducerRegistration>, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let conn = &mut self.pool.get()?;
        diesel::update(
            pending_producer_registrations
                .filter(status.eq(PendingProducerRegistrationStatus::Pending as i16))
                .filter(expires_at.lt(cutoff)),
        )
        .set(status.eq(PendingProducerRegistrationStatus::Expired as i16))
        .returning(PendingProducerRegistration::as_returning())
        .get_results(conn)
        .map_err(Into::into)
    }

    fn mark_confirmed_with_conn(
        &self,
        conn: &mut diesel::PgConnection,
        pending_id: Uuid,
        producer_id_in: Uuid,
        x25519_pub_key_bytes_base64_in: Option<String>,
        lora_addr_in: Option<i32>,
    ) -> Result<PendingProducerRegistration, DaoError> {
        use crate::model::schema::pending_producer_registrations::dsl::*;

        let now = Utc::now();
        let update = PendingProducerRegistrationUpdate {
            producer_id: Some(producer_id_in),
            status: Some(PendingProducerRegistrationStatus::Confirmed as i16),
            confirmed_at: Some(now),
            cancelled_at: None,
            x25519_pub_key_bytes_base64: x25519_pub_key_bytes_base64_in,
            lora_addr: lora_addr_in,
        };

        diesel::update(
            pending_producer_registrations
                .find(pending_id)
                .filter(status.eq(PendingProducerRegistrationStatus::Pending as i16)),
        )
        .set(&update)
        .returning(PendingProducerRegistration::as_returning())
        .get_result(conn)
        .map_err(Into::into)
    }
}
