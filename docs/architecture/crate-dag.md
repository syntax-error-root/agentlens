# AgentLens Crate Dependency Hierarchy (DAG)

All crates within the workspace must strictly adhere to this layered dependency structure:

- **Layer 0 (Foundation):**
  - `domain`: Core domain models, value objects, W3C ID validation. No I/O.
  - `config`: Environment configuration, validation. No database or HTTP dependencies.
  - `observability`: Internal metrics, tracing, logging subscribers.
- **Layer 1 (Core Logic & Translation):**
  - `privacy`: Redaction, PII scrubbing, attribute filtering. Depends on Layer 0.
  - `pricing`: Model rates, currency calculation, exact decimal math. Depends on Layer 0.
  - `telemetry`: OTLP protobuf/JSON parsing, canonical GenAI semantic normalization. Depends on Layer 0.
- **Layer 2 (Infrastructure & Persistence):**
  - `database`: PostgreSQL connection pools, SQLx migrations, repository traits. Depends on Layers 0–1.
  - `storage`: S3-compatible large payload storage. Depends on Layers 0–1.
  - `auth`: Sessions, Argon2id passwords, SHA-256 API key hashing, RBAC. Depends on Layers 0–1.
  - `notifications`: SSRF-safe webhook and messaging notification dispatch. Depends on Layers 0–1.
- **Layer 3 (Domain Modules):**
  - `organizations`: Organization, user memberships.
  - `projects`: Project and environment boundaries.
  - `agents`: Agent registry, versions, component health metrics.
  - `traces`: Trace and span querying, out-of-order reconciliation.
  - `analytics`: Token summaries, latency percentiles, rollups.
  - `alerts`: Alert evaluation state machine.
  - `jobs`: PostgreSQL transactional outbox leasing (`FOR UPDATE SKIP LOCKED`).
  - Depends on Layers 0–2.
- **Layer 4 (Orchestration & Transport):**
  - `app`: Application use cases coordinating domain modules.
  - `api`: Axum HTTP routes, middleware, OTLP receiver endpoints.
  - `binaries/api`: Entrypoint for `agentlens-api`.
  - `binaries/worker`: Entrypoint for `agentlens-worker`.

Every crate enforces `#![forbid(unsafe_code)]`.
