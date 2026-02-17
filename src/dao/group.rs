use crate::dao::{DaoError, DbPool};
use crate::model::models::{Group, GroupUpdate, NewFavoriteGroup, NewGroup};
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

// Helper Enum for Permissions
#[derive(Debug, PartialEq)]
pub enum GroupRole {
    Owner,
    Collaborator,
    Viewer,
    None,
}

pub trait GroupDao {
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError>;
    fn update_group(&self, group_id: Uuid, update: GroupUpdate) -> Result<Group, DaoError>;
    fn get_group(&self, group_id: Uuid) -> Result<Group, DaoError>;
    fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>, DaoError>;
    fn toggle_favorite_group(&self, user_id_in: Uuid, group_id_in: Uuid) -> Result<bool, DaoError>;
}

#[derive(Clone)]
pub struct PgGroupDao {
    pool: DbPool,
}

impl From<DbPool> for PgGroupDao {
    fn from(value: DbPool) -> Self {
        Self::new(value)
    }
}

impl PgGroupDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl GroupDao for PgGroupDao {
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError> {
        use crate::model::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(groups)
            .values(&new_group)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn update_group(&self, group_id_in: Uuid, update: GroupUpdate) -> Result<Group, DaoError> {
        use crate::model::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(groups.find(group_id_in))
            .set(&update)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_group(&self, group_id: Uuid) -> Result<Group, DaoError> {
        use crate::model::schema::groups;
        let conn = &mut self.pool.get()?;

        groups::table
            .find(group_id)
            .select(Group::as_select())
            .load(conn)?
            .into_iter()
            .next()
            .ok_or(DaoError::NotFound)
    }

    fn get_user_groups(&self, user_id_in: Uuid) -> Result<Vec<Group>, DaoError> {
        use crate::model::schema::groups;
        let conn = &mut self.pool.get()?;

        groups::table
            .filter(groups::owner_user_id.eq(user_id_in))
            .select(Group::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    fn toggle_favorite_group(&self, user_id_in: Uuid, group_id_in: Uuid) -> Result<bool, DaoError> {
        use crate::model::schema::favorite_groups::dsl::*;
        let conn = &mut self.pool.get()?;

        conn.transaction(|conn| {
            let exists: i64 = favorite_groups
                .filter(user_id.eq(user_id_in))
                .filter(group_id.eq(group_id_in))
                .count()
                .get_result(conn)?;

            if exists > 0 {
                // Remove
                diesel::delete(
                    favorite_groups
                        .filter(user_id.eq(user_id_in))
                        .filter(group_id.eq(group_id_in)),
                )
                .execute(conn)?;
                Ok(false)
            } else {
                // Add
                diesel::insert_into(favorite_groups)
                    .values(&NewFavoriteGroup {
                        user_id: user_id_in,
                        group_id: group_id_in,
                    })
                    .execute(conn)?;
                Ok(true)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{create_test_group, create_test_location, create_test_user, setup};

    mod unit_test {
        use super::*;

        #[test]
        fn test_create_group() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "create_group");
            let location = create_test_location(&ctx.pool, "create_group");

            let new_group = NewGroup {
                owner_user_id: user.id,
                location_id: location.id,
                name: "Test Group".to_string(),
                description: "Test Description".to_string(),
            };

            let result = dao.create_group(new_group).expect("Failed to create group");

            assert_eq!(result.name, "Test Group");
            assert_eq!(result.owner_user_id, user.id);
        }

        #[test]
        fn test_create_group_invalid_owner() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let location = create_test_location(&ctx.pool, "invalid_owner");

            let new_group = NewGroup {
                owner_user_id: Uuid::new_v4(), // Non-existent user
                location_id: location.id,
                name: "Test Group".to_string(),
                description: "Test Description".to_string(),
            };

            let result = dao.create_group(new_group);

            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }

        #[test]
        fn test_update_group() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "update_group");
            let location = create_test_location(&ctx.pool, "update_group");
            let group = create_test_group(&ctx.pool, user.id, location.id, "update_group");

            let update = GroupUpdate {
                name: Some("Updated Name".to_string()),
                description: Some("Updated Description".to_string()),
                location_id: None,
                is_public: true,
            };

            let result = dao
                .update_group(group.id, update)
                .expect("Failed to update group");

            assert_eq!(result.name, "Updated Name");
            assert_eq!(result.description, "Updated Description");
            assert!(result.is_public);
        }

        #[test]
        fn test_update_group_not_found() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());

            let update = GroupUpdate {
                name: Some("Updated Name".to_string()),
                description: None,
                location_id: None,
                is_public: false,
            };

            let result = dao.update_group(Uuid::new_v4(), update);

            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_get_group() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "get_group");
            let location = create_test_location(&ctx.pool, "get_group");
            let group = create_test_group(&ctx.pool, user.id, location.id, "get_group");

            let result = dao.get_group(group.id).expect("Failed to get group");

            assert_eq!(result.id, group.id);
            assert_eq!(result.name, group.name);
        }

        #[test]
        fn test_get_group_not_found() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());

