"""AgentLens Framework Integrations.

Non-invasive instrumentation adapters for LangGraph, LiteLLM, CrewAI, OpenAI Agents,
and the Model Context Protocol (MCP).
"""

from __future__ import annotations

from agentlens.integrations.base import (
    get_active_client,
    get_active_tracer,
    is_content_capture_enabled,
    is_instrumentation_suppressed,
    suppress_instrumentation,
)
from agentlens.integrations.crewai import (
    AgentLensCrewAICallback,
    trace_crew,
)
from agentlens.integrations.langgraph import (
    AgentLensLangGraphCallbackHandler,
)
from agentlens.integrations.litellm import (
    AgentLensLiteLLMLogger,
    instrument_litellm,
)
from agentlens.integrations.mcp import (
    AgentLensMCPMiddleware,
)
from agentlens.integrations.openai_agents import (
    AgentLensOpenAIAgentsTracer,
    instrument_agent,
)

__all__ = [
    "AgentLensCrewAICallback",
    "AgentLensLangGraphCallbackHandler",
    "AgentLensLiteLLMLogger",
    "AgentLensMCPMiddleware",
    "AgentLensOpenAIAgentsTracer",
    "get_active_client",
    "get_active_tracer",
    "instrument_agent",
    "instrument_litellm",
    "is_content_capture_enabled",
    "is_instrumentation_suppressed",
    "suppress_instrumentation",
    "trace_crew",
]
