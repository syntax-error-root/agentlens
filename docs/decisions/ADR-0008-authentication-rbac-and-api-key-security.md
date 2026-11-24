# ADR-0008: Authentication, RBAC, and Scoped API Key Security

## Status
Accepted

## Context
Phase 03 establishes control-plane authentication, first-owner bootstrap, session cookies, CSRF protection, RBAC, and high-entropy ingestion API keys for AgentLens.

## Decision
1. Implement atomic first-owner bootstrap guarded by a PostgreSQL advisory transaction lock and zero-user validation. Once any user exists, bootstrap is permanently disabled (HTTP 403).
2. Hash user passwords with Argon2id.
3. Secure web sessions using `HttpOnly`, `SameSite=Lax` cookies, with mandatory `X-CSRF-Token` header validation for all mutating operations (`POST`, `PATCH`, `PUT`, `DELETE`).
4. Implement a 4-tier RBAC model (`owner`, `admin`, `member`, `viewer`), strictly checking user membership on every organization and project resource. `viewer` is forbidden from mutations.
5. Ingestion API keys use format `agl_live_<32_byte_secret>`, storing only `key_prefix` and SHA-256 `key_hash`. Verification uses constant-time comparison via `subtle::ConstantTimeEq`. Ingestion keys are strictly rejected from `/api/v1/*` control-plane routes.
6. Record security audit events in `audit_logs` without storing sensitive secrets or credentials.

## Consequences
- Guaranteed single-owner bootstrap race-condition prevention.
- Timing-attack-resistant API key verification.
- Enforced multi-tenant isolation across all control-plane APIs.
