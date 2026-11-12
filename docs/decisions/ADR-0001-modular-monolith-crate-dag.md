# ADR-0001: Modular Monolith and Strict Layer 0–4 Crate DAG

## Status
Accepted

## Context
AgentLens requires modular separation of domain concerns without the operational complexity and network latency of microservices in v1.

## Decision
1. Implement a single Cargo workspace containing 18 library crates and 2 binary crates.
2. Enforce a strict layered DAG (Layer 0 Foundation -> Layer 1 Core Logic -> Layer 2 Infrastructure -> Layer 3 Domain Modules -> Layer 4 Application/API).
3. Enforce `#![forbid(unsafe_code)]` on all workspace crates.

## Consequences
- Clean compile-time boundary enforcement with no circular dependencies.
- Zero distributed transaction overhead.
- Single deployable Docker container footprint.
