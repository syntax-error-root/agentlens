# ADR-0005: Scoped API Key Authentication and SSRF Defense

## Status
Accepted

## Context
Ingestion endpoints require lightweight, tenant-isolated authentication. Webhooks and alert destinations must not be exploited for SSRF attacks against internal infrastructure.

## Decision
1. High-entropy ingestion keys (`agl_live_*`) are bound to specific `(organization_id, project_id, environment_id)`. Stored hashed with SHA-256 and compared in constant time.
2. Webhook egress validates target IPs against RFC 1918, loopback, and cloud metadata ranges at connect time, and disables HTTP redirects.
3. Notification channel secrets are encrypted using versioned AES-256-GCM.

## Consequences
- Guaranteed multi-tenant separation on telemetry ingestion.
- Internal network resources protected against unauthorized access via webhooks.
