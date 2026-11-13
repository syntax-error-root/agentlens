"""Integration tests for framework adapters: LangGraph, LiteLLM, CrewAI, OpenAI Agents, and MCP."""

from __future__ import annotations

import uuid

from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter
from opentelemetry.trace import StatusCode

from agentlens.integrations.base import (
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
)
from agentlens.integrations.mcp import (
    AgentLensMCPMiddleware,
)
from agentlens.integrations.openai_agents import (
    AgentLensOpenAIAgentsTracer,
)
from agentlens.processor import AgentLensSpanProcessor


def setup_test_tracer(
    capture_content: bool = False,
) -> tuple[TracerProvider, InMemorySpanExporter, AgentLensSpanProcessor]:
    memory_exporter = InMemorySpanExporter()
    processor = AgentLensSpanProcessor(
        exporter=memory_exporter,
        capture_content=capture_content,
        agent_name="test_framework_agent",
        agent_version="1.0.0",
    )
    provider = TracerProvider()
    provider.add_span_processor(processor)
    return provider, memory_exporter, processor


# ---------------------------------------------------------------------------
# 1. LangGraph Callback Handler Tests
# ---------------------------------------------------------------------------


def test_langgraph_callback_hierarchy_and_privacy() -> None:
    provider, exporter, processor = setup_test_tracer(capture_content=False)
    tracer = provider.get_tracer("langgraph_test")
    handler = AgentLensLangGraphCallbackHandler(tracer=tracer, capture_content=False)

    root_run_id = uuid.uuid4()
    child_llm_id = uuid.uuid4()
    child_tool_id = uuid.uuid4()

    # Root chain / node
    handler.on_chain_start(
        serialized={"name": "planner_node"},
        inputs={"prompt": "Super secret plan"},
        run_id=root_run_id,
        metadata={"langgraph_node": "planner", "langgraph_step": 1},
    )

    # Nested LLM step
    handler.on_llm_start(
        serialized={"name": "gpt-4o"},
        prompts=["Classified system prompt"],
        run_id=child_llm_id,
        parent_run_id=root_run_id,
    )

    class MockResponse:
        def __init__(self) -> None:
            self.llm_output = {
                "token_usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 50,
                }
            }
            self.generations = [[type("Gen", (), {"text": "Secret response"})()]]

    handler.on_llm_end(MockResponse(), run_id=child_llm_id)

    # Nested Tool step
    handler.on_tool_start(
        serialized={"name": "calculator"},
        input_str="2 + 2",
        run_id=child_tool_id,
        parent_run_id=root_run_id,
    )
    handler.on_tool_end("4", run_id=child_tool_id)

    # Complete root
    handler.on_chain_end({"output": "Plan result"}, run_id=root_run_id)

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 3

    # Spans are ended from inside out: llm, tool, root
    llm_span = next(s for s in spans if s.name == "llm.gpt-4o")
    tool_span = next(s for s in spans if s.name == "tool.calculator")
    root_span = next(s for s in spans if s.name == "planner_node")

    # Verify parent-child context propagation
    assert llm_span.parent is not None
    assert tool_span.parent is not None
    assert llm_span.parent.span_id == root_span.context.span_id
    assert tool_span.parent.span_id == root_span.context.span_id

    # Verify span types and metrics
    assert root_span.attributes["agentlens.span_type"] in ("agent", "chain")
    assert llm_span.attributes["agentlens.span_type"] == "llm"
    assert tool_span.attributes["agentlens.span_type"] == "tool"
    assert llm_span.attributes["gen_ai.usage.input_tokens"] == 100
    assert llm_span.attributes["gen_ai.usage.output_tokens"] == 50

    # Verify capture_content=False privacy guarantees
    for s in spans:
        attrs = s.attributes
        assert "gen_ai.prompt" not in attrs
        assert "gen_ai.completion" not in attrs
        assert "agentlens.tool.input" not in attrs
        assert "agentlens.tool.output" not in attrs
        assert "langgraph.node.input" not in attrs
        assert "langgraph.node.output" not in attrs


