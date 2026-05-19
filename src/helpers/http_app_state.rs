//! AppState, will mostly be used by the `http` interface

use crate::dao;

/// Shared application state passed to HTTP request handlers.
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool.
    pub pool: dao::DbPool,
    /// Broadcast channel sender for notifying about MQTT peer producer IDs.
    pub mqtt_peers_tx: tokio::sync::broadcast::Sender<uuid::Uuid>,
}
