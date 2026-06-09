//! Group management service layer.

use crate::dao::DbPool;
use crate::dao::group::GroupDao;
use crate::dao::location::LocationDao;
use crate::interface;
use crate::interface::types::group::Group;
use crate::model::models::{self, GroupUpdate};
use crate::service::ServiceError;
use uuid::Uuid;

// TODO: move firmware/MQTT-compatible name validation (MAX_FIRMWARE_NAME_LEN &
// contains_mqtt_topic_meta) to a shared helper used by provisioning and group
// services to avoid duplication.

const MAX_FIRMWARE_NAME_LEN: usize = 32;

fn contains_mqtt_topic_meta(v: &str) -> bool {
    v.contains('/') || v.contains('+') || v.contains('#')
}

fn validate_firmware_compatible_name(name: &str, field_name: &str) -> Result<(), ServiceError> {
    if name.is_empty() || name.len() > MAX_FIRMWARE_NAME_LEN {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(format!(
                "{field_name} must be 1..={MAX_FIRMWARE_NAME_LEN} characters for firmware compatibility"
            )),
        ));
    }

    if contains_mqtt_topic_meta(name) {
        return Err(ServiceError::TypeError(
            crate::interface::types::TypeError::Validation(format!(
                "{field_name} cannot include '/', '+' or '#'"
            )),
        ));
    }

    Ok(())
}

/// Service for group CRUD operations.
pub struct GroupService<G: GroupDao, L: LocationDao> {
    group_dao: G,
    location_dao: L,
}

impl<G, L> From<DbPool> for GroupService<G, L>
where
    G: GroupDao + Clone + From<DbPool>,
    L: LocationDao + Clone + From<DbPool>,
{
    fn from(value: DbPool) -> Self {
        let l_dao = L::from(value.clone());
        let g_dao = G::from(value);

        Self::new(g_dao, l_dao)
    }
}

impl<G: GroupDao + Clone, L: LocationDao + Clone> GroupService<G, L> {
    /// Creates a new [`GroupService`] with the given DAOs.
    pub fn new(group_dao: G, location_dao: L) -> Self {
        Self {
            group_dao,
            location_dao,
        }
    }

    /// Creates a new group with an associated location.
    pub fn create_group(
        &self,
        user_id: Uuid,
        new_group: interface::types::group::NewGroup,
    ) -> Result<Group, ServiceError> {
        let interface::types::group::NewGroup {
            name,
            description,
            is_public,
            location,
        } = new_group;
        let group_name_for_error = name.clone();
        // Validate group name for firmware compatibility early so we don't create
        // location resources when the group name is invalid.
        validate_firmware_compatible_name(&name, "group name")?;

        let interface::types::location::NewLocation {
            name: loc_name,
            geo_lat,
            geo_long,
        } = location;
        let location = models::NewLocation {
            name: loc_name,
            geo_lat,
            geo_long,
        };

        let location = self.location_dao.create_location(location)?;

        let new_group = models::NewGroup {
            owner_user_id: user_id,
            location_id: location.id,
            name,
            description,
            is_public,
        };

        self.group_dao
            .create_group(new_group)
            .map_err(|err| {
                map_unique_group_name_error(err, "group name", Some(group_name_for_error.as_str()))
            })
            .map(Into::into)
    }

    /// Only returns the group when the user has visibility permissions
    pub fn get_group(&self, user_id: Uuid, group_id: Uuid) -> Result<Group, ServiceError> {
        let group = self.group_dao.get_group(group_id)?;
        if !group.is_public && (group.owner_user_id != user_id) {
            Err(ServiceError::Forbidden)?
        }

        Ok(group.into())
    }

    /// Returns all groups owned by a user.
    pub fn get_groups(&self, user_id: Uuid) -> Result<Vec<Group>, ServiceError> {
        Ok(self
            .group_dao
            .get_user_groups(user_id)?
            .into_iter()
            .map(|g| g.into())
            .collect())
    }

    /// Deletes a group. Only the owner can delete.
    pub fn delete_group(&self, user_id: Uuid, group_id: Uuid) -> Result<(), ServiceError> {
        let group = self.group_dao.get_group(group_id)?;
        if group.owner_user_id != user_id {
            Err(ServiceError::Forbidden)?
        }

        self.group_dao.delete_group(group_id).map_err(Into::into)
    }

