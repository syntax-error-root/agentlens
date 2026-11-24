#![forbid(unsafe_code)]

//! Session management and trace aggregation endpoints with tenant isolation.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{OrganizationMembersRepository, ProjectsRepository, SessionsRepository};
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub project_id: Uuid,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ListSessionTracesQuery {
    pub limit: Option<i64>,
}

pub async fn list_sessions_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<ListSessionsQuery>,
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

    let sessions_repo = SessionsRepository::new(pool);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);

    match sessions_repo.list_by_project(query.project_id, limit).await {
        Ok(sessions) => Json(sessions).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list sessions: {e}") })),
        )
            .into_response(),
    }
}

pub async fn get_session_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(session_id): Path<Uuid>,
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

    let sessions_repo = SessionsRepository::new(pool);
    let sess = match sessions_repo.get_by_id_global(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Session {session_id} not found") })),
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
        .get_role(sess.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to session organization" })),
        )
            .into_response();
    }

    Json(sess).into_response()
}

pub async fn list_session_traces_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ListSessionTracesQuery>,
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

    let sessions_repo = SessionsRepository::new(pool);
    let sess = match sessions_repo.get_by_id_global(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Session {session_id} not found") })),
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
        .get_role(sess.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to session organization" })),
        )
            .into_response();
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    match sessions_repo
        .list_traces(sess.project_id, sess.id, limit)
        .await
    {
        Ok(traces) => Json(traces).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list session traces: {e}") })),
        )
            .into_response(),
    }
}
