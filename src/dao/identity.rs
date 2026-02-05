use crate::dao::{DaoError, DbPool};
use crate::model::models::{AuthMethod, NewAuthMethod, NewRefreshToken, NewUser, User};
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub trait IdentityDao {
    fn create_user(&self, new_user: NewUser) -> Result<User, DaoError>;
    fn find_by_login(&self, login: &str) -> Result<Option<User>, DaoError>;
    fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DaoError>;
    fn add_auth_method(&self, auth: NewAuthMethod) -> Result<AuthMethod, DaoError>;
    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<i64, DaoError>;
    fn verify_token(&self, user_id: &Uuid, version_n: i64) -> Result<bool, DaoError>;
}

pub struct PgIdentityDao {
    pool: DbPool,
}

impl PgIdentityDao {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl IdentityDao for PgIdentityDao {
    fn create_user(&self, new_user: NewUser) -> Result<User, DaoError> {
        use crate::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(users)
            .values(&new_user)
            .returning(User::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn find_by_login(&self, login_query: &str) -> Result<Option<User>, DaoError> {
        use crate::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        users
            .filter(login.eq(login_query))
            .select(User::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn find_by_id(&self, user_id_in: Uuid) -> Result<Option<User>, DaoError> {
        use crate::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        users
            .find(user_id_in)
            .select(User::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn add_auth_method(&self, auth: NewAuthMethod) -> Result<AuthMethod, DaoError> {
        use crate::schema::auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(auth_methods)
            .values(&auth)
            .returning(AuthMethod::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<i64, DaoError> {
        use crate::schema::refresh_tokens::dsl::*;
        let conn = &mut self.pool.get()?;

        diesel::update(refresh_tokens)
            .filter(user_id.eq(token.user_id))
            .set(claimed.eq(true))
            .execute(conn)?;

        let created_token: i64 = diesel::insert_into(refresh_tokens)
            .values(&token)
            .returning(version_n)
            .get_result(conn)?;

        Ok(created_token)
    }

    fn verify_token(&self, user_id_in: &Uuid, version_n_in: i64) -> Result<bool, DaoError> {
        use crate::schema::refresh_tokens::dsl::*;
        let conn = &mut self.pool.get()?;

        let count: i64 = refresh_tokens
            .filter(user_id.eq(user_id_in))
            .filter(version_n.eq(version_n_in))
            .filter(claimed.eq(false))
            .filter(expires_at.gt(chrono::Utc::now()))
            .count()
            .get_result(conn)?;

        Ok(count > 0)
    }
}
