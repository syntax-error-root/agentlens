# Contributing to AgentLens

Thank you for your interest in contributing to AgentLens! As an open-source, production-grade observability platform, we maintain strict architectural standards to ensure zero data loss, deterministic operations, and robust security.

---

## Code of Conduct

All contributors are expected to uphold our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Architectural Principles & Rules

1. **Rust-First Core:**
   All crates under `crates/` and `binaries/` must enforce `#![forbid(unsafe_code)]`. No `unsafe` blocks are permitted anywhere in the workspace.
2. **Layered DAG Architecture:**
   Dependencies must strictly flow downwards through the crate layers (Layer 0 Domain -> Layer 1 Foundation -> Layer 2 Business Logic -> Layer 3 Infrastructure -> Layer 4 Applications). Upward or circular dependencies are rejected.
3. **Transactional Authority:**
   All ingestion writes and outbox job schedules must be atomic within a single PostgreSQL transaction.
4. **Privacy by Default:**
   Content capture (`capture_content`) must default to `false`. Raw secrets (API keys, authorization headers, private keys) must never be logged or persisted in unredacted form.
5. **Architectural Decision Records (ADRs):**
   Substantial architectural modifications or new external protocol dependencies require an ADR in `docs/decisions/` before implementation.

---

## Local Development Workflow

### Prerequisites
- Rust 1.85+ (`cargo`, `rustfmt`, `clippy`)
- Node.js 20+ & `pnpm`
- Python 3.10+ & `uv`
- Docker & Docker Compose

### Building & Running
```bash
# Start background dependencies
docker compose up -d postgres redis minio

# Run database migrations and start API
cargo run --bin agentlens-api

# Start worker binary in a separate shell
cargo run --bin agentlens-worker

# Run Next.js dashboard
pnpm --filter dashboard dev
```

---

## Verification Suite

Before submitting any Pull Request, you must verify that all formatting, linting, and tests pass with zero warnings:

```bash
# Rust verification
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# TypeScript / Dashboard verification
pnpm --filter dashboard lint
pnpm --filter dashboard typecheck
pnpm --filter dashboard test
pnpm --filter dashboard build
pnpm --filter @agentlens/sdk test

# Python verification
cd sdk/python && uv run ruff check . && uv run mypy src && uv run pytest
```

---

## Commit Guidelines

We enforce the [Conventional Commits](https://www.conventionalcommits.org/) specification:
- `feat:` A new user-facing capability or API
- `fix:` A bug fix
- `docs:` Documentation updates
- `test:` Adding or refactoring test suites
- `refactor:` Code refactoring without behavioral alterations
- `chore:` Build scripts, dependencies, or maintenance tasks
