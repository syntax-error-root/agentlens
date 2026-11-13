"""AgentLens Multi-Agent Orchestration Example.

Demonstrates multi-agent routing with triage handoff, guardrail checks,
and MCP tool calls using AgentLensOpenAIAgentsTracer and AgentLensMCPMiddleware.
"""

from __future__ import annotations

import os
import sys
import time

import agentlens
from agentlens.integrations.mcp import AgentLensMCPMiddleware
from agentlens.integrations.openai_agents import AgentLensOpenAIAgentsTracer

# 1. Initialize AgentLens client
client = agentlens.init(
    api_key=os.getenv("AGENTLENS_API_KEY", "agl_live_example_key_0123456789"),
    endpoint=os.getenv("AGENTLENS_OTLP_ENDPOINT", "http://localhost:4318"),
    environment=os.getenv("APP_ENV", "development"),
    service_name="multi-agent-system",
    agent_name="orchestrator",
    agent_version="1.0.0",
    capture_content=False,
)

agent_tracer = AgentLensOpenAIAgentsTracer()
mcp_middleware = AgentLensMCPMiddleware()


def run_customer_support_pipeline(ticket_id: str, query: str) -> dict[str, str]:
    """Runs a multi-agent triage and specialist resolution pipeline."""
    print(f"[*] Processing support ticket {ticket_id}: '{query}'...")

    # Step 1: Root Triage Agent
    with agent_tracer.trace_agent("triage_agent", model="gpt-4o") as triage_span:
        print("[+] Triage Agent inspecting inquiry...")
        time.sleep(0.04)

        # Guardrail check
        with agent_tracer.trace_guardrail("prompt_injection_check"):
            time.sleep(0.02)
            print("[+] Guardrail passed: Safe query.")

        # Triage hands off to billing specialist
        agent_tracer.record_handoff(
            from_agent="triage_agent",
            to_agent="billing_specialist",
            reason="Inquiry pertains to invoice reconciliation",
        )
        print("[+] Handoff recorded: triage_agent -> billing_specialist")

    # Step 2: Billing Specialist Agent
    with agent_tracer.trace_agent("billing_specialist", model="gpt-4o-mini") as billing_span:
        print("[+] Billing Specialist resolving inquiry...")
        time.sleep(0.05)

        # Call MCP Tool
        with mcp_middleware.trace_tool_call(
            "postgres_query_invoices",
            arguments={"customer_id": "cust_9921", "limit": 5},
        ):
            time.sleep(0.03)
            print("[+] MCP Tool executed: postgres_query_invoices")

        # Read MCP Resource
        with mcp_middleware.trace_resource_read("billing://records/cust_9921/2026-Q3"):
            time.sleep(0.02)
            print("[+] MCP Resource fetched: billing records")

    print(f"[+] Pipeline complete for ticket {ticket_id}.")
    return {"ticket_id": ticket_id, "status": "resolved"}


if __name__ == "__main__":
    run_customer_support_pipeline("TCK-8819", "Why is my Q3 enterprise subscription pending?")
    client.flush(timeout_seconds=2.0)
    sys.exit(0)
