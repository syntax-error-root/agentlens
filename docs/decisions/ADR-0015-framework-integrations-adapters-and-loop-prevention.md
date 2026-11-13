# ADR-0015: Framework Integrations, Non-Invasive Adapters, and Loop Prevention

## Status
Accepted (Evaluated by System-1 Architect Model with 100% confidence)

## Context
AgentLens requires first-class observability support for multi-agent workflows and modern AI orchestration frameworks:
1. **LangGraph:** Capturing graph node transitions, chains, tools, and model generations across async graph branches.
2. **LiteLLM:** Capturing multi-provider LLM invocations, model mappings, and token usage breakdown.
3. **CrewAI:** Capturing Crew tasks, agents, and tool executions.
4. **OpenAI Agents SDK:** Capturing agent handoffs, guardrails, model generations, and tool invocations.
5. **Model Context Protocol (MCP):** Capturing JSON-RPC tool calls, prompt expansions, and resource reads.

Non-negotiable architectural requirements:
- **Zero Core Dependency Bloat:** Core backend (`crates/*`) and core SDK (`agentlens`) must never depend directly on heavy AI frameworks.
- **Loop & Duplicate Span Suppression:** Applications combining multiple frameworks (e.g., LangGraph invoking LiteLLM or LiteLLM invoking MCP tools) must not emit duplicate or fragmented traces.
- **Zero Privacy Leakage:** Framework adapters must inherit `capture_content = False` by default and never leak prompt text, completions, or tool payloads without explicit configuration.

## Decision
1. **Packaging & Distribution:**
   - Framework adapters are implemented in `sdk/python/src/agentlens/integrations/` and exposed top-level in `integrations/`.
   - Core `agentlens` keeps its minimal dependency footprint (`opentelemetry-api`, `opentelemetry-sdk`, `opentelemetry-exporter-otlp-proto-http`).
   - Optional extras are declared in `pyproject.toml`:
     - `pip install "agentlens[langgraph]"`
     - `pip install "agentlens[litellm]"`
     - `pip install "agentlens[crewai]"`
     - `pip install "agentlens[openai-agents]"`
     - `pip install "agentlens[mcp]"`
     - `pip install "agentlens[all-integrations]"`
   - Adapter imports are lazily loaded: if a framework is not installed, importing core `agentlens` succeeds without error. If an integration module is used without the underlying framework, a clean, descriptive `ImportError` explains how to install the extra.

2. **Loop & Duplicate Span Prevention:**
   - Adapters leverage standard OpenTelemetry context propagation (`opentelemetry.trace.get_current_span()` and `opentelemetry.trace.set_span_in_context()`).
   - When a higher-level framework (e.g. LangGraph) opens an active span, nested calls from downstream adapters (e.g. LiteLLM) automatically attach as child spans rather than starting disconnected root traces.
   - A thread-safe suppression context token (`_AGENTLENS_SUPPRESS_INSTRUMENTATION`) prevents re-entrant exporter loops and infinite recursion.

3. **Privacy Enforcement:**
   - Every integration adapter inspects the active `AgentLens` client config (`client.config.capture_content`).
   - By default (`capture_content = False`), prompt strings, completion bodies, function parameters, and MCP tool inputs/outputs are completely omitted. Only span type, name, status, model name, provider, token usage metrics, latency, and duration are recorded.

4. **Integration Implementations:**
   - `LangGraphCallbackHandler`: LangChain/LangGraph callback handler mapping node executions to `span_type = 'agent'` / `'chain'`, model calls to `span_type = 'llm'`, and tool runs to `span_type = 'tool'`.
   - `LiteLLMLogger`: LiteLLM custom callback logger capturing pre/post API calls, mapping tokens (`prompt_tokens`, `completion_tokens`, `cached_tokens`), and model parameters.
   - `CrewAICallback`: Step and task callbacks capturing Crew agent transitions and tool invocations.
   - `OpenAIAgentsTracer`: Agent handoff, guardrail check, and tool execution tracing.
   - `MCPMiddleware`: Client and server JSON-RPC wrapper for `tools/call`, `resources/read`, and `prompts/get`.

## Consequences
- Clean separation of concerns with zero bloat to core packages.
- Seamless multi-agent tracing with accurate parent-child waterfalls.
- Full compliance with privacy invariants.
