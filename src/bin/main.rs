use diesel::{PgConnection, r2d2::ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use dotenvy::dotenv;
use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::select;
use tokio::sync::oneshot;

use backend::dao::{DbPool, MAX_DB_POOL_SIZE};
use backend::interface::http::{self, DEFAULT_PORT};
use backend::interface::mqtt::{MqttBroker, MqttBrokerConfig};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

fn run_migrations(pool: &DbPool) {
    let mut conn = pool.get().expect("Failed to get DB conn");
    let migrations = conn
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");
    log::info!("Ran DB migrations: {migrations:?}");
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    env_logger::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool: DbPool = r2d2::Pool::builder()
        .max_size(MAX_DB_POOL_SIZE)
        .build(manager)
        .expect("Failed to create DbPool");

    // Log DB pool configuration at startup for diagnostics
    log::info!("DB pool created: max_size={}", MAX_DB_POOL_SIZE);
    run_migrations(&pool);

    let http_listen_address =
        std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT));

    let mqtt_config = MqttBrokerConfig::from_env();
    let mqtt_pool = pool.clone();
    let (mqtt_tx, mqtt_rx) = oneshot::channel();
    std::thread::spawn(move || {
        let result = ntex::rt::System::new("mqtt-broker", ntex::rt::DefaultRuntime)
            .block_on(async move { MqttBroker::run(mqtt_config, mqtt_pool).await });
        let _ = mqtt_tx.send(result);
    });

    let http_fut = http::Http::run_http_server(http_listen_address, pool);

    select! {
        val = mqtt_rx => {
            match val {
                Ok(Ok(())) => log::info!("MQTT broker stopped"),
                Ok(Err(err)) => log::error!("MQTT broker error: {err}"),
                Err(err) => log::error!("MQTT broker thread closed unexpectedly: {err}"),
            }
        },
        val = http_fut => {
            if let Err(err) = val {
                log::error!("HTTP server error: {err}");
            }
        },
    };
}
