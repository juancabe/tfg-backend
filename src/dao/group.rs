use crate::dao::{DaoError, DbPool};
use crate::model::models::{
    Group, GroupJoinRequest, GroupUpdate, NewFavoriteGroup, NewGroup, NewGroupJoinRequest,
    NewGroupMember,
};
use chrono::Utc;
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

// Helper Enum for Permissions
#[derive(Debug, PartialEq)]
pub enum GroupRole {
    Owner,
    Member,
    None,
}

pub trait GroupDao {
    fn create_group(&self, new_group: NewGroup) -> Result<Group, DaoError>;
    fn update_group(&self, group_id: Uuid, update: GroupUpdate) -> Result<Group, DaoError>;
    fn get_user_groups(&self, user_id: Uuid) -> Result<Vec<Group>, DaoError>;
    // Returns producer's groups, including main group
    fn get_producer_groups(
        &self,
        producer_id: Uuid,
        producer_main_group_id: Uuid,
    ) -> Result<Vec<Group>, DaoError>;

    fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError>;
    fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), DaoError>;

    fn check_permission(&self, user_id: Uuid, group_id: Uuid) -> Result<GroupRole, DaoError>;

    fn create_join_request(&self, req: NewGroupJoinRequest) -> Result<GroupJoinRequest, DaoError>;
    // Decision is true (accept) or false (decline)
    fn resolve_join_request(&self, req_id: Uuid, decision: bool) -> Result<(), DaoError>;

    fn toggle_favorite_group(&self, user_id: Uuid, group_id: Uuid) -> Result<bool, DaoError>;
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
        use crate::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(groups)
            .values(&new_group)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn update_group(&self, group_id_in: Uuid, update: GroupUpdate) -> Result<Group, DaoError> {
        use crate::schema::groups::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(groups.find(group_id_in))
            .set(&update)
            .returning(Group::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn get_user_groups(&self, user_id_in: Uuid) -> Result<Vec<Group>, DaoError> {
        use crate::schema::{group_members, groups};
        let conn = &mut self.pool.get()?;

        // Get groups owned by user
        let mut owned_groups: Vec<Group> = groups::table
            .filter(groups::owner_user_id.eq(user_id_in))
            .select(Group::as_select())
            .load(conn)?;

        // Get groups where user is a member
        let member_groups: Vec<Group> = group_members::table
            .inner_join(groups::table)
            .filter(group_members::user_id.eq(user_id_in))
            .select(Group::as_select())
            .load(conn)?;

        // Combine and deduplicate
        // NOTE: Logic depends on whether owners are implicitly members.
        // Assuming we want a unique list.
        owned_groups.extend(member_groups);
        owned_groups.sort_by_key(|g| g.id);
        owned_groups.dedup_by_key(|g| g.id);

        Ok(owned_groups)
    }

    fn get_producer_groups(
        &self,
        producer_id_in: Uuid,
        producer_main_group_id: Uuid,
    ) -> Result<Vec<Group>, DaoError> {
        use crate::schema::{groups, producers_for_groups};
        let conn = &mut self.pool.get()?;

        let mut extra = producers_for_groups::table
            .filter(producers_for_groups::producer_id.eq(producer_id_in))
            .inner_join(groups::table)
            .select(Group::as_select())
            .load(conn)
            .map_err(DaoError::from)?;

        let main = groups::table
            .find(producer_main_group_id)
            .select(Group::as_select())
            .first(conn)
            .optional()?.ok_or(DaoError::OtherRare(format!("Producer: {producer_id_in} doesn't have a valid main group: {producer_main_group_id}")))?;

        extra.push(main);
        Ok(extra)
    }

    fn add_member(&self, group_id_in: Uuid, user_id_in: Uuid) -> Result<(), DaoError> {
        use crate::schema::group_members::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(group_members)
            .values(&NewGroupMember {
                group_id: group_id_in,
                user_id: user_id_in,
            })
            .execute(conn)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn remove_member(&self, group_id_in: Uuid, user_id_in: Uuid) -> Result<(), DaoError> {
        use crate::schema::group_members::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::delete(
            group_members
                .filter(group_id.eq(group_id_in))
                .filter(user_id.eq(user_id_in)),
        )
        .execute(conn)
        .map(|_| ())
        .map_err(Into::into)
    }

    fn check_permission(&self, user_id_in: Uuid, group_id_in: Uuid) -> Result<GroupRole, DaoError> {
        use crate::schema::{group_members, groups};
        let conn = &mut self.pool.get()?;

        // Check ownership
        let is_owner = groups::table
            .find(group_id_in)
            .select(groups::owner_user_id)
            .first::<Uuid>(conn)
            .optional()?;

        if let Some(owner) = is_owner {
            if owner == user_id_in {
                return Ok(GroupRole::Owner);
            }
        } else {
            // Group doesn't exist
            return Err(DaoError::NotFound);
        }

        // Check membership
        let count: i64 = group_members::table
            .filter(group_members::group_id.eq(group_id_in))
            .filter(group_members::user_id.eq(user_id_in))
            .count()
            .get_result(conn)?;

        if count > 0 {
            Ok(GroupRole::Member)
        } else {
            Ok(GroupRole::None)
        }
    }

    fn create_join_request(&self, req: NewGroupJoinRequest) -> Result<GroupJoinRequest, DaoError> {
        use crate::schema::group_join_requests::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(group_join_requests)
            .values(&req)
            .returning(GroupJoinRequest::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn resolve_join_request(&self, req_id: Uuid, decision: bool) -> Result<(), DaoError> {
        use crate::schema::group_join_requests::dsl::*;
        let conn = &mut self.pool.get()?;

        conn.transaction(|conn| {
            let request: GroupJoinRequest = group_join_requests
                .find(req_id)
                .select(GroupJoinRequest::as_select())
                .first(conn)?;

            let now = Utc::now();

            if decision {
                // Accept
                diesel::update(group_join_requests.find(req_id))
                    .set(accepted_at.eq(now))
                    .execute(conn)?;

                // Add to members
                use crate::schema::group_members::dsl as gm;
                diesel::insert_into(gm::group_members)
                    .values(&NewGroupMember {
                        group_id: request.target_group_id,
                        user_id: request.solicitor_user_id,
                    })
                    .execute(conn)?;
            } else {
                // Decline
                diesel::update(group_join_requests.find(req_id))
                    .set(declined_at.eq(now))
                    .execute(conn)?;
            }

            Ok(())
        })
    }

    fn toggle_favorite_group(&self, user_id_in: Uuid, group_id_in: Uuid) -> Result<bool, DaoError> {
        use crate::schema::favorite_groups::dsl::*;
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
