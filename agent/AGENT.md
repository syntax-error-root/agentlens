# AgentLens — Full Implementation Blueprint

> **Product:** AgentLens  
> **Tagline:** Monitor. Trace. Understand AI.  
> **Category:** Open-source AI agent observability and monitoring  
> **License:** Apache-2.0 (subject to final legal/name review)  
> **Architecture:** Rust-first modular monolith with System-1 Architect Model pass-through governance  
> **Primary deployment:** Self-hosted Docker Compose  
> **Long-term deployment:** Kubernetes  
> **Status:** Implementation specification (Reviewed & Hardened)

---

## 0. Instructions for the implementation agent

You are the principal engineer responsible for implementing AgentLens from this specification. Treat this document as the product and architecture source of truth.

### 0.1 Execution rules

1. Inspect the existing repository before changing anything. Preserve working code and conventions unless this specification explicitly requires a change.
2. Implement one phase at a time. Do not begin a later phase until the current phase passes its acceptance gate.
3. Before each phase, provide a short plan listing files/modules to change, schema changes, risks, tests, and acceptance criteria.
4. **System-1 Architect Model Pass-Through (Mandatory):** Pass every architectural decision, ambiguity resolution, schema change, crate boundary modification, dependency selection, or API/SDK contract adjustment through the designated **System-1 Architect Model** before writing implementation code. Sub-agents or phase execution workers must never invent local architectural workarounds.
5. Never invent successful test results. Execute the tests and report exact commands and outcomes.
6. Do not create placeholder implementations, fake production data, hard-coded credentials, stub success responses, or TODO-only features in completed phases.
7. Do not silently change public APIs, event schemas, database contracts, or SDK behavior. Route a migration and compatibility proposal through the System-1 Architect Model first.
8. Prefer small, reviewable changes. Keep commits phase-scoped and use Conventional Commits.
9. Do not introduce microservices in the initial release. Keep module boundaries explicit inside the modular monolith.
10. Do not optimize based on assumptions. Add benchmarks and use measurements before adding complexity.
11. Do not claim a feature is production-ready until security, failure handling, migrations, tests, and documentation are complete.
12. Never execute real external side effects while processing captured telemetry. AgentLens is an observer, not an agent execution engine.

### 0.2 Mandatory definition of done

A phase is complete only when all applicable items are true:

- Implementation matches the approved design, API contracts, and System-1 Architect Model verdicts.
- Database migrations are reversible where practical and tested from an empty database and from the previous schema version.
- Unit tests pass.
- Integration tests pass against real PostgreSQL/Redis/S3-compatible dependencies where applicable.
- Relevant end-to-end tests pass.
- Formatting, linting, type checking, and builds pass.
- Security-sensitive paths have tests.
- Documentation, ADRs, and examples are updated.
- Docker Compose starts cleanly and all health checks pass.
- No secrets, credentials, generated artifacts, or local database files are committed.
- Known limitations are documented; there are no hidden failures.

### 0.3 Never do these things

- Never bypass the System-1 Architect Model for schema, crate-graph, protocol, security, or public API decisions.
- Never store plaintext API keys, passwords, OAuth tokens, or notification secrets.
- Never trust `organization_id`, `project_id`, `environment_id`, role, or scope values supplied by a client without server-side authorization.
- Never expose PostgreSQL, Redis, object storage, or internal worker ports publicly by default.
- Never block the monitored AI application indefinitely because AgentLens is unavailable.
- Never retry telemetry forever or allow an unbounded in-memory queue.
- Never treat a missing model price as zero cost.
- Never capture prompt/output content by default without an explicit project/SDK setting.
- Never execute recorded tool calls, SQL, HTTP requests, payments, emails, deployments, or other side effects as part of monitoring or replay.
- Never put customer telemetry, secrets, or real PII into fixtures, screenshots, logs, or test data.
- Never add a dependency without passing a justification (maintenance, license, and security posture check) through the System-1 Architect Model.

### 0.4 System-1 Architect Model Pass-Through Protocol

All architecture governance uses a single-model pass-through pattern (**System-1 Architect Model**):

1. **Single Authority:** One designated primary model instance acts as the architectural gatekeeper across all phases. Coding workers, test runners, and sub-agents are strictly execution layers.
2. **When to Trigger Pass-Through:**
   - Selecting or upgrading any third-party crate, npm package, or Python library.
   - Modifying any SQL migration, constraint, or index in `migrations/`.
   - Altering crate boundaries or visibility in `crates/`.
   - Resolving any ambiguity in OTLP decoding, span normalization, pricing math, or alert evaluation.
   - Modifying `api/openapi.yaml` or public SDK interfaces.
3. **Pass-Through Payload Format:**
   ```json
   {
     "decision_id": "ADR-XXXX",
     "phase": "Phase N",
     "context": "Exact technical constraint or ambiguity encountered",
     "options_considered": ["Option A", "Option B"],
     "proposed_decision": "Concrete proposal aligned with Section 3 & Section 22",
     "impact_scope": ["crates/...", "migrations/...", "api/openapi.yaml"]
   }
   ```
4. **Verdict Recording:** Every approved pass-through decision must be logged in `docs/decisions/ADR-XXXX-<slug>.md` before the corresponding code is committed.

---

# 1. Product definition

## 1.1 Vision

AgentLens is a self-hosted, open-source observability platform for AI agents and LLM applications. It gives developers one place to monitor agents, inspect execution traces, measure model usage and cost, diagnose tool failures, compare performance, and configure operational alerts.

AgentLens observes applications. It does not host foundation models, run customer agents, proxy every model request, or replace the customer's agent framework.

## 1.2 Target users

- AI/ML engineers building LLM applications.
- Backend developers implementing AI features.
- Teams building multi-agent systems.
- AI agencies operating customer workflows.
- Platform/DevOps engineers operating AI infrastructure.
- Self-hosting organizations with data residency requirements.

## 1.3 Core user journey

