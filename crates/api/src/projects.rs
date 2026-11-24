#![forbid(unsafe_code)]

//! Project, environment, and scoped API key management endpoints with strict RBAC.

use crate::AppState;
use agentlens_auth::{generate_api_key, SessionPayload};
use agentlens_database::{
    ApiKeysRepository, AuditLogsRepository, EnvironmentsRepository, OrganizationMembersRepository,
    ProjectsRepository,
};
use agentlens_organizations::require_admin_or_owner;
use agentlens_projects::{
    compute_key_expiration, validate_api_key_input, validate_project_input, ApiKeyCreatedOutput,
    CreateApiKeyInput, CreateProjectInput, UpdateProjectInput,
};
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
pub struct ListProjectsQuery {
    pub organization_id: Option<Uuid>,
}

pub async fn list_projects_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<ListProjectsQuery>,
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

    let org_id = match query.organization_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "organization_id query parameter is required" })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo
        .get_role(org_id, session.user_id)
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied: not a member of this organization" })),
        )
            .into_response();
    }

    let projects_repo = ProjectsRepository::new(pool);
    match projects_repo.list_for_org(org_id).await {
        Ok(projects) => (StatusCode::OK, Json(projects)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn create_project_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Json(input): Json<CreateProjectInput>,
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

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo
        .get_role(input.organization_id, session.user_id)
        .await
    {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    if let Err(e) = validate_project_input(&input, role) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let projects_repo = ProjectsRepository::new(pool);
    let audit_repo = AuditLogsRepository::new(pool);

    match projects_repo
        .create(
            input.organization_id,
            &input.name,
            &input.slug,
            input.description.as_deref(),
        )
        .await
    {
        Ok((project, envs)) => {
            let _ = audit_repo
                .record(
                    input.organization_id,
                    Some(session.user_id),
                    None,
                    "project.create",
                    "project",
                    Some(&project.id.to_string()),
                    None,
                    None,
                    json!({ "name": &project.name, "slug": &project.slug }),
                )
                .await;

            (
                StatusCode::CREATED,
                Json(json!({
                    "project": project,
                    "environments": envs
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_project_handler(
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

    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
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
            Json(json!({ "error": "Access denied: not a member of this project's organization" })),
        )
            .into_response();
    }

    (StatusCode::OK, Json(project)).into_response()
}

pub async fn update_project_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<UpdateProjectInput>,
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

    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
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
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
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

    if input.retention_days <= 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "retention_days must be greater than zero" })),
        )
            .into_response();
    }

    match projects_repo
        .update(
            project_id,
            &input.name,
            input.description.as_deref(),
            input.retention_days,
        )
        .await
    {
        Ok(updated) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    project.organization_id,
                    Some(session.user_id),
                    None,
                    "project.update",
                    "project",
                    Some(&project_id.to_string()),
                    None,
                    None,
                    json!({ "name": &updated.name }),
                )
                .await;

            (StatusCode::OK, Json(updated)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn list_environments_handler(
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

    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let env_repo = EnvironmentsRepository::new(pool);
    match env_repo.list_for_project(project_id).await {
        Ok(envs) => (StatusCode::OK, Json(envs)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Scoped API Key Endpoints
// ---------------------------------------------------------------------------

pub async fn create_api_key_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateApiKeyInput>,
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

    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
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
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
            )
                .into_response();
        }
    };

    if let Err(e) = validate_api_key_input(&input, role) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let (secret, prefix, hash) = generate_api_key();
    let scopes = input
        .scopes
        .map(|s| json!(s))
        .unwrap_or_else(|| json!(["telemetry:write"]));
    let expires_at = compute_key_expiration(input.expires_in_days);

    let keys_repo = ApiKeysRepository::new(pool);
    match keys_repo
        .create(
            project.organization_id,
            project.id,
            input.environment_id,
            &input.name,
            &prefix,
            &hash,
            scopes.clone(),
            expires_at,
        )
        .await
    {
        Ok(api_key) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    project.organization_id,
                    Some(session.user_id),
                    None,
                    "api_key.create",
                    "api_key",
                    Some(&api_key.id.to_string()),
                    None,
                    None,
                    json!({ "name": &api_key.name, "prefix": &prefix }),
                )
                .await;

            let output = ApiKeyCreatedOutput {
                id: api_key.id,
                organization_id: api_key.organization_id,
                project_id: api_key.project_id,
                environment_id: api_key.environment_id,
                name: api_key.name,
                key_prefix: api_key.key_prefix,
                secret,
                scopes: api_key.scopes,
                expires_at: api_key.expires_at,
                created_at: api_key.created_at,
            };

            (StatusCode::CREATED, Json(output)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn list_api_keys_handler(
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

    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let keys_repo = ApiKeysRepository::new(pool);
    match keys_repo.list_for_project(project_id).await {
        Ok(keys) => {
            // NEVER return key_hash or secrets
            let sanitized: Vec<_> = keys
                .into_iter()
                .map(|k| {
                    json!({
                        "id": k.id,
                        "organization_id": k.organization_id,
                        "project_id": k.project_id,
                        "environment_id": k.environment_id,
                        "name": k.name,
                        "key_prefix": k.key_prefix,
                        "scopes": k.scopes,
                        "last_used_at": k.last_used_at,
                        "expires_at": k.expires_at,
                        "revoked_at": k.revoked_at,
                        "created_at": k.created_at
                    })
                })
                .collect();
            (StatusCode::OK, Json(sanitized)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn rotate_api_key_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(key_id): Path<Uuid>,
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

    let keys_repo = ApiKeysRepository::new(pool);
    let old_key = match keys_repo.get_by_id(key_id).await {
        Ok(Some(k)) => k,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "API key not found" })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo
        .get_role(old_key.organization_id, session.user_id)
        .await
    {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
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

    if let Err(e) = keys_repo.revoke(key_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to revoke old key: {e}") })),
        )
            .into_response();
    }

    let (secret, prefix, hash) = generate_api_key();
    match keys_repo
        .create(
            old_key.organization_id,
            old_key.project_id,
            old_key.environment_id,
            &old_key.name,
            &prefix,
            &hash,
            old_key.scopes.clone(),
            old_key.expires_at,
        )
        .await
    {
        Ok(new_key) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    old_key.organization_id,
                    Some(session.user_id),
                    None,
                    "api_key.rotate",
                    "api_key",
                    Some(&new_key.id.to_string()),
                    None,
                    None,
                    json!({ "old_key_id": key_id, "new_key_id": new_key.id }),
                )
                .await;

            let output = ApiKeyCreatedOutput {
                id: new_key.id,
                organization_id: new_key.organization_id,
                project_id: new_key.project_id,
                environment_id: new_key.environment_id,
                name: new_key.name,
                key_prefix: new_key.key_prefix,
                secret,
                scopes: new_key.scopes,
                expires_at: new_key.expires_at,
                created_at: new_key.created_at,
            };

            (StatusCode::CREATED, Json(output)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn revoke_api_key_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(key_id): Path<Uuid>,
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

    let keys_repo = ApiKeysRepository::new(pool);
    let key = match keys_repo.get_by_id(key_id).await {
        Ok(Some(k)) => k,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "API key not found" })),
            )
                .into_response();
        }
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo
        .get_role(key.organization_id, session.user_id)
        .await
    {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
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

    match keys_repo.revoke(key_id).await {
        Ok(()) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    key.organization_id,
                    Some(session.user_id),
                    None,
                    "api_key.revoke",
                    "api_key",
                    Some(&key_id.to_string()),
                    None,
                    None,
                    json!({ "key_id": key_id }),
                )
                .await;

            (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
