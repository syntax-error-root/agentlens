#![forbid(unsafe_code)]

//! Organization management and membership RBAC endpoints.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{
    AuditLogsRepository, OrganizationMembersRepository, OrganizationsRepository,
};
use agentlens_domain::OrgRole;
use agentlens_organizations::{
    require_owner, validate_slug, AddMemberInput, CreateOrgInput, UpdateMemberRoleInput,
    UpdateOrgInput,
};
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list_organizations_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
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

    let repo = OrganizationsRepository::new(pool);
    match repo.list_for_user(session.user_id).await {
        Ok(memberships) => {
            let res: Vec<_> = memberships
                .into_iter()
                .map(|(org, role)| {
                    json!({
                        "id": org.id,
                        "name": org.name,
                        "slug": org.slug,
                        "role": role,
                        "created_at": org.created_at,
                        "updated_at": org.updated_at
                    })
                })
                .collect();
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn create_organization_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Json(input): Json<CreateOrgInput>,
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

    if let Err(e) = validate_slug(&input.slug) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    if input.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Organization name cannot be empty" })),
        )
            .into_response();
    }

    let org_repo = OrganizationsRepository::new(pool);
    let member_repo = OrganizationMembersRepository::new(pool);
    let audit_repo = AuditLogsRepository::new(pool);

    let org = match org_repo.create(&input.name, &input.slug).await {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    if let Err(e) = member_repo
        .add(org.id, session.user_id, OrgRole::Owner)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let _ = audit_repo
        .record(
            org.id,
            Some(session.user_id),
            None,
            "organization.create",
            "organization",
            Some(&org.id.to_string()),
            None,
            None,
            json!({ "name": &org.name, "slug": &org.slug }),
        )
        .await;

    (
        StatusCode::CREATED,
        Json(json!({
            "id": org.id,
            "name": org.name,
            "slug": org.slug,
            "role": OrgRole::Owner,
            "created_at": org.created_at,
            "updated_at": org.updated_at
        })),
    )
        .into_response()
}

pub async fn get_organization_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(org_id): Path<Uuid>,
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
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    let org_repo = OrganizationsRepository::new(pool);
    match org_repo.get_by_id(org_id).await {
        Ok(Some(org)) => (
            StatusCode::OK,
            Json(json!({
                "id": org.id,
                "name": org.name,
                "slug": org.slug,
                "role": role,
                "created_at": org.created_at,
                "updated_at": org.updated_at
            })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Organization not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn update_organization_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(org_id): Path<Uuid>,
    Json(input): Json<UpdateOrgInput>,
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
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    if let Err(e) = require_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    if input.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Name cannot be empty" })),
        )
            .into_response();
    }

    let org_repo = OrganizationsRepository::new(pool);
    match org_repo.update(org_id, &input.name).await {
        Ok(org) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    org.id,
                    Some(session.user_id),
                    None,
                    "organization.update",
                    "organization",
                    Some(&org.id.to_string()),
                    None,
                    None,
                    json!({ "name": &org.name }),
                )
                .await;

            (
                StatusCode::OK,
                Json(json!({
                    "id": org.id,
                    "name": org.name,
                    "slug": org.slug,
                    "role": role,
                    "created_at": org.created_at,
                    "updated_at": org.updated_at
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

pub async fn list_members_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(org_id): Path<Uuid>,
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

    match member_repo.list_members(org_id).await {
        Ok(members) => {
            let res: Vec<_> = members
                .into_iter()
                .map(|(user, role)| {
                    json!({
                        "user_id": user.id,
                        "email": user.email,
                        "display_name": user.display_name,
                        "role": role,
                        "created_at": user.created_at
                    })
                })
                .collect();
            (StatusCode::OK, Json(res)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn add_member_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(org_id): Path<Uuid>,
    Json(input): Json<AddMemberInput>,
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
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    if let Err(e) = require_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    match member_repo.add(org_id, input.user_id, input.role).await {
        Ok(member) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    org_id,
                    Some(session.user_id),
                    None,
                    "organization.member_add",
                    "organization_member",
                    Some(&input.user_id.to_string()),
                    None,
                    None,
                    json!({ "target_user_id": input.user_id, "role": input.role }),
                )
                .await;

            (StatusCode::CREATED, Json(json!(member))).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn update_member_role_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path((org_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<UpdateMemberRoleInput>,
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
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    if let Err(e) = require_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    match member_repo
        .update_role(org_id, target_user_id, input.role)
        .await
    {
        Ok(()) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    org_id,
                    Some(session.user_id),
                    None,
                    "organization.member_role_update",
                    "organization_member",
                    Some(&target_user_id.to_string()),
                    None,
                    None,
                    json!({ "target_user_id": target_user_id, "new_role": input.role }),
                )
                .await;

            (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn remove_member_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path((org_id, target_user_id)): Path<(Uuid, Uuid)>,
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
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied: not a member of this organization" })),
            )
                .into_response();
        }
    };

    if let Err(e) = require_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    match member_repo.remove(org_id, target_user_id).await {
        Ok(()) => {
            let audit_repo = AuditLogsRepository::new(pool);
            let _ = audit_repo
                .record(
                    org_id,
                    Some(session.user_id),
                    None,
                    "organization.member_remove",
                    "organization_member",
                    Some(&target_user_id.to_string()),
                    None,
                    None,
                    json!({ "target_user_id": target_user_id }),
                )
                .await;

            (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
