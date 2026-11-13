# AgentLens OpenAI Agents SDK Integration

```bash
pip install "agentlens[openai-agents]"
```

## Usage

```python
from agentlens.integrations.openai_agents import AgentLensOpenAIAgentsTracer

tracer = AgentLensOpenAIAgentsTracer()

with tracer.trace_agent("triage_agent"):
    # Agent handoff
    tracer.record_handoff(from_agent="triage_agent", to_agent="specialist_agent")
    
    # Guardrail check
    with tracer.trace_guardrail("input_moderation"):
        pass
        
    # Function tool execution
    with tracer.trace_tool("fetch_account_balance", arguments={"user_id": "123"}):
        pass
```
