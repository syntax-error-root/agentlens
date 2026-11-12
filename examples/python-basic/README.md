# AgentLens Python Basic Example

This example demonstrates instrumenting a multi-step AI agent using the `agentlens` Python SDK.

## Features Illustrated
- Global SDK initialization (`agentlens.init`)
- Agent tracing (`@agentlens.agent`)
- Tool execution instrumentation (`@agentlens.tool`)
- LLM API call tracing (`@agentlens.llm_call`)
- Client-side privacy enforcement (`capture_content = False`)
- Fail-open telemetry delivery and bounded flush

## Running the Example

```bash
cd examples/python-basic
uv run python main.py
```
