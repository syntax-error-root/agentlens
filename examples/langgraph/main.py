"""AgentLens LangGraph Tracing Example.

Demonstrates non-invasive tracing of a multi-node state graph (Planner -> Tool -> Writer)
using AgentLensLangGraphCallbackHandler.
"""

from __future__ import annotations

import os
import sys
import time
import uuid

import agentlens
from agentlens.integrations.langgraph import AgentLensLangGraphCallbackHandler

# 1. Initialize AgentLens client
client = agentlens.init(
    api_key=os.getenv("AGENTLENS_API_KEY", "agl_live_example_key_0123456789"),
    endpoint=os.getenv("AGENTLENS_OTLP_ENDPOINT", "http://localhost:4318"),
    environment=os.getenv("APP_ENV", "development"),
    service_name="research-workflow",
    agent_name="graph_agent",
    agent_version="1.0.0",
    capture_content=False,  # Privacy-preserving default
)

# 2. Instantiate LangGraph callback handler
handler = AgentLensLangGraphCallbackHandler()


def simulate_langgraph_execution(topic: str) -> dict[str, str]:
    """Simulates a 3-step LangGraph graph execution using the callback handler."""
    root_run_id = uuid.uuid4()
    print(f"[*] Starting LangGraph workflow for topic: '{topic}' (run_id={root_run_id})...")

    # Node 1: Planner
    handler.on_chain_start(
        serialized={"name": "planner_node"},
        inputs={"topic": topic},
        run_id=root_run_id,
        metadata={"langgraph_node": "planner", "langgraph_step": 1},
    )
    time.sleep(0.04)

    # Planner invokes LLM
    llm_run_id = uuid.uuid4()
    handler.on_llm_start(
        serialized={"name": "gpt-4o"},
        prompts=[f"Plan research for: {topic}"],
        run_id=llm_run_id,
        parent_run_id=root_run_id,
    )
    time.sleep(0.05)

    class MockLLMResponse:
        llm_output = {
            "token_usage": {
                "prompt_tokens": 120,
                "completion_tokens": 45,
                "total_tokens": 165,
            }
        }

    handler.on_llm_end(MockLLMResponse(), run_id=llm_run_id)

    # Node 2: Search Tool
    tool_run_id = uuid.uuid4()
    handler.on_tool_start(
        serialized={"name": "web_search"},
        input_str=f"Search queries for {topic}",
        run_id=tool_run_id,
        parent_run_id=root_run_id,
    )
    time.sleep(0.03)
    handler.on_tool_end("Top results: Autonomous AI agent systems and OTel standard", run_id=tool_run_id)

    # Node 3: Synthesis / Writer
    writer_llm_id = uuid.uuid4()
    handler.on_llm_start(
        serialized={"name": "claude-3-5-sonnet"},
        prompts=["Synthesize research findings"],
        run_id=writer_llm_id,
        parent_run_id=root_run_id,
    )
    time.sleep(0.06)

    class MockWriterResponse:
        llm_output = {
            "token_usage": {
                "prompt_tokens": 350,
                "completion_tokens": 180,
                "total_tokens": 530,
            }
        }

    handler.on_llm_end(MockWriterResponse(), run_id=writer_llm_id)

    # Complete root graph execution
    handler.on_chain_end({"summary": f"Comprehensive report on {topic}"}, run_id=root_run_id)
    print(f"[+] LangGraph workflow finished successfully.")

    return {"status": "completed", "topic": topic}


if __name__ == "__main__":
    simulate_langgraph_execution("Distributed Tracing in Multi-Agent LLMs")
    client.flush(timeout_seconds=2.0)
    sys.exit(0)
