use diesel::{PgConnection, r2d2::ConnectionManager};
use dotenvy::dotenv;
use std::env;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::select;

use backend::dao::{DbPool, MAX_DB_POOL_SIZE};
use backend::interface::http::{self, DEFAULT_PORT};
use backend::interface::mqtt::{MqttBroker, MqttBrokerConfig};

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

    let mqtt_config = MqttBrokerConfig::from_env();
    let mqtt_fut = MqttBroker::run(mqtt_config, pool.clone());
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