            let result = dao.get_group(Uuid::new_v4());

            assert!(matches!(result, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_get_user_groups() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "user_groups");
            let location = create_test_location(&ctx.pool, "user_groups");
            create_test_group(&ctx.pool, user.id, location.id, "1");
            create_test_group(&ctx.pool, user.id, location.id, "2");

            let result = dao
                .get_user_groups(user.id)
                .expect("Failed to get user groups");

            assert_eq!(result.len(), 2);
        }

        #[test]
        fn test_get_user_groups_empty() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "empty_groups");

            let result = dao
                .get_user_groups(user.id)
                .expect("Failed to get user groups");

            assert!(result.is_empty());
        }

        #[test]
        fn test_toggle_favorite_group() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "toggle_fav");
            let location = create_test_location(&ctx.pool, "toggle_fav");
            let group = create_test_group(&ctx.pool, user.id, location.id, "toggle_fav");

            // Toggle on
            let added = dao
                .toggle_favorite_group(user.id, group.id)
                .expect("Failed to toggle favorite");
            assert!(added);

            // Toggle off
            let removed = dao
                .toggle_favorite_group(user.id, group.id)
                .expect("Failed to toggle favorite");
            assert!(!removed);
        }

        #[test]
        fn test_toggle_favorite_group_invalid_group() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "invalid_fav");

            let result = dao.toggle_favorite_group(user.id, Uuid::new_v4());

            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }
    }

    mod integration_test {
        use super::*;

        #[test]
        fn test_group_management_flow() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "mgmt_flow");
            let location = create_test_location(&ctx.pool, "mgmt_flow");

            // 1. Create group
            let new_group = NewGroup {
                owner_user_id: user.id,
                location_id: location.id,
                name: "Initial Name".to_string(),
                description: "Initial Description".to_string(),
            };
            let created = dao.create_group(new_group).unwrap();

            // 2. Verify creation
            let fetched = dao.get_group(created.id).unwrap();
            assert_eq!(fetched.name, "Initial Name");

            // 3. Update group
            let update = GroupUpdate {
                name: Some("New Name".to_string()),
                description: None,
                location_id: None,
                is_public: true,
            };
            dao.update_group(created.id, update).unwrap();

            // 4. Verify update and presence in user groups
            let user_groups = dao.get_user_groups(user.id).unwrap();
            assert_eq!(user_groups.len(), 1);
            assert_eq!(user_groups[0].name, "New Name");
            assert!(user_groups[0].is_public);
        }

        #[test]
        fn test_favorite_toggle_flow() {
            let ctx = setup();
            let dao = PgGroupDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "fav_flow");
            let location = create_test_location(&ctx.pool, "fav_flow");

            let g1 = dao
                .create_group(NewGroup {
                    owner_user_id: user.id,
                    location_id: location.id,
                    name: "G1".to_string(),
                    description: "D1".to_string(),
                })
                .unwrap();

            // Toggle favorite
            assert!(dao.toggle_favorite_group(user.id, g1.id).unwrap()); // Added
            assert!(!dao.toggle_favorite_group(user.id, g1.id).unwrap()); // Removed
            assert!(dao.toggle_favorite_group(user.id, g1.id).unwrap()); // Added again
        }
    }
}
