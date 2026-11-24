#![forbid(unsafe_code)]

//! In-dashboard Agent Chat endpoints with SSRF-hardened OpenAI-compatible streaming proxy.

use crate::AppState;
use agentlens_auth::SessionPayload;
use agentlens_database::{AgentsRepository, ChatRepository, OrganizationMembersRepository};
use agentlens_notifications::is_ip_blocked;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::lookup_host;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateChatSessionInput {
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListChatSessionsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ListChatMessagesQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SendChatMessageInput {
    pub content: String,
    pub session_id: Option<Uuid>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    pub stream: bool,
}

/// Validates target URL against SSRF policy, respecting allow_local_agent_endpoints flag.
async fn validate_target_url(url_str: &str, allow_local: bool) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(url_str).map_err(|e| format!("Invalid URL: {e}"))?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Disallowed URL scheme: {other}. Only http/https supported")),
    }

    let host = url
        .host_str()
        .ok_or_else(|| "URL missing host".to_string())?;
    let port = url.port_or_known_default().unwrap_or(80);
    let host_port = format!("{host}:{port}");

    let addrs: Vec<SocketAddr> = lookup_host(&host_port)
        .await
        .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?
        .collect();

    if addrs.is_empty() {
        return Err("No IP addresses found for agent endpoint host".into());
    }

    for addr in addrs {
        let ip = addr.ip();
        if is_ip_blocked(ip) {
            let is_loopback = ip.is_loopback() || ip.to_string() == "127.0.0.1" || ip.to_string() == "::1";
            if allow_local && is_loopback {
                continue;
            }
            return Err(format!("Connection blocked to disallowed address: {ip}"));
        }
    }

    Ok(url)
}

/// GET /api/v1/agents/{agent_id}/chat/sessions
pub async fn list_chat_sessions_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<ListChatSessionsQuery>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "Database unavailable" }))).into_response(),
    };

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Agent not found" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo.get_role(agent.organization_id, session.user_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "Access denied" }))).into_response();
    }

    let chat_repo = ChatRepository::new(pool);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    match chat_repo.list_sessions(agent.project_id, agent.id, limit).await {
        Ok(sessions) => Json(sessions).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    }
}

/// POST /api/v1/agents/{agent_id}/chat/sessions
pub async fn create_chat_session_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<CreateChatSessionInput>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "Database unavailable" }))).into_response(),
    };

    let agent_repo = AgentsRepository::new(pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Agent not found" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo.get_role(agent.organization_id, session.user_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "Access denied" }))).into_response();
    }

    let chat_repo = ChatRepository::new(pool);
    let title = input.title.as_deref().unwrap_or("New Conversation");
    match chat_repo
        .create_session(agent.organization_id, agent.project_id, agent.id, Some(session.user_id), title)
        .await
    {
        Ok(sess) => (StatusCode::CREATED, Json(sess)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    }
}

/// GET /api/v1/chat/sessions/{session_id}/messages
pub async fn list_chat_messages_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ListChatMessagesQuery>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "Database unavailable" }))).into_response(),
    };

    let chat_repo = ChatRepository::new(pool);
    let limit = query.limit.unwrap_or(100).clamp(1, 500);

    // Fetch session first to verify project access
    let chat_session_row = match sqlx::query(
        r#"
        SELECT organization_id, project_id FROM chat_sessions WHERE id = $1
        "#,
    )
    .bind(session_id)
    .map(|r: sqlx::postgres::PgRow| {
        use sqlx::Row;
        (r.get::<Uuid, _>("organization_id"), r.get::<Uuid, _>("project_id"))
    })
    .fetch_optional(pool)
    .await
    {
        Ok(Some((org_id, proj_id))) => (org_id, proj_id),
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Chat session not found" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    };

    let member_repo = OrganizationMembersRepository::new(pool);
    if member_repo.get_role(chat_session_row.0, session.user_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "Access denied" }))).into_response();
    }

    match chat_repo.list_messages(chat_session_row.1, session_id, limit).await {
        Ok(messages) => Json(messages).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    }
}

