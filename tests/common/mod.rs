use backend::dao::DbPool;
use backend::model::models::{
    Group, Location, NewGroup, NewLocation, NewProducer, NewUser, Producer, User,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use dotenvy::dotenv;
use std::env;
use std::sync::{Mutex, MutexGuard, OnceLock};
use uuid::Uuid;

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
pub fn create_test_user(pool: &DbPool, suffix: &str) -> User {
    use backend::schema::users::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(users)
        .values(backend::model::models::NewUser {
            login: &format!("user_{}", suffix),
        })
        .returning(User::as_returning())
        .get_result(&mut conn)
        .unwrap()
}

pub fn create_test_location(pool: &DbPool, suffix: &str) -> Location {
    use backend::schema::locations::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(locations)
        .values(NewLocation {
            name: Some(format!("loc_{}", suffix)),
            geo_lat: Some(0.0),
            geo_long: Some(0.0),
        })
        .returning(Location::as_returning())
        .get_result(&mut conn)
        .unwrap()
}

pub fn create_test_group(pool: &DbPool, owner_id: Uuid, loc_id: Uuid, suffix: &str) -> Group {
    use backend::schema::groups::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(groups)
        .values(NewGroup {
            owner_user_id: owner_id,
            location_id: loc_id,
            is_personal_group: false,
            name: format!("group_{}", suffix),
            description: "desc".to_string(),
            modification: 1,
            visibility: 1,
        })
        .returning(Group::as_returning())
        .get_result(&mut conn)
        .unwrap()
}

pub fn create_test_producer(pool: &DbPool, grp_id: Uuid, loc_id: Uuid, suffix: &str) -> Producer {
    use backend::schema::producers::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(producers)
        .values(NewProducer {
            location_id: loc_id,
            main_group_id: grp_id,
            name: format!("prod_{}", suffix),
            description: "desc".to_string(),
            forwarder_id: None,
        })
        .returning(Producer::as_returning())
        .get_result(&mut conn)
        .unwrap()
}
