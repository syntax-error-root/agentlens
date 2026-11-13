#![forbid(unsafe_code)]

//! `agentlens-api` crate.
//! HTTP routing, middleware, health endpoints, and control-plane API handlers.

pub mod agents;
pub mod alerts;
pub mod analytics;
pub mod auth;
pub mod middleware;
pub mod organizations;
pub mod otlp;
pub mod overview;
pub mod projects;
pub mod sessions;
pub mod traces;

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
    pub metrics: Arc<agentlens_observability::ObservabilityRegistry>,
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
        // Project Overview
        .route(
            "/api/v1/projects/{project_id}/overview",
            get(overview::get_project_overview_handler),
        )
        // Agents
        .route(
            "/api/v1/projects/{project_id}/agents",
            get(agents::list_agents_handler).post(agents::create_agent_handler),
        )
        .route(
            "/api/v1/agents/{agent_id}",
            get(agents::get_agent_handler).patch(agents::update_agent_handler),
        )
        .route(
            "/api/v1/agents/{agent_id}/versions",
            get(agents::list_agent_versions_handler),
        )
        .route(
            "/api/v1/agents/{agent_id}/traces",
            get(agents::list_agent_traces_handler),
        )
        .route(
            "/api/v1/agents/{agent_id}/health",
            get(agents::get_agent_health_handler),
        )
        // Traces and Spans
        .route("/api/v1/traces", get(traces::list_traces_handler))
        .route(
            "/api/v1/traces/{trace_record_id}",
            get(traces::get_trace_handler),
        )
        .route(
            "/api/v1/traces/{trace_record_id}/spans",
            get(traces::list_trace_spans_handler),
        )
        .route(
            "/api/v1/spans/{span_record_id}",
            get(traces::get_span_handler),
        )
        // Sessions
        .route("/api/v1/sessions", get(sessions::list_sessions_handler))
        .route(
            "/api/v1/sessions/{session_id}",
            get(sessions::get_session_handler),
        )
        .route(
            "/api/v1/sessions/{session_id}/traces",
            get(sessions::list_session_traces_handler),
        )
        // Analytics
        .route(
            "/api/v1/analytics/overview",
            get(analytics::get_overview_handler),
        )
        .route(
            "/api/v1/analytics/tokens",
            get(analytics::get_tokens_handler),
        )
        .route("/api/v1/analytics/costs", get(analytics::get_costs_handler))
        .route(
            "/api/v1/analytics/models",
            get(analytics::get_models_handler),
        )
        .route(
            "/api/v1/analytics/agents",
            get(analytics::get_agents_handler),
        )
        .route("/api/v1/analytics/tools", get(analytics::get_tools_handler))
        .route(
            "/api/v1/analytics/latency",
            get(analytics::get_latency_handler),
        )
        .route(
            "/api/v1/analytics/errors",
            get(analytics::get_errors_handler),
        )
        // Pricing
        .route(
            "/api/v1/pricing",
            get(analytics::list_pricing_handler).post(analytics::create_pricing_handler),
        )
        // Alert Rules
        .route(
            "/api/v1/projects/{project_id}/alert-rules",
            get(alerts::list_alert_rules_handler).post(alerts::create_alert_rule_handler),
        )
        .route(
            "/api/v1/alert-rules/{rule_id}",
            get(alerts::get_alert_rule_handler)
                .patch(alerts::update_alert_rule_handler)
                .delete(alerts::delete_alert_rule_handler),
        )
        // Alert Events
        .route(
            "/api/v1/projects/{project_id}/alert-events",
            get(alerts::list_alert_events_handler),
        )
        .route(
            "/api/v1/alert-events/{event_id}",
            get(alerts::get_alert_event_handler),
        )
        .route(
            "/api/v1/alert-events/{event_id}/acknowledge",
            post(alerts::acknowledge_alert_event_handler),
        )
        // Notification Channels
        .route(
            "/api/v1/projects/{project_id}/notification-channels",
            get(alerts::list_notification_channels_handler)
                .post(alerts::create_notification_channel_handler),
        )
        .route(
            "/api/v1/notification-channels/{channel_id}",
            get(alerts::get_notification_channel_handler)
                .delete(alerts::delete_notification_channel_handler),
        )
        .route(
            "/api/v1/notification-channels/{channel_id}/test",
            post(alerts::test_notification_channel_handler),
        )
        .layer(from_fn_with_state(
            state_arc.clone(),
            control_plane_auth_guard,
        ));

    let public_routes = Router::new()
        .route("/health/live", get(health_live_handler))
        .route("/health/ready", get(health_ready_handler))
        .route("/metrics", get(metrics_handler))
        .route("/v1/traces", post(otlp::otlp_traces_handler))
        .route("/api/v1/auth/bootstrap", post(auth::bootstrap_handler))
        .route(
            "/api/v1/auth/bootstrap/status",
            get(auth::bootstrap_status_handler),
        )
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

async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.metrics.render_prometheus(),
    )
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

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let app = create_router(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);
        assert!(body_str.contains("agentlens_outbox_pending_jobs"));
        assert!(body_str.contains("agentlens_worker_jobs_processed_total"));
        assert!(body_str.contains("agentlens_otlp_spans_ingested_total"));
    }

    #[tokio::test]
    async fn test_bootstrap_status_endpoint_is_public() {
        let app = create_router(AppState::default());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/auth/bootstrap/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Without DB it returns 503 SERVICE_UNAVAILABLE, NOT 401 UNAUTHORIZED
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_query_endpoints_require_authentication() {
        let app = create_router(AppState::default());
        let protected_uris = [
            "/api/v1/projects/00000000-0000-0000-0000-000000000001/overview",
            "/api/v1/projects/00000000-0000-0000-0000-000000000001/agents",
            "/api/v1/traces?project_id=00000000-0000-0000-0000-000000000001",
            "/api/v1/sessions?project_id=00000000-0000-0000-0000-000000000001",
        ];

        for uri in protected_uris {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "Expected 401 Unauthorized for {uri}"
            );
        }
    }
}
