use std::net::{Ipv4Addr, SocketAddrV4};

use backend::interface::http::{self, DEFAULT_PORT};

#[tokio::main]
async fn main() {
    env_logger::init();

    let http_listen_address =
        std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DEFAULT_PORT));

    http::Http::run_http_server(http_listen_address)
        .await
        .unwrap();
}
