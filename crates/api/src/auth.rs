#![forbid(unsafe_code)]

//! Control plane authentication endpoints (bootstrap, login, logout, me, OIDC).

use crate::AppState;
use agentlens_auth::{
    build_clear_cookie, build_session_cookie, create_session_token, generate_oidc_param,
    generate_pkce, hash_password, verify_password, MockOidcProvider, OidcProvider, SessionPayload,
};
use agentlens_database::{
    BootstrapRepository, DatabaseError, OrganizationsRepository, UsersRepository,
};
use agentlens_domain::{OrgRole, Organization};
use axum::{
    extract::{Extension, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct BootstrapInput {
    pub email: String,
    pub password: String,
    pub organization_name: String,
    pub organization_slug: String,
    #[serde(default = "default_project_name")]
    pub default_project_name: String,
}

fn default_project_name() -> String {
    "Default Project".to_string()
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct OrgWithRole {
    pub organization: Organization,
    pub role: OrgRole,
}

pub async fn bootstrap_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<BootstrapInput>,
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

    if input.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be at least 8 characters" })),
        )
            .into_response();
    }

    let password_hash = match hash_password(&input.password) {
        Ok(h) => h,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Hashing error: {e}") })),
            )
                .into_response();
        }
    };

    let ip = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok());

    let repo = BootstrapRepository::new(pool);
    match repo
        .execute(
            &input.email,
            &password_hash,
            &input.organization_name,
            &input.organization_slug,
            &input.default_project_name,
            ip,
            user_agent,
        )
        .await
    {
        Ok((user, org, project, envs)) => {
            let secret = state.config.security.session_secret.as_bytes();
            let is_prod = state.config.environment == "production";
            let (token, csrf_token) = match create_session_token(user.id, 86400 * 7, secret) {
                Ok(res) => res,
                Err(err) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Session error: {err}") })),
                    )
                        .into_response();
                }
            };

            let cookie = build_session_cookie(&token, 86400 * 7, is_prod);
            (
                StatusCode::CREATED,
                [(header::SET_COOKIE, cookie)],
                Json(json!({
                    "user": user,
                    "organization": org,
                    "project": project,
                    "environments": envs,
                    "csrf_token": csrf_token
                })),
            )
                .into_response()
        }
        Err(DatabaseError::BootstrapAlreadyCompleted) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Bootstrap already completed; control plane is locked" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<LoginInput>,
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

    let users_repo = UsersRepository::new(pool);
    let user = match users_repo.get_by_email(&input.email).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid email or password" })),
            )
                .into_response();
        }
    };

    if user.status != "active" {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "User account is inactive" })),
        )
            .into_response();
    }

    let valid_pw = match &user.password_hash {
        Some(hash) => verify_password(&input.password, hash).unwrap_or(false),
        None => false,
    };

    if !valid_pw {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid email or password" })),
        )
            .into_response();
    }

    let secret = state.config.security.session_secret.as_bytes();
    let is_prod = state.config.environment == "production";
    let (token, csrf_token) = match create_session_token(user.id, 86400 * 7, secret) {
        Ok(res) => res,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Session error: {err}") })),
            )
                .into_response();
        }
    };

    let orgs_repo = OrganizationsRepository::new(pool);
    let org_memberships = orgs_repo.list_for_user(user.id).await.unwrap_or_default();
    let orgs: Vec<OrgWithRole> = org_memberships
        .into_iter()
        .map(|(org, role)| OrgWithRole {
            organization: org,
            role,
        })
        .collect();

    let cookie = build_session_cookie(&token, 86400 * 7, is_prod);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({
            "user": user,
            "organizations": orgs,
            "csrf_token": csrf_token
        })),
    )
        .into_response()
}

pub async fn logout_handler(State(state): State<Arc<AppState>>) -> Response {
    let is_prod = state.config.environment == "production";
    let cookie = build_clear_cookie(is_prod);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({ "status": "ok" })),
    )
        .into_response()
}