1. A user deploys AgentLens using Docker Compose.
2. The user completes first-owner bootstrap and creates an organization, project, and environment.
3. The user creates a project- and environment-scoped ingestion API key.
4. The user installs an AgentLens SDK or configures a standard OpenTelemetry exporter.
5. The monitored application sends OTLP traces and spans.
6. AgentLens stores normalized telemetry durably and processes derived metrics asynchronously.
7. The user views agents, traces, models, tools, latency, token usage, and estimated cost.
8. The user configures alert rules and notification destinations.

## 1.4 Product principles

- **Open standards first:** Prefer OpenTelemetry/OTLP and interoperable data formats over a proprietary tracing protocol.
- **Self-hosting first:** Core functionality must work without an AgentLens cloud account.
- **Privacy by default:** Capture metadata by default; prompt and response capture must be explicit and configurable.
- **Low overhead:** Instrumentation must be bounded, asynchronous where possible, and safe when the collector is offline.
- **Truthful analytics:** Distinguish observed values from estimates; expose missing/unknown values rather than fabricating them.
- **Modular simplicity:** One backend codebase and two deployable processes (`agentlens-api` and `agentlens-worker`) before considering service extraction.
- **Single-model architectural consistency:** Route all architectural decisions through the System-1 Architect Model so crate boundaries, schemas, and contracts never drift.

## 1.5 Explicit non-goals for v1

- Agent execution, orchestration, or workflow authoring.
- Foundation-model hosting, training, or fine-tuning.
- A mandatory model gateway or proxy.
- Prompt marketplace or prompt IDE.
- Automated autonomous remediation.
- Full incident-to-regression replay product (possible later expansion).
- ClickHouse requirement for a basic installation.
- Kubernetes-only deployment.

---

# 2. Scope and release boundaries

## 2.1 MVP (first useful public release)

The first useful release must deliver one complete vertical slice:

- Docker Compose installation.
- Bootstrap owner and organization.
- Project and environment creation (with automatic default environments created per project).
- Scoped ingestion API key creation and revocation.
- Python SDK and TypeScript SDK capable of exporting OpenTelemetry-compatible traces.
- OTLP/HTTP trace ingestion at `POST /v1/traces`.
- Agent registration and version metadata.
- Out-of-order-safe trace and span persistence.
- Trace list and detail pages.
- Agent list and detail pages.
- Basic token and latency metrics when instrumentation supplies the data.
- SDK buffering, timeouts, retry limits, and graceful shutdown.
- Documentation and runnable examples.

## 2.2 v1.0 scope

Add the following after MVP:

- Usage and cost analytics with versioned model pricing.
- Tool analytics.
- Health indicators with transparent component metrics.
- Alert rules and notification delivery.
- Session grouping.
- Framework integrations.
- OIDC support and complete RBAC.
- Retention controls, redaction, and operational hardening.
- Backup/restore guide and upgrade process.
- Load and security testing.

## 2.3 Post-v1 candidates

- ClickHouse analytics backend when PostgreSQL measurements justify it.
- OTLP/gRPC receiver on port 4317.
- Agent dependency graph.
- Advanced anomaly detection.
- Evaluation datasets and LLM-as-a-judge.
- Prompt/version comparisons.
- Replay/simulation features with strict tool isolation.
- Kubernetes Helm chart and autoscaling guidance.
- Team/project usage quotas.

Do not implement post-v1 features ahead of the MVP acceptance gates.

---

# 3. Architecture decisions

## 3.1 Final technology choices

| Layer | Choice | Notes |
|---|---|---|
| Architecture Governance | System-1 Architect Model | Single-model pass-through for all ADRs and contract changes |
| Backend language | Rust | All backend services and workers |
| Async runtime | Tokio | Shared async runtime |
| HTTP framework | Axum | REST API, middleware, and OTLP/HTTP handlers |
| Middleware | Tower / tower-http | Timeouts, tracing, compression, decompression limits, CORS |
| Database | PostgreSQL | Source of truth for v1 |
| Database access | SQLx | Explicit SQL, migrations, and typed query support |
| OTLP decoding | `opentelemetry-proto` + `prost` | Canonical OTLP protobuf (and optional serde JSON) decoding |
| Decimal math | `rust_decimal` | Exact monetary and cost calculation matching Postgres `NUMERIC` |
| Serialization | Serde | JSON and internal data structures |
| Validation | `validator` + domain validation | Validate syntax and business rules separately |
| Logging/tracing | `tracing` ecosystem | Structured application telemetry |
| Background processing | PostgreSQL transactional outbox + workers | Durable jobs; Redis is not the sole source of truth |
| Cache/coordination | Redis | Cache, rate limits, short-lived coordination, wake-up pub/sub |
| Object storage | S3-compatible API / MinIO | Optional large payload storage |
| Frontend | Next.js App Router + TypeScript | Dashboard and admin UI |
| UI | Tailwind CSS + shadcn/ui | Accessible component system |
| Charts | Recharts | Time-series and aggregate visuals |
| Graphs | React Flow | Agent/tool relationship visualization |
| Python SDK | Python + OpenTelemetry APIs/SDK | Thin instrumentation/export layer |
| TypeScript SDK | TypeScript + OpenTelemetry APIs/SDK | Thin instrumentation/export layer |
| API contract | OpenAPI 3.1 | Dashboard and programmatic REST API |
| Telemetry protocol | OTLP/HTTP | Standard `POST /v1/traces` endpoint |
| Deployment | Docker Compose | Primary supported deployment for v1 |
| Later orchestration | Kubernetes | After Compose release is stable |

### Version policy

Do not blindly copy version numbers without verification. During Phase 0, select mutually compatible stable versions from official documentation and registries, pass the dependency manifest through the System-1 Architect Model, record exact versions in lockfiles, and commit those lockfiles. Use Dependabot or Renovate for controlled updates.

## 3.2 Modular monolith

Build one Rust workspace with clear domain crates. Run two initial processes from the same codebase:

