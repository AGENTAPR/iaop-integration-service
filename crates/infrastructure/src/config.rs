//! Typed application configuration loaded from environment variables.

use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingVar(&'static str),
    #[error("invalid value for {0}: {1}")]
    InvalidValue(&'static str, String),
}

/// Service configuration. Secrets (e.g. DATABASE_URL) are never logged;
/// use [`AppConfig::redacted_summary`] for safe diagnostics.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub service_name: String,
    pub service_version: String,
    pub environment: String,
    pub bind_addr: String,
    /// Optional: services without persistence leave this unset and
    /// `/health/ready` does not probe a database.
    pub database_url: Option<String>,
    pub database_max_connections: u32,
    pub request_timeout: Duration,
    pub is_production: bool,

    // -- events & messaging ---------------------------------------------
    pub event_transport: String,
    pub kafka_brokers: Option<String>,
    pub kafka_topic_prefix: Option<String>,
    pub outbox_poll_interval: Duration,
    pub outbox_batch_size: i64,
    pub outbox_max_attempts: i32,

    // -- connectors & callbacks -----------------------------------------
    pub connector_mode: String,
    pub callback_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = env_or("ENVIRONMENT", "local");
        let is_production = environment.eq_ignore_ascii_case("production");

        Ok(Self {
            service_name: env_or("SERVICE_NAME", "iaop-integration-service"),
            service_version: env_or("SERVICE_VERSION", env!("CARGO_PKG_VERSION")),
            environment,
            bind_addr: env_or("BIND_ADDR", "0.0.0.0:8080"),
            database_url: std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty()),
            database_max_connections: env_parse("DATABASE_MAX_CONNECTIONS", 10)?,
            request_timeout: Duration::from_secs(env_parse("REQUEST_TIMEOUT_SECS", 30)?),
            is_production,
            event_transport: env_or("EVENT_TRANSPORT", "log"),
            kafka_brokers: std::env::var("KAFKA_BROKERS")
                .ok()
                .filter(|s| !s.is_empty()),
            kafka_topic_prefix: std::env::var("KAFKA_TOPIC_PREFIX")
                .ok()
                .filter(|s| !s.is_empty()),
            outbox_poll_interval: Duration::from_millis(env_parse("OUTBOX_POLL_MS", 1000)?),
            outbox_batch_size: env_parse("OUTBOX_BATCH_SIZE", 50)?,
            outbox_max_attempts: env_parse("OUTBOX_MAX_ATTEMPTS", 10)?,
            connector_mode: env_or("CONNECTOR_MODE", "hybrid"),
            callback_secret: env_or("CALLBACK_SECRET", "dev-insecure-callback-secret-123456"),
        })
    }

    /// Diagnostics-safe summary: never includes secrets.
    pub fn redacted_summary(&self) -> String {
        format!(
            "service={} version={} env={} bind={} db={} timeout={}s",
            self.service_name,
            self.service_version,
            self.environment,
            self.bind_addr,
            if self.database_url.is_some() {
                "configured"
            } else {
                "disabled"
            },
            self.request_timeout.as_secs(),
        )
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_parse<T: std::str::FromStr>(key: &'static str, default: T) -> Result<T, ConfigError> {
    match std::env::var(key) {
        Ok(v) => v
            .parse::<T>()
            .map_err(|_| ConfigError::InvalidValue(key, v)),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_loads_with_defaults() {
        let cfg = AppConfig::from_env().expect("defaults must be valid");
        assert!(!cfg.service_name.is_empty());
        assert!(cfg.redacted_summary().contains("db="));
    }
}
