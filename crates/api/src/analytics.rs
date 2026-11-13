#![forbid(unsafe_code)]

//! Analytics and model pricing endpoints with strict tenant isolation.

use crate::AppState;
use agentlens_analytics::AnalyticsService;
use agentlens_auth::SessionPayload;
use agentlens_database::{
    ModelPricingRepository, OrganizationMembersRepository, ProjectsRepository,
};
use agentlens_domain::{AnalyticsFilters, ModelPricing};
use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AnalyticsQueryParams {
    pub project_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

async fn authorize_project_access(
    state: &AppState,
    session: &SessionPayload,
    project_id: Uuid,
) -> Result<(), Response> {
    let pool = state.db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Database unavailable" })),
        )
            .into_response()
    })?;

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Project {project_id} not found") })),
            )
                .into_response());
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response());
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
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Access denied to project organization" })),
        )
            .into_response());
    }

    Ok(())
}

/// GET /api/v1/analytics/overview
pub async fn get_overview_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_overview(&filters).await {
        Ok(overview) => Json(overview).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch overview: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/tokens
pub async fn get_tokens_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_tokens(&filters).await {
        Ok(tokens) => Json(tokens).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch tokens: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/costs
pub async fn get_costs_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_costs(&filters).await {
        Ok(costs) => Json(costs).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch costs: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/models
pub async fn get_models_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_models(&filters).await {
        Ok(models) => Json(models).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch models: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/agents
pub async fn get_agents_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_agents(&filters).await {
        Ok(agents) => Json(agents).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch agents: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/tools
pub async fn get_tools_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_tools(&filters).await {
        Ok(tools) => Json(tools).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch tools: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/latency
pub async fn get_latency_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_latency(&filters).await {
        Ok(latency) => Json(latency).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch latency: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/analytics/errors
pub async fn get_errors_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Query(query): Query<AnalyticsQueryParams>,
) -> Response {
    if let Err(res) = authorize_project_access(&state, &session, query.project_id).await {
        return res;
    }

    let pool = state.db.clone().unwrap();
    let service = AnalyticsService::new(pool);
    let filters = AnalyticsFilters {
        project_id: query.project_id,
        environment_id: query.environment_id,
        from: query.from,
        to: query.to,
    };

    match service.get_errors(&filters).await {
        Ok(errors) => Json(errors).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch errors: {e}") })),
        )
            .into_response(),
    }
}

/// GET /api/v1/pricing
pub async fn list_pricing_handler(
    State(state): State<Arc<AppState>>,
    Extension(_session): Extension<SessionPayload>,
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

    let repo = ModelPricingRepository::new(pool);
    match repo.list_all().await {
        Ok(cards) => Json(cards).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list pricing cards: {e}") })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreatePricingCardPayload {
    pub provider: String,
    pub model_pattern: String,
    pub match_type: String,
    pub currency: Option<String>,
    pub input_per_million: Option<Decimal>,
    pub output_per_million: Option<Decimal>,
    pub cached_input_per_million: Option<Decimal>,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    pub source_checked_at: Option<DateTime<Utc>>,
    pub version: String,
}

/// POST /api/v1/pricing
pub async fn create_pricing_handler(
    State(state): State<Arc<AppState>>,
    Extension(_session): Extension<SessionPayload>,
    Json(payload): Json<CreatePricingCardPayload>,
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

    let card = ModelPricing {
        id: Uuid::new_v4(),
        provider: payload.provider.trim().to_lowercase(),
        model_pattern: payload.model_pattern.trim().to_string(),
        match_type: payload.match_type.trim().to_string(),
        currency: payload.currency.unwrap_or_else(|| "USD".to_string()),
        input_per_million: payload.input_per_million,
        output_per_million: payload.output_per_million,
        cached_input_per_million: payload.cached_input_per_million,
        effective_from: payload.effective_from,
        effective_until: payload.effective_until,
        source_url: payload.source_url,
        source_checked_at: payload.source_checked_at,
        version: payload.version,
        created_at: Utc::now(),
    };

    let repo = ModelPricingRepository::new(pool);
    match repo.create(&card).await {
        Ok(created) => (StatusCode::CREATED, Json(created)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create pricing card: {e}") })),
        )
            .into_response(),
    }
}