1. `agentlens-api`: HTTP control-plane API (`/api/v1/*` on port `8080`) and OTLP/HTTP ingestion (`POST /v1/traces` mounted on both port `4318` for standard local OTLP exporters and port `8080` for single-port reverse proxies).
2. `agentlens-worker`: Durable background processing, trace finalization, usage/cost aggregation, health calculation, alert evaluation, notification delivery, and retention cleanup.

## 3.3 Proposed architecture

```text
+-------------------------------------------------------------------+
|               SYSTEM-1 ARCHITECT MODEL (Governance)               |
|  Single pass-through authority for ADRs, schemas, crate graph,    |
|  dependency versions, OpenAPI contracts, and SDK semantics        |
+-------------------------------------------------------------------+
                                  |
                                  v (Enforced Contracts)
AI APPLICATIONS
  ├── Python SDK (`agentlens`)
  ├── TypeScript SDK (`@agentlens/sdk`)
  ├── Standard OTel Exporters
  └── Framework Instrumentations
           |
           v
OTLP/HTTP Collector Endpoint (Port 4318 & Port 8080)
  POST /v1/traces
           |
           v
AGENTLENS API (`binaries/api` - Axum)
  ├── Scoped API Key Auth & Project/Environment Resolution
  ├── Payload/Decompression Limits & Protocol Decoding (`prost`)
  ├── Redaction & Attribute Validation
  ├── OTLP-to-Canonical Normalization (GenAI + OTel semantics)
  ├── Out-of-Order-Safe Upsert (Provisional Trace + Spans + Usage)
  └── Transactional Outbox Insert (`outbox_jobs`)
           |
           +--------------------------+
           |                          |
           v                          v
POSTGRESQL (Durable Authority)   REDIS (Optional Ephemeral)
  ├── Organizations / Users      ├── Rate-limit counters
  ├── Projects / Environments    ├── Short-lived query cache
  ├── Agents / Versions          └── Worker wake-up pub/sub
  ├── Traces / Spans / Events
  ├── Token Usages / Pricing
  ├── Outbox Jobs (Leased)
  └── Alerts / Deliveries / Metrics
           |
           v
AGENTLENS WORKER (`binaries/worker`)
  ├── Outbox Lease Claiming (`FOR UPDATE SKIP LOCKED`)
  ├── Trace Finalization & Root Span Reconciliation
  ├── Exact Decimal Cost Calculation & Daily Rollups
  ├── Agent Health Evaluation
  ├── Alert State Machine & SSRF-Safe Notification Delivery
  └── Retention & S3 Artifact Cleanup
           |
           v
DASHBOARD (`apps/dashboard` - Next.js)
```

## 3.4 Data durability and out-of-order ingestion decision

1. **Transactional Durability:** PostgreSQL is the authoritative durable store. `POST /v1/traces` returns HTTP `200 OK` only after the normalized traces, spans, events, links, token usages, and the corresponding `outbox_jobs` row are committed in a single PostgreSQL transaction.
2. **Out-of-Order and Multi-Batch Trace Assembly:** Spans belonging to a single `trace_id` frequently arrive across multiple HTTP requests, and child spans routinely arrive before the root span (`parent_span_id IS NULL`).
   - When a batch arrives, the ingestion transaction upserts a row in `traces` for each distinct `(project_id, trace_id)` **before** upserting `spans`.
   - If the batch contains the root span (`parent_span_id IS NULL`), `traces.name`, `traces.status`, `traces.root_span_id`, and `traces.agent_id` are set from the root span.
   - If the batch contains only child spans, `traces` is created as a provisional summary (`root_span_id = NULL`, `name` populated provisionally with `COALESCE(traces.name, EXCLUDED.name)` until the root span arrives).
   - `traces.started_at` is always updated via `LEAST(traces.started_at, EXCLUDED.started_at)`, and `traces.ended_at` via `GREATEST(traces.ended_at, EXCLUDED.ended_at)`.
   - The background `agentlens-worker` recomputes exact aggregate token counts, wall-clock duration ($\text{ended\_at} - \text{started\_at}$), and estimated cost from the persisted spans during trace finalization.

## 3.5 OTLP protocol decision

- Implement OTLP/HTTP trace ingestion at `POST /v1/traces`.
- Support `application/x-protobuf` (`ExportTraceServiceRequest` / `ExportTraceServiceResponse`) as the primary wire format.
- Support `application/json` OTLP protobuf mapping with strict hex-to-bytes decoding for `traceId` and `spanId`.
- Enforce both compressed (`Content-Length`) and post-decompression (`tower-http` `RequestDecompressionLayer` + bounded reader) size limits (default max 4 MiB compressed, 16 MiB decompressed).
- Return OTLP-compliant `ExportTraceServiceResponse` payloads (including `partial_success` when individual malformed spans are rejected) rather than the control-plane JSON envelope.
- Follow OTLP retry guidance, returning `429 Too Many Requests` or `503 Service Unavailable` with `Retry-After` headers when backpressure triggers.

## 3.6 Multi-tenancy and environment resolution

- Every tenant-owned row carries `organization_id` and `project_id` validated by composite foreign keys.
- Every newly created project automatically creates three default environments (`production`, `staging`, `development`) within the same database transaction.
- Ingestion API keys (`telemetry:write`) **must** be bound to a specific `project_id` and `environment_id`. This guarantees `traces.environment_id` is always deterministically resolved on the server from the authenticated API key, never trusted from arbitrary client payloads.

---

# 4. Repository and workspace structure

Use a Cargo workspace for Rust, `pnpm` for TypeScript, and `uv` for Python.

