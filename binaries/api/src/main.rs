#![forbid(unsafe_code)]

use agentlens_api::{create_router, AppState};
use agentlens_config::AppConfig;
use agentlens_database::create_pool;
use std::net::SocketAddr;
use tokio::signal;
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
    info!("Starting agentlens-api (env: {})", config.environment);

    let db_pool = match create_pool(&config).await {
        Ok(pool) => {
            info!("Connected to PostgreSQL at {}", config.database.url);
            Some(pool)
        }
        Err(err) => {
            warn!(
                "PostgreSQL unavailable at startup: {}. Running in degraded state.",
                err
            );
            None
        }
    };

    let state = AppState {
        db: db_pool,
        config: config.clone(),
    };
    let app = create_router(state);

    let api_addr: SocketAddr = config.server.api_bind_addr.parse().unwrap_or_else(|_| {
        warn!(
            "Invalid API_BIND_ADDR: '{}', falling back to 0.0.0.0:8080",
            config.server.api_bind_addr
        );
        "0.0.0.0:8080".parse().unwrap()
    });

    let otlp_addr: SocketAddr = config
        .server
        .otlp_http_bind_addr
        .parse()
        .unwrap_or_else(|_| {
            warn!(
                "Invalid OTLP_HTTP_BIND_ADDR: '{}', falling back to 0.0.0.0:4318",
                config.server.otlp_http_bind_addr
            );
            "0.0.0.0:4318".parse().unwrap()
        });

    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    info!("agentlens-api HTTP listening on http://{}", api_addr);

    let otlp_listener = match tokio::net::TcpListener::bind(otlp_addr).await {
        Ok(listener) => {
            info!("agentlens-api OTLP/HTTP listening on http://{}", otlp_addr);
            Some(listener)
        }
        Err(err) => {
            warn!("Could not bind OTLP/HTTP to {}: {}", otlp_addr, err);
            None
        }
    };

    let api_server =
        axum::serve(api_listener, app.clone()).with_graceful_shutdown(shutdown_signal());

    if let Some(otlp_listener) = otlp_listener {
        let otlp_server = axum::serve(otlp_listener, app).with_graceful_shutdown(shutdown_signal());

        tokio::select! {
            res = api_server => {
                if let Err(e) = res {
                    error!("API server error: {}", e);
                }
            }
            res = otlp_server => {
                if let Err(e) = res {
                    error!("OTLP server error: {}", e);
                }
            }
        }
    } else if let Err(e) = api_server.await {
        error!("API server error: {}", e);
    }

    info!("agentlens-api terminated cleanly.");
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

    info!("Shutdown signal received, starting graceful termination...");
}
