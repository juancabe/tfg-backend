//! DAO for user related identity persistence work

use crate::dao::{DaoError, DbPool};
use crate::model::models::{EmailAuthMethod, NewEmailAuthMethod, NewRefreshToken, NewUser, User};
use crate::model::schema;
use diesel::prelude::*;
use diesel::{ExpressionMethods, RunQueryDsl, SelectableHelper};
use uuid::Uuid;

pub trait IdentityDao {
    /// inserts a [`NewUser`] in db `users` table
    fn create_user(&self, new_user: NewUser) -> Result<User, DaoError>;

    /// finds [`User`] by `login`
    fn find_by_login(&self, login: &str) -> Result<Option<User>, DaoError>;

    /// finds [`User`] by `id`
    fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DaoError>;

    /// finds [`User`] and stored hash by email
    fn find_by_email(&self, email_in: &str) -> Result<Option<(User, String)>, DaoError>;

    /// inserts a [`NewEmailAuthMethod`] in db `email_auth_methods` table
    fn add_email_auth_method(&self, auth: NewEmailAuthMethod) -> Result<EmailAuthMethod, DaoError>;

    /// sets all other `user_id` refresh tokens to `claimed` and inserts new one ([`NewRefreshToken`]).
    /// # Returns
    /// The new token `version_n`
    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<i64, DaoError>;

    /// verifies whether certain `user_id` refresh token exists and is currently on `version_n` version_n
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
        use crate::model::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(users)
            .values(&new_user)
            .returning(User::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn find_by_login(&self, login_query: &str) -> Result<Option<User>, DaoError> {
        use crate::model::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        users
            .filter(login.eq(login_query))
            .select(User::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn find_by_id(&self, user_id_in: Uuid) -> Result<Option<User>, DaoError> {
        use crate::model::schema::users::dsl::*;
        let conn = &mut self.pool.get()?;
        users
            .find(user_id_in)
            .select(User::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn add_email_auth_method(&self, auth: NewEmailAuthMethod) -> Result<EmailAuthMethod, DaoError> {
        use crate::model::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(email_auth_methods)
            .values(&auth)
            .returning(EmailAuthMethod::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn rotate_refresh_token(&self, token: NewRefreshToken) -> Result<i64, DaoError> {
        use crate::model::schema::refresh_tokens::dsl::*;
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
        use crate::model::schema::refresh_tokens::dsl::*;
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

    fn find_by_email(&self, email_in: &str) -> Result<Option<(User, String)>, DaoError> {
        use crate::model::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        let res = email_auth_methods
            .inner_join(schema::users::table)
            .filter(email.eq(email_in))
            .select((User::as_select(), hashed_pass))
            .first::<(User, String)>(conn)
            .optional()?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::models::{NewEmailAuthMethod, NewRefreshToken, NewUser};
    use crate::test_utils::{create_test_user, setup};
    use chrono::{Duration, Utc};

    mod unit_test {
        use super::*;

        #[test]
        fn test_create_user() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let new_user = NewUser { login: "test_user" };

            let result = dao.create_user(new_user).expect("Failed to create user");

            assert_eq!(result.login, "test_user");
        }

        #[test]
        fn test_create_user_duplicate() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let new_user = NewUser { login: "test_user" };

            dao.create_user(new_user.clone())
                .expect("Failed to create user");
            let result = dao.create_user(new_user);

            assert!(matches!(result, Err(DaoError::UniqueViolation(_))));
        }

        #[test]
        fn test_find_by_login() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "find_by_login");

            let found = dao.find_by_login(&user.login).expect("Failed to find user");

            assert!(found.is_some());
            assert_eq!(found.unwrap().id, user.id);
        }

        #[test]
        fn test_find_by_login_not_found() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());

            let found = dao
                .find_by_login("non_existent")
                .expect("Failed to find user");

            assert!(found.is_none());
        }

        #[test]
        fn test_find_by_id() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "find_by_id");

            let found = dao.find_by_id(user.id).expect("Failed to find user");

            assert!(found.is_some());
            assert_eq!(found.unwrap().login, user.login);
        }

        #[test]
        fn test_find_by_id_not_found() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());

            let found = dao.find_by_id(Uuid::new_v4()).expect("Failed to find user");

            assert!(found.is_none());
        }

