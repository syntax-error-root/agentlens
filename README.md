# AgentLens

<div align="center">

**Monitor. Trace. Understand AI.**

[![CI](https://github.com/syntax-error-root/agentlens/actions/workflows/ci.yml/badge.svg)](https://github.com/syntax-error-root/agentlens/actions)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![TypeScript](https://img.shields.io/badge/typescript-5.5%2B-blue.svg)](https://www.typescriptlang.org)
[![Python](https://img.shields.io/badge/python-3.10%2B-yellow.svg)](https://www.python.org)

*Open-source, self-hosted, Rust-first AI agent observability platform.*

</div>

---

## Highlights

- **Rust-First High-Throughput Engine:** Core ingestion and background workers written in Rust (`#![forbid(unsafe_code)]`), capable of processing > 1,400 spans/sec with sub-6ms $P_{50}$ latency.
- **OpenTelemetry & GenAI Semantic Conventions:** Native OTLP/HTTP receiver (`POST /v1/traces` on ports `4318` and `8080`), compatible with standard OpenTelemetry collectors, OpenInference, and custom SDKs.
- **Durable PostgreSQL Authority:** Transactional outbox engine (`FOR UPDATE SKIP LOCKED`) providing zero-data-loss durability and seamless handling of out-of-order span arrival.
- **Privacy by Default:** Prompt, completion, and tool payloads are never captured unless explicitly opted in (`capture_content = false`). Sensitive attributes, credentials, and API keys are automatically redacted in-stream.
- **Versioned Exact Model Pricing:** Exact decimal cost tracking (`rust_decimal`) with versioned provider rate cards. Unpriced models surface truthfully as `NULL` / `has_unpriced_usage` rather than misleading $0.00 estimates.
- **Production-Ready Dashboard:** Next.js App Router console featuring first-owner bootstrap, agent registry, cursor-paginated trace explorer, interactive span waterfall, and analytics charts.
- **Framework Integrations:** First-class, non-invasive callbacks and adapters for **LangGraph**, **LiteLLM**, **CrewAI**, **OpenAI Agents SDK**, and **MCP (Model Context Protocol)** tools.
- **Production Hardened:** Non-root OCI containers (`UID:GID 65532:65532`), network-isolated compose tiers, automated large payload offloading to S3/MinIO (> 64 KiB), retention cleanup, and verified SHA-256 backup/restore tooling.

---

## System Architecture

```
                      +-----------------------------+
                      |   AI Agents & Frameworks    |
                      | (Python, TS, LangGraph, MCP)|
                      +--------------+--------------+
                                     | OTLP / HTTP (4318 / 8080)
                                     v
                      +-----------------------------+
                      |     agentlens-api (Rust)     |
                      |  - Ingestion Authentication  |
                      |  - Secret Redaction Engine   |
                      |  - ACID Persistence         |
                      +--------------+--------------+
                                     |
             +-----------------------+-----------------------+
             |                                               |
             v                                               v
+--------------------------+                   +---------------------------+
|      PostgreSQL 16       |                   |       Object Storage      |
|  - Traces & Spans        |                   | (MinIO / S3 > 64 KiB)     |
|  - Transactional Outbox  |                   |                           |
+------------+-------------+                   +-------------+-------------+
             ^                                               ^
             | FOR UPDATE SKIP LOCKED                        |
+------------+-------------+                                 |
|   agentlens-worker (Rust) |---------------------------------+
|  - Aggregations          | (Prune expired payloads)
|  - Rate Card Pricing     |
|  - Alert Rule Evaluation |
|  - Retention Pruning     |
+------------+-------------+
             |
             v
+--------------------------+                   +---------------------------+
| Alert Channels (SSRF-Safe)|                   |    apps/dashboard (Next)  |
| (Slack, Discord, TG, Web) |                   |  (Browser Console & Trace)|
+--------------------------+                   +---------------------------+
```

---

## Quickstart

### 1. Launch with Docker Compose

Clone the repository and launch the self-hosted stack:

```bash
git clone https://github.com/syntax-error-root/agentlens.git
cd agentlens

# Start PostgreSQL, Redis, MinIO, API, Worker, and Dashboard
docker compose up -d
```

Once running:
- **Web Dashboard:** [http://localhost:3000](http://localhost:3000)
- **OTLP Ingestion Receiver:** `http://localhost:4318/v1/traces`
- **Control-Plane API:** `http://localhost:8080/api/v1`

First time setup? Visit [http://localhost:3000/bootstrap](http://localhost:3000/bootstrap) to create your initial administrator account and organization.

---

## Instrumentation & SDKs

### Python SDK (`agentlens`)

Install the lightweight Python SDK:

```bash
pip install agentlens
# Or with optional framework integrations:
pip install "agentlens[langgraph,litellm,crewai]"
```

Basic instrumentation:

```python
from agentlens import AgentLens

client = AgentLens(
    endpoint="http://localhost:4318",
    api_key="agl_live_your_project_api_key",
    agent_name="support-agent",
    agent_version="1.0.0",
)

with client.trace("user-session-123", session_id="sess_42") as trace:
    with trace.span("call_llm", span_type="llm") as span:
        span.set_attribute("gen_ai.system", "openai")
        span.set_attribute("gen_ai.request.model", "gpt-4o")
        span.set_attribute("gen_ai.usage.input_tokens", 250)
        span.set_attribute("gen_ai.usage.output_tokens", 85)
```

### TypeScript SDK (`@agentlens/sdk`)

Install via npm or pnpm:

```bash
pnpm add @agentlens/sdk
```

Basic instrumentation:

```typescript
import { AgentLens } from "@agentlens/sdk";

const client = new AgentLens({
  endpoint: "http://localhost:4318",
  apiKey: "agl_live_your_project_api_key",
  agentName: "billing-agent",
  agentVersion: "1.0.0",
});

await client.withTrace("query-billing", async (trace) => {
  await trace.withSpan("calculate-invoice", { spanType: "tool" }, async (span) => {
    span.setAttribute("billing.customer_id", "cust_99");
  });
});
```

---

## Production Operations

### Hardened Production Deployment

To run in production environments with non-root security contexts (`UID:GID 65532:65532`) and completely unexposed database ports:

```bash
docker compose -f deployments/docker/docker-compose.prod.yml up -d
```

### Backup & Disaster Recovery

AgentLens includes automated, checksum-verified backup and restore tools:

```bash
# Capture verified snapshot with SHA-256 manifest
./scripts/backup.sh

# Validate and restore snapshot
./scripts/restore.sh ./backups/20261113T160000Z --force
```

Refer to the [Backup & Restore Runbook](docs/deployment/backup-restore.md) for full disaster recovery drill instructions.

---

## Documentation

- [Architecture & Crate DAG](docs/architecture/crate-dag.md)
- [Performance Benchmarks](docs/architecture/benchmarks.md)
- [Backup & Disaster Recovery Guide](docs/deployment/backup-restore.md)
- [OpenAPI Specification](docs/api/openapi.yaml)
- [Architectural Decision Records (ADRs)](docs/decisions/)

---

## License

AgentLens is distributed under the [Apache-2.0 License](LICENSE).
