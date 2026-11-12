#![forbid(unsafe_code)]

//! `agentlens-config` crate.
//! Strongly typed configuration with fail-closed production validation.

use serde::Deserialize;
use std::env;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub api_bind_addr: String,
    pub otlp_http_bind_addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityConfig {
    pub session_secret: String,
    pub app_encryption_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub redis_url: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetentionConfig {
    pub default_retention_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub environment: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
    pub retention: RetentionConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let env_mode = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());

        Self {
            environment: env_mode,
            server: ServerConfig {
                api_bind_addr: env::var("API_BIND_ADDR")
                    .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
                otlp_http_bind_addr: env::var("OTLP_HTTP_BIND_ADDR")
                    .unwrap_or_else(|_| "0.0.0.0:4318".to_string()),
            },
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://postgres:postgres@localhost:5432/agentlens".to_string()
                }),
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(20),
                min_connections: env::var("DB_MIN_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2),
            },
            security: SecurityConfig {
                session_secret: env::var("SESSION_SECRET")
                    .unwrap_or_else(|_| "development_secret_session_key_32_bytes_min!".to_string()),
                app_encryption_key: env::var("APP_ENCRYPTION_KEY").unwrap_or_else(|_| {
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
                }),
            },
            storage: StorageConfig {
                redis_url: env::var("REDIS_URL").ok(),
                s3_endpoint: env::var("S3_ENDPOINT").ok(),
                s3_bucket: env::var("S3_BUCKET").ok(),
            },
            retention: RetentionConfig {
                default_retention_days: env::var("RETENTION_DAYS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30),
            },
        }
    }
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self::default()
    }

    /// Validates the configuration fail-closed, especially under production.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let is_prod = self.environment.eq_ignore_ascii_case("production")
            || self.environment.eq_ignore_ascii_case("prod");

        if is_prod {
            if self.security.session_secret.len() < 32 {
                return Err(ConfigError::Validation(
                    "SESSION_SECRET must be at least 32 characters in production".to_string(),
                ));
            }
            if self.security.session_secret.contains("change_me")
                || self.security.session_secret.contains("development")
            {
                return Err(ConfigError::Validation(
                    "SESSION_SECRET must not contain development or placeholder values in production".to_string(),
                ));
            }

            if self.security.app_encryption_key.len() < 32 {
                return Err(ConfigError::Validation(
                    "APP_ENCRYPTION_KEY must be at least 32 bytes/characters in production"
                        .to_string(),
                ));
            }
            if self.security.app_encryption_key.contains("change_me")
                || self
                    .security
                    .app_encryption_key
                    .starts_with("0123456789abcdef")
            {
                return Err(ConfigError::Validation(
                    "APP_ENCRYPTION_KEY must not use known insecure default/example keys in production".to_string(),
                ));
            }
        }

        if self.database.url.is_empty() {
            return Err(ConfigError::Validation(
                "DATABASE_URL cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    // Convenience accessors for backwards compatibility
    #[must_use]
    pub fn api_bind_addr(&self) -> &str {
        &self.server.api_bind_addr
    }

    #[must_use]
    pub fn otlp_http_bind_addr(&self) -> &str {
        &self.server.otlp_http_bind_addr
    }

    #[must_use]
    pub fn database_url(&self) -> &str {
        &self.database.url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_development_config_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_production_fails_with_insecure_defaults() {
        let mut config = AppConfig::default();
        config.environment = "production".to_string();
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("insecure default") || err_msg.contains("placeholder"));
    }

    #[test]
    fn test_production_valid_with_strong_keys() {
        let mut config = AppConfig::default();
        config.environment = "production".to_string();
        config.security.session_secret =
            "super_strong_production_session_secret_at_least_32_bytes_long!".to_string();
        config.security.app_encryption_key =
            "a9f4c3b8e1d2a5f6e7c8b9a0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0".to_string();
        assert!(config.validate().is_ok());
    }
}
