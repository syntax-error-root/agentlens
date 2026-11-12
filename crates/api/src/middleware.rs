#![forbid(unsafe_code)]

//! Control plane authentication, CSRF validation, and route isolation middleware.

use agentlens_auth::{verify_csrf_token, verify_session_token, SessionPayload};
use axum::{
    extract::Request,
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub const SESSION_COOKIE_NAME: &str = "agentlens_session";

/// Extracts session token string from `Cookie` request header.
pub fn extract_cookie_token(req: &Request) -> Option<String> {
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let trimmed = pair.trim();
        if let Some(val) = trimmed.strip_prefix("agentlens_session=") {
            return Some(val.to_string());
        }
    }
    None
}

/// Control-plane guard middleware:
/// 1. Rejects any ingestion API keys (`agl_live_*`) from accessing control plane (`/api/v1/*`).
/// 2. Validates session cookie authenticity and expiration.
/// 3. Validates CSRF token for mutating HTTP verbs (POST, PATCH, PUT, DELETE).
pub async fn control_plane_auth_middleware(secret: &[u8], req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // 1. Immediately reject ingestion API keys on control plane routes
    if let Some(auth_val) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if auth_val.contains("agl_live_") {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Ingestion API keys cannot access control-plane routes" })),
            )
                .into_response();
        }
    }

    // Public auth routes that do not require prior session
    if path == "/api/v1/auth/bootstrap"
        || path == "/api/v1/auth/login"
        || path.starts_with("/api/v1/auth/oidc")
    {
        return next.run(req).await;
    }

    // 2. Extract and verify session cookie
    let token_str = match extract_cookie_token(&req) {
        Some(token) => token,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Authentication required: missing session cookie" })),
            )
                .into_response();
        }
    };

    let session_payload = match verify_session_token(&token_str, secret) {
        Ok(payload) => payload,
        Err(err) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": format!("Invalid session: {err}") })),
            )
                .into_response();
        }
    };

    // 3. CSRF validation for mutating methods (exempting login/logout/bootstrap)
    let is_mutating = matches!(
        method,
        Method::POST | Method::PATCH | Method::PUT | Method::DELETE
    );

    if is_mutating && path != "/api/v1/auth/logout" {
        let csrf_header = req
            .headers()
            .get("x-csrf-token")
            .and_then(|h| h.to_str().ok());

        match csrf_header {
            Some(submitted_csrf)
                if verify_csrf_token(&session_payload.csrf_token, submitted_csrf) =>
            {
                // CSRF passed
            }
            _ => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "CSRF validation failed: invalid or missing X-CSRF-Token header" })),
                )
                    .into_response();
            }
        }
    }

    let mut req = req;
    req.extensions_mut().insert(session_payload);
    next.run(req).await
}

/// Helper to get authenticated session from request extensions.
pub fn get_session_from_req(req: &Request) -> Option<SessionPayload> {
    req.extensions().get::<SessionPayload>().cloned()
}
