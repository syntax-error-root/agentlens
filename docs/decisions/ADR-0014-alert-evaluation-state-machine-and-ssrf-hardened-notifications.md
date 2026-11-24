# ADR-0014: Alert Evaluation Engine, State Machine Transitions, AES-256-GCM Channel Encryption, and SSRF Hardening

## Status
Accepted

## Context
AgentLens requires automated agent health observability, continuous alert rule evaluation against operational metrics, state-machine event tracking, and multichannel notification delivery (Generic Webhooks, Telegram, Discord, Slack) without introducing security vulnerabilities (such as SSRF or credential leakage) or operational failure modes (such as alert notification storms or metric flapping).

## Decision Architecture

### 1. Alert Evaluation SQL Queries & Metrics Allowlist
- **Metrics Allowlist:**
  - `agent_error_rate`: Proportion of traces with `status = 'error'` over trailing `window_seconds`.
  - `agent_p95_latency_ms`: $P_{95}$ duration of traces computed via `percentile_cont(0.95)` within `window_seconds`.
  - `agent_inactivity_seconds`: Time elapsed since `last_seen_at` (or trace recorded timestamp).
  - `llm_error_rate`: Span-level error rate for `span_type = 'llm'`.
  - `tool_failure_rate`: Span-level error rate for `span_type = 'tool'`.
  - `token_usage_total`: Sum of all token usage within `window_seconds`.
  - `estimated_cost_usd`: Sum of `cost_estimate` within `window_seconds`.
- **Sample Count Constraint:** All metrics except `agent_inactivity_seconds` require `sample_count >= min_sample_count` to evaluate a breach, preventing false positives from low-volume noise.
- **Deterministic Fingerprinting:** `SHA-256(rule_id:group_by_dimension_key)`.

### 2. State-Machine Transitions and Anti-Flapping
- **States:** `OK` (idle), `FIRING` (active breach), `ACKNOWLEDGED` (acknowledged by operator), `RESOLVED` (recovered to normal bounds).
- **Transitions:**
  - `Normal -> FIRING`: Threshold breached + sample count met + no active event for `fingerprint` + outside `cooldown_seconds` from previous resolution. Creates `alert_events` row and enqueues `notification_deliveries`.
  - `FIRING / ACKNOWLEDGED (Ongoing)`: Threshold breached + active event exists. Updates `last_triggered_at` and current evaluated values in `details`. Does NOT create duplicate events or send notification storms.
  - `FIRING -> ACKNOWLEDGED`: Operator acknowledges event via API. Updates `acknowledged_at` and `acknowledged_by`.
  - `FIRING / ACKNOWLEDGED -> RESOLVED`: Metric evaluates within normal thresholds for the entire evaluation window. Sets `resolved_at = now()`, transitions status to `resolved`, and enqueues resolution notification delivery.

### 3. AES-256-GCM Channel Credential Encryption
- **Format:** `v1:<base64(nonce_12_bytes)>:<base64(ciphertext_with_tag)>`
- **Key:** Derived from `APP_ENCRYPTION_KEY` (32 bytes).
- **Protection:** API `GET` endpoints never expose raw credentials; tokens, secrets, and URLs are masked (`https://discord.com/api/webhooks/...****`).

### 4. SSRF Connection-Time IP Validator & Delivery Security
- **Threat Vector:** Malicious webhook URLs targeting loopback (`127.0.0.0/8`), private subnets (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), link-local metadata services (`169.254.169.254`), or DNS rebinding / HTTP redirect attacks.
- **Defense Architecture:**
  1. DNS pre-flight and connection resolution validates every IP against blocked CIDRs.
  2. HTTP client disables automatic redirects (`Policy::none()`) so 3xx status codes cannot redirect into internal infrastructure.
  3. Strict timeout (10s connect/read), bounded response body limits, and exponential backoff retry budget for notification delivery.
  4. Test flag `allow_loopback_for_testing: bool` enabled only in integration test harnesses.

## Consequences
- Guaranteed zero notification storms during sustained outages.
- Complete SSRF defense at the transport layer immune to DNS rebinding and redirect pivoting.
- Zero credential exposure at rest or in API responses.
