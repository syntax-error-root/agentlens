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

##  Instructions for the implementation agent

You are the principal engineer responsible for implementing AgentLens from this specification. Treat this document as the product and architecture source of truth.

###  Execution rules

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

