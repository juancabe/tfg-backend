//! Location DAO operations for CRUD on geographic locations.

use crate::dao::{DaoError, DbPool};
use crate::model::models::{Location, NewLocation};
use diesel::prelude::*;
use diesel::{RunQueryDsl, SelectableHelper};
use uuid::Uuid;

/// Trait defining location-related database operations.
pub trait LocationDao {
    /// Creates a new location.
    fn create_location(&self, new_location: NewLocation) -> Result<Location, DaoError>;
    /// Creates a new location using an existing database connection.
    fn create_location_with_conn(
        &self,
        conn: &mut diesel::PgConnection,
        new_location: NewLocation,
    ) -> Result<Location, DaoError>;
    /// Retrieves a location by its ID.
    fn get_location(&self, id: Uuid) -> Result<Location, DaoError>;
    /// Updates a location by its ID.
    fn update_location(
        &self,
        id: Uuid,
        update: crate::model::models::LocationUpdate,
    ) -> Result<Location, DaoError>;
    /// Deletes a location by its ID.
    fn delete_location(&self, id: Uuid) -> Result<(), DaoError>;
}

/// PostgreSQL implementation of [`LocationDao`].
#[derive(Clone)]
pub struct PgLocationDao {
    pool: DbPool,
}

impl PgLocationDao {
    /// Creates a new [`PgLocationDao`] with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl From<DbPool> for PgLocationDao {
    fn from(pool: DbPool) -> Self {
        Self::new(pool)
    }
}

impl LocationDao for PgLocationDao {
    fn create_location(&self, new_loc: NewLocation) -> Result<Location, DaoError> {
        use crate::model::schema::locations::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(locations)
            .values(&new_loc)
            .returning(Location::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn create_location_with_conn(
        &self,
        conn: &mut diesel::PgConnection,
        new_loc: NewLocation,
    ) -> Result<Location, DaoError> {
        use crate::model::schema::locations::dsl::*;
        diesel::insert_into(locations)
            .values(&new_loc)
            .returning(Location::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_location(&self, loc_id: Uuid) -> Result<Location, DaoError> {
        use crate::model::schema::locations::dsl::*;
        let conn = &mut self.pool.get()?;
        locations
            .find(loc_id)
            .select(Location::as_select())
            .first(conn)
            .map_err(Into::into)
    }

    fn update_location(
        &self,
        loc_id: Uuid,
        update: crate::model::models::LocationUpdate,
    ) -> Result<Location, DaoError> {
        use crate::model::schema::locations::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(locations.find(loc_id))
            .set(&update)
            .returning(Location::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn delete_location(&self, loc_id: Uuid) -> Result<(), DaoError> {
        use crate::model::schema::locations::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::delete(locations.find(loc_id))
            .execute(conn)
            .map(|_| ())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{create_test_location, setup};

    mod unit_test {
        use super::*;

        #[test]
        fn test_create_location() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());
            let new_loc = NewLocation {
                name: Some("Test Location".to_string()),
                geo_lat: Some(40.4168),
                geo_long: Some(-3.7038),
            };

            let result = dao
                .create_location(new_loc)
                .expect("Failed to create location");

            assert_eq!(result.name, Some("Test Location".to_string()));
            assert_eq!(result.geo_lat, Some(40.4168));
            assert_eq!(result.geo_long, Some(-3.7038));
        }

        #[test]
        fn test_get_location() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());
            let loc = create_test_location(&ctx.pool, "get_test");

            let found = dao.get_location(loc.id).expect("Failed to get location");

            assert_eq!(found.id, loc.id);
            assert_eq!(found.name, loc.name);
        }

        #[test]
        fn test_get_location_not_found() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());

            let result = dao.get_location(Uuid::new_v4());

            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_delete_location() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());
            let loc = create_test_location(&ctx.pool, "delete_test");

            dao.delete_location(loc.id)
                .expect("Failed to delete location");

            let result = dao.get_location(loc.id);
            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_update_location() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());
            let loc = create_test_location(&ctx.pool, "update_test");

            let update = crate::model::models::LocationUpdate {
                name: Some(Some("Updated Name".to_string())),
                geo_lat: Some(Some(12.34)),
                geo_long: Some(Some(56.78)),
            };

            let updated = dao
                .update_location(loc.id, update)
                .expect("Failed to update location");

            assert_eq!(updated.name, Some("Updated Name".to_string()));
            assert_eq!(updated.geo_lat, Some(12.34));
            assert_eq!(updated.geo_long, Some(56.78));

            // Test partial update (set geo_lat to None/NULL)
            let update_null = crate::model::models::LocationUpdate {
                name: None,
                geo_lat: Some(None),
                geo_long: None,
            };

            let updated_null = dao
                .update_location(loc.id, update_null)
                .expect("Failed to update location to null");

            assert_eq!(updated_null.name, Some("Updated Name".to_string()));
            assert_eq!(updated_null.geo_lat, None);
            assert_eq!(updated_null.geo_long, Some(56.78));
        }
    }

    mod integration_test {
        use super::*;

        #[test]
        fn test_location_lifecycle_flow() {
            let ctx = setup();
            let dao = PgLocationDao::new(ctx.pool.clone());

            // 1. Create
            let new_loc = NewLocation {
                name: Some("Lifecycle Location".to_string()),
                geo_lat: Some(10.0),
                geo_long: Some(20.0),
            };
            let created = dao.create_location(new_loc).unwrap();
            let id = created.id;

            // 2. Get
            let found = dao.get_location(id).unwrap();
            assert_eq!(found.name, Some("Lifecycle Location".to_string()));

            // 3. Delete
            dao.delete_location(id).unwrap();

            // 4. Verify deleted
            let result = dao.get_location(id);
            assert!(matches!(result, Err(DaoError::NotFound)));
        }
    }
}
