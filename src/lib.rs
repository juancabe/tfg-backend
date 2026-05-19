//! Server implementation
//!
//! # General notes
//!
//! ## Architecture
//!
//! The server is divided in modules [`interface`], [`service`], [`dao`], [`model`] (and [`helpers`]) with their
//! corresponding responsibilities. A layer based approach is intended, where [`interface`]
//! subsystems call [`service`]'s ones, [`service`] calls [`dao`] and [`dao`] uses [`model`]
//!
//! ### Implications
//!
//! - Each module contains its type interface definitions.
//! - If Module A needs to call Module B, A will use B defined types on the interface parameters and B defined types will also be returned.
//! - Exceptions may happen to occur, for example [`helpers`] Module types and (of course) [`std`] types can be returned whenever it's necessary, no need to wrap them.
//! - In order to avoid _boilerplate burden_, [`service`] layer is used as a mapper, [`interface`] defined types *can* be directly accepted and returned on the [`service`] layer.
//!

pub mod dao;
pub mod helpers;
pub mod interface;
pub mod model;
pub mod service;

#[cfg(test)]
pub mod test_utils;

/// Application configuration loaded from environment variables.
pub mod config {
    use std::sync::OnceLock;

    fn get_env_var_or_log_error<T>(var_name: &str) -> Option<T>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let val_str = std::env::var(var_name)
            .inspect_err(|e| log::warn!("[env var get] for: {var_name}, e: {e}"))
            .ok()?;

