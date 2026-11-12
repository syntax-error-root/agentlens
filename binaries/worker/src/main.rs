#![forbid(unsafe_code)]

use agentlens_config::AppConfig;
use agentlens_database::create_pool;
use std::time::Duration;
use tokio::signal;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,agentlens=debug".into()),
        )
        .init();

    let config = AppConfig::from_env();
    info!("Starting agentlens-worker (env: {})", config.environment);

    let _db_pool = match create_pool(&config).await {
        Ok(pool) => {
            info!("Worker connected to PostgreSQL at {}", config.database_url);
            Some(pool)
        }
        Err(err) => {
            warn!("PostgreSQL unavailable for worker at startup: {}.", err);
            None
        }
    };

    let mut shutdown = Box::pin(shutdown_signal());

    info!("agentlens-worker polling loop initialized.");
    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("Worker received shutdown signal.");
                break;
            }
            _ = sleep(Duration::from_millis(500)) => {
                // Outbox job leasing will be executed here in Phase 05
            }
        }
    }

    info!("agentlens-worker shut down cleanly.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received in worker, terminating gracefully...");
}
