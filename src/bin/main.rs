use diesel::{PgConnection, r2d2::ConnectionManager};
use dotenvy::dotenv;
use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::{join, select};

use backend::dao::{DbPool, MAX_DB_POOL_SIZE};
use backend::interface::http::{self, DEFAULT_PORT};
use backend::interface::mqtt::MqttClient;

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

    let http_listen_address =
        std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT));

    // Initialize internal MQTT Client to listen to our own broker
    let (mqtt_client, eventloop) = MqttClient::new().await;
    let mqtt_pool = pool.clone();

    // Spawn internal MQTT client loop
    let mqtt_fut = mqtt_client.run(eventloop, mqtt_pool);
    let http_fut = http::Http::run_http_server(http_listen_address, pool);

    select! {
        val = mqtt_fut => {
            val.unwrap();
        },
        val = http_fut => {
            val.unwrap();
        },
    };
}
