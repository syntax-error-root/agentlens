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