        #[test]
        fn test_add_email_auth_method() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "email_auth");
            let auth = NewEmailAuthMethod {
                user_id: user.id,
                email: "test@example.com",
                hashed_pass: "hashed_password",
            };

            let result = dao
                .add_email_auth_method(auth)
                .expect("Failed to add auth method");

            assert_eq!(result.user_id, user.id);
            assert_eq!(result.email, "test@example.com");
        }

        #[test]
        fn test_add_email_auth_method_duplicate_email() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user1 = create_test_user(&ctx.pool, "user1");
            let user2 = create_test_user(&ctx.pool, "user2");
            let email = "test@example.com";

            dao.add_email_auth_method(NewEmailAuthMethod {
                user_id: user1.id,
                email,
                hashed_pass: "pass1",
            })
            .expect("Failed to add first auth method");

            let result = dao.add_email_auth_method(NewEmailAuthMethod {
                user_id: user2.id,
                email,
                hashed_pass: "pass2",
            });

            assert!(matches!(result, Err(DaoError::UniqueViolation(_))));
        }

        #[test]
        fn test_find_by_email() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "find_email_pass");
            let email = "test@example.com";
            let pass = "hashed_password";
            dao.add_email_auth_method(NewEmailAuthMethod {
                user_id: user.id,
                email,
                hashed_pass: pass,
            })
            .expect("Failed to add auth method");

            let found = dao
                .find_by_email(email)
                .expect("Failed to find user");

            assert!(found.is_some());
            let (found_user, found_hash) = found.unwrap();
            assert_eq!(found_user.id, user.id);
            assert_eq!(found_hash, pass);
        }

        #[test]
        fn test_find_by_email_not_found() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());

            let found = dao
                .find_by_email("non_existent@example.com")
                .expect("Failed to find user");

            assert!(found.is_none());
        }

        #[test]
        fn test_rotate_refresh_token() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "rotate_token");
            let expires_at = Utc::now() + Duration::days(1);
            let token = NewRefreshToken {
                user_id: user.id,
                expires_at,
            };

            let version = dao
                .rotate_refresh_token(token)
                .expect("Failed to rotate token");

            assert!(version > 0);
        }

        #[test]
        fn test_rotate_refresh_token_user_not_found() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let expires_at = Utc::now() + Duration::days(1);
            let token = NewRefreshToken {
                user_id: Uuid::new_v4(),
                expires_at,
            };

            let result = dao.rotate_refresh_token(token);

            assert!(matches!(result, Err(DaoError::ForeignKeyViolation(_))));
        }

        #[test]
        fn test_verify_token() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "verify_token");
            let expires_at = Utc::now() + Duration::days(1);
            let token = NewRefreshToken {
                user_id: user.id,
                expires_at,
            };

            let version = dao
                .rotate_refresh_token(token)
                .expect("Failed to rotate token");
            let is_valid = dao
                .verify_token(&user.id, version)
                .expect("Failed to verify token");

            assert!(is_valid);
        }

        #[test]
        fn test_verify_token_expired() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "expired_token");
            let expires_at = Utc::now() - Duration::days(1);
            let token = NewRefreshToken {
                user_id: user.id,
                expires_at,
            };

            let version = dao
                .rotate_refresh_token(token)
                .expect("Failed to rotate token");
            let is_valid = dao
                .verify_token(&user.id, version)
                .expect("Failed to verify token");

            assert!(!is_valid);
        }
    }

    mod integration_test {
        use super::*;

        #[test]
        fn test_auth_flow() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());

            // 1. Create user
            let user = dao.create_user(NewUser { login: "flow_user" }).unwrap();

            // 2. Add auth method
            dao.add_email_auth_method(NewEmailAuthMethod {
                user_id: user.id,
                email: "flow@example.com",
                hashed_pass: "secret",
            })
            .unwrap();

            // 3. Login (find by email/pass)
            let found = dao
                .find_by_email("flow@example.com")
                .unwrap()
                .expect("User should be found");

            assert_eq!(found.0.id, user.id);
            assert_eq!(found.1, "secret");
        }

        #[test]
        fn test_token_rotation_flow() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "token_flow");

            // 1. First token
            let v1 = dao
                .rotate_refresh_token(NewRefreshToken {
                    user_id: user.id,
                    expires_at: Utc::now() + Duration::days(1),
                })
                .unwrap();

            assert!(dao.verify_token(&user.id, v1).unwrap());

            // 2. Rotate to second token
            let v2 = dao
                .rotate_refresh_token(NewRefreshToken {
                    user_id: user.id,
                    expires_at: Utc::now() + Duration::days(1),
                })
                .unwrap();

            // 3. v1 should be invalid (claimed), v2 should be valid
            assert!(!dao.verify_token(&user.id, v1).unwrap());
            assert!(dao.verify_token(&user.id, v2).unwrap());
        }
    }
}
