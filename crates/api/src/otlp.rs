#![forbid(unsafe_code)]

//! OTLP/HTTP trace receiver endpoint (`POST /v1/traces`).

use crate::AppState;
use agentlens_auth::verify_api_key_hash;
use agentlens_database::{ApiKeysRepository, ProjectsRepository};
use agentlens_privacy::RedactionRule;
use agentlens_telemetry::TelemetryError;
use agentlens_traces::{IngestionContext, TraceIngestionError, TraceIngestionService};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use prost::Message;
use serde_json::json;
use std::sync::Arc;

pub async fn otlp_traces_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 1. Authenticate Ingestion API Key format & presence
    let raw_key = match extract_api_key(&headers) {
        Some(k) => k,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing or invalid ingestion API key" })),
            )
                .into_response();
        }
    };

    if !raw_key.starts_with("agl_live_") || raw_key.len() < 17 {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid API key format" })),
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

    let prefix = format!("agl_live_{}", &raw_key["agl_live_".len()..17]);
    let keys_repo = ApiKeysRepository::new(pool);
    let candidate_keys = match keys_repo.find_by_prefix(&prefix).await {
        Ok(keys) => keys,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let matched_key = candidate_keys
        .into_iter()
        .find(|k| verify_api_key_hash(&k.key_hash, &raw_key));

    let api_key = match matched_key {
        Some(k) => k,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid API key" })),
            )
                .into_response();
        }
    };

    if api_key.revoked_at.is_some() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "API key has been revoked" })),
        )
            .into_response();
    }

    if let Some(exp) = api_key.expires_at {
        if chrono::Utc::now() > exp {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "API key has expired" })),
            )
                .into_response();
        }
    }

    // Touch last used asynchronously in background
    let pool_clone = pool.clone();
    let key_id = api_key.id;
    tokio::spawn(async move {
        let repo = ApiKeysRepository::new(&pool_clone);
        let _ = repo.touch_last_used(key_id).await;
    });

    // 2. Fetch Project Configuration (content capture & redaction rules)
    let projects_repo = ProjectsRepository::new(pool);
    let project = match projects_repo.get_by_id(api_key.project_id).await {
        Ok(Some(p)) => p,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Associated project not found" })),
            )
                .into_response();
        }
    };

    let redaction_rules: Vec<RedactionRule> =
        serde_json::from_value(project.redaction_rules).unwrap_or_default();

    let ctx = IngestionContext {
        organization_id: api_key.organization_id,
        project_id: api_key.project_id,
        environment_id: api_key.environment_id,
        capture_content: project.capture_content_default,
        redaction_rules,
    };

    // 3. Decompression & Ingestion
    let content_encoding = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|h| h.to_str().ok());
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok());

    let service = TraceIngestionService::new(pool);
    let resp = match service
        .ingest_otlp_payload(&body, content_encoding, content_type, &ctx)
        .await
    {
        Ok(r) => r,
        Err(TraceIngestionError::Telemetry(TelemetryError::PayloadTooLarge(msg))) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, Json(json!({ "error": msg }))).into_response();
        }
        Err(TraceIngestionError::Telemetry(err)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Malformed OTLP payload: {err}") })),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Ingestion error: {err}") })),
            )
                .into_response();
        }
    };

    // 4. Content negotiation for response
    let is_json = content_type.is_some_and(|ct| ct.contains("application/json"));
    if is_json {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            Json(json!(resp)),
        )
            .into_response()
    } else {
        let mut proto_bytes = Vec::new();
        let _ = resp.encode(&mut proto_bytes);
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/x-protobuf")],
            proto_bytes,
        )
            .into_response()
    }
}

fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        let trimmed = auth.trim();
        if let Some(token) = trimmed.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }
    if let Some(key) = headers.get("x-agentlens-key").and_then(|h| h.to_str().ok()) {
        return Some(key.trim().to_string());
    }
    None
}
