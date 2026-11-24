"""Tests for AgentLensSpanProcessor privacy scrubbing, redaction, and drop counters."""

from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter

from agentlens.processor import AgentLensSpanProcessor


def test_span_processor_metadata_injection() -> None:
    memory_exporter = InMemorySpanExporter()
    processor = AgentLensSpanProcessor(
        exporter=memory_exporter,
        agent_name="planner-agent",
        agent_version="1.2.0",
        environment="production",
    )
    provider = TracerProvider()
    provider.add_span_processor(processor)
    tracer = provider.get_tracer("test")

    with tracer.start_as_current_span("agent_step"):
        pass

    processor.force_flush()
    spans = memory_exporter.get_finished_spans()
    assert len(spans) == 1
    attrs = spans[0].attributes
    assert attrs["agent.name"] == "planner-agent"
    assert attrs["agent.version"] == "1.2.0"
    assert attrs["deployment.environment"] == "production"


def test_span_processor_content_scrubbing_default() -> None:
    memory_exporter = InMemorySpanExporter()
    processor = AgentLensSpanProcessor(
        exporter=memory_exporter,
        capture_content=False,
    )
    provider = TracerProvider()
    provider.add_span_processor(processor)
    tracer = provider.get_tracer("test")

    with tracer.start_as_current_span("llm_generation") as span:
        span.set_attribute("gen_ai.prompt", "Top secret prompt text")
        span.set_attribute("gen_ai.completion", "Confidential answer text")
        span.set_attribute("agentlens.tool.input", "{'query': 'bank balance'}")
        span.set_attribute("agentlens.tool.output", "{'balance': 1000000}")
        span.set_attribute("safe_metric", 42)

    processor.force_flush()
    spans = memory_exporter.get_finished_spans()
    assert len(spans) == 1
    attrs = spans[0].attributes

    for content_key in (
        "gen_ai.prompt",
        "gen_ai.completion",
        "agentlens.tool.input",
        "agentlens.tool.output",
    ):
        assert content_key not in attrs

    assert attrs["safe_metric"] == 42


def test_span_processor_content_scrubbing_disabled() -> None:
    memory_exporter = InMemorySpanExporter()
    processor = AgentLensSpanProcessor(
        exporter=memory_exporter,
        capture_content=True,
    )
    provider = TracerProvider()
    provider.add_span_processor(processor)
    tracer = provider.get_tracer("test")

    with tracer.start_as_current_span("llm_generation") as span:
        span.set_attribute("gen_ai.prompt", "Authorized prompt text")
        span.set_attribute("gen_ai.completion", "Authorized answer text")

    processor.force_flush()
    spans = memory_exporter.get_finished_spans()
    assert len(spans) == 1
    attrs = spans[0].attributes
    assert attrs["gen_ai.prompt"] == "Authorized prompt text"
    assert attrs["gen_ai.completion"] == "Authorized answer text"


def test_span_processor_regex_and_key_redaction() -> None:
    memory_exporter = InMemorySpanExporter()
    processor = AgentLensSpanProcessor(
        exporter=memory_exporter,
        redacted_keys=["client_national_id", "pin_code"],
    )
    provider = TracerProvider()
    provider.add_span_processor(processor)
    tracer = provider.get_tracer("test")

    with tracer.start_as_current_span("sensitive_step") as span:
        span.set_attribute("db_password", "supersecret123")
        span.set_attribute("user_auth_token", "jwt-token-xyz")
        span.set_attribute("client_national_id", "ID-999-888")
        span.set_attribute("pin_code", 1234)
        span.set_attribute("request_header", "Authorization: Bearer my-secret-token")
        span.set_attribute("customer_contact", "Reach alice at alice@corp.internal")
        span.set_attribute("nested_key", "Found key agl_live_abcdef1234567890 in log")
        span.set_attribute("regular_key", "visible-text")

    processor.force_flush()
    spans = memory_exporter.get_finished_spans()
    assert len(spans) == 1
    attrs = spans[0].attributes
    assert attrs["db_password"] == "[REDACTED]"
    assert attrs["user_auth_token"] == "[REDACTED]"
    assert attrs["client_national_id"] == "[REDACTED]"
    assert attrs["pin_code"] == "[REDACTED]"
    assert attrs["request_header"] == "Authorization: Bearer [REDACTED]"
    assert attrs["customer_contact"] == "Reach alice at [REDACTED_EMAIL]"
    assert attrs["nested_key"] == "Found key [REDACTED_KEY] in log"
    assert attrs["regular_key"] == "visible-text"
