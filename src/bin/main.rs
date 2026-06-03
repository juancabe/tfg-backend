//! Server entry point. Initializes configuration, database, MQTT broker, and HTTP server.

use diesel::{PgConnection, r2d2::ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use dotenvy::dotenv;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::select;
use tokio::sync::oneshot;

use backend::dao::{DbPool, MAX_DB_POOL_SIZE};
use backend::interface::http::{self};
use backend::interface::mqtt::{MqttBroker, MqttBrokerConfig};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Runs all pending database migrations.
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

    backend::config::CONFIG
        .set(backend::config::Config::init())
        .map_err(|_| "Config already initialized")
        .unwrap();

    let database_url = backend::config::get().get_database_url();
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool: DbPool = r2d2::Pool::builder()
        .max_size(MAX_DB_POOL_SIZE)
        .build(manager)
        .expect("Failed to create DbPool");

    // Log DB pool configuration at startup for diagnostics
    log::info!("DB pool created: max_size={}", MAX_DB_POOL_SIZE);
    run_migrations(&pool);

    let http_listen_address = std::net::SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        backend::config::get().get_http_port(),
    ));

    let (mqtt_peers_tx, _) = tokio::sync::broadcast::channel::<uuid::Uuid>(1024);
    let (mqtt_erase_tx, _) = tokio::sync::broadcast::channel::<uuid::Uuid>(1024);

    let mqtt_config = MqttBrokerConfig::from_env();
    let mqtt_pool = pool.clone();
    let mqtt_peers_tx_clone = mqtt_peers_tx.clone();
    let mqtt_erase_tx_clone = mqtt_erase_tx.clone();
    let (mqtt_tx, mqtt_rx) = oneshot::channel();
    std::thread::spawn(move || {
        let result =
            ntex::rt::System::new("mqtt-broker", ntex::rt::DefaultRuntime).block_on(async move {
                MqttBroker::run(
                    mqtt_config,
                    mqtt_pool,
                    mqtt_peers_tx_clone,
                    mqtt_erase_tx_clone,
                )
                .await
            });
        let _ = mqtt_tx.send(result);
    });

    let http_fut =
        http::Http::run_http_server(http_listen_address, pool, mqtt_peers_tx, mqtt_erase_tx);

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
