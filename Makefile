.PHONY: setup up down migrate fmt lint test build clean

setup:
	pnpm install
	cd sdk/python && uv venv && uv pip install -e ".[dev]"

up:
	docker compose up -d postgres redis

down:
	docker compose down

migrate:
	cargo run --bin agentlens-api -- --migrate-only || true

fmt:
	cargo fmt --all
	pnpm --filter dashboard lint --fix || true
	cd sdk/python && uv run ruff format .

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	pnpm --filter dashboard lint
	pnpm --filter @agentlens/sdk lint
	cd sdk/python && uv run ruff check .
	cd sdk/python && uv run mypy src

test:
	cargo test --workspace
	pnpm --filter @agentlens/sdk test
	cd sdk/python && uv run pytest

build:
	cargo build --workspace
	pnpm --filter dashboard build
	pnpm --filter @agentlens/sdk build

clean:
	cargo clean
	rm -rf target apps/dashboard/.next sdk/typescript/dist
