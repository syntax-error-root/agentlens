"""Tests for AgentLens client decorators, context managers, and fail-open guarantees."""

from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter

from agentlens import AgentLens


def test_agent_and_tool_decorators() -> None:
    memory_exporter = InMemorySpanExporter()
    client = AgentLens(
        api_key="agl_live_test_client",
        endpoint="http://localhost:4318",
        agent_name="unit-tester",
        agent_version="1.0.0",
        exporter=memory_exporter,
    )

    @client.tool(name="calculator")
    def add(a: int, b: int) -> int:
        return a + b

    @client.llm_call(model="gpt-4o", provider="openai")
    def prompt_llm(user_input: str) -> str:
        return f"Response to {user_input}"

    @client.agent(name="math_solver", version="1.0.1")
    def run_agent(x: int, y: int) -> dict:
        total = add(x, y)
        llm_res = prompt_llm(f"Total is {total}")
        return {"total": total, "explanation": llm_res}

    result = run_agent(5, 7)
    assert result["total"] == 12
    assert "Response to Total is 12" in result["explanation"]

    client.flush()
    spans = memory_exporter.get_finished_spans()
    span_names = [s.name for s in spans]
    assert "tool:calculator" in span_names
    assert "llm:gpt-4o" in span_names
    assert "math_solver" in span_names


def test_fail_open_on_offline_collector() -> None:
    """Telemetry failures must never crash the host application."""
    client = AgentLens(
        api_key="agl_live_invalid",
        endpoint="http://127.0.0.1:59999",  # unreachable endpoint
        export_timeout_millis=500,
    )

    @client.agent(name="resilient_agent")
    def execute() -> str:
        return "operation successful"

    # Should execute successfully without throwing exporter connection errors
    res = execute()
    assert res == "operation successful"

    # Flush should return without crashing
    flushed = client.flush(timeout_seconds=0.5)
    assert isinstance(flushed, bool)
    client.shutdown()


def test_custom_trace_context_manager() -> None:
    memory_exporter = InMemorySpanExporter()
    client = AgentLens(
        api_key="agl_live_test_ctx",
        endpoint="http://localhost:4318",
        exporter=memory_exporter,
    )

    with client.trace("data_ingestion", span_type="pipeline", attributes={"records": 100}) as span:
        span.set_attribute("status", "processed")

    client.flush()
    spans = memory_exporter.get_finished_spans()
    assert any(s.name == "data_ingestion" for s in spans)
    ingest_span = next(s for s in spans if s.name == "data_ingestion")
    assert ingest_span.attributes["agentlens.span_type"] == "pipeline"
    assert ingest_span.attributes["records"] == 100
    assert ingest_span.attributes["status"] == "processed"
