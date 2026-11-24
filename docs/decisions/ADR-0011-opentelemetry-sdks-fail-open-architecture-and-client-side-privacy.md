# ADR-0011: OpenTelemetry SDKs, Fail-Open Architecture, and Client-Side Privacy

## Context
AgentLens requires native developer SDKs for Python (`agentlens`) and TypeScript (`@agentlens/sdk`) to instrument AI agents, LLM invocations, tool executions, and retrieval pipelines.
In high-throughput and enterprise production environments:
1. Application availability is paramount: telemetry failures (network timeout, 503, DNS issues, or full AgentLens downtime) must never crash host application business logic (Fail-Open Guarantee).
2. Privacy regulations require zero-trust defaults: sensitive prompt/completion bodies and user credentials must be stripped locally on the client machine before ever crossing the network boundary (`capture_content = False`).
3. Observability libraries must not exhaust process memory: queues must have strict capacity ceilings and expose dropped span telemetry when saturated.

## Decision
1. **OpenTelemetry-Compatible Architecture:**
   - Both SDKs wrap OpenTelemetry Tracing (`opentelemetry-sdk` / `@opentelemetry/sdk-trace-base`), exporting standard W3C traces over OTLP/HTTP with `Authorization: Bearer <agl_live_secret>` headers.
2. **Local Content Stripping & Redaction (`AgentLensSpanProcessor`):**
   - By default (`capture_content = False`), prompt, completion, message arrays, and tool arguments (`gen_ai.prompt`, `gen_ai.completion`, `gen_ai.input.messages`, `gen_ai.output.messages`, `agentlens.tool.input`, `agentlens.tool.output`) are stripped in-process during `on_end`.
   - Keys in `redacted_keys` and matching sensitive regex patterns (Bearer tokens, API keys, emails) are replaced with `"[REDACTED]"`.
3. **Fail-Open & Bounded Buffering:**
   - The SDK uses a bounded queue (`max_queue_size = 2048`).
   - If the queue is saturated, incoming spans are dropped and an internal atomic drop counter (`dropped_span_count`) is incremented.
   - All network, HTTP, and export errors are caught, logged via standard logging facilities, and suppressed so that host user code never experiences unhandled exceptions.
4. **Ergonomic Decorators & Context Handlers:**
   - Python: `@agentlens.agent(...)`, `@agentlens.tool(...)`, `@agentlens.llm_call(...)`, `with agentlens.trace(...)`.
   - TypeScript: `tracer.withSpan(...)`, `withAgentSpan(...)`, `withToolSpan(...)`, `withLlmSpan(...)`.
5. **Runnable Reference Applications:**
   - Provide minimal, dependency-isolated examples: `examples/python-basic` and `examples/typescript-basic`.

## Consequences
- Guaranteed zero customer business disruption if AgentLens is degraded or unreachable.
- Zero customer PII leaves the customer environment unless explicitly opted in.
- Full compatibility with OpenTelemetry ecosystem, collectors, and auto-instrumentation libraries.
