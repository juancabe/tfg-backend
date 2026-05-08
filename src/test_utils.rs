use crate::dao::DbPool;
use crate::helpers::data_templates::{
    RANDOMFLOAT_TEMPLATE_FIELD_ID, RANDOMFLOAT_TEMPLATE_FIELD_JSON_KEY,
    RANDOMFLOAT_TEMPLATE_FIELD_NAME, RANDOMFLOAT_TEMPLATE_ID, RANDOMFLOAT_TEMPLATE_NAME,
};
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
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .try_init();

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Clean DB
    diesel::sql_query("TRUNCATE TABLE users, locations, packet_templates, groups, producers, internet_auths, mqtt_broker_auths, pending_producer_registrations RESTART IDENTITY CASCADE")
        .execute(&mut conn)
        .expect("Failed to truncate tables");

    seed_default_templates(&mut conn);

    TestContext { pool, _lock }
}

fn seed_default_templates(conn: &mut PgConnection) {
    let insert_template_sql = format!(
        "INSERT INTO packet_templates (id, creator_user_id, name) VALUES ('{}', NULL, '{}') ON CONFLICT (id) DO NOTHING",
        RANDOMFLOAT_TEMPLATE_ID, RANDOMFLOAT_TEMPLATE_NAME,
    );

    diesel::sql_query(insert_template_sql)
        .execute(conn)
        .expect("Failed to seed default packet template");

    let insert_field_sql = format!(
        "INSERT INTO template_fields (id, template_id, name, json_key, repr_enum_value, unit_enum_value, unit_enum_other_payload) VALUES ('{}', '{}', '{}', '{}', 1, 5, NULL) ON CONFLICT (id) DO NOTHING",
        RANDOMFLOAT_TEMPLATE_FIELD_ID,
        RANDOMFLOAT_TEMPLATE_ID,
        RANDOMFLOAT_TEMPLATE_FIELD_NAME,
        RANDOMFLOAT_TEMPLATE_FIELD_JSON_KEY,
    );

    diesel::sql_query(insert_field_sql)
        .execute(conn)
        .expect("Failed to seed default packet template field");
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

#[allow(dead_code)]
pub fn create_test_location(pool: &DbPool, suffix: &str) -> crate::model::models::Location {
    use crate::model::schema::locations::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(locations)
        .values(crate::model::models::NewLocation {
            name: Some(format!("location_{}", suffix)),
            geo_lat: Some(0.0),
            geo_long: Some(0.0),
        })
        .returning(crate::model::models::Location::as_returning())
        .get_result(&mut conn)
        .unwrap()
}

#[allow(dead_code)]
pub fn create_test_group(
    pool: &DbPool,
    owner_id: uuid::Uuid,
    location_id_arg: uuid::Uuid,
    suffix: &str,
) -> crate::model::models::Group {
    use crate::model::schema::groups::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(groups)
        .values(crate::model::models::NewGroup {
            owner_user_id: owner_id,
            location_id: location_id_arg,
            name: format!("group_{}", suffix),
            description: "desc".to_string(),
            is_public: false,
        })
        .returning(crate::model::models::Group::as_returning())
        .get_result(&mut conn)
        .unwrap()
}

#[allow(dead_code)]
pub fn create_test_producer(
    pool: &DbPool,
    group_id_in: uuid::Uuid,
    location_id_in: uuid::Uuid,
    suffix: &str,
) -> crate::model::models::Producer {
    use crate::model::schema::producers::dsl::*;
    let mut conn = pool.get().unwrap();
    diesel::insert_into(producers)
        .values(crate::model::models::NewProducer {
            location_id: location_id_in,
            group_id: group_id_in,
            name: format!("producer_{}", suffix),
            ble_name: format!("BLE_producer_{}", suffix),
            description: "desc".to_string(),
            forwarder_id: None,
            lora_addr: None,
            forward_lora: false,
        })
        .returning(crate::model::models::Producer::as_returning())
        .get_result(&mut conn)
        .unwrap()
}
