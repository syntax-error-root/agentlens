# AgentLens

> **Monitor. Trace. Understand AI.**  
> Self-hosted, open-source AI agent observability platform written in Rust.

---

## Overview

AgentLens provides unified observability for AI agents and LLM applications. It collects execution traces, tracks model usage and token costs, diagnoses tool failures, monitors performance metrics, and triggers operational alerts—all under your own infrastructure.

### Architectural Invariants
- **Rust-First Modular Monolith:** 18 layered domain/infrastructure crates with `#![forbid(unsafe_code)]` powering two binaries (`agentlens-api` and `agentlens-worker`).
- **PostgreSQL Durable Authority:** Atomic trace ingestion and outbox pattern (`FOR UPDATE SKIP LOCKED`) guaranteeing zero data loss.
- **Out-of-Order OTLP Ingestion:** Provisional trace records accommodate out-of-order span arrival.
- **Privacy by Default:** Prompt and response content capture is strictly opt-in (`capture_content = false`).
- **Truthful Analytics:** Missing model rates surface as `NULL` / `has_unpriced_usage = true` instead of fabricated $0.00 metrics.

---

## Quickstart

```bash
# Clone the repository
git clone https://github.com/syntax-error-root/agentlens.git
cd agentlens

# Start infrastructure with Docker Compose
docker compose up -d postgres redis

# Build and run API
cargo run --bin agentlens-api
```

---

## License

Apache-2.0 License. See [LICENSE](LICENSE) for details.
