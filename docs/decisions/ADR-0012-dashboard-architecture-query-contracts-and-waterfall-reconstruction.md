# ADR-0012: Dashboard Architecture, Query Contracts, and Waterfall Reconstruction

## Status
Accepted

## Context
AgentLens requires control-plane REST query endpoints for agents, traces, spans, sessions, and project metrics, paired with a Next.js App Router web console. The telemetry dataset features high write velocity, concurrent out-of-order span ingestion, and potentially in-flight traces where child spans may arrive before their parent spans. 

The dashboard must provide:
1. Low-latency, tenant-isolated trace exploration with cursor-based pagination resistant to offset drift under high insertion volume.
2. An interactive span waterfall timeline capable of constructing parent-child execution hierarchies from unordered span batches without infinite recursion or client rendering crashes.
3. Next.js App Router route protection with session validation, bootstrap state routing, CSRF defense, and XSS sanitization across all user-supplied telemetry strings.

## Decision
1. **Deterministic Keyset Cursor Pagination**:
   - `GET /api/v1/traces` implements keyset cursor pagination using composite tuple ordering: `(started_at DESC, id DESC)`.
   - Cursors are base64url-encoded JSON payloads containing `{ "started_at": "...", "id": "..." }`.
   - All trace queries require an active tenant boundary (`project_id` resolved via authenticated session context or explicit query param validated against user membership).
2. **Span Waterfall Hierarchy Reconstruction**:
   - Spans for a trace are fetched via `GET /api/v1/traces/{id}/spans` ordered by `start_time ASC`.
   - The tree constructor creates an index map `Map<SpanId, SpanNode>`.
   - Spans referencing missing or in-flight parents (`parent_span_id` not in current batch) are elevated to top-level roots in a multi-root forest instead of being dropped.
   - Child nodes within each span are sorted chronologically by `start_time`.
   - Horizontal timeline bar positions are computed as percentages relative to the overall trace time span (`(span.start_time - trace.start_time) / trace.duration_ms * 100`).
3. **Next.js Dashboard Security & Protection**:
   - Server-side Next.js middleware interrogates the session cookie (`agentlens_session`) on all protected routes (`/`, `/dashboard/*`, `/traces/*`, `/agents/*`, `/settings/*`).
   - If `/api/v1/auth/bootstrap/status` reports uninitialized state (`can_bootstrap: true`), traffic is redirected to `/bootstrap`.
   - State mutations attach `X-CSRF-Token` retrieved from the session context.
   - All telemetry attribute values, error messages, and agent identifiers are rendered via standard React text nodes and escaping primitives to prevent cross-site scripting (XSS).

## Consequences
- Keyset pagination guarantees consistent iteration even during high-throughput ingestion spikes.
- Waterfall view is completely immune to missing parent span race conditions.
- Zero synthetic mock data in production builds: the UI binds directly to live SQL query endpoints.