def test_langgraph_error_handling() -> None:
    provider, exporter, processor = setup_test_tracer()
    tracer = provider.get_tracer("langgraph_test")
    handler = AgentLensLangGraphCallbackHandler(tracer=tracer)

    run_id = uuid.uuid4()
    handler.on_chain_start(
        serialized={"name": "failing_node"},
        inputs={},
        run_id=run_id,
    )
    handler.on_chain_error(RuntimeError("Graph node timed out"), run_id=run_id)

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 1
    assert spans[0].status.status_code == StatusCode.ERROR
    assert "Graph node timed out" in (spans[0].status.description or "")


# ---------------------------------------------------------------------------
# 2. LiteLLM Logger Tests
# ---------------------------------------------------------------------------


def test_litellm_logger_token_extraction_and_privacy() -> None:
    provider, exporter, processor = setup_test_tracer(capture_content=False)
    tracer = provider.get_tracer("litellm_test")
    litellm_logger = AgentLensLiteLLMLogger(tracer=tracer, capture_content=False)

    kwargs = {
        "model": "claude-3-5-sonnet",
        "custom_llm_provider": "anthropic",
        "messages": [{"role": "user", "content": "Sensitive query"}],
    }
    response_obj = {
        "model": "claude-3-5-sonnet-20241022",
        "usage": {
            "prompt_tokens": 150,
            "completion_tokens": 80,
            "total_tokens": 230,
            "prompt_tokens_details": {"cached_tokens": 25},
            "completion_tokens_details": {"reasoning_tokens": 15},
        },
        "choices": [{"message": {"content": "Confidential completion"}}],
    }

    litellm_logger.log_success_event(kwargs, response_obj, 0.0, 1.0)

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 1
    span = spans[0]

    attrs = span.attributes
    assert attrs["agentlens.span_type"] == "llm"
    assert attrs["gen_ai.system"] == "anthropic"
    assert attrs["gen_ai.request.model"] == "claude-3-5-sonnet"
    assert attrs["gen_ai.response.model"] == "claude-3-5-sonnet-20241022"
    assert attrs["gen_ai.usage.input_tokens"] == 150
    assert attrs["gen_ai.usage.output_tokens"] == 80
    assert attrs["gen_ai.usage.cached_tokens"] == 25
    assert attrs["gen_ai.usage.reasoning_tokens"] == 15

    # Confirm privacy zero-leakage
    assert "gen_ai.prompt" not in attrs
    assert "gen_ai.completion" not in attrs


def test_litellm_logger_failure_event() -> None:
    provider, exporter, processor = setup_test_tracer()
    tracer = provider.get_tracer("litellm_test")
    litellm_logger = AgentLensLiteLLMLogger(tracer=tracer)

    litellm_logger.log_failure_event(
        {"model": "gpt-4o"},
        ValueError("Rate limit exceeded"),
        0.0,
        1.0,
    )

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 1
    assert spans[0].status.status_code == StatusCode.ERROR
    assert "Rate limit exceeded" in (spans[0].status.description or "")


# ---------------------------------------------------------------------------
# 3. CrewAI Callback Tests
# ---------------------------------------------------------------------------


def test_crewai_callback_and_trace_crew() -> None:
    provider, exporter, processor = setup_test_tracer(capture_content=False)
    tracer = provider.get_tracer("crewai_test")
    callback = AgentLensCrewAICallback(tracer=tracer, capture_content=False)

    class MockStep:
        tool = "database_lookup"
        tool_input = "SELECT * FROM secrets"
        log = "Searching records"

    class MockTask:
        description = "Reconcile accounts"
        agent = "Senior Auditor"
        output = "Sensitive financial summary"

    with trace_crew("audit_crew", tracer=tracer):
        callback.step_callback(MockStep())
        callback.task_callback(MockTask())

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 3

    step_span = next(s for s in spans if "database_lookup" in s.name)
    task_span = next(s for s in spans if "Reconcile accounts" in s.name)
    root_crew_span = next(s for s in spans if s.name == "crew.audit_crew")

    assert step_span.parent is not None
    assert task_span.parent is not None
    assert step_span.parent.span_id == root_crew_span.context.span_id
    assert task_span.parent.span_id == root_crew_span.context.span_id

    # Verify privacy
    for s in spans:
        assert "crewai.task.output" not in s.attributes
        assert "agentlens.tool.input" not in s.attributes


# ---------------------------------------------------------------------------
# 4. OpenAI Agents SDK Tracer Tests
# ---------------------------------------------------------------------------


