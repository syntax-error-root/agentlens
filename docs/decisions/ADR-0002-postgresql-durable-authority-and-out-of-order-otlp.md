# ADR-0002: PostgreSQL Durable Authority and Out-of-Order OTLP Ingestion

## Status
Accepted

## Context
OTLP spans often arrive out of order (child spans arriving prior to parent/root spans) across distinct batches and network retries.

## Decision
1. PostgreSQL is the authoritative store. Ingestion transactions commit traces, spans, and outbox jobs atomically before returning HTTP 200.
2. Ingestion creates a provisional `traces` row (`root_span_id = NULL`) if child spans arrive first, updating the authoritative root metadata when `parent_span_id IS NULL` arrives.
3. Validate and reject all-zero W3C trace and span IDs.

## Consequences
- No lost spans due to missing parent foreign keys.
- Guaranteed durability on acknowledgment.