```text
agentlens/
├── apps/
│   ├── dashboard/                 # Next.js App Router application
│   └── docs/                      # Optional documentation site
│
├── crates/
│   ├── api/                       # Axum routes, middleware, HTTP & OTLP state
│   ├── app/                       # Use cases / application services
│   ├── domain/                    # Pure domain types, value objects, business rules
│   ├── config/                    # Typed configuration and startup validation
│   ├── database/                  # SQLx pool, transactions, repositories
│   ├── auth/                      # Sessions, CSRF, API keys, OIDC integration
│   ├── organizations/             # Organization and membership logic
│   ├── projects/                  # Projects and environments
│   ├── agents/                    # Agent registry and versions
│   ├── telemetry/                 # OTLP decoding and canonical normalization
│   ├── traces/                    # Trace/span domain and query services
│   ├── analytics/                 # Metrics, percentiles, and aggregate calculations
│   ├── pricing/                   # Versioned model pricing and rust_decimal estimates
│   ├── alerts/                    # Typed rule model, state machine, evaluator
│   ├── notifications/             # SSRF-safe adapters and delivery records
│   ├── storage/                   # S3-compatible payload storage
│   ├── jobs/                      # PostgreSQL outbox leasing and worker handlers
│   ├── privacy/                   # Redaction, attribute filtering, and retention
│   └── observability/             # Internal logs, Prometheus metrics, self-tracing guard
│
├── binaries/
│   ├── api/                       # agentlens-api entry point
│   └── worker/                    # agentlens-worker entry point
│
├── sdk/
│   ├── python/                    # `agentlens` Python package
│   └── typescript/                # `@agentlens/sdk` TypeScript package
│
├── integrations/
│   ├── langgraph/
│   ├── crewai/
│   ├── litellm/
│   ├── openai-agents/
│   └── mcp/
│
├── migrations/                    # Ordered SQLx migrations
├── api/
│   └── openapi.yaml               # OpenAPI 3.1 control-plane specification
├── deployments/
│   ├── compose/
│   ├── docker/
│   └── kubernetes/
├── examples/
│   ├── python-basic/
│   ├── typescript-basic/
│   ├── langgraph/
│   └── multi-agent/
├── docs/
│   ├── architecture/
│   ├── decisions/                 # System-1 Architect Model ADRs
│   ├── api/
│   ├── sdk/
│   ├── deployment/
│   ├── security/
│   └── contributing/
├── tests/
│   ├── e2e/
│   ├── load/
│   └── fixtures/
├── scripts/
├── .github/
│   ├── workflows/
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── pnpm-workspace.yaml
├── package.json
├── docker-compose.yml
├── Makefile
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── CODE_OF_CONDUCT.md
├── LICENSE
└── .gitignore
```

### Strict crate dependency hierarchy (DAG)

To prevent circular dependencies, all crates must obey this layered DAG (verified by the System-1 Architect Model):

- **Layer 0 (Foundation):** `domain`, `config`, `observability` (Zero dependencies on SQLx, Axum, or external I/O crates).
- **Layer 1 (Core Logic & Translation):** `privacy`, `pricing`, `telemetry` (Depend only on Layer 0).
- **Layer 2 (Infrastructure & Persistence):** `database`, `storage`, `auth`, `notifications` (Depend on Layers 0–1).
- **Layer 3 (Domain Modules):** `organizations`, `projects`, `agents`, `traces`, `analytics`, `alerts`, `jobs` (Depend on Layers 0–2).
- **Layer 4 (Orchestration & Transport):** `app` -> `api` -> `binaries/api` and `binaries/worker`.
- Use `#![forbid(unsafe_code)]` across all workspace crates.

---

# 5. Domain model and telemetry data contract

## 5.1 Core entities

- Organization
- User
- OrganizationMembership
- Project
- Environment
- ApiKey
- Agent
- AgentVersion
- Session
- Trace
- Span
- SpanEvent
- SpanLink
- TokenUsage
- ModelPricing
- OutboxJob
- AlertRule
- AlertEvent
- NotificationChannel
- NotificationDelivery
- AuditLog
- DailyMetric

## 5.2 Canonical trace model

AgentLens preserves OpenTelemetry identifiers and semantics:

- **Trace ID:** 32 lowercase hexadecimal characters (16 bytes). Must match `^[0-9a-f]{32}$` and **must not** equal `'00000000000000000000000000000000'`.
- **Span ID:** 16 lowercase hexadecimal characters (8 bytes). Must match `^[0-9a-f]{16}$` and **must not** equal `'0000000000000000'`.
- **Parent Span ID:** Optional 16 lowercase hexadecimal characters. If all-zeros in binary OTLP (`[0; 8]`), normalize to `NULL`.
- **Timestamps:** Converted from OTLP `fixed64` Unix nanoseconds into UTC `TIMESTAMPTZ` plus exact millisecond duration computation:
  $$\text{duration\_ms} = \left\lfloor \frac{\text{end\_time\_unix\_nano} - \text{start\_time\_unix\_nano}}{10^6} \right\rfloor$$
- **Status:** Canonical `unset`, `ok`, or `error` mapped from OTLP `StatusCode`.
- **Attributes:** Typed JSONB representation; never silently coerce numbers or booleans into strings.
- **Resource & Scope Attributes:** Preserved on every span and summarized on the parent trace.

## 5.3 Semantic attribute normalization (`crates/telemetry`)

The normalizer extracts canonical columns while preserving original attributes in `attributes` JSONB:

- **Provider (`provider`):** First non-empty string of `gen_ai.provider.name`, `gen_ai.system`, `llm.system`, `llm.provider`.
- **Model (`model`):** First non-empty string of `gen_ai.response.model`, `gen_ai.request.model`, `llm.model_name`.
- **Input Tokens:** First non-empty integer of `gen_ai.usage.input_tokens`, `gen_ai.usage.prompt_tokens`, `llm.token_count.prompt`.
- **Output Tokens:** First non-empty integer of `gen_ai.usage.output_tokens`, `gen_ai.usage.completion_tokens`, `llm.token_count.completion`.
- **Cached Input Tokens:** `gen_ai.usage.cache_read_input_tokens` or `agentlens.usage.cached_input_tokens`.
- **Reasoning Tokens:** `gen_ai.usage.reasoning_tokens` or `agentlens.usage.reasoning_tokens`.
- **Agent Identity:** `agent.name` (or `gen_ai.agent.name` / `service.name` fallback when `span_type = 'agent'`), `agent.version`, `agent.framework`.
- **Session Identity:** `session.id`, `gen_ai.conversation.id`, or `agentlens.session.id`.
- **Span Classification (`span_type`):** Derived deterministically from explicit `agentlens.span.type`, `gen_ai.operation.name` (`chat`, `text_completion`, `embeddings` $\rightarrow$ `llm`; `execute_tool` $\rightarrow$ `tool`; `invoke_agent` $\rightarrow$ `agent`), or standard OTel HTTP/DB attributes (`http.request.method` $\rightarrow$ `http`, `db.system` $\rightarrow$ `database`), defaulting to `custom`.

