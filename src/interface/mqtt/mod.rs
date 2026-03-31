//! MQTT interface module.
//!
//! This module hosts the ingest-focused broker implementation and its
//! authentication/ingestion subcomponents.

/// Database-backed MQTT authentication helpers.
pub mod auth;
/// Publish ingestion and authorization-aware ack behavior.
pub mod ingest;
/// Embedded MQTT broker bootstrap and runtime wiring.
pub mod server;

/// Embedded MQTT broker type.
///
/// Re-exported for use by the application entrypoint.
pub use server::{MqttBroker, MqttBrokerConfig, MqttError};