pub async fn me_handler(
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

    let users_repo = UsersRepository::new(pool);
    let user = match users_repo.get_by_id(session.user_id).await {
        Ok(Some(u)) => u,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            )
                .into_response();
        }
    };

    let orgs_repo = OrganizationsRepository::new(pool);
    let org_memberships = orgs_repo.list_for_user(user.id).await.unwrap_or_default();
    let orgs: Vec<OrgWithRole> = org_memberships
        .into_iter()
        .map(|(org, role)| OrgWithRole {
            organization: org,
            role,
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "user": user,
            "organizations": orgs,
            "csrf_token": session.csrf_token
        })),
    )
        .into_response()
}

pub async fn oidc_start_handler() -> Response {
    let state = generate_oidc_param();
    let nonce = generate_oidc_param();
    let pkce = generate_pkce();
    let provider = MockOidcProvider;
    let auth_url = provider.authorization_url(&state, &nonce, &pkce.challenge);

    (
        StatusCode::OK,
        Json(json!({
            "authorization_url": auth_url,
            "state": state,
            "nonce": nonce,
            "code_verifier": pkce.verifier
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackInput {
    pub code: String,
    pub nonce: String,
    pub code_verifier: String,
    pub state: Option<String>,
}

pub async fn oidc_callback_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<OidcCallbackInput>,
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

    if let Some(s) = &input.state {
        if s.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "OIDC state parameter cannot be empty" })),
            )
                .into_response();
        }
    }

    let provider = MockOidcProvider;
    let tokens = match provider
        .exchange_code(&input.code, &input.code_verifier)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Token exchange error: {e}") })),
            )
                .into_response();
        }
    };

    let claims = match provider
        .verify_id_token(&tokens.id_token, &input.nonce)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("ID token verification error: {e}") })),
            )
                .into_response();
        }
    };

    let users_repo = UsersRepository::new(pool);
    // 1. Try matching by existing OIDC (issuer, subject)
    let user = match users_repo
        .get_by_oidc(&claims.issuer, &claims.subject)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            // 2. Fall back to matching by verified email address
            match users_repo.get_by_email(&claims.email).await {
                Ok(Some(existing_user)) => {
                    // Link OIDC identity to existing user account
                    match users_repo
                        .link_oidc(existing_user.id, &claims.issuer, &claims.subject)
                        .await
                    {
                        Ok(linked) => linked,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({ "error": format!("Failed to link OIDC identity: {e}") })),
                            )
                                .into_response();
                        }
                    }
                }
                Ok(None) => {
                    // 3. Automatically provision new user and link OIDC identity
                    match users_repo
                        .create(&claims.email, None, claims.display_name.as_deref())
                        .await
                    {
                        Ok(new_user) => {
                            match users_repo
                                .link_oidc(new_user.id, &claims.issuer, &claims.subject)
                                .await
                            {
                                Ok(linked) => linked,
                                Err(e) => {
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({ "error": format!("Failed to link OIDC identity: {e}") })),
                                    )
                                        .into_response();
                                }
                            }
                        }
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({ "error": format!("User creation error: {e}") })),
                            )
                                .into_response();
                        }
                    }
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Database error: {e}") })),
                    )
                        .into_response();
                }
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {e}") })),
            )
                .into_response();
        }
    };

    let secret = state.config.security.session_secret.as_bytes();
    let is_prod = state.config.environment == "production";
    let (token, csrf_token) = match create_session_token(user.id, 86400 * 7, secret) {
        Ok(res) => res,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Session error: {err}") })),
            )
                .into_response();
        }
    };

    let cookie = build_session_cookie(&token, 86400 * 7, is_prod);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(json!({
            "user": user,
            "csrf_token": csrf_token
        })),
    )
        .into_response()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapStatusResponse {
    pub can_bootstrap: bool,
}

pub async fn bootstrap_status_handler(State(state): State<Arc<AppState>>) -> Response {
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

    let repo = BootstrapRepository::new(pool);
    match repo.can_bootstrap().await {
        Ok(can_bootstrap) => Json(BootstrapStatusResponse { can_bootstrap }).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to check bootstrap status: {e}") })),
        )
            .into_response(),
    }
}
