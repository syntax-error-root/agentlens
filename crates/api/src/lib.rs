#![forbid(unsafe_code)]

//! `agentlens-api` crate.
//! HTTP routing, middleware, health endpoints, and control-plane API handlers.

pub mod auth;
pub mod middleware;
pub mod organizations;
pub mod otlp;
pub mod projects;

use agentlens_config::AppConfig;
use agentlens_database::{check_readiness, DbPool};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct AppState {
    pub db: Option<DbPool>,
    pub config: AppConfig,
}

pub fn create_router(state: AppState) -> Router {
    let state_arc = Arc::new(state);

    let protected_routes = Router::new()
        // Auth me / logout
        .route("/api/v1/auth/me", get(auth::me_handler))
        .route("/api/v1/auth/logout", post(auth::logout_handler))
        // Organizations
        .route(
            "/api/v1/organizations",
            get(organizations::list_organizations_handler)
                .post(organizations::create_organization_handler),
        )
        .route(
            "/api/v1/organizations/{org_id}",
            get(organizations::get_organization_handler)
                .patch(organizations::update_organization_handler),
        )
        .route(
            "/api/v1/organizations/{org_id}/members",
            get(organizations::list_members_handler).post(organizations::add_member_handler),
        )
        .route(
            "/api/v1/organizations/{org_id}/members/{user_id}",
            patch(organizations::update_member_role_handler)
                .delete(organizations::remove_member_handler),
        )
        // Projects
        .route(
            "/api/v1/projects",
            get(projects::list_projects_handler).post(projects::create_project_handler),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(projects::get_project_handler).patch(projects::update_project_handler),
        )
        .route(
            "/api/v1/projects/{project_id}/environments",
            get(projects::list_environments_handler),
        )
        // Project API keys
        .route(
            "/api/v1/projects/{project_id}/api-keys",
            get(projects::list_api_keys_handler).post(projects::create_api_key_handler),
        )
        .route(
            "/api/v1/api-keys/{key_id}/rotate",
            post(projects::rotate_api_key_handler),
        )
        .route(
            "/api/v1/api-keys/{key_id}",
            delete(projects::revoke_api_key_handler),
        )
        .layer(from_fn_with_state(
            state_arc.clone(),
            control_plane_auth_guard,
        ));

    let public_routes = Router::new()
        .route("/health/live", get(health_live_handler))
        .route("/health/ready", get(health_ready_handler))
        .route("/v1/traces", post(otlp::otlp_traces_handler))
        .route("/api/v1/auth/bootstrap", post(auth::bootstrap_handler))
        .route("/api/v1/auth/login", post(auth::login_handler))
        .route("/api/v1/auth/oidc/start", get(auth::oidc_start_handler))
        .route(
            "/api/v1/auth/oidc/callback",
            post(auth::oidc_callback_handler),
        );

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state_arc)
}

async fn control_plane_auth_guard(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    middleware::control_plane_auth_middleware(
        state.config.security.session_secret.as_bytes(),
        req,
        next,
    )
    .await
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
        let app = create_router(AppState::default());
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
        let app = create_router(AppState::default());
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

    #[tokio::test]
    async fn test_ingestion_api_key_rejected_on_control_plane() {
        let app = create_router(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/organizations")
                    .header("Authorization", "Bearer agl_live_fake1234567890abcdef")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_unauthenticated_request_rejected() {
        let app = create_router(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/organizations")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_otlp_unauthenticated_rejected() {
        let app = create_router(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
