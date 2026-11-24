# ADR-0007: Database Schema Composite Foreign Keys and Out-of-Order Provisional Traces

## Status
Accepted

## Context
Phase 02 requires establishing complete PostgreSQL schemas 0002..0005 with strict multi-tenant isolation, composite foreign keys `(organization_id, project_id)`, idempotent OTLP telemetry ingestion, out-of-order span persistence, and fail-closed security configuration.

## Decision
1. Implement ordered migrations `0002_tenancy_and_auth.sql`, `0003_agents_and_sessions.sql`, `0004_traces_spans_and_usage.sql`, and `0005_alerts_notifications_metrics.sql`.
2. Enforce composite foreign keys on all tenant tables ensuring cross-project or cross-tenant references fail at the database level.
3. Validate and enforce W3C `TraceId` (32 hex, non-zero) and `SpanId` (16 hex, non-zero) both in Rust domain value objects and database `CHECK` constraints.
4. Implement out-of-order-safe provisional trace upserts in SQLx repositories (`traces.started_at = LEAST`, `traces.ended_at = GREATEST`, root span sets authoritative `name`/`status`/`root_span_id`).
5. Fail closed on startup if `APP_ENV == "production"` and secrets (`APP_ENCRYPTION_KEY`, `SESSION_SECRET`) are missing, short, or placeholder values.

## Consequences
- Complete tenant boundary enforcement at the database constraint level.
- Out-of-order OTLP batches cannot orphan child spans or corrupt trace timing.
- Cryptographic keys cannot accidentally run in production with insecure defaults.
