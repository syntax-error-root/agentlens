#![forbid(unsafe_code)]

//! `agentlens-api` crate.

use agentlens_database::{check_readiness, DbPool};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Option<DbPool>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(health_live_handler))
        .route("/health/ready", get(health_ready_handler))
        .with_state(Arc::new(state))
}

async fn health_live_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn health_ready_handler(State(state): State<Arc<AppState>>) -> Response {
    if let Some(ref pool) = state.db {
        match check_readiness(pool).await {
            Ok(()) => (
                StatusCode::OK,
                Json(json!({
                    "status": "ready",
                    "database": "connected"
                })),
            )
                .into_response(),
            Err(err) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "unavailable",
                    "database": "disconnected",
                    "error": err.to_string()
                })),
            )
                .into_response(),
        }
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unavailable",
                "database": "unconfigured"
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_live() {
        let app = create_router(AppState { db: None });
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_ready_without_db() {
        let app = create_router(AppState { db: None });
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
