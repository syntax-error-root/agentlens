# AgentLens — GitHub Workflow & Step-by-Step Push Playbook

> **Purpose:** Place this file in your `agent/` directory alongside the phase specifications. The implementation agent must follow this playbook for every commit, branch, pull request, CI check, and release tag from **Phase 00** through **Phase 11**.

---

## 0\. Non-Negotiable Git & GitHub Rules for the Agent

1. **Never Commit Secrets or Local State:** Before every `git add`, verify that `.env`, `.env.local`, `*.pem`, `*.key`, SQLite/Postgres data directories, MinIO buckets, `target/`, `node_modules/`, `.next/`, and `.venv/` are excluded via `.gitignore`.  
2. **One Phase \= One Branch \= One Pull Request (or Atomic Phase Merge):** Never mix multiple phases in a single branch or pull request. Do not start the next phase branch until the current phase branch passes CI and is merged into `main`.  
3. **System-1 Architect Traceability:** Every phase commit message and pull request body must reference the approved **System-1 Architect Model** ADR IDs (`System-1-ADR: ADR-XXXX`).  
4. **Never Push Broken Code:** Run all local formatting, linter, typecheck, and test gates **before** executing `git commit` or `git push`.  
5. **Never Use `--no-verify` or `--force` on `main`:** Do not bypass pre-commit hooks, and never force-push to `main`.  
6. **Use Conventional Commits:** All commit titles must follow `<type>(<scope>): <imperative summary>` (`feat`, `fix`, `docs`, `test`, `ci`, `chore`, `refactor`, `perf`).

---

## 1\. Repository Bootstrap & Hardened `.gitignore`

Before pushing the first commit in **Phase 00**, ensure the repository root has this exact `.gitignore` and initialize the `main` branch cleanly.

### 1.1 Required `.gitignore`

```gitignore
# Rust
/target/
**/*.rs.bk
Cargo.lock.bak

# Node / Next.js / TypeScript
node_modules/
.pnp
.pnp.js
.next/
out/
build/
dist/
*.tsbuildinfo
next-env.d.ts
.pnpm-debug.log*

# Python
__pycache__/
*.py[cod]
*$py.class
.venv/
venv/
ENV/
.pytest_cache/
.mypy_cache/
.ruff_cache/
*.egg-info/
.eggs/

# Environment & Secrets (CRITICAL: NEVER COMMIT)
.env
.env.*
!.env.example
*.pem
*.key
*.crt
*.p12
secrets/

# Local Infrastructure & Volumes
.data/
postgres-data/
redis-data/
minio-data/
*.db
*.sqlite
*.sqlite3

# Testing & Coverage
coverage/
lcov.info
playwright-report/
test-results/

# OS & IDEs
.DS_Store
Thumbs.db
.idea/
.vscode/*
!.vscode/settings.json
!.vscode/extensions.json
*.swp
*.swo
```

&nbsp;

### 1.2 Initial Git & GitHub Remote Setup

```sh
# 1. Initialize repository on main branch
git init -b main

# 2. Configure commit hygiene
git config pull.rebase true
git config core.autocrlf input

# 3. Verify remote (or create via GitHub CLI)
# Using GitHub CLI (if creating a new repo):
gh repo create agentlens --public --source=. --remote=origin --description "Monitor. Trace. Understand AI. Open-source AI agent observability platform in Rust."

# Or attach to an existing GitHub repository:
# git remote add origin git@github.com:<ORG_OR_USER>/agentlens.git
```

---

## 2\. Pre-Push Verification Ritual (Mandatory Before Every Push)

Before pushing **any** commit in any phase, the agent must execute this 6-step ritual in order:

### Step 1: Secret & Untracked File Audit

```sh
git status --ignored
# Verify no .env, credentials, API keys, or binary build artifacts are staged
git status
```

### Step 2: Automated Secret Pattern Check on Staged Changes

```sh
# Stage intended files explicitly (avoid blind `git add .` when untracked files exist)
git add <files>

# Scan staged diff for accidental secrets or private keys
if git diff --cached | grep -E "(agl_live_[a-zA-Z0-9]{16,}|BEGIN (RSA|OPENSSH|EC|PGP) PRIVATE KEY|POSTGRES_PASSWORD=[^R]|APP_ENCRYPTION_KEY=[0-9a-fA-F]{32,})"; then
  echo "ERROR: Potential secret detected in staged diff! Aborting."
  exit 1
fi
```

### Step 3: Execute Phase Quality Gates

