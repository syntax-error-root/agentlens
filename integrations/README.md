# AgentLens Framework Integrations

This directory contains reference documentation and adapter pointers for popular AI agent frameworks:

| Framework | Extra / Package | Description |
|---|---|---|
| **LangGraph** | `pip install "agentlens[langgraph]"` | Tracing graph node transitions, model generations, and tool runs across async branches |
| **LiteLLM** | `pip install "agentlens[litellm]"` | Multi-provider LLM logger capturing tokens (prompt, completion, cached, reasoning), latency, and model details |
| **CrewAI** | `pip install "agentlens[crewai]"` | Step and task callbacks mapping Crew agents, tasks, and tools into nested waterfall traces |
| **OpenAI Agents** | `pip install "agentlens[openai-agents]"` | Tracing agent handoffs, guardrail checks, model generations, and function tool calls |
| **MCP** | `pip install "agentlens[mcp]"` | JSON-RPC middleware for Model Context Protocol `tools/call`, `resources/read`, and `prompts/get` |

All adapters strictly inherit the active client's `capture_content = False` setting by default, ensuring zero leakage of prompts, completions, and sensitive tool parameters.
