//! AppState, will mostly be used by the `http` interface

use crate::dao;

#[derive(Clone)]
pub struct AppState {
    pub pool: dao::DbPool,
    pub mqtt_peers_tx: tokio::sync::broadcast::Sender<uuid::Uuid>,
}
