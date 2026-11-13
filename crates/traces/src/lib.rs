#![forbid(unsafe_code)]

//! `agentlens-traces` crate.
//! Trace ingestion coordination, privacy filtering, and single-transaction persistence.

use agentlens_database::{DbPool, TracesRepository};
use agentlens_observability::ObservabilityRegistry;
use agentlens_privacy::{PrivacyPolicy, RedactionRule};
use agentlens_telemetry::{
    build_otlp_response, decode_otlp_request, normalize_trace_request, ExportTraceServiceResponse,
    TelemetryError,
};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TraceIngestionError {
    #[error("Telemetry error: {0}")]
    Telemetry(#[from] TelemetryError),
    #[error("Database error: {0}")]
    Database(#[from] agentlens_database::DatabaseError),
    #[error("Privacy policy error: {0}")]
    Privacy(#[from] agentlens_privacy::PrivacyError),
}

pub struct IngestionContext {
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub capture_content: bool,
    pub redaction_rules: Vec<RedactionRule>,
}

pub struct TraceIngestionService<'a> {
    pool: &'a DbPool,
    registry: Option<Arc<ObservabilityRegistry>>,
}

impl<'a> TraceIngestionService<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self {
            pool,
            registry: None,
        }
    }

    pub fn with_registry(pool: &'a DbPool, registry: Arc<ObservabilityRegistry>) -> Self {
        Self {
            pool,
            registry: Some(registry),
        }
    }

    /// Ingests an incoming raw OTLP payload end-to-end:
    /// 1. Decodes and decompresses payload with 16 MiB guard
    /// 2. Normalizes GenAI semantic conventions and validates W3C IDs
    /// 3. Applies privacy policy and PII redaction to attributes
    /// 4. Atomically persists spans, traces, outbox jobs in PostgreSQL
    /// 5. Returns protocol-accurate OTLP response
    pub async fn ingest_otlp_payload(
        &self,
        raw_body: &[u8],
        content_encoding: Option<&str>,
        content_type: Option<&str>,
        ctx: &IngestionContext,
    ) -> Result<ExportTraceServiceResponse, TraceIngestionError> {
        // 1. Decode & decompress
        let otlp_req = decode_otlp_request(raw_body, content_encoding, content_type)?;

        // 2. Normalize
        let mut norm_result = normalize_trace_request(otlp_req);

        // 3. Privacy & Redaction
        let policy = PrivacyPolicy::with_rules(
            ctx.capture_content,
            agentlens_privacy::DEFAULT_MAX_STRING_LENGTH,
            &ctx.redaction_rules,
        )?;

        for span in &mut norm_result.spans {
            policy.sanitize_attributes(&mut span.attributes);
            policy.sanitize_attributes(&mut span.resource_attributes);
            policy.sanitize_attributes(&mut span.scope_attributes);

            for ev in &mut span.events {
                policy.sanitize_attributes(&mut ev.attributes);
            }
            for link in &mut span.links {
                policy.sanitize_attributes(&mut link.attributes);
            }
        }

        // 4. Atomic PostgreSQL Transaction
        if !norm_result.spans.is_empty() {
            if let Some(ref reg) = self.registry {
                reg.record_spans_ingested(norm_result.spans.len() as u64);
            }
            let traces_repo = TracesRepository::new(self.pool);
            traces_repo
                .persist_batch(
                    ctx.organization_id,
                    ctx.project_id,
                    ctx.environment_id,
                    &norm_result.spans,
                )
                .await?;
        }

        // 5. Response with partial success tracking
        let error_msg = if norm_result.errors.is_empty() {
            None
        } else {
            Some(norm_result.errors.join("; "))
        };

        let response = build_otlp_response(norm_result.rejected_count, error_msg);
        Ok(response)
    }

    /// Reconciles and finalizes a trace's summary metrics and status from raw spans and usages.
    pub async fn finalize_trace(
        &self,
        project_id: Uuid,
        trace_id: &agentlens_domain::TraceId,
    ) -> Result<Option<agentlens_domain::Trace>, agentlens_database::DatabaseError> {
        let repo = TracesRepository::new(self.pool);
        repo.finalize_trace(project_id, trace_id).await
    }
}
