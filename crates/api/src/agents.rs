#![forbid(unsafe_code)]

//! Agent registry and version management endpoints with strict tenant RBAC.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{AgentsRepository, OrganizationMembersRepository, ProjectsRepository};
use agentlens_organizations::require_admin_or_owner;
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
pub struct CreateAgentInput {
    pub name: String,
    pub framework: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentInput {
    pub framework: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListAgentTracesQuery {
    pub limit: Option<i64>,
}

pub async fn list_agents_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
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
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Project {project_id} not found") })),
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

    let agent_repo = AgentsRepository::new(pool);
    match agent_repo.list_by_project(project_id).await {
        Ok(agents) => Json(agents).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list agents: {e}") })),
        )
            .into_response(),
    }
}

pub async fn create_agent_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateAgentInput>,
) -> Response {
    let trimmed_name = input.name.trim();
    if trimmed_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Agent name cannot be empty" })),
        )
            .into_response();
    }

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
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Project {project_id} not found") })),
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
    let role = match member_repo
        .get_role(project.organization_id, session.user_id)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied to organization" })),
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

    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let agent_repo = AgentsRepository::new(pool);
    let metadata = input.metadata.unwrap_or_else(|| json!({}));
    match agent_repo
        .create(
            project.organization_id,
            project_id,
            trimmed_name,
            input.framework.as_deref(),
            input.language.as_deref(),
            input.description.as_deref(),
            metadata,
        )
        .await
    {
        Ok(agent) => (StatusCode::CREATED, Json(agent)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create agent: {e}") })),
        )
            .into_response(),
    }
}

pub async fn get_agent_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
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

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Agent {agent_id} not found") })),
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
        .get_role(agent.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to agent organization" })),
        )
            .into_response();
    }

    Json(agent).into_response()
}

pub async fn update_agent_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<UpdateAgentInput>,
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

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Agent {agent_id} not found") })),
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
    let role = match member_repo
        .get_role(agent.organization_id, session.user_id)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied to agent organization" })),
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

    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    match agent_repo
        .update(
            agent.project_id,
            agent.id,
            input.framework.as_deref(),
            input.language.as_deref(),
            input.description.as_deref(),
            input.metadata,
        )
        .await
    {
        Ok(updated) => Json(updated).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to update agent: {e}") })),
        )
            .into_response(),
    }
}

pub async fn list_agent_versions_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
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

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Agent {agent_id} not found") })),
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
        .get_role(agent.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to agent organization" })),
        )
            .into_response();
    }

    match agent_repo.list_versions(agent.project_id, agent.id).await {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list agent versions: {e}") })),
        )
            .into_response(),
    }
}

pub async fn list_agent_traces_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<ListAgentTracesQuery>,
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

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Agent {agent_id} not found") })),
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
        .get_role(agent.organization_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to agent organization" })),
        )
            .into_response();
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    match agent_repo
        .list_traces(agent.project_id, agent.id, limit)
        .await
    {
        Ok(traces) => Json(traces).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list agent traces: {e}") })),
        )
            .into_response(),
    }
}
