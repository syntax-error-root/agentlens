#![forbid(unsafe_code)]

//! `agentlens-domain` crate.
//! Pure domain entities, value objects, and business rules without external I/O.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("Invalid TraceId: must be 32 lowercase hex characters and non-zero: {0}")]
    InvalidTraceId(String),
    #[error("Invalid SpanId: must be 16 lowercase hex characters and non-zero: {0}")]
    InvalidSpanId(String),
    #[error("Invalid enum value: {0}")]
    InvalidEnumValue(String),
}

// ---------------------------------------------------------------------------
// Value Objects
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TraceId(String);

impl TraceId {
    pub fn new(val: impl Into<String>) -> Result<Self, DomainError> {
        let s = val.into().to_ascii_lowercase();
        if s.len() != 32
            || s == "00000000000000000000000000000000"
            || !s.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(DomainError::InvalidTraceId(s));
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TraceId({})", self.0)
    }
}

impl FromStr for TraceId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for TraceId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TraceId> for String {
    fn from(id: TraceId) -> Self {
        id.0
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SpanId(String);

impl SpanId {
    pub fn new(val: impl Into<String>) -> Result<Self, DomainError> {
        let s = val.into().to_ascii_lowercase();
        if s.len() != 16 || s == "0000000000000000" || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidSpanId(s));
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpanId({})", self.0)
    }
}

impl FromStr for SpanId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for SpanId {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SpanId> for String {
    fn from(id: SpanId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpanType {
    Agent,
    Llm,
    Tool,
    Retrieval,
    Chain,
    Http,
    Database,
    #[default]
    Custom,
}

impl SpanType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Llm => "llm",
            Self::Tool => "tool",
            Self::Retrieval => "retrieval",
            Self::Chain => "chain",
            Self::Http => "http",
            Self::Database => "database",
            Self::Custom => "custom",
        }
    }
}

impl FromStr for SpanType {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "agent" => Ok(Self::Agent),
            "llm" => Ok(Self::Llm),
            "tool" => Ok(Self::Tool),
            "retrieval" => Ok(Self::Retrieval),
            "chain" => Ok(Self::Chain),
            "http" => Ok(Self::Http),
            "database" => Ok(Self::Database),
            "custom" => Ok(Self::Custom),
            _ => Err(DomainError::InvalidEnumValue(format!(
                "Invalid span_type: {s}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TraceStatus {
    #[default]
    Unset,
    Ok,
    Error,
}

impl TraceStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

impl FromStr for TraceStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "unset" => Ok(Self::Unset),
            "ok" => Ok(Self::Ok),
            "error" => Ok(Self::Error),
            _ => Err(DomainError::InvalidEnumValue(format!(
                "Invalid status: {s}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl OrgRole {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
            Self::Viewer => "viewer",
        }
    }

    #[must_use]
    pub fn can_manage_organization(&self) -> bool {
        matches!(self, Self::Owner)
    }

    #[must_use]
    pub fn can_manage_members(&self) -> bool {
        matches!(self, Self::Owner)
    }

    #[must_use]
    pub fn can_manage_projects(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    #[must_use]
    pub fn can_manage_api_keys(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    #[must_use]
    pub fn can_manage_alerts(&self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    #[must_use]
    pub fn can_mutate(&self) -> bool {
        !matches!(self, Self::Viewer)
    }
}

impl FromStr for OrgRole {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            "viewer" => Ok(Self::Viewer),
            _ => Err(DomainError::InvalidEnumValue(format!("Invalid role: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    #[default]
    ProviderReported,
    Estimated,
}

impl UsageSource {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProviderReported => "provider_reported",
            Self::Estimated => "estimated",
        }
    }
}

impl std::str::FromStr for UsageSource {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "provider_reported" => Ok(Self::ProviderReported),
            "estimated" => Ok(Self::Estimated),
            other => Err(DomainError::InvalidEnumValue(format!(
                "invalid usage source: '{other}', expected 'provider_reported' or 'estimated'"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub oidc_subject: Option<String>,
    pub oidc_issuer: Option<String>,
    pub display_name: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role: OrgRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub capture_content_default: bool,
    pub redaction_rules: serde_json::Value,
    pub retention_days: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: Vec<u8>,
    pub scopes: serde_json::Value,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_api_key_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub framework: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub metadata: serde_json::Value,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVersion {
    pub id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    pub version: String,
    pub configuration: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub external_session_id: String,
    pub user_ref: Option<String>,
    pub metadata: serde_json::Value,
    pub started_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub trace_id: TraceId,
    pub root_span_id: Option<SpanId>,
    pub name: Option<String>,
    pub status: TraceStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub span_count: i32,
    pub error_span_count: i32,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cached_input_tokens: i64,
    pub total_reasoning_tokens: i64,
    pub estimated_cost: Option<Decimal>,
    pub has_unpriced_usage: bool,
    pub currency: String,
    pub resource_attributes: serde_json::Value,
    pub attributes: serde_json::Value,
    pub received_at: DateTime<Utc>,
    pub finalized_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub agent_id: Option<Uuid>,
    pub name: String,
    pub span_type: SpanType,
    pub status: TraceStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tool_name: Option<String>,
    pub attributes: serde_json::Value,
    pub resource_attributes: serde_json::Value,
    pub scope_attributes: serde_json::Value,
    pub payload_s3_key: Option<String>,
    pub error: Option<serde_json::Value>,
    pub received_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub event_index: i32,
    pub event_name: String,
    pub occurred_at: DateTime<Utc>,
    pub attributes: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub link_index: i32,
    pub linked_trace_id: String,
    pub linked_span_id: String,
    pub attributes: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub cost_estimate: Option<Decimal>,
    pub currency: String,
    pub pricing_version: Option<String>,
    pub input_rate_per_million: Option<Decimal>,
    pub output_rate_per_million: Option<Decimal>,
    pub cached_rate_per_million: Option<Decimal>,
    pub usage_source: UsageSource,
    pub recorded_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub id: Uuid,
    pub provider: String,
    pub model_pattern: String,
    pub match_type: String,
    pub currency: String,
    pub input_per_million: Option<Decimal>,
    pub output_per_million: Option<Decimal>,
    pub cached_input_per_million: Option<Decimal>,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    pub source_checked_at: Option<DateTime<Utc>>,
    pub version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxJob {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub job_type: String,
    pub dedup_key: Option<String>,
    pub payload: serde_json::Value,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub available_at: DateTime<Utc>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanDetail {
    #[serde(flatten)]
    pub span: Span,
    pub events: Vec<SpanEvent>,
    pub links: Vec<SpanLink>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceQueryFilters {
    pub project_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub status: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub span_type: Option<String>,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub search_trace_id: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceQueryResult {
    pub items: Vec<Trace>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOverviewMetrics {
    pub active_agents_count: i64,
    pub total_traces_count: i64,
    pub error_rate: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub recent_traces: Vec<Trace>,
    pub recent_errors: Vec<Trace>,
}

pub fn encode_cursor(started_at: DateTime<Utc>, id: Uuid) -> String {
    let raw = format!("{}_{}", started_at.to_rfc3339(), id);
    hex::encode(raw)
}

pub fn decode_cursor(cursor: &str) -> Option<(DateTime<Utc>, Uuid)> {
    let bytes = hex::decode(cursor).ok()?;
    let s = String::from_utf8(bytes).ok()?;
    let (ts_str, id_str) = s.split_once('_')?;
    let ts = DateTime::parse_from_rfc3339(ts_str)
        .ok()?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id_str).ok()?;
    Some((ts, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_trace_id() {
        let valid = "4bf92f3577b34da6a3ce929d0e0e4736";
        let trace_id = TraceId::new(valid).unwrap();
        assert_eq!(trace_id.as_str(), valid);
    }

    #[test]
    fn test_rejects_zero_trace_id() {
        let zero = "00000000000000000000000000000000";
        assert!(TraceId::new(zero).is_err());
    }

    #[test]
    fn test_rejects_invalid_length_trace_id() {
        assert!(TraceId::new("1234abcd").is_err());
    }

    #[test]
    fn test_valid_span_id() {
        let valid = "00f067aa0ba902b7";
        let span_id = SpanId::new(valid).unwrap();
        assert_eq!(span_id.as_str(), valid);
    }

    #[test]
    fn test_rejects_zero_span_id() {
        let zero = "0000000000000000";
        assert!(SpanId::new(zero).is_err());
    }

    #[test]
    fn test_span_type_parsing() {
        assert_eq!(SpanType::from_str("llm").unwrap(), SpanType::Llm);
        assert_eq!(SpanType::from_str("agent").unwrap(), SpanType::Agent);
        assert_eq!(SpanType::from_str("tool").unwrap(), SpanType::Tool);
        assert!(SpanType::from_str("invalid").is_err());
    }

    #[test]
    fn test_cursor_encoding_and_decoding() {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let encoded = encode_cursor(now, id);
        let decoded = decode_cursor(&encoded).expect("cursor must decode");
        assert_eq!(decoded.1, id);
        assert_eq!(decoded.0.timestamp_millis(), now.timestamp_millis());
    }
}