## 5.4 Agent health

Do not present a single unexplained health score as objective truth. Show independent indicators over a selected time window:

- Last seen / telemetry freshness.
- Success rate.
- Error rate.
- $P_{50}$ and $P_{95}$ latency.
- Timeout rate.
- Recent execution volume.

## 5.5 Content capture and privacy

- Capture span metadata, status, timing, provider/model identifiers, and token usage by default.
- Do not capture prompt or model response bodies by default (`capture_content_default = FALSE`).
- Allow explicit per-project and per-SDK content capture settings.
- Apply field-level redaction and regex/key blocklists before persistence and before logging.
- Store large opted-in payloads ($> 64\text{ KiB}$) in S3-compatible storage and reference via `spans.payload_s3_key`.

---

# 6. Database specification

PostgreSQL is the v1 source of truth. All monetary values use `NUMERIC(24, 12)` and `rust_decimal::Decimal`.

## 6.1 Complete DDL baseline (`migrations/`)

```sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(320) NOT NULL UNIQUE,
    password_hash TEXT,
    oidc_subject VARCHAR(255),
    oidc_issuer VARCHAR(255),
    display_name VARCHAR(255),
    status VARCHAR(30) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (oidc_issuer, oidc_subject),
    CHECK (status IN ('active', 'invited', 'disabled'))
);

CREATE TABLE organization_members (
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(30) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, user_id),
    CHECK (role IN ('owner', 'admin', 'member', 'viewer'))
);

CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL,
    description TEXT,
    capture_content_default BOOLEAN NOT NULL DEFAULT FALSE,
    redaction_rules JSONB NOT NULL DEFAULT '[]'::jsonb,
    retention_days INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, slug),
    UNIQUE (organization_id, id),
    CHECK (retention_days > 0)
);

CREATE TABLE environments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    name VARCHAR(100) NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, name),
    UNIQUE (project_id, id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    key_prefix VARCHAR(32) NOT NULL,
    key_hash BYTEA NOT NULL UNIQUE,
    scopes JSONB NOT NULL DEFAULT '["telemetry:write"]'::jsonb,
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_api_keys_prefix ON api_keys(key_prefix) WHERE revoked_at IS NULL;

CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    framework VARCHAR(100),
    language VARCHAR(50),
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    UNIQUE (project_id, name),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE agent_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    version VARCHAR(100) NOT NULL,
    configuration JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (agent_id, version),
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE CASCADE
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    external_session_id VARCHAR(255) NOT NULL,
    user_ref VARCHAR(255),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    started_at TIMESTAMPTZ,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, external_session_id),
    UNIQUE (project_id, id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE traces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    agent_id UUID,
    session_id UUID,
    trace_id VARCHAR(32) NOT NULL,
    root_span_id VARCHAR(16),
    name VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'unset',
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_ms BIGINT,
    span_count INTEGER NOT NULL DEFAULT 0,
    error_span_count INTEGER NOT NULL DEFAULT 0,
    total_input_tokens BIGINT NOT NULL DEFAULT 0,
    total_output_tokens BIGINT NOT NULL DEFAULT 0,
    total_cached_input_tokens BIGINT NOT NULL DEFAULT 0,
    total_reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    estimated_cost NUMERIC(24, 12),
    has_unpriced_usage BOOLEAN NOT NULL DEFAULT FALSE,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    resource_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finalized_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE SET NULL,
    FOREIGN KEY (project_id, session_id)
        REFERENCES sessions(project_id, id) ON DELETE SET NULL,
    CHECK (trace_id ~ '^[0-9a-f]{32}$' AND trace_id <> '00000000000000000000000000000000'),
    CHECK (root_span_id IS NULL OR (root_span_id ~ '^[0-9a-f]{16}$' AND root_span_id <> '0000000000000000')),
    CHECK (status IN ('unset', 'ok', 'error')),
    CHECK (total_input_tokens >= 0 AND total_output_tokens >= 0),
    CHECK (total_cached_input_tokens >= 0 AND total_reasoning_tokens >= 0),
    CHECK (duration_ms IS NULL OR duration_ms >= 0)
);

CREATE TABLE spans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    parent_span_id VARCHAR(16),
    agent_id UUID,
    name VARCHAR(255) NOT NULL,
    span_type VARCHAR(50) NOT NULL DEFAULT 'custom',
    status VARCHAR(20) NOT NULL DEFAULT 'unset',
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_ms BIGINT,
    provider VARCHAR(100),
    model VARCHAR(255),
    tool_name VARCHAR(255),
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    resource_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    scope_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    payload_s3_key TEXT,
    error JSONB,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id),
    FOREIGN KEY (project_id, trace_id)
        REFERENCES traces(project_id, trace_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE SET NULL,
    CHECK (trace_id ~ '^[0-9a-f]{32}$' AND trace_id <> '00000000000000000000000000000000'),
    CHECK (span_id ~ '^[0-9a-f]{16}$' AND span_id <> '0000000000000000'),
    CHECK (parent_span_id IS NULL OR (parent_span_id ~ '^[0-9a-f]{16}$' AND parent_span_id <> '0000000000000000')),
    CHECK (span_type IN ('agent', 'llm', 'tool', 'retrieval', 'chain', 'http', 'database', 'custom')),
    CHECK (status IN ('unset', 'ok', 'error')),
    CHECK (duration_ms IS NULL OR duration_ms >= 0)
);

CREATE INDEX idx_traces_project_started
    ON traces(project_id, started_at DESC, id DESC);
CREATE INDEX idx_traces_project_env_started
    ON traces(project_id, environment_id, started_at DESC, id DESC);
CREATE INDEX idx_traces_project_agent_started
    ON traces(project_id, agent_id, started_at DESC);
CREATE INDEX idx_traces_project_status_started
    ON traces(project_id, status, started_at DESC);
CREATE INDEX idx_spans_project_trace
    ON spans(project_id, trace_id, started_at);
CREATE INDEX idx_spans_project_parent
    ON spans(project_id, trace_id, parent_span_id);
CREATE INDEX idx_spans_project_started
    ON spans(project_id, started_at DESC);
CREATE INDEX idx_spans_project_tool
    ON spans(project_id, tool_name, started_at DESC)
    WHERE span_type = 'tool';

CREATE TABLE span_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    event_index INTEGER NOT NULL,
    event_name VARCHAR(255) NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id, event_index),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE
);

CREATE TABLE span_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    link_index INTEGER NOT NULL,
    linked_trace_id VARCHAR(32) NOT NULL,
    linked_span_id VARCHAR(16) NOT NULL,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id, link_index),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE
);

CREATE TABLE token_usages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    provider VARCHAR(100) NOT NULL,
    model VARCHAR(255) NOT NULL,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cached_input_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    cost_estimate NUMERIC(24, 12),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    pricing_version VARCHAR(100),
    input_rate_per_million NUMERIC(24, 12),
    output_rate_per_million NUMERIC(24, 12),
    cached_rate_per_million NUMERIC(24, 12),
    usage_source VARCHAR(30) NOT NULL DEFAULT 'provider_reported',
    recorded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE,
    CHECK (input_tokens >= 0 AND output_tokens >= 0),
    CHECK (cached_input_tokens >= 0 AND reasoning_tokens >= 0),
    CHECK (usage_source IN ('provider_reported', 'estimated'))
);

CREATE INDEX idx_token_usage_project_recorded
    ON token_usages(project_id, recorded_at DESC);

CREATE TABLE model_pricing (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider VARCHAR(100) NOT NULL,
    model_pattern VARCHAR(255) NOT NULL,
    match_type VARCHAR(20) NOT NULL DEFAULT 'exact',
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    input_per_million NUMERIC(24, 12),
    output_per_million NUMERIC(24, 12),
    cached_input_per_million NUMERIC(24, 12),
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    source_url TEXT,
    source_checked_at TIMESTAMPTZ,
    version VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, model_pattern, version),
    CHECK (match_type IN ('exact', 'prefix')),
    CHECK (input_per_million IS NULL OR input_per_million >= 0),
    CHECK (output_per_million IS NULL OR output_per_million >= 0),
    CHECK (cached_input_per_million IS NULL OR cached_input_per_million >= 0)
);

CREATE TABLE outbox_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID,
    job_type VARCHAR(100) NOT NULL,
    dedup_key VARCHAR(255),
    payload JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 10,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_expires_at TIMESTAMPTZ,
    locked_at TIMESTAMPTZ,
    locked_by VARCHAR(255),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'processing', 'completed', 'dead')),
    CHECK (attempts >= 0 AND max_attempts > 0)
);

CREATE INDEX idx_outbox_claimable
    ON outbox_jobs(available_at, created_at)
    WHERE status IN ('pending', 'processing');

CREATE TABLE alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    metric VARCHAR(100) NOT NULL,
    operator VARCHAR(10) NOT NULL,
    threshold NUMERIC(24, 6) NOT NULL,
    window_seconds INTEGER NOT NULL,
    min_sample_count INTEGER NOT NULL DEFAULT 1,
    group_by JSONB NOT NULL DEFAULT '[]'::jsonb,
    filters JSONB NOT NULL DEFAULT '{}'::jsonb,
    cooldown_seconds INTEGER NOT NULL DEFAULT 300,
    severity VARCHAR(20) NOT NULL DEFAULT 'warning',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE,
    CHECK (operator IN ('gt', 'gte', 'lt', 'lte', 'eq')),
    CHECK (severity IN ('info', 'warning', 'critical')),
    CHECK (window_seconds >= 60 AND min_sample_count >= 1 AND cooldown_seconds >= 0)
);

CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    rule_id UUID NOT NULL,
    fingerprint VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'firing',
    severity VARCHAR(20) NOT NULL,
    summary TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    first_triggered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_triggered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, rule_id)
        REFERENCES alert_rules(project_id, id) ON DELETE CASCADE,
    CHECK (status IN ('firing', 'acknowledged', 'resolved')),
    CHECK (severity IN ('info', 'warning', 'critical'))
);

CREATE INDEX idx_alert_events_active
    ON alert_events(project_id, rule_id, fingerprint)
    WHERE status IN ('firing', 'acknowledged');

CREATE TABLE notification_channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID,
    channel_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    encrypted_config TEXT NOT NULL,
    key_version VARCHAR(32) NOT NULL DEFAULT 'v1',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (channel_type IN ('webhook', 'telegram', 'discord', 'slack', 'email'))
);

CREATE TABLE notification_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_event_id UUID NOT NULL REFERENCES alert_events(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    provider_message_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'delivered', 'failed'))
);

CREATE TABLE daily_metrics (
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    metric_date DATE NOT NULL,
    dimension_type VARCHAR(50) NOT NULL,
    dimension_key VARCHAR(255) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    count_value BIGINT NOT NULL DEFAULT 0,
    sum_value NUMERIC(28, 12) NOT NULL DEFAULT 0,
    min_value NUMERIC(28, 12),
    max_value NUMERIC(28, 12),
    p50_value NUMERIC(28, 12),
    p95_value NUMERIC(28, 12),
    p99_value NUMERIC(28, 12),
    estimated_cost NUMERIC(24, 12),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE
);

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    actor_api_key_id UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,
    resource_id VARCHAR(255),
    ip_address VARCHAR(64),
    user_agent TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_logs_org_created
    ON audit_logs(organization_id, created_at DESC);
```

