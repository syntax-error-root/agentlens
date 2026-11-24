# ADR-0009: OTLP Ingestion Pipeline, Semantic Normalization, and Transactional Persistence

## Status
Accepted

## Context
AgentLens receives OpenTelemetry traces at `POST /v1/traces` on both the standard OTLP port (`4318`) and the API control port (`8080`). Traces originate from heterogeneous agent frameworks (OpenAI, Anthropic, LangChain, LlamaIndex, AutoGen), encoded in Protobuf or JSON, optionally compressed with gzip. The ingestion pipeline must prevent denial-of-service via decompression bombs, enforce tenant isolation via scoped ingestion keys, redact sensitive data according to project privacy settings, normalize GenAI conventions into a unified schema, and guarantee zero data loss via single-transaction PostgreSQL commits.

## Decision
1. **Receiver Topology & Authentication:**
   - Mount `POST /v1/traces` on both `0.0.0.0:4318` (dedicated OTLP listener) and `0.0.0.0:8080` (main API).
   - Authenticate ingestion API keys (`Authorization: Bearer agl_live_<secret>`), resolving `(organization_id, project_id, environment_id)` in constant time.
2. **Decompression & Payload Limits:**
   - Support `Content-Encoding: gzip` using a bounded reader with a strict post-decompression limit of 16 MiB.
   - Reject payloads exceeding 16 MiB decompressed with HTTP 413 (`Payload Too Large`).
   - Support both `application/x-protobuf` and `application/json`.
   - Malformed payloads return HTTP 400 (`Bad Request`).
   - Return protocol-accurate `ExportTraceServiceResponse` with `partial_success` reporting any rejected spans and diagnostic error messages.
3. **W3C ID Validation & Canonical Normalization:**
   - Validate 16-byte `trace_id` (32 lowercase hex chars) and 8-byte `span_id` (16 lowercase hex chars), rejecting all-zero identifiers (`0000...`).
   - Convert all-zero `parent_span_id` to `None`.
   - Normalize GenAI semantic conventions (`gen_ai.system`, `gen_ai.request.model`, `llm.model_name`, token counts, tool calls, agent metadata) into canonical `SpanType`, `TraceStatus`, and `TokenUsage`.
4. **Privacy & Redaction by Default:**
   - When project `capture_content_default` is `false` (the default), strip prompts, completions, message arrays, and raw tool arguments (`gen_ai.prompt`, `gen_ai.completion`, `gen_ai.input.messages`, `gen_ai.output.messages`, `llm.prompts`, `llm.completions`, `agentlens.tool.input`, `agentlens.tool.output`).
   - Apply project-configured `redaction_rules` (regex patterns for tokens, keys, emails, PII) and truncate attributes exceeding maximum value length.
5. **Transactional Persistence & Outbox Guarantee:**
   - Ingest all spans within a single PostgreSQL transaction:
     1. Upsert referenced `agents`, `agent_versions`, and `sessions`.
     2. Upsert `traces` for each distinct `(project_id, trace_id)` using out-of-order safe rules (`started_at = LEAST`, `ended_at = GREATEST`, root span sets authoritative metadata).
     3. Upsert `spans`, `span_events`, `span_links`, and `token_usages` using idempotent constraints.
     4. Insert a `trace_ingested` job into `outbox_jobs` with affected trace IDs.
   - Commit the transaction before returning HTTP 200.
   - Any Redis wake-up notification is non-blocking and fires post-commit; Redis unavailability never fails ingestion or loses data.

## Consequences
- Guaranteed zero-trust privacy and DoS defense against decompression attacks.
- Out-of-order span arrival safely reconciled without orphan records.
- Standard OpenTelemetry client and collector interoperability.
