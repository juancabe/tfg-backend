//! DAO for user related identity persistence work

use crate::dao::{DaoError, DbPool};
use crate::model::models::{
    EmailAuthMethod, EmailVerification, NewEmailAuthMethod, NewEmailVerification, NewPasswordReset,
    NewRefreshToken, NewUser, PasswordReset, User,
};
use crate::model::schema;
use chrono::Utc;
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

    /// finds [`EmailAuthMethod`] by `user_id`
    fn find_email_auth_by_user_id(
        &self,
        user_id: &Uuid,
    ) -> Result<Option<EmailAuthMethod>, DaoError>;

    /// updates stored hash for [`EmailAuthMethod`] by `user_id`
    fn update_email_password(
        &self,
        user_id: &Uuid,
        hashed_pass: &str,
    ) -> Result<EmailAuthMethod, DaoError>;

    /// inserts a new [`EmailVerification`] token
    fn create_email_verification(
        &self,
        verification: NewEmailVerification,
    ) -> Result<EmailVerification, DaoError>;

    /// finds [`EmailVerification`] by token id
    fn find_email_verification(&self, token: &Uuid) -> Result<Option<EmailVerification>, DaoError>;

    /// inserts a new [`PasswordReset`] row entity
    fn create_password_reset(
        &self,
        new_password_reset: NewPasswordReset,
    ) -> Result<PasswordReset, DaoError>;

    /// finds [`PasswordReset`] by code
    fn find_password_reset(
        &self,
        code: &str,
        user_id: &Uuid,
    ) -> Result<Option<PasswordReset>, DaoError>;

    /// consumes [`PasswordReset`] by id
    fn consume_password_reset(&self, id: &Uuid) -> Result<Option<PasswordReset>, DaoError>;

    /// marks [`EmailVerification`] token as consumed
    fn consume_email_verification(&self, token: &Uuid) -> Result<EmailVerification, DaoError>;

    /// `attempt++` on every `in_user: &Uuid` corresponding password reset rows
    /// # Returns
    /// Number of affected rows
    fn add_attempt_user_verifications(&self, in_user: &Uuid) -> Result<usize, DaoError>;

    /// marks email auth as verified
    fn mark_email_verified(&self, user_id: &Uuid) -> Result<EmailAuthMethod, DaoError>;

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

    fn find_email_auth_by_user_id(
        &self,
        user_id_in: &Uuid,
    ) -> Result<Option<EmailAuthMethod>, DaoError> {
        use crate::model::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        email_auth_methods
            .filter(user_id.eq(user_id_in))
            .select(EmailAuthMethod::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn update_email_password(
        &self,
        user_id_in: &Uuid,
        hashed_pass_in: &str,
    ) -> Result<EmailAuthMethod, DaoError> {
        use crate::model::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(email_auth_methods.filter(user_id.eq(user_id_in)))
            .set((
                hashed_pass.eq(hashed_pass_in),
                changed_pass_at.eq(Utc::now()),
            ))
            .returning(EmailAuthMethod::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn create_email_verification(
        &self,
        verification: NewEmailVerification,
    ) -> Result<EmailVerification, DaoError> {
        use crate::model::schema::email_verifications::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(email_verifications)
            .values(&verification)
            .returning(EmailVerification::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn find_email_verification(&self, token: &Uuid) -> Result<Option<EmailVerification>, DaoError> {
        use crate::model::schema::email_verifications::dsl::*;
        let conn = &mut self.pool.get()?;
        email_verifications
            .filter(id.eq(token))
            .select(EmailVerification::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn consume_email_verification(&self, token: &Uuid) -> Result<EmailVerification, DaoError> {
        use crate::model::schema::email_verifications::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(email_verifications.filter(id.eq(token)))
            .set(consumed_at.eq(Utc::now()))
            .returning(EmailVerification::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn mark_email_verified(&self, user_id_in: &Uuid) -> Result<EmailAuthMethod, DaoError> {
        use crate::model::schema::email_auth_methods::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(email_auth_methods.filter(user_id.eq(user_id_in)))
            .set(is_verified.eq(true))
            .returning(EmailAuthMethod::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn create_password_reset(
        &self,
        password_reset: NewPasswordReset,
    ) -> Result<PasswordReset, DaoError> {
        use crate::model::schema::password_resets::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::insert_into(password_resets)
            .values(&password_reset)
            .returning(PasswordReset::as_returning())
            .get_result(conn)
            .map_err(Into::into)
    }

    fn find_password_reset(
        &self,
        in_code: &str,
        in_user: &Uuid,
    ) -> Result<Option<PasswordReset>, DaoError> {
        use crate::model::schema::password_resets::dsl::*;
        let conn = &mut self.pool.get()?;
        password_resets
            .filter(code.eq(in_code))
            .filter(user_id.eq(in_user))
            .select(PasswordReset::as_select())
            .first(conn)
            .optional()
            .map_err(Into::into)
    }

    fn consume_password_reset(&self, in_id: &Uuid) -> Result<Option<PasswordReset>, DaoError> {
        use crate::model::schema::password_resets::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(password_resets.filter(id.eq(in_id)))
            .set(consumed_at.eq(Some(chrono::Utc::now())))
            .returning(PasswordReset::as_returning())
            .get_result(conn)
            .optional()
            .map_err(Into::into)
    }

    fn add_attempt_user_verifications(&self, in_user: &Uuid) -> Result<usize, DaoError> {
        use crate::model::schema::password_resets::dsl::*;
        let conn = &mut self.pool.get()?;
        diesel::update(password_resets.filter(user_id.eq(in_user)))
            .set(attempts.eq(attempts + 1))
            .execute(conn)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::models::{NewEmailAuthMethod, NewPasswordReset, NewRefreshToken, NewUser};
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

            let found = dao.find_by_email(email).expect("Failed to find user");

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
        fn test_update_email_password() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "update_email_pass");
            let email = "test@example.com";
            dao.add_email_auth_method(NewEmailAuthMethod {
                user_id: user.id,
                email,
                hashed_pass: "old_pass",
            })
            .expect("Failed to add auth method");

            let updated = dao
                .update_email_password(&user.id, "new_pass")
                .expect("Failed to update password");

            assert_eq!(updated.user_id, user.id);
            assert_eq!(updated.hashed_pass, "new_pass");
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

        #[test]
        fn test_create_and_consume_email_verification() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "email_verify");
            let email = "verify@example.com";
            let expires_at = Utc::now() + Duration::hours(24);

            let verification = dao
                .create_email_verification(NewEmailVerification {
                    user_id: user.id,
                    email,
                    expires_at,
                })
                .expect("Failed to create verification");

            assert_eq!(verification.user_id, user.id);
            assert_eq!(verification.email, email);
            assert!(verification.consumed_at.is_none());

            let consumed = dao
                .consume_email_verification(&verification.id)
                .expect("Failed to consume verification");

            assert!(consumed.consumed_at.is_some());
        }

        #[test]
        fn test_create_and_find_password_reset() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "password_reset");
            let email = "reset@example.com";
            let code = "123456";

            let created = dao
                .create_password_reset(NewPasswordReset {
                    user_id: user.id,
                    email,
                    code,
                    expires_at: Utc::now() + Duration::hours(2),
                })
                .expect("Failed to create password reset");

            assert_eq!(created.user_id, user.id);
            assert_eq!(created.email, email);
            assert_eq!(created.code, code);
            assert_eq!(created.attempts, 0);
            assert!(created.consumed_at.is_none());

            let found = dao
                .find_password_reset(code, &user.id)
                .expect("Failed to find password reset");
            assert!(found.is_some());
            let found = found.unwrap();
            assert_eq!(found.id, created.id);
        }

        #[test]
        fn test_consume_password_reset() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "consume_reset");
            let created = dao
                .create_password_reset(NewPasswordReset {
                    user_id: user.id,
                    email: "consume@example.com",
                    code: "654321",
                    expires_at: Utc::now() + Duration::hours(2),
                })
                .expect("Failed to create password reset");

            let consumed = dao
                .consume_password_reset(&created.id)
                .expect("Failed to consume password reset");

            assert!(consumed.is_some());
            assert!(consumed.unwrap().consumed_at.is_some());
        }

        #[test]
        fn test_add_attempt_user_verifications() {
            let ctx = setup();
            let dao = PgIdentityDao::new(ctx.pool.clone());
            let user = create_test_user(&ctx.pool, "attempt_reset");

            dao.create_password_reset(NewPasswordReset {
                user_id: user.id,
                email: "attempt@example.com",
                code: "000001",
                expires_at: Utc::now() + Duration::hours(2),
            })
            .expect("Failed to create password reset");

            let updated = dao
                .add_attempt_user_verifications(&user.id)
                .expect("Failed to update attempts");
            assert_eq!(updated, 1);

            let found = dao
                .find_password_reset("000001", &user.id)
                .expect("Failed to find password reset")
                .unwrap();
            assert_eq!(found.attempts, 1);
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
