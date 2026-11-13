#![forbid(unsafe_code)]

//! Trace and span query endpoints supporting keyset pagination and waterfall reconstruction.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{
    OrganizationMembersRepository, ProjectsRepository, SpansRepository, TracesRepository,
};
use agentlens_domain::{SpanType, TraceQueryFilters, TraceStatus};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListTracesQuery {
    pub project_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub status: Option<TraceStatus>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub span_type: Option<SpanType>,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub search_trace_id: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_traces_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<ListTracesQuery>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response();
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(query.project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Project {} not found", query.project_id) })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo
        .get_role(project.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to project organization" })),
        )
            .into_response();
    }

    let filters = TraceQueryFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        agent_id: query.agent_id,
        status: query.status.map(|s| s.as_str().to_string()),
        provider: query.provider,
        model: query.model,
        span_type: query.span_type.map(|s| s.as_str().to_string()),
        min_duration_ms: query.min_duration_ms,
        max_duration_ms: query.max_duration_ms,
        from: query.from,
        to: query.to,
        search_trace_id: query.search_trace_id,
        cursor: query.cursor,
        limit: query.limit,
    };

    let traces_repo = TracesRepository::new(pool);
    match traces_repo.list_traces(&filters).await {
        Ok(res) => Json(res).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to query traces: {e}") })),
        )
            .into_response(),
    }
}

pub async fn get_trace_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(trace_record_id): Path<String>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response();
        }
    };

    let traces_repo = TracesRepository::new(pool);
    let trace = match traces_repo
        .get_by_id_or_trace_id_global(&trace_record_id)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Trace '{trace_record_id}' not found") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo
        .get_role(trace.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to trace organization" })),
        )
            .into_response();
    }

    Json(trace).into_response()
}

pub async fn list_trace_spans_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(trace_record_id): Path<String>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response();
        }
    };

    let traces_repo = TracesRepository::new(pool);
    let trace = match traces_repo
        .get_by_id_or_trace_id_global(&trace_record_id)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Trace '{trace_record_id}' not found") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo
        .get_role(trace.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to trace organization" })),
        )
            .into_response();
    }

    let spans_repo = SpansRepository::new(pool);
    match spans_repo
        .list_for_trace(trace.project_id, &trace.trace_id)
        .await
    {
        Ok(spans) => Json(spans).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list trace spans: {e}") })),
        )
            .into_response(),
    }
}

pub async fn get_span_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(span_record_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response();
        }
    };

    let spans_repo = SpansRepository::new(pool);
    let span_detail = match spans_repo.get_span_detail_global(span_record_id).await {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Span '{span_record_id}' not found") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo
        .get_role(span_detail.span.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to span organization" })),
        )
            .into_response();
    }

    Json(span_detail).into_response()
}
