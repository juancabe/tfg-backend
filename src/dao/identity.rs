use crate::dao::{DaoError, DbPool};
use crate::helpers::auth::EmailPass;
use crate::model::models::{EmailAuthMethod, NewEmailAuthMethod, NewRefreshToken, NewUser, User};
use crate::schema;
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub trait IdentityDao {
    /// inserts a `NewUser` in db `users` table
    fn create_user(&self, new_user: NewUser) -> Result<User, DaoError>;

    /// finds `User` by `login`
    fn find_by_login(&self, login: &str) -> Result<Option<User>, DaoError>;

    /// finds `User` by `id`
    fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DaoError>;

    /// finds `User` by `id`
    fn find_by_email_pass(&self, email_pass: EmailPass) -> Result<Option<User>, DaoError>;

    /// inserts a `NewEmailAuthMethod` in db `email_auth_methods` table
    fn add_email_auth_method(&self, auth: NewEmailAuthMethod) -> Result<EmailAuthMethod, DaoError>;

    /// sets all other `user_id` `refresh_tokens` to `claimed` and inserts new one
    /// # Returns
    /// The new token `version_n`
    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<i64, DaoError>;

    /// verifies wether certain `user_id` `refresh_token` exists and is currently on `version_n` version_n
    ///
    /// # Returns
    /// - true: parameters match
    /// - false: parameters do not match
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

    fn add_email_auth_method(&self, auth: NewEmailAuthMethod) -> Result<EmailAuthMethod, DaoError> {
        use crate::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(email_auth_methods)
            .values(&auth)
            .returning(EmailAuthMethod::as_returning())
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

        let created_token_version_n: i64 = diesel::insert_into(refresh_tokens)
            .values(&token)
            .returning(version_n)
            .get_result(conn)?;

        Ok(created_token_version_n)
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

    fn find_by_email_pass(&self, email_pass: EmailPass) -> Result<Option<User>, DaoError> {
        use crate::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        let user = email_auth_methods
            .inner_join(schema::users::table)
            .filter(email.eq(email_pass.email))
            .filter(hashed_pass.eq(email_pass.hashed_pass))
            .select(User::as_select())
            .first(conn)
            .optional()?;
        Ok(user)
    }
}
