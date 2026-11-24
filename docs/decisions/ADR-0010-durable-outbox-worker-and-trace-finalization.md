# ADR-0010: Durable Outbox Worker and Trace Finalization Engine

## Context
Trace spans and token usages arrive out-of-order over OTLP and are persisted atomically inside single PostgreSQL transactions alongside transactional outbox jobs (`outbox_jobs`) with `job_type = 'trace_ingested'`.
To ensure high ingestion throughput and resilient background processing, the system requires a durable, horizontally scalable worker architecture that:
1. Claims pending outbox jobs and reclaims stalled jobs from crashed workers using PostgreSQL's native concurrency control (`FOR UPDATE SKIP LOCKED`).
2. Implements exponential backoff with jitter on handler failure, moving poison jobs to `status = 'dead'` after `max_attempts`.
3. Idempotently reconciles and finalizes trace summaries (`traces.started_at`, `ended_at`, wall-clock `duration_ms`, `span_count`, `error_span_count`, and token usage totals) from raw `spans` and `token_usages`.
4. Updates agent and session `last_seen_at` timestamps accurately.
5. Exposes internal Prometheus metrics tracking worker queue lag, processing throughput, and dead-letter counts.

## Decision
1. **Outbox Lease-Claiming (`crates/jobs`):**
   - The worker periodically polls or wakes via pub/sub to lease batches using:
     ```sql
     WITH claimable AS (
         SELECT id
         FROM outbox_jobs
         WHERE (status = 'pending' AND available_at <= now())
            OR (status = 'processing' AND lease_expires_at <= now())
         ORDER BY available_at ASC, id ASC
         LIMIT $1
         FOR UPDATE SKIP LOCKED
     )
     UPDATE outbox_jobs
     SET status = 'processing',
         locked_by = $2,
         locked_at = now(),
         lease_expires_at = now() + $3,
         attempts = attempts + 1
     FROM claimable
     WHERE outbox_jobs.id = claimable.id
     RETURNING outbox_jobs.id, outbox_jobs.organization_id, outbox_jobs.project_id,
               outbox_jobs.job_type, outbox_jobs.dedup_key, outbox_jobs.payload,
               outbox_jobs.status, outbox_jobs.attempts, outbox_jobs.max_attempts,
               outbox_jobs.available_at, outbox_jobs.lease_expires_at,
               outbox_jobs.locked_at, outbox_jobs.locked_by, outbox_jobs.last_error,
               outbox_jobs.created_at, outbox_jobs.completed_at;
     ```
2. **Backoff and Dead-Letter Transition:**
   - On handler failure:
     - If `attempts >= max_attempts`, mark `status = 'dead'`, `last_error = error_str`, `completed_at = now()`.
     - Else mark `status = 'pending'`, `lease_expires_at = NULL`, `locked_by = NULL`, `locked_at = NULL`, and `available_at = now() + min(300, 2^attempts) + jitter`.
3. **Trace Finalization & Reconciliation Query:**
   - Execute single-query atomic reconciliation against `spans` and `token_usages`:
     - Wall-clock `duration_ms = EXTRACT(EPOCH FROM (MAX(spans.ended_at) - MIN(spans.started_at))) * 1000` (rejecting cumulative sum of child spans).
     - Surface `error` status if root span reports error or if any child span has `status = 'error'` and root is unset.
     - Update `agents.last_seen_at = GREATEST(agents.last_seen_at, trace.ended_at)`.
4. **Prometheus Metrics (`crates/observability`):**
   - Metrics exposed at `GET /metrics`:
     - `agentlens_outbox_pending_jobs` (Gauge)
     - `agentlens_outbox_oldest_pending_age_seconds` (Gauge)
     - `agentlens_worker_jobs_processed_total` (Counter)
     - `agentlens_worker_jobs_dead_total` (Counter)
     - `agentlens_otlp_spans_ingested_total` (Counter)

## Consequences
- Single PostgreSQL database serves as the source of truth for both data and background tasks without requiring external message brokers (Kafka/RabbitMQ).
- Safe against worker crash failures: lease expiration ensures stalled jobs are recovered without duplication or data loss.
- Strict idempotency: multiple executions of the same `trace_ingested` job compute identical aggregate states.