---

# 7. API specification

## 7.1 API conventions

- Dashboard/control-plane API base path: `/api/v1` on port `8080`.
- OTLP/HTTP trace receiver: `POST /v1/traces` on port `4318` and port `8080`.
- API format: JSON unless the endpoint is explicitly OTLP/protobuf.
- Timestamp format: RFC 3339 UTC.
- IDs: UUID for internal resources, OTLP lowercase hex IDs for trace/span identifiers.
- Pagination: cursor-based (`?cursor=<opaque_base64>&limit=50`), ordered deterministically by `(started_at DESC, id DESC)`.
- Every response includes a request ID through `X-Request-Id` header and the error/data envelope.
- OpenAPI 3.1 (`api/openapi.yaml`) is the single source of truth for the control-plane API.

## 7.2 Authentication

Dashboard user authentication:

- Secure session cookie (`HttpOnly`, `Secure` in production, `SameSite=Lax`).
- CSRF token header (`X-CSRF-Token`) required on all cookie-authenticated state-changing endpoints (`POST`, `PATCH`, `PUT`, `DELETE`).
- Atomic first-owner bootstrap at `POST /api/v1/auth/bootstrap` (disabled permanently as soon as the first user exists).
- Argon2id for local passwords.
- OIDC integration (`GET /api/v1/auth/oidc/start` and `GET /api/v1/auth/oidc/callback`) with state, nonce, and PKCE validation.