Run the phase-appropriate verification suite (from Phase 01 onward):

```sh
# Rust checks
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# Frontend checks
pnpm --filter dashboard lint
pnpm --filter dashboard typecheck
pnpm --filter dashboard build

# TypeScript SDK checks
pnpm --filter @agentlens/sdk lint
pnpm --filter @agentlens/sdk typecheck
pnpm --filter @agentlens/sdk test

# Python SDK checks
cd sdk/python && uv run ruff check . && uv run ruff format --check . && uv run mypy src && uv run pytest && cd ../..
```

### Step 4: Inspect Final Staged Diff

```sh
git diff --cached --stat
```

### Step 5: Commit with System-1 Architect Trailer

Use a structured commit message that records the System-1 ADR verdict and exact test status:

```sh
git commit -m "<type>(<scope>): <concise summary>" \
  -m "- Detail 1" \
  -m "- Detail 2" \
  -m "System-1-ADR: ADR-0001, ADR-0002" \
  -m "Verified-By: cargo test --workspace, pnpm build, pytest"
```

### Step 6: Push Branch, Open PR, Wait for Green CI, and Merge

```sh
git push -u origin <branch-name>

# Create Pull Request with GitHub CLI
gh pr create \
  --base main \
  --head <branch-name> \
  --title "<type>(<scope>): <Phase Title>" \
  --body-file <pr-summary-file-or-inline>

# Watch GitHub Actions CI until all checks pass
gh pr checks --watch

# Merge only after all checks are green
gh pr merge --squash --delete-branch
git checkout main
git pull --ff-only origin main
```

---

## 3\. Step-by-Step Phase Push Playbook (Phase 00 to Phase 11\)

Follow these exact branch names, commit structures, and tag milestones for every phase.

---

### Phase 00 — Repository Reconnaissance & System-1 ADRs

```sh
# 1. Create branch
git checkout -b docs/phase-00-recon-and-adrs

# 2. Stage governance, blueprint, and ADR files
git add .gitignore README.md LICENSE CODE_OF_CONDUCT.md CONTRIBUTING.md SECURITY.md
git add agent/ docs/decisions/ docs/architecture/

# 3. Verify staged diff
git diff --cached --stat

# 4. Commit
git commit -m "docs(arch): complete phase 00 reconnaissance and system-1 ADRs" \
  -m "- Add System-1 Architect governance and master implementation blueprint" \
  -m "- Add ADR-0001 through ADR-0006 in docs/decisions/" \
  -m "- Document Layer 0-4 crate DAG, dependency matrix, and risk register" \
  -m "System-1-ADR: ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006"

# 5. Push and merge
git push -u origin docs/phase-00-recon-and-adrs
gh pr create --base main --head docs/phase-00-recon-and-adrs \
  --title "docs(arch): Phase 00 — Reconnaissance and System-1 ADRs" \
  --body "Completes Phase 00 acceptance gate. Adds ADR-0001..0006, crate DAG, dependency matrix, and risk register."
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase00 -m "Milestone M0: Architecture and System-1 ADRs"
git push origin v0.1.0-phase00
```

---

### Phase 01 — Workspace and Development Foundation

```sh
# 1. Create branch
git checkout -b feat/phase-01-workspace-foundation

# 2. Run all verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard build
pnpm --filter @agentlens/sdk typecheck
cd sdk/python && uv run ruff check . && uv run mypy src && uv run pytest && cd ../..

# 3. Stage files
git add Cargo.toml Cargo.lock rust-toolchain.toml rustfmt.toml clippy.toml
git add package.json pnpm-workspace.yaml pnpm-lock.yaml
git add Makefile docker-compose.yml .env.example
git add .github/ crates/ binaries/ migrations/ apps/dashboard/ sdk/

# 4. Commit
git commit -m "feat(workspace): initialize monorepo foundation and health checks" \
  -m "- Scaffold 18 workspace crates obeying Layer 0-4 DAG with #![forbid(unsafe_code)]" \
  -m "- Add agentlens-api (ports 8080 & 4318) and agentlens-worker binaries" \
  -m "- Implement /health/live and /health/ready with PostgreSQL readiness check" \
  -m "- Initialize Next.js dashboard, Python SDK, TypeScript SDK, Compose, and GitHub Actions CI" \
  -m "System-1-ADR: ADR-0001"

# 5. Push, wait for CI, merge, and tag M1
git push -u origin feat/phase-01-workspace-foundation
gh pr create --base main --head feat/phase-01-workspace-foundation \
  --title "feat(workspace): Phase 01 — Monorepo Foundation, Compose & CI" \
  --body "Completes Phase 01 acceptance gate (Milestone M1)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase01 -m "Milestone M1: Workspace and Compose Foundation"
git push origin v0.1.0-phase01
```

