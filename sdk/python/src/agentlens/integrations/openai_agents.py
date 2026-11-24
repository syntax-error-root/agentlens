"""OpenAI Agents SDK / Swarm instrumentation adapter for AgentLens observability."""

from __future__ import annotations

import functools
import logging
from collections.abc import Callable, Generator
from contextlib import contextmanager
from typing import Any, TypeVar

from opentelemetry import trace as otel_trace
from opentelemetry.trace import Status, StatusCode

from agentlens.integrations.base import (
    get_active_tracer,
    is_content_capture_enabled,
    is_instrumentation_suppressed,
)

logger = logging.getLogger("agentlens.integrations.openai_agents")

F = TypeVar("F", bound=Callable[..., Any])


class AgentLensOpenAIAgentsTracer:
    """Tracing processor and context manager suite for OpenAI Agents, handoffs, and guardrails."""

    def __init__(
        self,
        tracer: otel_trace.Tracer | None = None,
        capture_content: bool | None = None,
    ) -> None:
        self._tracer = tracer
        self._capture_content = capture_content

    @property
    def tracer(self) -> otel_trace.Tracer:
        if self._tracer is None:
            self._tracer = get_active_tracer("agentlens.integrations.openai_agents")
        return self._tracer

    def _should_capture_content(self) -> bool:
        if self._capture_content is not None:
            return self._capture_content
        return is_content_capture_enabled()

    @contextmanager
    def trace_agent(
        self,
        agent_name: str,
        instructions: str | None = None,
        model: str | None = None,
        attributes: dict[str, Any] | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager creating an agent execution span."""
        attrs = dict(attributes or {})
        attrs["agentlens.span_type"] = "agent"
        attrs["agent.name"] = agent_name
        if model:
            attrs["gen_ai.request.model"] = model

        if self._should_capture_content() and instructions:
            attrs["agent.instructions"] = instructions[:2000]

        with self.tracer.start_as_current_span(f"agent.{agent_name}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise

    def record_handoff(
        self,
        from_agent: str,
        to_agent: str,
        reason: str | None = None,
    ) -> otel_trace.Span:
        """Records an agent handoff event/span."""
        attrs: dict[str, Any] = {
            "agentlens.span_type": "agent",
            "openai_agents.handoff.from": from_agent,
            "openai_agents.handoff.to": to_agent,
        }
        if reason:
            attrs["openai_agents.handoff.reason"] = reason[:500]

        span = self.tracer.start_span(f"handoff.{from_agent}->{to_agent}", attributes=attrs)
        span.set_status(Status(StatusCode.OK))
        span.end()
        return span

    @contextmanager
    def trace_guardrail(
        self,
        guardrail_name: str,
        input_data: Any | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager tracing a guardrail check."""
        attrs: dict[str, Any] = {
            "agentlens.span_type": "guardrail",
            "guardrail.name": guardrail_name,
        }
        if self._should_capture_content() and input_data is not None:
            attrs["guardrail.input"] = str(input_data)[:2000]

        with self.tracer.start_as_current_span(f"guardrail.{guardrail_name}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise

    @contextmanager
    def trace_tool(
        self,
        tool_name: str,
        arguments: dict[str, Any] | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager tracing an OpenAI function tool invocation."""
        attrs: dict[str, Any] = {
            "agentlens.span_type": "tool",
            "agentlens.tool.name": tool_name,
        }
        if arguments is not None:
            if self._should_capture_content():
                attrs["agentlens.tool.input"] = str(arguments)[:2000]
            else:
                attrs["agentlens.tool.input_keys"] = list(arguments.keys())

        with self.tracer.start_as_current_span(f"tool.{tool_name}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise


def instrument_agent(
    name: str | None = None,
    tracer: AgentLensOpenAIAgentsTracer | None = None,
) -> Callable[[F], F]:
    """Decorator for instrumenting OpenAI agent runner functions."""
    t = tracer or AgentLensOpenAIAgentsTracer()

    def decorator(func: F) -> F:
        agent_name = name or func.__name__

        @functools.wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> Any:
            if is_instrumentation_suppressed():
                return func(*args, **kwargs)
            with t.trace_agent(agent_name):
                return func(*args, **kwargs)

        return wrapper  # type: ignore[return-value]

    return decorator