SDK ingestion authentication:

```http
Authorization: Bearer agl_live_<secret>
```

- High-entropy 256-bit random secret encoded in base62.
- Displayed only once at creation.
- Stored as a SHA-256 hash in `api_keys.key_hash`; prefix stored in `api_keys.key_prefix` for indexed lookup, verified via constant-time comparison.
- Scoped to one `project_id` and one `environment_id` with `telemetry:write` scope.
- Ingestion keys are strictly forbidden from accessing `/api/v1/*` control-plane endpoints.

## 7.3 Core endpoint inventory

### Auth and account
- `POST /api/v1/auth/bootstrap` — Initial owner & organization setup (when user count is 0)
- `POST /api/v1/auth/login` — Local login
- `POST /api/v1/auth/logout` — End session
- `GET /api/v1/auth/me` — Current user & active memberships
- `GET /api/v1/auth/oidc/start` — Start OIDC flow
- `GET /api/v1/auth/oidc/callback` — OIDC callback

### Organizations and projects
- `GET /api/v1/organizations` — List accessible organizations
- `POST /api/v1/organizations` — Create organization
- `GET /api/v1/organizations/{org_id}` — Organization details
- `GET /api/v1/projects` — List accessible projects
- `POST /api/v1/projects` — Create project (and default environments)
- `GET /api/v1/projects/{project_id}` — Project details
- `PATCH /api/v1/projects/{project_id}` — Update project settings/redaction/retention
- `DELETE /api/v1/projects/{project_id}` — Delete project
- `GET /api/v1/projects/{project_id}/environments` — List environments
- `POST /api/v1/projects/{project_id}/environments` — Create environment

### API keys
- `GET /api/v1/projects/{project_id}/api-keys` — List key metadata
- `POST /api/v1/projects/{project_id}/api-keys` — Create key; return plaintext secret once
- `POST /api/v1/api-keys/{key_id}/rotate` — Rotate key
- `DELETE /api/v1/api-keys/{key_id}` — Revoke key

### Agents
- `GET /api/v1/projects/{project_id}/agents` — List agents
- `POST /api/v1/projects/{project_id}/agents` — Register agent
- `GET /api/v1/agents/{agent_id}` — Agent detail
- `PATCH /api/v1/agents/{agent_id}` — Update metadata
- `GET /api/v1/agents/{agent_id}/versions` — Version history
- `GET /api/v1/agents/{agent_id}/health` — Component health indicators
- `GET /api/v1/agents/{agent_id}/traces` — Agent traces

### Traces and sessions
- `GET /api/v1/traces` — Search/list traces with cursor pagination
- `GET /api/v1/traces/{trace_record_id}` — Trace summary
- `GET /api/v1/traces/{trace_record_id}/spans` — Trace spans for waterfall tree
- `GET /api/v1/spans/{span_record_id}` — Span detail, events, links, and opted-in payload
- `GET /api/v1/sessions` — List sessions
- `GET /api/v1/sessions/{session_id}` — Session detail
- `GET /api/v1/sessions/{session_id}/traces` — Session traces

### Analytics
- `GET /api/v1/analytics/overview` — Overview cards and time-series
- `GET /api/v1/analytics/tokens` — Token usage breakdown
- `GET /api/v1/analytics/costs` — Estimated costs and unpriced usage indicator
- `GET /api/v1/analytics/models` — Model/provider statistics
- `GET /api/v1/analytics/agents` — Agent performance statistics
- `GET /api/v1/analytics/tools` — Tool invocation and error statistics
- `GET /api/v1/analytics/latency` — Latency distributions ($P_{50}, P_{95}, P_{99}$)
- `GET /api/v1/analytics/errors` — Error analysis

### Alerts and notification channels
- `GET /api/v1/alert-rules` — List rules
- `POST /api/v1/alert-rules` — Create rule
- `PATCH /api/v1/alert-rules/{rule_id}` — Update rule
- `DELETE /api/v1/alert-rules/{rule_id}` — Delete rule
- `GET /api/v1/alert-events` — Alert history
- `POST /api/v1/alert-events/{event_id}/acknowledge` — Acknowledge event
- `GET /api/v1/notification-channels` — List channel metadata
- `POST /api/v1/notification-channels` — Create channel
- `POST /api/v1/notification-channels/{id}/test` — Send test notification
- `DELETE /api/v1/notification-channels/{id}` — Delete channel