---

### Phase 02 — Domain, Configuration, and Database Core

```sh
# 1. Create branch
git checkout -b feat/phase-02-domain-and-database

# 2. Run verification gates against PostgreSQL
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# 3. Stage files
git add migrations/ crates/config/ crates/domain/ crates/database/ docs/decisions/

# 4. Commit
git commit -m "feat(db): implement domain types, migrations, and sqlx repositories" \
  -m "- Add fail-closed typed configuration in crates/config" \
  -m "- Implement W3C TraceId/SpanId validation (rejecting all-zero IDs) in crates/domain" \
  -m "- Add ordered migrations 0002..0005 with composite tenant foreign keys" \
  -m "- Enforce idempotent uniqueness on spans, token_usages, span_events, and span_links" \
  -m "- Implement out-of-order-safe provisional trace upserts and repository integration tests" \
  -m "System-1-ADR: ADR-0002, ADR-0003, ADR-0004"

# 5. Push, verify CI, and merge
git push -u origin feat/phase-02-domain-and-database
gh pr create --base main --head feat/phase-02-domain-and-database \
  --title "feat(db): Phase 02 — Domain Model, Configuration & PostgreSQL Repositories" \
  --body "Completes Phase 02 acceptance gate."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase02 -m "Phase 02: Domain and Database Core"
git push origin v0.1.0-phase02
```

---

### Phase 03 — Authentication, Authorization, and Tenant Isolation

```sh
# 1. Create branch
git checkout -b feat/phase-03-auth-and-tenancy

# 2. Run verification gates (including cross-tenant isolation suite)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# 3. Stage files
git add api/openapi.yaml crates/auth/ crates/organizations/ crates/projects/ crates/app/ crates/api/ docs/decisions/

# 4. Commit
git commit -m "feat(auth): implement bootstrap, rbac, scoped api keys, and tenant isolation" \
  -m "- Implement atomic first-owner POST /api/v1/auth/bootstrap and Argon2id auth" \
  -m "- Add HttpOnly session cookies with X-CSRF-Token protection" \
  -m "- Enforce owner/admin/member/viewer RBAC and strict organization/project isolation" \
  -m "- Implement SHA-256 hashed scoped ingestion API keys (agl_live_*) with constant-time compare" \
  -m "- Add rate limiting and security audit logging" \
  -m "System-1-ADR: ADR-0005"

# 5. Push, verify CI, merge, and tag M2
git push -u origin feat/phase-03-auth-and-tenancy
gh pr create --base main --head feat/phase-03-auth-and-tenancy \
  --title "feat(auth): Phase 03 — Authentication, RBAC, Scoped API Keys & Tenancy" \
  --body "Completes Phase 03 acceptance gate (Milestone M2)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase03 -m "Milestone M2: Auth and Tenant Isolation"
git push origin v0.1.0-phase03
```

---

### Phase 04 — OTLP Ingestion and Trace Engine

```sh
# 1. Create branch
git checkout -b feat/phase-04-otlp-ingestion

# 2. Run verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# 3. Stage files
git add crates/privacy/ crates/telemetry/ crates/agents/ crates/traces/ crates/database/ crates/api/ docs/decisions/

# 4. Commit
git commit -m "feat(telemetry): implement otlp http ingestion engine and canonical normalization" \
  -m "- Mount POST /v1/traces on ports 4318 and 8080 with protobuf, JSON, and bounded gzip support" \
  -m "- Normalize OTLP and GenAI semantic conventions into canonical domain records" \
  -m "- Strip prompt/completion bodies by default when capture_content is false" \
  -m "- Persist provisional/root traces, spans, events, links, token_usages, and outbox_jobs in one tx" \
  -m "System-1-ADR: ADR-0002, ADR-0004, ADR-0005"

# 5. Push, verify CI, merge, and tag M3
git push -u origin feat/phase-04-otlp-ingestion
gh pr create --base main --head feat/phase-04-otlp-ingestion \
  --title "feat(telemetry): Phase 04 — OTLP/HTTP Ingestion & Canonical Trace Engine" \
  --body "Completes Phase 04 acceptance gate (Milestone M3)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase04 -m "Milestone M3: OTLP Ingestion Engine"
git push origin v0.1.0-phase04
```

