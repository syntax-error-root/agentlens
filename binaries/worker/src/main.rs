#![forbid(unsafe_code)]

use agentlens_config::AppConfig;
use agentlens_database::create_pool;
use agentlens_jobs::{OutboxWorker, WorkerConfig};
use agentlens_observability::ObservabilityRegistry;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::watch;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,agentlens=debug".into()),
        )
        .init();

    let config = AppConfig::from_env();
    config.validate()?;
    info!("Starting agentlens-worker (env: {})", config.environment);

    let db_pool = match create_pool(&config).await {
        Ok(pool) => {
            info!("Worker connected to PostgreSQL at {}", config.database.url);
            Some(pool)
        }
        Err(err) => {
            warn!("PostgreSQL unavailable for worker at startup: {}.", err);
            None
        }
    };

    let registry = Arc::new(ObservabilityRegistry::new());
    let worker_config = WorkerConfig::default();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    if let Some(pool) = db_pool {
        let worker = OutboxWorker::new(pool, registry, worker_config);
        let worker_task = tokio::spawn(async move {
            worker.run(shutdown_rx).await;
        });

        shutdown_signal().await;
        info!("Shutdown signal received. Notifying worker task...");
        let _ = shutdown_tx.send(true);
        let _ = worker_task.await;
    } else {
        error!("Worker cannot start without valid database pool.");
        shutdown_signal().await;
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
}
