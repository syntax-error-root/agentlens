#![forbid(unsafe_code)]

//! `agentlens-config` crate.

use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub api_bind_addr: String,
    pub otlp_http_bind_addr: String,
    pub database_url: String,
    pub redis_url: Option<String>,
    pub environment: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            otlp_http_bind_addr: env::var("OTLP_HTTP_BIND_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:4318".to_string()),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/agentlens".to_string()
            }),
            redis_url: env::var("REDIS_URL").ok(),
            environment: env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
        }
    }
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.api_bind_addr, "0.0.0.0:8080");
        assert_eq!(config.otlp_http_bind_addr, "0.0.0.0:4318");
    }
}