---

### Phase 05 — Durable Workers and Trace Finalization

```sh
# 1. Create branch
git checkout -b feat/phase-05-durable-workers

# 2. Run verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace

# 3. Stage files
git add crates/jobs/ crates/observability/ crates/traces/ crates/database/ binaries/worker/ docs/decisions/

# 4. Commit
git commit -m "feat(worker): implement durable postgres outbox worker and trace finalization" \
  -m "- Implement FOR UPDATE SKIP LOCKED outbox claiming with lease_expires_at recovery" \
  -m "- Add exponential backoff, jitter, and dead-letter transition on max_attempts" \
  -m "- Reconcile trace wall-clock duration, span/error counts, token sums, and root span metadata" \
  -m "- Expose Prometheus /metrics for outbox queue depth and worker health" \
  -m "System-1-ADR: ADR-0003"

# 5. Push, verify CI, and merge
git push -u origin feat/phase-05-durable-workers
gh pr create --base main --head feat/phase-05-durable-workers \
  --title "feat(worker): Phase 05 — Durable PostgreSQL Outbox Worker & Trace Finalization" \
  --body "Completes Phase 05 acceptance gate."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase05 -m "Phase 05: Durable Workers"
git push origin v0.1.0-phase05
```

---

### Phase 06 — SDKs and Example Applications

```sh
# 1. Create branch
git checkout -b feat/phase-06-sdks-and-examples

# 2. Run verification gates
cd sdk/python && uv run ruff check . && uv run ruff format --check . && uv run mypy src && uv run pytest && cd ../..
pnpm --filter @agentlens/sdk lint && pnpm --filter @agentlens/sdk typecheck && pnpm --filter @agentlens/sdk test && pnpm --filter @agentlens/sdk build
cargo test --workspace

# 3. Stage files
git add sdk/python/ sdk/typescript/ examples/python-basic/ examples/typescript-basic/ docs/sdk/ docs/decisions/

# 4. Commit
git commit -m "feat(sdk): implement python and typescript otel sdks and examples" \
  -m "- Build Python agentlens SDK and TypeScript @agentlens/sdk over OpenTelemetry OTLP/HTTP" \
  -m "- Enforce capture_content=False default via local AgentLensSpanProcessor stripping" \
  -m "- Add bounded queues, exponential backoff, drop counters, and fail-open offline resilience" \
  -m "- Add runnable python-basic and typescript-basic example agents" \
  -m "System-1-ADR: ADR-0006"

# 5. Push, verify CI, merge, and tag M4
git push -u origin feat/phase-06-sdks-and-examples
gh pr create --base main --head feat/phase-06-sdks-and-examples \
  --title "feat(sdk): Phase 06 — Python & TypeScript SDKs and Example Agents" \
  --body "Completes Phase 06 acceptance gate (Milestone M4)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.1.0-phase06 -m "Milestone M4: Worker and SDKs"
git push origin v0.1.0-phase06
```

---

### Phase 07 — Dashboard Foundation and Trace Explorer (MVP Release Slice)

```sh
# 1. Create branch
git checkout -b feat/phase-07-dashboard-trace-explorer

# 2. Run verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard test && pnpm --filter dashboard build

# 3. Stage files
git add api/openapi.yaml crates/agents/ crates/traces/ crates/api/ apps/dashboard/ tests/e2e/ docs/decisions/

# 4. Commit
git commit -m "feat(dashboard): implement trace explorer, waterfall view, and agent registry ui" \
  -m "- Add control-plane query endpoints for agents, traces, spans, and sessions" \
  -m "- Build Next.js bootstrap, login, org/project/environment switcher, and API key UI" \
  -m "- Build Overview, Agent Registry/Detail, cursor-paginated Trace Explorer, and Span Waterfall" \
  -m "- Add Playwright E2E test verifying full MVP slice from Python SDK to UI waterfall" \
  -m "System-1-ADR: ADR-0001, ADR-0004"

# 5. Push, verify CI, merge, and tag MVP (v0.2.0-mvp / M5)
git push -u origin feat/phase-07-dashboard-trace-explorer
gh pr create --base main --head feat/phase-07-dashboard-trace-explorer \
  --title "feat(dashboard): Phase 07 — Dashboard & Trace Waterfall Explorer (MVP)" \
  --body "Completes Phase 07 acceptance gate and full MVP vertical slice (Milestone M5)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.2.0-mvp -m "Milestone M5: AgentLens MVP Release"
git push origin v0.2.0-mvp
```

