use crate::dao::{DaoError, DbPool};
use crate::model::models::{AuthMethod, NewAuthMethod, NewRefreshToken, NewUser, User};
use uuid::Uuid;

// Trait Definition
pub trait IdentityDao {
    fn create_user(&self, new_user: NewUser) -> Result<User, DaoError>;
    fn find_by_login(&self, login: &str) -> Result<Option<User>, DaoError>;
    fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DaoError>;
    fn add_auth_method(&self, auth: NewAuthMethod) -> Result<AuthMethod, DaoError>;
    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<String, DaoError>;
    fn verify_refresh_token(&self, token_hash: &str) -> Result<bool, DaoError>;
}

// Concrete Implementation
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
        // Implementation placeholder
        // use crate::schema::users::dsl::*;
        // let conn = &mut self.pool.get()?;
        // diesel::insert_into(users).values(&new_user).get_result(conn).map_err(Into::into)
        todo!()
    }

    fn find_by_login(&self, login_query: &str) -> Result<Option<User>, DaoError> {
        todo!()
    }

    fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>, DaoError> {
        todo!()
    }

    fn add_auth_method(&self, auth: NewAuthMethod) -> Result<AuthMethod, DaoError> {
        todo!()
    }

    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<String, DaoError> {
        todo!()
    }

    fn verify_refresh_token(&self, token_hash_query: &str) -> Result<bool, DaoError> {
        todo!()
    }
}