    /// Updates a group's metadata. Only the owner can update.
    pub fn update_group(
        &self,
        user_id: Uuid,
        group_id: Uuid,
        patch: interface::types::group::UpdateGroup,
    ) -> Result<Group, ServiceError> {
        let current = self.group_dao.get_group(group_id)?;
        if current.owner_user_id != user_id {
            Err(ServiceError::Forbidden)?
        }

        if let Some(new_loc) = patch.new_location {
            let loc_update: models::LocationUpdate = new_loc.try_into()?;
            self.location_dao.update_location(current.location_id, loc_update)?;
        }

        let update = GroupUpdate {
            description: patch.description,
            location_id: None,
            is_public: patch.is_public.unwrap_or(current.is_public),
        };

        self.group_dao
            .update_group(group_id, update)
            .map_err(Into::into)
            .map(Into::into)
    }
}

fn map_unique_group_name_error(
    err: crate::dao::DaoError,
    field_name: &str,
    attempted_name: Option<&str>,
) -> ServiceError {
    match err {
        crate::dao::DaoError::UniqueViolation(_) => {
            let message = match attempted_name {
                Some(name) => format!("{field_name} '{name}' already exists"),
                None => format!("{field_name} already exists"),
            };
            ServiceError::TypeError(crate::interface::types::TypeError::Validation(message))
        }
        other => ServiceError::DbError(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dao::DaoError;
    use crate::dao::group::PgGroupDao;
    use crate::dao::location::PgLocationDao;
    use crate::interface::types::group::NewGroup as InterfaceNewGroup;
    use crate::interface::types::location::NewLocation as InterfaceNewLocation;
    use crate::test_utils::{create_test_group, create_test_location, create_test_user, setup};

    mod integration_test {
        use super::*;

        #[test]
        fn test_create_group_success() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "create_suc");

            let new_group_input = InterfaceNewGroup {
                name: "New Group".to_string(),
                description: "Desc".to_string(),
                is_public: true,
                location: InterfaceNewLocation {
                    name: Some("Loc".to_string()),
                    geo_lat: Some(1.0),
                    geo_long: Some(1.0),
                },
            };

            let result = service
                .create_group(user.id, new_group_input)
                .expect("Failed to create group");

            assert_eq!(result.name, "New Group");
            assert_eq!(result.owner_user_id, user.id);
        }

        #[test]
        fn test_create_group_invalid_name() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "create_inv");
            let invalid_repeated = "a".repeat(33).to_string();
            let invalid_names = vec!["", &invalid_repeated, "has/slash", "has+plus", "has#hash"];

            for name in invalid_names {
                let new_group_input = InterfaceNewGroup {
                    name: name.to_string(),
                    description: "Desc".to_string(),
                    is_public: true,
                    location: InterfaceNewLocation {
                        name: Some("Loc".to_string()),
                        geo_lat: Some(1.0),
                        geo_long: Some(1.0),
                    },
                };

                let result = service.create_group(user.id, new_group_input);
                assert!(matches!(result, Err(ServiceError::TypeError(_))));
            }
        }

        #[test]
        fn test_get_group_owner() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "get_owner");
            let location = create_test_location(&ctx.pool, "get_owner");
            let group = create_test_group(&ctx.pool, user.id, location.id, "get_owner");

            let result = service
                .get_group(user.id, group.id)
                .expect("Failed to get group");

            assert_eq!(result.id, group.id);
        }

        #[test]
        fn test_get_group_public_other_user() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let owner = create_test_user(&ctx.pool, "owner");
            let viewer = create_test_user(&ctx.pool, "viewer");
            let location = create_test_location(&ctx.pool, "pub");

            // Create public group manually to ensure it is public
            use crate::model::models::NewGroup;
            let group = service
                .group_dao
                .create_group(NewGroup {
                    owner_user_id: owner.id,
                    location_id: location.id,
                    name: "Public Group".to_string(),
                    description: "Desc".to_string(),
                    is_public: true,
                })
                .unwrap();

            let result = service
                .get_group(viewer.id, group.id)
                .expect("Failed to get public group");

            assert_eq!(result.id, group.id);
        }

        #[test]
        fn test_get_group_private_forbidden() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let owner = create_test_user(&ctx.pool, "owner_priv");
            let viewer = create_test_user(&ctx.pool, "viewer_priv");
            let location = create_test_location(&ctx.pool, "priv");

            // Create private group
            use crate::model::models::NewGroup;
            let group = service
                .group_dao
                .create_group(NewGroup {
                    owner_user_id: owner.id,
                    location_id: location.id,
                    name: "Private Group".to_string(),
                    description: "Desc".to_string(),
                    is_public: false,
                })
                .unwrap();

            let result = service.get_group(viewer.id, group.id);

            assert!(matches!(result, Err(ServiceError::Forbidden)));
        }

        #[test]
        fn test_get_group_not_found() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "not_found");

            let result = service.get_group(user.id, Uuid::new_v4());

            // Should be mapped to DaoError::NotFound -> ServiceError::DbError
            assert!(matches!(
                result,
                Err(ServiceError::DbError(DaoError::NotFound))
            ));
        }

        #[test]
        fn test_get_groups() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "get_groups");
            let location = create_test_location(&ctx.pool, "get_groups");

            create_test_group(&ctx.pool, user.id, location.id, "1");
            create_test_group(&ctx.pool, user.id, location.id, "2");

            let result = service.get_groups(user.id).expect("Failed to get groups");

            assert_eq!(result.len(), 2);
        }

        #[test]
        fn test_delete_group_success() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "del_suc");
            let location = create_test_location(&ctx.pool, "del_suc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "del_suc");

            service
                .delete_group(user.id, group.id)
                .expect("Failed to delete group");

            let found = service.group_dao.get_group(group.id);
            assert!(matches!(found, Err(DaoError::NotFound)));
        }

        #[test]
        fn test_delete_group_forbidden() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let owner = create_test_user(&ctx.pool, "owner_del");
            let other = create_test_user(&ctx.pool, "other_del");
            let location = create_test_location(&ctx.pool, "del_forb");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "del_forb");

            let result = service.delete_group(other.id, group.id);

            assert!(matches!(result, Err(ServiceError::Forbidden)));
        }

        #[test]
        fn test_delete_group_not_found() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "del_nf");

            let result = service.delete_group(user.id, Uuid::new_v4());

            assert!(matches!(
                result,
                Err(ServiceError::DbError(DaoError::NotFound))
            ));
        }

        #[test]
        fn test_update_group_success() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let user = create_test_user(&ctx.pool, "upd_group_svc");
            let location = create_test_location(&ctx.pool, "upd_group_svc");
            let group = create_test_group(&ctx.pool, user.id, location.id, "upd_group_svc");

            let updated = service
                .update_group(
                    user.id,
                    group.id,
                    interface::types::group::UpdateGroup {
                        description: Some("Updated desc".to_string()),
                        is_public: Some(true),
                        new_location: Some(InterfaceNewLocation {
                            name: Some("Updated Loc".to_string()),
                            geo_lat: Some(1.0),
                            geo_long: Some(2.0),
                        }),
                    },
                )
                .expect("Failed to update group");

            assert_eq!(updated.name, group.name);
            assert_eq!(updated.description, "Updated desc");
            assert!(updated.is_public);
            
            let updated_loc = service.location_dao.get_location(updated.location_id).unwrap();
            assert_eq!(updated_loc.name, Some("Updated Loc".to_string()));
            assert_eq!(updated_loc.geo_lat, Some(1.0));
            assert_eq!(updated_loc.geo_long, Some(2.0));
        }

        #[test]
        fn test_update_group_forbidden() {
            let ctx = setup();
            let group_dao = PgGroupDao::new(ctx.pool.clone());
            let location_dao = PgLocationDao::new(ctx.pool.clone());
            let service = GroupService::new(group_dao, location_dao);
            let owner = create_test_user(&ctx.pool, "upd_group_owner");
            let other = create_test_user(&ctx.pool, "upd_group_other");
            let location = create_test_location(&ctx.pool, "upd_group_forb");
            let group = create_test_group(&ctx.pool, owner.id, location.id, "upd_group_forb");

            let result = service.update_group(
                other.id,
                group.id,
                interface::types::group::UpdateGroup {
                    description: None,
                    is_public: None,
                    new_location: None,
                },
            );

            assert!(matches!(result, Err(ServiceError::Forbidden)));
        }
    }
}
