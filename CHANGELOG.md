# Changelog

All notable changes to the **AgentLens** observability platform are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.0] - 2026-11-13

### Highlights
- Production-hardened v1.0.0 release of AgentLens: open-source, Rust-first AI agent observability platform.
- Complete support for OTLP ingestion, out-of-order span assembly, GenAI semantic conventions, transactional outbox durability, exact decimal model pricing, transparent agent health, SSRF-safe alerts, framework integrations, and non-root isolated container operations.

### Added
- **Core Architecture & Tenancy (Phase 01–03):**
  - Layered Rust workspace with 18 crates adhering to `#![forbid(unsafe_code)]`.
  - Multi-tenant data model with Organizations, Projects, Environments, and Role-Based Access Control (`owner`, `admin`, `member`, `viewer`).
  - Argon2id password hashing, constant-time API key verification (`agl_live_*`), and session tokens.
- **OTLP Ingestion & Redaction Pipeline (Phase 04):**
  - High-performance `POST /v1/traces` receiver on ports `4318` and `8080` handling Protobuf and JSON OTLP formats.
  - Streaming in-memory secret redaction engine sanitizing authorization tokens, private keys, passwords, and sensitive keys.
  - Support for out-of-order span arrival via provisional trace records and ACID batch writes.
- **Transactional Outbox & Workers (Phase 05):**
  - Background worker engine using PostgreSQL `FOR UPDATE SKIP LOCKED` work queue with exponential backoff and lease recovery.
  - Idempotent trace summary finalization and agent activity tracking.
- **Client SDKs & Instrumentation (Phase 06):**
  - Production Python SDK (`agentlens`) and TypeScript SDK (`@agentlens/sdk`) with local bounded batch processors and opt-in content capture.
  - Runnable end-to-end example applications in Python and TypeScript.
- **Control-Plane API & Next.js Console (Phase 07):**
  - App Router dashboard with first-owner bootstrap, agent registry, cursor-paginated trace explorer, and interactive span waterfall.
  - OpenAPI 3.1 specification for control-plane endpoints.
- **Exact Decimal Pricing & Analytics (Phase 08):**
  - Curated rate cards for OpenAI, Anthropic, Google Gemini, Mistral, and DeepSeek using `rust_decimal`.
  - Prefix and exact model matching, truthful unpriced tracking (`NULL` cost estimates), and daily metric rollups.
- **Transparent Health, Alerts & Notifications (Phase 09):**
  - Transparent agent health scores and freshness indicators (`active`, `idle`, `stale`, `never_seen`).
  - Scheduled background alert rule evaluator with four-state lifecycle (`OK -> FIRING -> ACKNOWLEDGED -> RESOLVED`).
  - AES-256-GCM encrypted notification channels and SSRF-hardened dispatchers for Generic Webhooks, Telegram, Discord, and Slack.
- **Framework Integrations (Phase 10):**
  - Non-invasive instrumentation adapters for LangGraph, LiteLLM, CrewAI, OpenAI Agents SDK, and MCP (Model Context Protocol).
  - Multi-agent workflow example suites.
- **Production Hardening & Operations (Phase 11):**
  - End-to-end OIDC SSO flow (`/api/v1/auth/oidc/start`, `/api/v1/auth/oidc/callback`, and Dashboard SSO UI).
  - Large payload offload to S3/MinIO (> 64 KiB) and automated retention cleanup worker jobs.
  - Hardened non-root OCI containers (`UID:GID 65532:65532`) and network-isolated production Docker Compose configuration.
  - Automated disaster recovery scripts (`scripts/backup.sh`, `scripts/restore.sh`) with SHA-256 checksum manifest validation.
  - Saturated load test benchmarks validating > 1,400 spans/sec throughput and sub-6ms $P_{50}$ latency.

---

## [0.1.0-phase10] - 2026-11-13
- Completed Framework Integrations (LangGraph, LiteLLM, CrewAI, OpenAI Agents, MCP).

## [0.1.0-phase09] - 2026-11-13
- Completed Agent Health, Alert Evaluation Engine, and SSRF-Safe Notifications.

## [0.1.0-phase08] - 2026-11-13
- Completed Versioned Model Pricing, Exact Decimal Cost Engine, and Daily Analytics.

## [0.1.0-phase07] - 2026-11-13
- Completed Control-Plane Query Endpoints and Next.js Trace Explorer Console.

## [0.1.0-phase06] - 2026-11-13
- Completed Python and TypeScript SDKs with OpenTelemetry Compatibility.

## [0.1.0-phase05] - 2026-11-13
- Completed Durable Background Workers, Outbox Leasing Engine, and Trace Finalization.

## [0.1.0-phase04] - 2026-11-13
- Completed OTLP Ingestion Receiver, GenAI Semantic Conventions, and Redaction Pipeline.

## [0.1.0-phase03] - 2026-11-13
- Completed Authentication, Multi-Tenancy, and Argon2id Ingestion Key Management.

## [0.1.0-phase02] - 2026-11-13
- Completed PostgreSQL Schema Migrations, Indexes, and Constraints.

## [0.1.0-phase01] - 2026-11-13
- Initial Repository Setup, Domain Layer, Configuration, and Architectural Decision Records.