        val_str
            .parse::<T>()
            .inspect_err(|e| log::error!("[env var parse] for: {var_name}, e: {e}"))
            .ok()
    }

    /// Centralized application configuration.
    /// Should be loaded eagerly at startup to ensure all required environment variables are present.
    pub struct Config {
        /// The port on which the HTTP server listens. (Env: `HTTP_PORT`, Default: `8443`)
        http_port: u16,
        /// The public hostname or IP of the server. Used for generating URLs (e.g., email verification). (Env: `SERVER_NAME`, Default: `localhost`)
        server_name: String,
        /// The PostgreSQL connection string. (Env: `DATABASE_URL`)
        database_url: String,
        /// The base64-encoded secret key used for signing and verifying JWTs. (Env: `JWT_KEY_BASE64`)
        jwt_key_base64: String,
        /// The Gmail address used for sending emails via SMTP. (Env: `GMAIL_USER`)
        gmail_user: String,
        /// The Gmail app password for SMTP authentication. (Env: `GMAIL_PASS`)
        gmail_pass: String,
        /// The local network interface address where the embedded MQTT broker listens. (Env: `MQTT_BIND_HOST`, Default: `0.0.0.0`)
        mqtt_bind_host: String,
        /// The local TCP port where the embedded MQTT broker listens. (Env: `MQTT_BIND_PORT` or `MQTT_PORT`, Default: `8883`)
        mqtt_bind_port: u16,
        /// Path to the TLS certificate chain for the MQTT broker. (Env: `MQTT_TLS_CERT_PATH`, Default: `mqtt-server-cert.pem`)
        mqtt_tls_cert_path: String,
        /// Path to the TLS private key for the MQTT broker. (Env: `MQTT_TLS_KEY_PATH`, Default: `mqtt-server-key.pem`)
        mqtt_tls_key_path: String,
        /// Number of ntex worker threads for processing MQTT connections. (Env: `MQTT_WORKERS`, Default: `1`)
        mqtt_workers: usize,
        /// The public host address given to provisioned devices so they can connect to the MQTT broker. (Env: `MQTT_PROVISION_HOST` or `MQTT_HOST`, Default: `localhost`)
        mqtt_provision_host: String,
        /// The public port given to provisioned devices so they can connect to the MQTT broker.
        /// This may differ from `mqtt_bind_port` if the backend is behind a reverse proxy or Docker port forwarding. (Env: `MQTT_PROVISION_PORT` or `MQTT_PORT`, Default: `1883`)
        mqtt_provision_port: i16,
    }

    const DEFAULT_PORT: u16 = 8443;
    const DEFAULT_MQTT_BIND_PORT: u16 = 8883;
    const DEFAULT_MQTT_PROVISION_PORT: i16 = 8883;

    impl Config {
        /// Loads configuration from environment variables.
        pub fn init() -> Self {
            Self {
                http_port: get_env_var_or_log_error("HTTP_PORT").unwrap_or(DEFAULT_PORT),
                database_url: get_env_var_or_log_error("DATABASE_URL").unwrap(),
                server_name: get_env_var_or_log_error("SERVER_NAME")
                    .unwrap_or_else(|| "localhost".to_string()),
                jwt_key_base64: get_env_var_or_log_error("JWT_KEY_BASE64").unwrap(),
                gmail_user: get_env_var_or_log_error("GMAIL_USER").unwrap(),
                gmail_pass: get_env_var_or_log_error("GMAIL_PASS").unwrap(),
                mqtt_bind_host: get_env_var_or_log_error("MQTT_BIND_HOST")
                    .unwrap_or_else(|| "0.0.0.0".to_string()),
                mqtt_bind_port: get_env_var_or_log_error("MQTT_BIND_PORT")
                    .or_else(|| get_env_var_or_log_error("MQTT_PORT"))
                    .unwrap_or(DEFAULT_MQTT_BIND_PORT),
                mqtt_tls_cert_path: get_env_var_or_log_error("MQTT_TLS_CERT_PATH")
                    .unwrap_or_else(|| "mqtt-server-cert.pem".to_string()),
                mqtt_tls_key_path: get_env_var_or_log_error("MQTT_TLS_KEY_PATH")
                    .unwrap_or_else(|| "mqtt-server-key.pem".to_string()),
                mqtt_workers: get_env_var_or_log_error("MQTT_WORKERS").unwrap_or(1),
                mqtt_provision_host: get_env_var_or_log_error("MQTT_PROVISION_HOST")
                    .or_else(|| get_env_var_or_log_error("MQTT_HOST"))
                    .unwrap_or_else(|| "localhost".to_string()),
                mqtt_provision_port: get_env_var_or_log_error("MQTT_PROVISION_PORT")
                    .or_else(|| get_env_var_or_log_error("MQTT_PORT"))
                    .unwrap_or(DEFAULT_MQTT_PROVISION_PORT),
            }
        }

        /// Returns the HTTP server port.
        pub fn get_http_port(&self) -> u16 {
            self.http_port
        }
        /// Returns the PostgreSQL connection string.
        pub fn get_database_url(&self) -> &str {
            &self.database_url
        }
        /// Returns the server hostname.
        pub fn get_server_name(&self) -> &str {
            &self.server_name
        }
        /// Returns the JWT signing key (base64).
        pub fn get_jwt_key_base64(&self) -> &str {
            &self.jwt_key_base64
        }
        /// Returns the Gmail SMTP user.
        pub fn get_gmail_user(&self) -> &str {
            &self.gmail_user
        }
        /// Returns the Gmail SMTP password.
        pub fn get_gmail_pass(&self) -> &str {
            &self.gmail_pass
        }
        /// Returns the MQTT bind host.
        pub fn get_mqtt_bind_host(&self) -> &str {
            &self.mqtt_bind_host
        }
        /// Returns the MQTT bind port.
        pub fn get_mqtt_bind_port(&self) -> u16 {
            self.mqtt_bind_port
        }
        /// Returns the MQTT TLS certificate path.
        pub fn get_mqtt_tls_cert_path(&self) -> &str {
            &self.mqtt_tls_cert_path
        }
        /// Returns the MQTT TLS key path.
        pub fn get_mqtt_tls_key_path(&self) -> &str {
            &self.mqtt_tls_key_path
        }
        /// Returns the number of MQTT worker threads.
        pub fn get_mqtt_workers(&self) -> usize {
            self.mqtt_workers
        }
        /// Returns the public MQTT host for provisioned devices.
        pub fn get_mqtt_provision_host(&self) -> &str {
            &self.mqtt_provision_host
        }
        /// Returns the public MQTT port for provisioned devices.
        pub fn get_mqtt_provision_port(&self) -> i16 {
            self.mqtt_provision_port
        }
    }

    /// Global application configuration singleton.
    pub static CONFIG: OnceLock<Config> = OnceLock::new();

    /// Returns the global application configuration.
    pub fn get() -> &'static Config {
        CONFIG.get().expect("Config must be initialized before use")
    }
}
