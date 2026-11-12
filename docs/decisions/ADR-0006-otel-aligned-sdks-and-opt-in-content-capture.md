# ADR-0006: OpenTelemetry-Aligned SDKs and Opt-In Content Capture

## Status
Accepted

## Context
Monitoring AI agents requires low-overhead instrumentation without vendor lock-in or accidental leakage of sensitive customer data.

## Decision
1. SDKs for Python (`agentlens`) and TypeScript (`@agentlens/sdk`) build on standard OpenTelemetry APIs and export via standard OTLP/HTTP.
2. Content capture of prompts and responses is disabled by default (`capture_content = false`).
3. Local SDK processors drop or redact content before network transmission when disabled.

## Consequences
- Compliance with data privacy regulations by default.
- Seamless compatibility with existing OpenTelemetry collector tooling.