### Internal operations
- `GET /health/live` — Process liveness
- `GET /health/ready` — Dependency and migration readiness check
- `GET /metrics` — Internal Prometheus metrics (private/protected)

---

# 8. SDK specification

## 8.1 SDK philosophy

AgentLens SDKs (`sdk/python` and `sdk/typescript`) are thin, OpenTelemetry-compatible instrumentation and configuration packages. They export standard OTLP/HTTP traces to `/v1/traces` while providing ergonomic helpers for agents, models, and tools.

## 8.2 Python SDK (`agentlens`)

- Uses `opentelemetry-sdk` and `opentelemetry-exporter-otlp-proto-http`.
- Defaults `capture_content=False`. When `False`, a local `SpanProcessor` strips prompt/completion bodies before spans enter the batch queue.
- Bounded queue (`max_queue_size=2048`), connect/read timeouts, exponential backoff with jitter, `Retry-After` compliance, atomic dropped-span counter, and bounded `flush(timeout=5)`.

## 8.3 TypeScript SDK (`@agentlens/sdk`)

- Uses `@opentelemetry/sdk-trace-base` and `@opentelemetry/exporter-trace-otlp-proto`.
- Targets Node.js server environments first; never exposes ingestion keys in browser bundles.
- Provides `withSpan` and semantic attribute helpers, local `SpanProcessor` redaction (`captureContent: false` default), bounded buffering, drop counters, and `flush({ timeoutMs: 5000 })`.

---

# 9. Frontend specification

## 9.1 Main navigation and MVP views

- **Overview:** Active agents, total traces, error rate, input/output tokens, estimated cost (with unpriced usage badge), $P_{50}/P_{95}$ latency, recent traces, recent errors.
- **Agents List & Detail:** Agent metadata, framework, versions, last seen, execution time-series, component health metrics, and filtered trace list.
- **Trace Explorer & Trace Detail:** Multi-dimensional filter bar (project, environment, agent, status, model, tool, duration, time range), cursor pagination, interactive span waterfall timeline, parent-child tree, token/cost breakdown, events, and redacted error stack traces.
- **Analytics:** Time-series charts for tokens, decimal cost estimates, model/provider comparisons, tool failure rates, and latency percentiles.
- **Alerts & Settings:** Constrained rule builder, alert event state history (`firing`, `acknowledged`, `resolved`), encrypted notification channels, API key management, and retention configuration.

---

# 10. Analytics and pricing logic

1. **Exact Decimal Cost Math:**
   $$\text{cost\_estimate} = \frac{\text{non\_cached\_input} \times R_{\text{in}} + \text{cached\_input} \times R_{\text{cache}} + (\text{output} + \text{reasoning}) \times R_{\text{out}}}{10^6}$$
   where all rates $R$ and results use `rust_decimal::Decimal` and PostgreSQL `NUMERIC(24, 12)`.
2. **Unknown Price Handling:** If no active `model_pricing` row matches `(provider, model)` at `recorded_at`, `cost_estimate` **must** remain `NULL` and `traces.has_unpriced_usage` **must** be set to `TRUE`.
3. **Latency Percentiles:** Computed via PostgreSQL `percentile_cont(ARRAY[0.50, 0.95, 0.99]) WITHIN GROUP (ORDER BY duration_ms)` over bounded time windows. Never average percentiles across buckets, and never sum nested span durations as wall-clock trace duration.

---

# 11. Alerting and notifications

- **Allowed Metrics:** `agent_error_rate`, `agent_p95_latency_ms`, `agent_inactivity_seconds`, `llm_error_rate`, `tool_failure_rate`, `token_usage_total`, `estimated_cost_usd`.
- **State Machine:** `OK -> FIRING -> ACKNOWLEDGED -> RESOLVED` with deterministic fingerprint deduplication and `cooldown_seconds` anti-flapping suppression.
- **SSRF Defense:** Custom DNS/IP validation at socket connection time blocking loopback (`127.0.0.0/8`, `::1`), RFC 1918 private ranges, link-local/cloud metadata (`169.254.0.0/16`, `fe80::/10`), and CGNAT (`100.64.0.0/10`), with HTTP redirects disabled (`Policy::none()`).

---

# 12. Security, privacy, reliability, and operations

- **Secrets:** Argon2id for passwords; SHA-256 + constant-time compare for high-entropy API keys; versioned AES-256-GCM (`v1:<nonce>:<ciphertext>`) for notification channel configs using `APP_ENCRYPTION_KEY`. Fail startup in production if `APP_ENCRYPTION_KEY` or `SESSION_SECRET` is missing or weak.
- **Reliability Targets:** Benchmark with 100 concurrent clients and 1,000 spans/sec sustained; verify zero acknowledged data loss across worker crashes and Redis restarts.
- **Docker Compose Hardening:** Multi-stage builds, non-root runtime user, internal-only networks for Postgres/Redis/MinIO, and verified backup/restore scripts.

---

# 13. Decisions the agent must not change without a System-1 ADR

1. All architectural, schema, dependency, crate-boundary, and contract decisions pass through the **System-1 Architect Model**.
2. Rust is the backend language; Next.js/TypeScript is the dashboard stack.
3. Python and TypeScript SDKs are first-class OpenTelemetry-compatible deliverables.
4. Architecture is a modular monolith (`agentlens-api` + `agentlens-worker`) for v1.
5. PostgreSQL is the v1 durable source of truth; Redis is never the sole durable queue.
6. Standard OTLP/HTTP trace endpoint is `POST /v1/traces` (served on ports `4318` and `8080`).
7. Content capture is off by default (`capture_content = false`).
8. SDK export is bounded and fail-open by default.
9. Tenant isolation is enforced on every query and mutation path.
10. Unknown model pricing remains `NULL` (unknown), never `0`.
11. AgentLens observes customer applications; it never executes recorded tools or external side effects.