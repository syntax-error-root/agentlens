# AgentLens Model Context Protocol (MCP) Integration

```bash
pip install "agentlens[mcp]"
```

## Usage

```python
from agentlens.integrations.mcp import AgentLensMCPMiddleware

mcp_tracer = AgentLensMCPMiddleware()

# Trace MCP tool invocation
with mcp_tracer.trace_tool_call("fetch_weather", arguments={"city": "Tokyo"}):
    # execute tool
    pass

# Trace MCP resource read
with mcp_tracer.trace_resource_read("postgres://schema/public"):
    # read resource
    pass

# Trace MCP prompt retrieval
with mcp_tracer.trace_prompt_get("summarize_code", arguments={"language": "rust"}):
    # fetch prompt
    pass
```
