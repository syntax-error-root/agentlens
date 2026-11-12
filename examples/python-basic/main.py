"""AgentLens Python Basic Example: Multi-step AI agent with LLM and tool calls."""

import os
import sys
import time

import agentlens

# Initialize AgentLens client
client = agentlens.init(
    api_key=os.getenv("AGENTLENS_API_KEY", "agl_live_example_key_0123456789"),
    endpoint=os.getenv("AGENTLENS_OTLP_ENDPOINT", "http://localhost:4318"),
    environment=os.getenv("APP_ENV", "development"),
    service_name="research-assistant",
    agent_name="financial_analyst",
    agent_version="1.0.0",
    capture_content=False,  # Enforce client-side privacy by default
)


@agentlens.tool(name="web_search")
def web_search(query: str) -> str:
    """Simulate querying an external search engine."""
    time.sleep(0.05)
    return f"Search results for: '{query}'. Revenue: $1,250,000, Expenses: $850,000."


@agentlens.tool(name="calculator")
def calculate_net_profit(revenue: float, expenses: float) -> float:
    """Simulate calculating net operating margin."""
    time.sleep(0.02)
    return revenue - expenses


@agentlens.llm_call(model="gpt-4o", provider="openai")
def generate_summary(prompt: str) -> str:
    """Simulate generating response from an LLM."""
    time.sleep(0.08)
    return f"Financial Analysis Summary based on: {prompt}"


@agentlens.agent(name="financial_analyst", version="1.0.0")
def run_analysis(company_name: str) -> dict:
    """Agent entrypoint orchestrating research, calculation, and synthesis."""
    print(f"[*] Starting analysis for {company_name}...")

    # Step 1: Tool call - search
    search_data = web_search(f"{company_name} Q3 financials")
    print(f"[+] Retrieved data: {search_data}")

    # Step 2: Tool call - math
    net_profit = calculate_net_profit(1250000.0, 850000.0)
    print(f"[+] Calculated net profit: ${net_profit:,.2f}")

    # Step 3: LLM generation
    summary = generate_summary(f"{company_name} net profit ${net_profit:,.2f}")
    print(f"[+] LLM synthesis: {summary}")

    return {
        "company": company_name,
        "net_profit": net_profit,
        "summary": summary,
    }


def main() -> None:
    print("=== AgentLens Python Basic Example ===")
    try:
        result = run_analysis("Acme Corp")
        print("\nAnalysis Result:")
        print(result)
    finally:
        print("\n[*] Flushing telemetry to AgentLens...")
        flushed = agentlens.flush(timeout_seconds=3.0)
        print(f"[*] Telemetry flushed: {flushed}")
        agentlens.shutdown()
        print("[*] Shutdown complete.")


if __name__ == "__main__":
    main()
