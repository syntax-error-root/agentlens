# AgentLens System Architecture Overview

AgentLens operates as a modular monolith running two deployable binaries:

1. **agentlens-api**:
   - Axum HTTP control-plane API on port 8080 (`/api/v1/*`).
   - OTLP/HTTP collector mounted on port 4318 and 8080 (`POST /v1/traces`).
   - Normalizes OTLP payloads into canonical GenAI domain entities.
   - Atomically commits provisional traces, spans, and `outbox_jobs` into PostgreSQL.

2. **agentlens-worker**:
   - Claims background tasks from `outbox_jobs` using PostgreSQL `FOR UPDATE SKIP LOCKED`.
   - Reconciles out-of-order traces, recalculates duration and token aggregates.
   - Computes model costs using exact `rust_decimal` math.
   - Evaluates alert thresholds and dispatches SSRF-safe notifications.
