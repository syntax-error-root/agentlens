#![forbid(unsafe_code)]

//! Alert rules, alert events, and notification channel management endpoints with RBAC and AES encryption.

use std::str::FromStr;
use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{
    AlertEventsRepository, AlertRulesRepository, NotificationChannelsRepository,
    OrganizationMembersRepository, ProjectsRepository,
};
use agentlens_domain::{
    AlertMetric, AlertOperator, AlertSeverity, NotificationChannel, NotificationChannelType,
};
use agentlens_notifications::{
    decrypt_channel_config, encrypt_channel_config, mask_channel_config, send_test_notification,
};
use agentlens_organizations::require_admin_or_owner;

#[derive(Debug, Deserialize)]
pub struct CreateAlertRuleInput {
    pub environment_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub metric: String,
    pub operator: String,
    pub threshold: Decimal,
    pub window_seconds: i32,
    pub min_sample_count: Option<i32>,
    pub group_by: Option<serde_json::Value>,
    pub filters: Option<serde_json::Value>,
    pub cooldown_seconds: Option<i32>,
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertRuleInput {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub enabled: Option<bool>,
    pub metric: Option<String>,
    pub operator: Option<String>,
    pub threshold: Option<Decimal>,
    pub window_seconds: Option<i32>,
    pub min_sample_count: Option<i32>,
    pub cooldown_seconds: Option<i32>,
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListAlertEventsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationChannelInput {
    pub channel_type: String,
    pub name: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationChannelInput {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SafeNotificationChannelResponse {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Option<Uuid>,
    pub channel_type: NotificationChannelType,
    pub name: String,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

fn to_safe_channel_response(
    channel: NotificationChannel,
    encryption_key: &[u8],
) -> SafeNotificationChannelResponse {
    let config_json = match decrypt_channel_config(encryption_key, &channel.encrypted_config) {
        Ok(decrypted) => serde_json::from_str(&decrypted).unwrap_or_else(|_| json!({})),
        Err(_) => json!({ "error": "decryption_failed" }),
    };

    let masked = mask_channel_config(channel.channel_type, &config_json);

    SafeNotificationChannelResponse {
        id: channel.id,
        organization_id: channel.organization_id,
        project_id: channel.project_id,
        channel_type: channel.channel_type,
        name: channel.name,
        config: masked,
        enabled: channel.enabled,
        created_at: channel.created_at,
        updated_at: channel.updated_at,
    }
}

// ---------------------------------------------------------------------------
// Alert Rules Handlers
// ---------------------------------------------------------------------------

pub async fn list_alert_rules_handler(
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
                .into_response()
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
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
            Json(json!({ "error": "Access denied to project" })),
        )
            .into_response();
    }

    let rules_repo = AlertRulesRepository::new(pool);
    match rules_repo.list_by_project(project_id).await {
        Ok(rules) => Json(rules).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn create_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateAlertRuleInput>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
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
                .into_response()
        }
    };

    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let metric = match AlertMetric::from_str(&input.metric) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid metric: {e}") })),
            )
                .into_response()
        }
    };

    let operator = match AlertOperator::from_str(&input.operator) {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid operator: {e}") })),
            )
                .into_response()
        }
    };

    let s = input.severity.as_deref().unwrap_or("warning");
    let severity = match AlertSeverity::from_str(s) {
        Ok(sev) => sev,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid severity: {e}") })),
            )
                .into_response()
        }
    };

    let rules_repo = AlertRulesRepository::new(pool);
    match rules_repo
        .create(
            project.organization_id,
            project_id,
            input.environment_id,
            &input.name,
            input.description.as_deref(),
            input.enabled.unwrap_or(true),
            metric,
            operator,
            input.threshold,
            input.window_seconds.max(60),
            input.min_sample_count.unwrap_or(1).max(1),
            input.group_by.unwrap_or_else(|| json!([])),
            input.filters.unwrap_or_else(|| json!({})),
            input.cooldown_seconds.unwrap_or(300).max(0),
            severity,
            Some(session.user_id),
        )
        .await
    {
        Ok(rule) => (StatusCode::CREATED, Json(rule)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(rule_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    let row = match sqlx::query(
        "SELECT id, organization_id, project_id, environment_id, name, description, enabled, metric, operator, threshold, window_seconds, min_sample_count, group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at FROM alert_rules WHERE id = $1",
    )
    .bind(rule_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Rule not found" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    };

    use sqlx::Row;
    let org_id: Uuid = row.get("organization_id");
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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let project_id: Uuid = row.get("project_id");
    let rules_repo = AlertRulesRepository::new(pool);
    match rules_repo.get_by_id(project_id, rule_id).await {
        Ok(Some(rule)) => Json(rule).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn update_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(rule_id): Path<Uuid>,
    Json(input): Json<UpdateAlertRuleInput>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row =
        match sqlx::query("SELECT organization_id, project_id FROM alert_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "Rule not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
                    .into_response()
            }
        };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Uuid = meta_row.get("project_id");

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
            )
                .into_response()
        }
    };
    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let metric = input
        .metric
        .as_deref()
        .map(AlertMetric::from_str)
        .transpose()
        .unwrap_or(None);
    let operator = input
        .operator
        .as_deref()
        .map(AlertOperator::from_str)
        .transpose()
        .unwrap_or(None);
    let severity = input
        .severity
        .as_deref()
        .map(AlertSeverity::from_str)
        .transpose()
        .unwrap_or(None);

    let desc_ref = input.description.as_ref().map(|d| d.as_deref());

    let rules_repo = AlertRulesRepository::new(pool);
    match rules_repo
        .update(
            project_id,
            rule_id,
            input.name.as_deref(),
            desc_ref,
            input.enabled,
            metric,
            operator,
            input.threshold,
            input.window_seconds,
            input.min_sample_count,
            input.cooldown_seconds,
            severity,
        )
        .await
    {
        Ok(Some(rule)) => Json(rule).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(rule_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row =
        match sqlx::query("SELECT organization_id, project_id FROM alert_rules WHERE id = $1")
            .bind(rule_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "Rule not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
                    .into_response()
            }
        };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Uuid = meta_row.get("project_id");

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
            )
                .into_response()
        }
    };
    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let rules_repo = AlertRulesRepository::new(pool);
    match rules_repo.delete(project_id, rule_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Alert Events Handlers
// ---------------------------------------------------------------------------

pub async fn list_alert_events_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListAlertEventsQuery>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
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

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let events_repo = AlertEventsRepository::new(pool);
    match events_repo
        .list_by_project(project_id, query.status.as_deref(), limit, offset)
        .await
    {
        Ok(events) => Json(events).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_alert_event_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(event_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row =
        match sqlx::query("SELECT organization_id, project_id FROM alert_events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "Event not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
                    .into_response()
            }
        };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Uuid = meta_row.get("project_id");

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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let events_repo = AlertEventsRepository::new(pool);
    match events_repo.get_by_id(project_id, event_id).await {
        Ok(Some(event)) => Json(event).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Event not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn acknowledge_alert_event_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(event_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row =
        match sqlx::query("SELECT organization_id, project_id FROM alert_events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(r)) => r,
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": "Event not found" })),
                )
                    .into_response()
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e.to_string() })),
                )
                    .into_response()
            }
        };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Uuid = meta_row.get("project_id");

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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let events_repo = AlertEventsRepository::new(pool);
    match events_repo
        .acknowledge(project_id, event_id, Some(session.user_id))
        .await
    {
        Ok(Some(event)) => Json(event).into_response(),
        Ok(None) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Event not found or is not in firing state" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Notification Channels Handlers
// ---------------------------------------------------------------------------

pub async fn list_notification_channels_handler(
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
                .into_response()
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
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

    let channels_repo = NotificationChannelsRepository::new(pool);
    match channels_repo.list_by_project(project_id).await {
        Ok(channels) => {
            let enc_key = state.config.security.app_encryption_key.as_bytes();
            let safe: Vec<SafeNotificationChannelResponse> = channels
                .into_iter()
                .map(|c| to_safe_channel_response(c, enc_key))
                .collect();
            Json(safe).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn create_notification_channel_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateNotificationChannelInput>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    let proj_repo = ProjectsRepository::new(pool);
    let project = match proj_repo.get_by_id(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Project not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
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
                .into_response()
        }
    };
    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let channel_type = match NotificationChannelType::from_str(&input.channel_type) {
        Ok(ct) => ct,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    };

    let config_str = input.config.to_string();
    let enc_key = state.config.security.app_encryption_key.as_bytes();
    let encrypted_config = match encrypt_channel_config(enc_key, &config_str) {
        Ok(enc) => enc,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let channels_repo = NotificationChannelsRepository::new(pool);
    match channels_repo
        .create(
            project.organization_id,
            Some(project_id),
            channel_type,
            &input.name,
            &encrypted_config,
            "v1",
        )
        .await
    {
        Ok(channel) => {
            let safe = to_safe_channel_response(channel, enc_key);
            (StatusCode::CREATED, Json(safe)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn get_notification_channel_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(channel_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row = match sqlx::query(
        "SELECT organization_id, project_id FROM notification_channels WHERE id = $1",
    )
    .bind(channel_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Channel not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Option<Uuid> = meta_row.get("project_id");

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
            Json(json!({ "error": "Access denied" })),
        )
            .into_response();
    }

    let channels_repo = NotificationChannelsRepository::new(pool);
    let channel = match channels_repo
        .get_by_id(project_id.unwrap_or(Uuid::nil()), channel_id)
        .await
    {
        Ok(Some(c)) => c,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Channel not found" })),
            )
                .into_response()
        }
    };

    let enc_key = state.config.security.app_encryption_key.as_bytes();
    let safe = to_safe_channel_response(channel, enc_key);
    Json(safe).into_response()
}

pub async fn delete_notification_channel_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(channel_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row = match sqlx::query(
        "SELECT organization_id, project_id FROM notification_channels WHERE id = $1",
    )
    .bind(channel_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Channel not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Option<Uuid> = meta_row.get("project_id");

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
            )
                .into_response()
        }
    };
    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let channels_repo = NotificationChannelsRepository::new(pool);
    match channels_repo
        .delete(project_id.unwrap_or(Uuid::nil()), channel_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Channel not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn test_notification_channel_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(channel_id): Path<Uuid>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Database unavailable" })),
            )
                .into_response()
        }
    };

    use sqlx::Row;
    let meta_row = match sqlx::query(
        "SELECT organization_id, project_id FROM notification_channels WHERE id = $1",
    )
    .bind(channel_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Channel not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let org_id: Uuid = meta_row.get("organization_id");
    let project_id: Option<Uuid> = meta_row.get("project_id");

    let member_repo = OrganizationMembersRepository::new(pool);
    let role = match member_repo.get_role(org_id, session.user_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Access denied" })),
            )
                .into_response()
        }
    };
    if let Err(e) = require_admin_or_owner(role) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    let channels_repo = NotificationChannelsRepository::new(pool);
    let channel = match channels_repo
        .get_by_id(project_id.unwrap_or(Uuid::nil()), channel_id)
        .await
    {
        Ok(Some(c)) => c,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Channel not found" })),
            )
                .into_response()
        }
    };

    let enc_key = state.config.security.app_encryption_key.as_bytes();
    let allow_loopback =
        state.config.environment == "development" || state.config.environment == "test";

    match send_test_notification(&channel, enc_key, allow_loopback).await {
        Ok(()) => {
            Json(json!({ "success": true, "message": "Test notification delivered successfully" }))
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "success": false, "error": format!("Test delivery failed: {e}") })),
        )
            .into_response(),
    }
}
