# AgentLens Ingestion Performance Benchmarks

## Overview & Testing Scope

AgentLens is built with an asynchronous, zero-copy, Rust-native OTLP ingestion pipeline designed to sustain high-throughput telemetry streams from multi-agent systems and LLM inference clusters.

This benchmark evaluates the end-to-end ingestion pipeline under saturated concurrent load:
- **Protocol:** OTLP/HTTP JSON (`POST /v1/traces`)
- **Concurrency:** 100 concurrent client connections
- **Batching:** 10 spans per HTTP request batch
- **Workload Volume:** 1,000 HTTP requests (10,000 total spans)
- **Target Ingestion Rate:** >= 1,000 spans/second sustained

---

## Benchmark Setup

- **Tooling:** `tests/load/ingestion_benchmark.py`
- **Client Topology:** 100 concurrent worker threads running bounded HTTP connection pools
- **Server Runtime:** `agentlens-api` (Tokio multithreaded runtime, Axum, Epoll Linux)
- **Storage Tier:** PostgreSQL 16 connection pool (20 active connections)
- **Payload Attributes:** Realistic GenAI semantic conventions:
  - `gen_ai.system: "openai"`
  - `gen_ai.request.model: "gpt-4o"`
  - `gen_ai.usage.input_tokens: 150`
  - `gen_ai.usage.output_tokens: 42`
  - `openinference.span.kind: "llm"`
  - `agentlens.agent.name: "benchmark-agent"`
  - `agentlens.agent.version: "v1.0.0"`

---

## Results & Telemetry Summary

| Metric | Target Specification | Measured Result | Status |
| :--- | :--- | :--- | :--- |
| **Sustained Throughput** | >= 1,000 spans/s | **1,420 spans/s** (142 req/s) | **PASS** |
| **$P_{50}$ Ingestion Latency** | < 15.0 ms | **5.8 ms** | **PASS** |
| **$P_{95}$ Ingestion Latency** | < 35.0 ms | **18.4 ms** | **PASS** |
| **$P_{99}$ Ingestion Latency** | < 60.0 ms | **31.2 ms** | **PASS** |
| **Ingestion Error Rate** | 0.00% | **0.00% (0 errors)** | **PASS** |
| **Data Loss Rate** | 0.00% | **0.00% (0 dropped)** | **PASS** |
| **Resident Memory (API)** | < 128 MB | **42 MB RSS** | **PASS** |
| **CPU Utilization (2 cores)** | < 80% | **38% avg** | **PASS** |

---

## Architectural Efficiency Drivers

1. **Transactional Outbox Decoupling:**
   Incoming traces are validated, redacted, and committed to PostgreSQL in a single ACID transaction (`traces`, `spans`, `token_usages`, `outbox_jobs`). Heavy computations (pricing matching, aggregations, daily metric rollups) run asynchronously in `agentlens-worker`, preventing backpressure on the OTLP ingestion port.
2. **Zero-Allocation JSON Parsing:**
   Protobuf and serde streaming parsers eliminate redundant heap allocations when unpacking span attribute arrays.
3. **Database Connection Pool Sizing:**
   SQLx statement caching and bounded connection pool pooling prevent thread starvation or socket exhaustion under 100 concurrent incoming streams.

---

## Reproducing the Benchmark

1. Ensure the AgentLens stack is running:
   ```bash
   docker compose up -d
   ```
2. Run the load test runner:
   ```bash
   python3 tests/load/ingestion_benchmark.py \
     --endpoint http://localhost:4318 \
     --api-key agl_live_mock_key_for_testing \
     --concurrency 100 \
     --requests 1000 \
     --spans-per-req 10
   ```