---

### Phase 08 — Analytics and Pricing

```sh
# 1. Create branch
git checkout -b feat/phase-08-analytics-and-pricing

# 2. Run verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard test && pnpm --filter dashboard build

# 3. Stage files
git add migrations/ api/openapi.yaml crates/pricing/ crates/analytics/ crates/jobs/ crates/api/ apps/dashboard/ docs/decisions/

# 4. Commit
git commit -m "feat(analytics): implement versioned decimal pricing and usage analytics" \
  -m "- Implement rust_decimal model pricing matcher with snapshotted rate versions" \
  -m "- Keep unknown model pricing as NULL and surface has_unpriced_usage indicator" \
  -m "- Add percentile_cont P50/P95/P99 latency distributions and daily_metrics rollups" \
  -m "- Build Analytics dashboard views for tokens, costs, models, tools, and latency" \
  -m "System-1-ADR: ADR-0004"

# 5. Push, verify CI, merge, and tag M6
git push -u origin feat/phase-08-analytics-and-pricing
gh pr create --base main --head feat/phase-08-analytics-and-pricing \
  --title "feat(analytics): Phase 08 — Versioned Decimal Pricing & Usage Analytics" \
  --body "Completes Phase 08 acceptance gate (Milestone M6)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.3.0-phase08 -m "Milestone M6: Analytics and Pricing"
git push origin v0.3.0-phase08
```

---

### Phase 09 — Agent Health, Alerts, and Notifications

```sh
# 1. Create branch
git checkout -b feat/phase-09-alerts-and-notifications

# 2. Run verification gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard test && pnpm --filter dashboard build

# 3. Stage files
git add api/openapi.yaml crates/agents/ crates/alerts/ crates/notifications/ crates/jobs/ crates/api/ apps/dashboard/ docs/decisions/

# 4. Commit
git commit -m "feat(alerts): implement agent health, alert engine, and ssrf-safe notifications" \
  -m "- Add transparent component health indicators at GET /api/v1/agents/{id}/health" \
  -m "- Implement typed alert evaluator, fingerprint deduplication, cooldowns, and state machine" \
  -m "- Encrypt notification channel configs with versioned AES-256-GCM" \
  -m "- Add SSRF-safe DNS/IP connector and disable redirects for webhook/Telegram/Discord/Slack" \
  -m "System-1-ADR: ADR-0005"

# 5. Push, verify CI, merge, and tag M7
git push -u origin feat/phase-09-alerts-and-notifications
gh pr create --base main --head feat/phase-09-alerts-and-notifications \
  --title "feat(alerts): Phase 09 — Agent Health, Alert Engine & SSRF-Safe Notifications" \
  --body "Completes Phase 09 acceptance gate (Milestone M7)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.4.0-phase09 -m "Milestone M7: Health, Alerts, and Notifications"
git push origin v0.4.0-phase09
```

---

### Phase 10 — Framework Integrations

```sh
# 1. Create branch
git checkout -b feat/phase-10-framework-integrations

# 2. Run verification gates
cd sdk/python && uv run ruff check . && uv run mypy src && uv run pytest && cd ../..
pnpm --filter @agentlens/sdk test
cargo test --workspace

# 3. Stage files
git add integrations/ sdk/ examples/langgraph/ examples/multi-agent/ docs/sdk/ docs/decisions/

# 4. Commit
git commit -m "feat(integrations): add langgraph, litellm, crewai, openai-agents, and mcp adapters" \
  -m "- Add optional non-invasive adapters for LangGraph, LiteLLM, CrewAI, OpenAI Agents, and MCP" \
  -m "- Preserve parent-child trace context across async boundaries without duplicate spans" \
  -m "- Enforce capture_content=False privacy defaults across all framework callbacks" \
  -m "System-1-ADR: ADR-0006"

# 5. Push, verify CI, merge, and tag M8
git push -u origin feat/phase-10-framework-integrations
gh pr create --base main --head feat/phase-10-framework-integrations \
  --title "feat(integrations): Phase 10 — LangGraph, LiteLLM, CrewAI, OpenAI Agents & MCP" \
  --body "Completes Phase 10 acceptance gate (Milestone M8)."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main
git tag -a v0.5.0-phase10 -m "Milestone M8: Framework Integrations"
git push origin v0.5.0-phase10
```

---

### Phase 11 — Production Hardening and `v1.0.0` Public Release

