#![forbid(unsafe_code)]

//! `agentlens-database` crate.

use agentlens_config::AppConfig;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLx pool error: {0}")]
    Pool(#[from] sqlx::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub type DbPool = PgPool;

/// Create a PostgreSQL connection pool with standard production timeouts.
pub async fn create_pool(config: &AppConfig) -> Result<DbPool, DatabaseError> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(&config.database_url)
        .await?;

    Ok(pool)
}

/// Run pending migrations from the repository root `migrations/` directory.
pub async fn run_migrations(pool: &DbPool) -> Result<(), DatabaseError> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

/// Check connectivity by running a basic query.
pub async fn check_readiness(pool: &DbPool) -> Result<(), DatabaseError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_module_exists() {
        let _ = DatabaseError::Pool(sqlx::Error::RowNotFound);
    }
}