/// POST /api/v1/agents/{agent_id}/chat - Stream interaction via SSE
pub async fn send_chat_message_handler(
    State(state): State<Arc<AppState>>,
    Extension(session): Extension<SessionPayload>,
    Path(agent_id): Path<Uuid>,
    Json(input): Json<SendChatMessageInput>,
) -> Response {
    let pool = match &state.db {
        Some(p) => p.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "Database unavailable" }))).into_response(),
    };

    let user_prompt = input.content.trim().to_string();
    if user_prompt.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Message content cannot be empty" }))).into_response();
    }

    let agent_repo = AgentsRepository::new(&pool);
    let agent = match agent_repo.get_by_id_global(agent_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Agent not found" }))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("{e}") }))).into_response(),
    };

    let member_repo = OrganizationMembersRepository::new(&pool);
    if member_repo.get_role(agent.organization_id, session.user_id).await.ok().flatten().is_none() {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "Access denied to agent organization" }))).into_response();
    }

    let endpoint_url = match &agent.endpoint_url {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Agent has no endpoint URL configured" }))).into_response(),
    };

    let allow_local = state.config.server.allow_local_agent_endpoints;
    let validated_url = match validate_target_url(endpoint_url, allow_local).await {
        Ok(u) => u,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": format!("SSRF validation failed: {e}") }))).into_response(),
    };

    let chat_repo = ChatRepository::new(&pool);
    let chat_session = match input.session_id {
        Some(sid) => match chat_repo.get_session(agent.project_id, sid).await {
            Ok(Some(s)) => s,
            _ => return (StatusCode::NOT_FOUND, Json(json!({ "error": "Specified chat session not found" }))).into_response(),
        },
        None => {
            let auto_title = if user_prompt.chars().count() > 30 {
                let s: String = user_prompt.chars().take(30).collect();
                format!("{s}...")
            } else {
                user_prompt.clone()
            };
            match chat_repo.create_session(agent.organization_id, agent.project_id, agent.id, Some(session.user_id), &auto_title).await {
                Ok(s) => s,
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed creating chat session: {e}") }))).into_response(),
            }
        }
    };

    // Record user message
    if let Err(e) = chat_repo
        .append_message(agent.organization_id, agent.project_id, chat_session.id, "user", &user_prompt, None, None)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed to record message: {e}") }))).into_response();
    }

    // Load conversation history for OpenAI-compatible context
    let history = chat_repo.list_messages(agent.project_id, chat_session.id, 50).await.unwrap_or_default();
    let open_ai_messages: Vec<OpenAiMessage> = history
        .into_iter()
        .map(|m| OpenAiMessage {
            role: m.role,
            content: m.content,
        })
        .collect();

    let target_model = input
        .model
        .or(agent.default_model)
        .unwrap_or_else(|| "gpt-4o-mini".to_string());

    let payload = OpenAiChatRequest {
        model: target_model,
        messages: open_ai_messages,
        stream: true,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default();

    let mut req_builder = client.post(validated_url.as_str()).json(&payload);

    if let Some(token) = &agent.auth_token {
        if !token.trim().is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", token.trim()));
        }
    }

    let agent_res = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(json!({ "error": format!("Failed to connect to agent endpoint: {e}") }))).into_response(),
    };

    if !agent_res.status().is_success() {
        let err_text = agent_res.text().await.unwrap_or_default();
        return (StatusCode::BAD_GATEWAY, Json(json!({ "error": format!("Agent endpoint returned error: {err_text}") }))).into_response();
    }

    let org_id = agent.organization_id;
    let proj_id = agent.project_id;
    let chat_sess_id = chat_session.id;

    // Stream SSE bytes to client while accumulating assistant response for persistence
    let byte_stream = agent_res.bytes_stream();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(100);

    tokio::spawn(async move {
        let mut full_assistant_content = String::new();
        let mut stream = byte_stream;

        // Emit initial session_id metadata event
        let _ = tx
            .send(Ok(Event::default().event("meta").data(json!({ "session_id": chat_sess_id }).to_string())))
            .await;

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        let trimmed = line.trim();
                        if let Some(data_str) = trimmed.strip_prefix("data:") {
                            let data_clean = data_str.trim();
                            if data_clean == "[DONE]" {
                                continue;
                            }
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_clean) {
                                if let Some(content_chunk) = v
                                    .pointer("/choices/0/delta/content")
                                    .and_then(|c| c.as_str())
                                {
                                    full_assistant_content.push_str(content_chunk);
                                    let _ = tx
                                        .send(Ok(Event::default()
                                            .event("token")
                                            .data(json!({ "token": content_chunk }).to_string())))
                                        .await;
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        // Persist accumulated assistant response
        if !full_assistant_content.is_empty() {
            let chat_repo = ChatRepository::new(&pool);
            let _ = chat_repo
                .append_message(org_id, proj_id, chat_sess_id, "assistant", &full_assistant_content, None, None)
                .await;
        }

        let _ = tx
            .send(Ok(Event::default().event("done").data(json!({ "status": "completed" }).to_string())))
            .await;
    });

    let sse_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Sse::new(sse_stream).keep_alive(KeepAlive::default()).into_response()
}
