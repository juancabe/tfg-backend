use crate::dao::DbPool;
use crate::model::models::User;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use dotenvy::dotenv;
use std::env;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

static DB_LOCK: Mutex<()> = Mutex::new(());
static POOL: OnceLock<DbPool> = OnceLock::new();

pub struct TestContext {
    pub pool: DbPool,
    _lock: MutexGuard<'static, ()>,
}

pub fn get_pool() -> DbPool {
    POOL.get_or_init(|| {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("Failed to create pool")
    })
    .clone()
}

pub fn setup() -> TestContext {
    let _lock = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let pool = get_pool();
    let mut conn = pool.get().expect("Failed to get connection");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Clean DB
    diesel::sql_query("TRUNCATE TABLE users, locations, packet_templates RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .expect("Failed to truncate tables");

    TestContext { pool, _lock }
}

// Factories
#[allow(dead_code)]
pub fn create_test_user(pool: &DbPool, suffix: &str) -> User {
    use crate::model::schema::users::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(users)
        .values(crate::model::models::NewUser {
            login: &format!("user_{}", suffix),
        })
        .returning(User::as_returning())
        .get_result(&mut conn)
        .unwrap()
}