```sh
# 1. Create branch
git checkout -b chore/phase-11-production-hardening-v1

# 2. Run full verification, backup/restore drill, and load benchmark gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard test && pnpm --filter dashboard build
pnpm --filter @agentlens/sdk test
cd sdk/python && uv run ruff check . && uv run mypy src && uv run pytest && cd ../..

# 3. Stage files
git add crates/ binaries/ apps/ sdk/ deployments/ scripts/ tests/ docs/ README.md CHANGELOG.md

# 4. Commit
git commit -m "chore(release): complete production hardening and prepare v1.0.0" \
  -m "- Complete OIDC authentication flow with state, nonce, and PKCE verification" \
  -m "- Implement S3/MinIO large payload storage and scheduled retention cleanup job" \
  -m "- Harden multi-stage non-root Dockerfiles and production Docker Compose" \
  -m "- Add verified backup/restore scripts, load test benchmarks, and complete docs" \
  -m "System-1-ADR: ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006"

# 5. Push, verify CI, merge, and publish v1.0.0 release
git push -u origin chore/phase-11-production-hardening-v1
gh pr create --base main --head chore/phase-11-production-hardening-v1 \
  --title "chore(release): Phase 11 — Production Hardening & v1.0.0 Release" \
  --body "Completes Phase 11 acceptance gate (Milestones M9 & M10). Ready for v1.0.0 tag."
gh pr checks --watch
gh pr merge --squash --delete-branch
git checkout main && git pull --ff-only origin main

# 6. Tag and create official GitHub Release
git tag -a v1.0.0 -m "AgentLens v1.0.0 — Self-Hosted AI Agent Observability Platform"
git push origin v1.0.0
gh release create v1.0.0 --title "AgentLens v1.0.0" --notes-file CHANGELOG.md
```

---

## 4\. Required GitHub Templates & Workflow Files

Ensure the following files are committed in `.github/` during **Phase 01**:

### 4.1 `.github/PULL_REQUEST_TEMPLATE.md`

```markdown
## Summary
- **Phase:** Phase XX
- **System-1 ADR(s):** `ADR-XXXX`

## Changes
- 

## Verification & Quality Gates Executed
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `pnpm --filter dashboard lint && pnpm --filter dashboard typecheck && pnpm --filter dashboard build`
- [ ] `pnpm --filter @agentlens/sdk test`
- [ ] `cd sdk/python && uv run ruff check . && uv run mypy src && uv run pytest`
- [ ] Database migrations tested from empty DB and previous version
- [ ] Zero secrets, `.env` files, or placeholder stubs included
```

&nbsp;

### 4.2 Least-Privilege CI Workflow Rule (`.github/workflows/ci.yml`)

Every GitHub Actions workflow must declare explicit least-privilege permissions at the top level:

```yaml
permissions:
  contents: read
```

&nbsp;

Never pass production secrets to pull-request workflows triggered from forks, and run `cargo test` against isolated service containers (`postgres:16-alpine`, `redis:7-alpine`).

---

## 5\. Emergency Recovery & Mistake Handling Protocol

1. **If a secret or `.env` file is staged before commit:**

```sh
git reset HEAD <file>
# Add pattern to .gitignore immediately
```

2. **If a secret is accidentally committed locally (before `git push`):**

```sh
git reset --soft HEAD~1
git reset HEAD <secret-file>
# Rotate the secret immediately if it was ever real, update .gitignore, and re-commit
```

3. **If CI fails on a pushed PR branch:**  
   - Do **not** merge the PR.  
   - Fix the issue locally on the phase branch, re-run the entire verification suite in Section 2, create a clean fix commit (`fix(<scope>): resolve ci failure in ...`), push to the same branch, and re-run `gh pr checks --watch`.

---

## 6\. Master Prompt to Give Your Agent for GitHub Execution

```
Follow `agent/GITHUB_WORKFLOW_AND_RELEASE_PLAYBOOK.md` strictly for all Git and GitHub operations across Phase 00 through Phase 11.
1. Never push directly to `main` after initial repo setup; create the exact phase branch specified in Section 3 for the current phase.
2. Before every commit, execute the 6-step Pre-Push Verification Ritual in Section 2 (secret scan, formatting, clippy, unit/integration tests, staged diff inspection, and System-1-ADR commit trailer).
3. Push the phase branch to `origin`, open a Pull Request using `.github/PULL_REQUEST_TEMPLATE.md`, verify all GitHub Actions CI checks pass (`gh pr checks --watch`), squash-merge into `main`, tag the phase milestone, and report the exact commit SHA and PR status before moving to the next phase.
```

&nbsp;