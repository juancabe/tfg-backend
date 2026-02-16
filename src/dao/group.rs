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
    // TODO: unit test
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError> {
        use crate::model::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(groups)
            .values(&new_group)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    // TODO: unit test
    fn update_group(&self, group_id_in: Uuid, update: GroupUpdate) -> Result<Group, DaoError> {
        use crate::model::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(groups.find(group_id_in))
            .set(&update)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    // TODO: unit test
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

    // TODO: unit test
    fn get_user_groups(&self, user_id_in: Uuid) -> Result<Vec<Group>, DaoError> {
        use crate::model::schema::groups;
        let conn = &mut self.pool.get()?;

        groups::table
            .filter(groups::owner_user_id.eq(user_id_in))
            .select(Group::as_select())
            .load(conn)
            .map_err(Into::into)
    }

    // TODO: unit test
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
