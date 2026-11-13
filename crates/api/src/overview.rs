#![forbid(unsafe_code)]

//! Project overview metrics aggregation endpoint with strict tenant isolation.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{OrganizationMembersRepository, ProjectsRepository, TracesRepository};
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
pub struct GetOverviewQuery {
    pub environment_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

pub async fn get_project_overview_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<GetOverviewQuery>,
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

    let traces_repo = TracesRepository::new(pool);
    match traces_repo
        .get_project_overview(project_id, query.environment_id, query.from, query.to)
        .await
    {
        Ok(metrics) => Json(metrics).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to compute project overview: {e}") })),
        )
            .into_response(),
    }
}
