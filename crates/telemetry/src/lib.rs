#![forbid(unsafe_code)]

//! `agentlens-telemetry` crate.
//! OTLP protobuf and JSON decoding, bounded decompression, W3C ID validation,
//! and canonical GenAI semantic normalization.

use agentlens_domain::{SpanId, SpanType, TraceId, TraceStatus};
use chrono::{DateTime, TimeZone, Utc};
use flate2::read::GzDecoder;
pub use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTracePartialSuccess, ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use opentelemetry_proto::tonic::common::v1::any_value::Value as ProtoAnyValue;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::trace::v1::status::StatusCode;
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::Read;
use thiserror::Error;

pub const MAX_DECOMPRESSED_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("Payload exceeds maximum decompressed size limit (16 MiB): {0}")]
    PayloadTooLarge(String),
    #[error("Decompression failed: {0}")]
    Decompression(String),
    #[error("Invalid protobuf payload: {0}")]
    ProtobufDecode(String),
    #[error("Invalid JSON payload: {0}")]
    JsonDecode(String),
    #[error("Unsupported content-type: {0}")]
    UnsupportedContentType(String),
    #[error("Invalid span identifier: {0}")]
    InvalidIdentifier(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSpanEvent {
    pub event_index: i32,
    pub event_name: String,
    pub occurred_at: DateTime<Utc>,
    pub attributes: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSpanLink {
    pub link_index: i32,
    pub linked_trace_id: String,
    pub linked_span_id: String,
    pub attributes: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedUsage {
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSpan {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub name: String,
    pub span_type: SpanType,
    pub status: TraceStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub tool_name: Option<String>,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub session_id: Option<String>,
    pub attributes: Value,
    pub resource_attributes: Value,
    pub scope_attributes: Value,
    pub error: Option<Value>,
    pub events: Vec<NormalizedSpanEvent>,
    pub links: Vec<NormalizedSpanLink>,
    pub usage: Option<NormalizedUsage>,
}

#[derive(Debug, Default)]
pub struct NormalizationResult {
    pub spans: Vec<NormalizedSpan>,
    pub rejected_count: i64,
    pub errors: Vec<String>,
}

/// Decompresses gzip input with a strict post-decompression size ceiling.
pub fn decompress_gzip_bounded(input: &[u8], max_bytes: usize) -> Result<Vec<u8>, TelemetryError> {
    let mut decoder = GzDecoder::new(input);
    let mut buffer = Vec::new();
    let mut take = (&mut decoder).take(max_bytes as u64 + 1);
    take.read_to_end(&mut buffer)
        .map_err(|e| TelemetryError::Decompression(e.to_string()))?;

    if buffer.len() > max_bytes {
        return Err(TelemetryError::PayloadTooLarge(format!(
            "Decompressed payload exceeded limit of {max_bytes} bytes"
        )));
    }
    Ok(buffer)
}

/// Decodes raw HTTP body bytes into `ExportTraceServiceRequest`.
pub fn decode_otlp_request(
    body: &[u8],
    content_encoding: Option<&str>,
    content_type: Option<&str>,
) -> Result<ExportTraceServiceRequest, TelemetryError> {
    let raw_bytes = if let Some(enc) = content_encoding {
        if enc.eq_ignore_ascii_case("gzip") {
            decompress_gzip_bounded(body, MAX_DECOMPRESSED_BYTES)?
        } else {
            body.to_vec()
        }
    } else {
        body.to_vec()
    };

    let ct = content_type.unwrap_or("application/x-protobuf");
    if ct.contains("application/x-protobuf") || ct.contains("application/octet-stream") {
        ExportTraceServiceRequest::decode(&raw_bytes[..])
            .map_err(|e| TelemetryError::ProtobufDecode(e.to_string()))
    } else if ct.contains("application/json") {
        serde_json::from_slice::<ExportTraceServiceRequest>(&raw_bytes)
            .map_err(|e| TelemetryError::JsonDecode(e.to_string()))
    } else {
        // Fallback: try protobuf first, then JSON
        ExportTraceServiceRequest::decode(&raw_bytes[..])
            .or_else(|_| serde_json::from_slice::<ExportTraceServiceRequest>(&raw_bytes))
            .map_err(|_| TelemetryError::UnsupportedContentType(ct.to_string()))
    }
}

/// Builds an OTLP protocol-accurate response with optional partial success metrics.
#[must_use]
pub fn build_otlp_response(
    rejected_spans: i64,
    error_message: Option<String>,
) -> ExportTraceServiceResponse {
    let partial_success = if rejected_spans > 0 || error_message.is_some() {
        Some(ExportTracePartialSuccess {
            rejected_spans,
            error_message: error_message.unwrap_or_default(),
        })
    } else {
        None
    };

    ExportTraceServiceResponse { partial_success }
}

/// Normalizes an `ExportTraceServiceRequest` into canonical `NormalizedSpan` domain objects.
#[must_use]
pub fn normalize_trace_request(req: ExportTraceServiceRequest) -> NormalizationResult {
    let mut result = NormalizationResult::default();

    for resource_span in req.resource_spans {
        let resource_attrs = resource_span
            .resource
            .map(|r| key_values_to_json(&r.attributes))
            .unwrap_or_else(|| Value::Object(Map::new()));

        for scope_span in resource_span.scope_spans {
            let scope_attrs = scope_span
                .scope
                .map(|s| key_values_to_json(&s.attributes))
                .unwrap_or_else(|| Value::Object(Map::new()));

            for span in scope_span.spans {
                // 1. W3C ID validation
                let trace_hex = hex::encode(&span.trace_id);
                let trace_id = match TraceId::new(&trace_hex) {
                    Ok(id) => id,
                    Err(e) => {
                        result.rejected_count += 1;
                        result
                            .errors
                            .push(format!("Rejected invalid trace_id '{trace_hex}': {e}"));
                        continue;
                    }
                };

                let span_hex = hex::encode(&span.span_id);
                let span_id = match SpanId::new(&span_hex) {
                    Ok(id) => id,
                    Err(e) => {
                        result.rejected_count += 1;
                        result
                            .errors
                            .push(format!("Rejected invalid span_id '{span_hex}': {e}"));
                        continue;
                    }
                };

                let parent_span_id =
                    if span.parent_span_id.is_empty() || span.parent_span_id == [0; 8] {
                        None
                    } else {
                        let parent_hex = hex::encode(&span.parent_span_id);
                        SpanId::new(&parent_hex).ok()
                    };

                let attributes = key_values_to_json(&span.attributes);

                // 2. Timestamps
                let started_at = nanos_to_datetime(span.start_time_unix_nano);
                let ended_at = if span.end_time_unix_nano > span.start_time_unix_nano {
                    Some(nanos_to_datetime(span.end_time_unix_nano))
                } else {
                    None
                };
                let duration_ms = ended_at.map(|e| (e - started_at).num_milliseconds());

                // 3. Status
                let (status, error_val) = match span.status {
                    Some(s) => match StatusCode::try_from(s.code) {
                        Ok(StatusCode::Ok) => (TraceStatus::Ok, None),
                        Ok(StatusCode::Error) => {
                            let msg = if s.message.is_empty() {
                                "Span reported error".to_string()
                            } else {
                                s.message
                            };
                            (
                                TraceStatus::Error,
                                Some(serde_json::json!({ "message": msg })),
                            )
                        }
                        _ => (TraceStatus::Unset, None),
                    },
                    None => (TraceStatus::Unset, None),
                };

                // 4. Semantic Conventions Extraction
                let provider = extract_string_attr(&attributes, &["gen_ai.system", "llm.provider"])
                    .or_else(|| {
                        extract_string_attr(&resource_attrs, &["gen_ai.system", "service.name"])
                    });

                let model = extract_string_attr(
                    &attributes,
                    &[
                        "gen_ai.request.model",
                        "gen_ai.response.model",
                        "llm.model_name",
                        "model",
                    ],
                );

                let tool_name = extract_string_attr(
                    &attributes,
                    &["gen_ai.tool.name", "tool.name", "agentlens.tool.name"],
                );

                let agent_name = extract_string_attr(
                    &attributes,
                    &["agent.name", "gen_ai.agent.name", "agentlens.agent.name"],
                )
                .or_else(|| extract_string_attr(&resource_attrs, &["agent.name", "service.name"]));

                let agent_version = extract_string_attr(
                    &attributes,
                    &[
                        "agent.version",
                        "gen_ai.agent.version",
                        "agentlens.agent.version",
                    ],
                )
                .or_else(|| extract_string_attr(&resource_attrs, &["service.version"]));

                let session_id = extract_string_attr(
                    &attributes,
                    &["session.id", "gen_ai.session.id", "agentlens.session.id"],
                );

                let span_type = determine_span_type(
                    &attributes,
                    tool_name.is_some(),
                    agent_name.is_some(),
                    provider.is_some() || model.is_some(),
                );

                // 5. Events & Links
                let events = span
                    .events
                    .into_iter()
                    .enumerate()
                    .map(|(i, ev)| NormalizedSpanEvent {
                        event_index: i as i32,
                        event_name: ev.name,
                        occurred_at: nanos_to_datetime(ev.time_unix_nano),
                        attributes: key_values_to_json(&ev.attributes),
                    })
                    .collect();

                let links = span
                    .links
                    .into_iter()
                    .enumerate()
                    .map(|(i, link)| NormalizedSpanLink {
                        link_index: i as i32,
                        linked_trace_id: hex::encode(link.trace_id),
                        linked_span_id: hex::encode(link.span_id),
                        attributes: key_values_to_json(&link.attributes),
                    })
                    .collect();

                // 6. Token Usage
                let usage = extract_usage(&attributes, provider.as_deref(), model.as_deref());

                result.spans.push(NormalizedSpan {
                    trace_id,
                    span_id,
                    parent_span_id,
                    name: span.name,
                    span_type,
                    status,
                    started_at,
                    ended_at,
                    duration_ms,
                    provider,
                    model,
                    tool_name,
                    agent_name,
                    agent_version,
                    session_id,
                    attributes,
                    resource_attributes: resource_attrs.clone(),
                    scope_attributes: scope_attrs.clone(),
                    error: error_val,
                    events,
                    links,
                    usage,
                });
            }
        }
    }

    result
}

fn nanos_to_datetime(nanos: u64) -> DateTime<Utc> {
    if nanos == 0 {
        return Utc::now();
    }
    let secs = (nanos / 1_000_000_000) as i64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    Utc.timestamp_opt(secs, subsec_nanos)
        .single()
        .unwrap_or_else(Utc::now)
}

fn determine_span_type(attrs: &Value, has_tool: bool, has_agent: bool, has_llm: bool) -> SpanType {
    if has_tool {
        return SpanType::Tool;
    }
    if let Some(st) = extract_string_attr(attrs, &["agentlens.span_type", "span_type"]) {
        if let Ok(parsed) = st.parse() {
            return parsed;
        }
    }
    if has_llm {
        return SpanType::Llm;
    }
    if has_agent {
        return SpanType::Agent;
    }
    if attrs.get("retrieval").is_some() || attrs.get("vector_db").is_some() {
        return SpanType::Retrieval;
    }
    SpanType::Custom
}

fn extract_string_attr(attrs: &Value, keys: &[&str]) -> Option<String> {
    let map = attrs.as_object()?;
    for k in keys {
        if let Some(v) = map.get(*k) {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    return Some(s.trim().to_string());
                }
            }
        }
    }
    None
}

fn extract_i64_attr(attrs: &Value, keys: &[&str]) -> Option<i64> {
    let map = attrs.as_object()?;
    for k in keys {
        if let Some(v) = map.get(*k) {
            if let Some(i) = v.as_i64() {
                return Some(i);
            }
        }
    }
    None
}

fn extract_usage(
    attrs: &Value,
    provider_opt: Option<&str>,
    model_opt: Option<&str>,
) -> Option<NormalizedUsage> {
    let input = extract_i64_attr(
        attrs,
        &[
            "gen_ai.usage.input_tokens",
            "llm.usage.prompt_tokens",
            "usage.prompt_tokens",
        ],
    );
    let output = extract_i64_attr(
        attrs,
        &[
            "gen_ai.usage.output_tokens",
            "llm.usage.completion_tokens",
            "usage.completion_tokens",
        ],
    );

    if input.is_none() && output.is_none() {
        return None;
    }

    let cached = extract_i64_attr(
        attrs,
        &[
            "gen_ai.usage.cached_input_tokens",
            "llm.usage.cache_read_tokens",
        ],
    )
    .unwrap_or(0);

    let reasoning = extract_i64_attr(
        attrs,
        &[
            "gen_ai.usage.reasoning_tokens",
            "llm.usage.reasoning_tokens",
        ],
    )
    .unwrap_or(0);

    Some(NormalizedUsage {
        provider: provider_opt.unwrap_or("unknown").to_string(),
        model: model_opt.unwrap_or("unknown").to_string(),
        input_tokens: input.unwrap_or(0),
        output_tokens: output.unwrap_or(0),
        cached_input_tokens: cached,
        reasoning_tokens: reasoning,
    })
}

pub fn key_values_to_json(kvs: &[KeyValue]) -> Value {
    let mut map = Map::with_capacity(kvs.len());
    for kv in kvs {
        if let Some(ref val) = kv.value {
            map.insert(kv.key.clone(), any_value_to_json(val));
        }
    }
    Value::Object(map)
}

fn any_value_to_json(val: &AnyValue) -> Value {
    match val.value {
        Some(ProtoAnyValue::StringValue(ref s)) => Value::String(s.clone()),
        Some(ProtoAnyValue::BoolValue(b)) => Value::Bool(b),
        Some(ProtoAnyValue::IntValue(i)) => serde_json::json!(i),
        Some(ProtoAnyValue::DoubleValue(d)) => serde_json::json!(d),
        Some(ProtoAnyValue::ArrayValue(ref arr)) => {
            Value::Array(arr.values.iter().map(any_value_to_json).collect())
        }
        Some(ProtoAnyValue::KvlistValue(ref kv)) => {
            let mut map = Map::new();
            for item in &kv.values {
                if let Some(ref v) = item.value {
                    map.insert(item.key.clone(), any_value_to_json(v));
                }
            }
            Value::Object(map)
        }
        Some(ProtoAnyValue::BytesValue(ref b)) => Value::String(hex::encode(b)),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use opentelemetry_proto::tonic::common::v1::any_value::Value as PV;
    use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
    use std::io::Write;

    #[test]
    fn test_bounded_gzip_decompression_rejects_bomb() {
        // Compress 2000 zero bytes
        let large_payload = vec![0u8; 2000];
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&large_payload).unwrap();
        let compressed = encoder.finish().unwrap();

        // Limit to 500 bytes -> must reject
        let res = decompress_gzip_bounded(&compressed, 500);
        assert!(matches!(res, Err(TelemetryError::PayloadTooLarge(_))));

        // Limit to 3000 bytes -> succeeds
        let valid = decompress_gzip_bounded(&compressed, 3000).unwrap();
        assert_eq!(valid.len(), 2000);
    }

    #[test]
    fn test_rejects_all_zero_ids() {
        let span = Span {
            trace_id: vec![0; 16],
            span_id: vec![0; 8],
            name: "zero_span".to_string(),
            ..Default::default()
        };

        let req = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let result = normalize_trace_request(req);
        assert_eq!(result.spans.len(), 0);
        assert_eq!(result.rejected_count, 1);
    }

    #[test]
    fn test_canonical_genai_normalization() {
        let trace_id = hex::decode("4bf92f3577b34da6a3ce929d0e0e4736").unwrap();
        let span_id = hex::decode("00f067aa0ba902b7").unwrap();

        let span = Span {
            trace_id,
            span_id,
            name: "chat_completion".to_string(),
            attributes: vec![
                KeyValue {
                    key: "gen_ai.system".to_string(),
                    value: Some(AnyValue {
                        value: Some(PV::StringValue("openai".to_string())),
                    }),
                },
                KeyValue {
                    key: "gen_ai.request.model".to_string(),
                    value: Some(AnyValue {
                        value: Some(PV::StringValue("gpt-4o".to_string())),
                    }),
                },
                KeyValue {
                    key: "gen_ai.usage.input_tokens".to_string(),
                    value: Some(AnyValue {
                        value: Some(PV::IntValue(250)),
                    }),
                },
                KeyValue {
                    key: "gen_ai.usage.output_tokens".to_string(),
                    value: Some(AnyValue {
                        value: Some(PV::IntValue(120)),
                    }),
                },
            ],
            ..Default::default()
        };

        let req = ExportTraceServiceRequest {
            resource_spans: vec![ResourceSpans {
                scope_spans: vec![ScopeSpans {
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };

        let result = normalize_trace_request(req);
        assert_eq!(result.spans.len(), 1);
        let s = &result.spans[0];
        assert_eq!(s.span_type, SpanType::Llm);
        assert_eq!(s.provider.as_deref(), Some("openai"));
        assert_eq!(s.model.as_deref(), Some("gpt-4o"));

        let usage = s.usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, 250);
        assert_eq!(usage.output_tokens, 120);
    }
}