def test_openai_agents_tracer_handoff_and_guardrails() -> None:
    provider, exporter, processor = setup_test_tracer(capture_content=False)
    tracer = provider.get_tracer("openai_agents_test")
    agent_tracer = AgentLensOpenAIAgentsTracer(tracer=tracer, capture_content=False)

    with agent_tracer.trace_agent("triage_agent", model="gpt-4o"):
        agent_tracer.record_handoff(
            from_agent="triage_agent",
            to_agent="billing_agent",
            reason="Routing invoice inquiry",
        )
        with agent_tracer.trace_guardrail("moderation_filter", input_data="User text"):
            pass
        with agent_tracer.trace_tool("refund_customer", arguments={"id": 123, "amount": 50}):
            pass

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 4

    handoff_span = next(s for s in spans if "handoff." in s.name)
    guardrail_span = next(s for s in spans if "guardrail." in s.name)
    tool_span = next(s for s in spans if "tool." in s.name)
    next(s for s in spans if s.name == "agent.triage_agent")

    assert handoff_span.attributes["openai_agents.handoff.from"] == "triage_agent"
    assert handoff_span.attributes["openai_agents.handoff.to"] == "billing_agent"
    assert guardrail_span.attributes["agentlens.span_type"] == "guardrail"
    assert tool_span.attributes["agentlens.span_type"] == "tool"

    # With capture_content=False, tool argument keys are recorded, not sensitive values
    assert "agentlens.tool.input_keys" in tool_span.attributes
    assert "agentlens.tool.input" not in tool_span.attributes
    assert "guardrail.input" not in guardrail_span.attributes


# ---------------------------------------------------------------------------
# 5. Model Context Protocol (MCP) Middleware Tests
# ---------------------------------------------------------------------------


def test_mcp_middleware_tool_and_resource_tracing() -> None:
    provider, exporter, processor = setup_test_tracer(capture_content=False)
    tracer = provider.get_tracer("mcp_test")
    mcp = AgentLensMCPMiddleware(tracer=tracer, capture_content=False)

    with mcp.trace_tool_call("fetch_customer_order", arguments={"order_id": "ORD-1234"}):
        pass

    with mcp.trace_resource_read("postgres://customers/orders"):
        pass

    with mcp.trace_prompt_get("generate_summary", arguments={"target": "report"}):
        pass

    processor.force_flush()
    spans = exporter.get_finished_spans()
    assert len(spans) == 3

    tool_span = next(s for s in spans if s.name == "mcp.tool.fetch_customer_order")
    resource_span = next(s for s in spans if "mcp.resource." in s.name)
    prompt_span = next(s for s in spans if "mcp.prompt." in s.name)

    assert tool_span.attributes["agentlens.span_type"] == "tool"
    assert tool_span.attributes["mcp.method"] == "tools/call"
    assert tool_span.attributes["agentlens.tool.name"] == "fetch_customer_order"
    assert "mcp.tool.arguments" not in tool_span.attributes

    assert resource_span.attributes["agentlens.span_type"] == "resource"
    assert resource_span.attributes["mcp.resource.uri"] == "postgres://customers/orders"

    assert prompt_span.attributes["agentlens.span_type"] == "prompt"
    assert prompt_span.attributes["mcp.prompt.name"] == "generate_summary"


# ---------------------------------------------------------------------------
# 6. Suppression and Multi-Framework Loop Prevention Tests
# ---------------------------------------------------------------------------


def test_instrumentation_suppression() -> None:
    provider, exporter, processor = setup_test_tracer()
    tracer = provider.get_tracer("suppression_test")
    handler = AgentLensLangGraphCallbackHandler(tracer=tracer)
    litellm_logger = AgentLensLiteLLMLogger(tracer=tracer)
    mcp = AgentLensMCPMiddleware(tracer=tracer)

    assert not is_instrumentation_suppressed()

    with suppress_instrumentation():
        assert is_instrumentation_suppressed()
        # All adapters should no-op
        handler.on_chain_start(
            serialized={"name": "test"},
            inputs={},
            run_id=uuid.uuid4(),
        )
        litellm_logger.log_success_event({"model": "gpt-4o"}, {}, 0.0, 1.0)
        with mcp.trace_tool_call("suppressed_tool"):
            pass

    assert not is_instrumentation_suppressed()

    processor.force_flush()
    spans = exporter.get_finished_spans()
    # Zero spans emitted under suppression
    assert len(spans) == 0
