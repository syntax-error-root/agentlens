# ADR-0003: Transactional Outbox and SKIP LOCKED Worker Leasing

## Status
Accepted

## Context
Trace finalization, metric rollups, and alerts must be processed asynchronously without losing tasks if workers fail.

## Decision
1. Persist `outbox_jobs` inside the ingestion database transaction.
2. Background workers claim pending jobs using `SELECT ... FOR UPDATE SKIP LOCKED` with explicit `lease_expires_at` recovery.
3. Redis remains strictly ephemeral for caching, rate limiting, and pub/sub wake-up triggers.

## Consequences
- Guaranteed at-least-once task delivery without Redis persistence dependencies.
