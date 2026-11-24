"""CrewAI instrumentation adapter for AgentLens observability."""

from __future__ import annotations

import logging
from collections.abc import Generator
from contextlib import contextmanager
from typing import Any

from opentelemetry import trace as otel_trace
from opentelemetry.trace import Status, StatusCode

from agentlens.integrations.base import (
    get_active_tracer,
    is_content_capture_enabled,
    is_instrumentation_suppressed,
)

logger = logging.getLogger("agentlens.integrations.crewai")


class AgentLensCrewAICallback:
    """Callback adapter mapping CrewAI agent steps and task executions into nested AgentLens spans."""

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
            self._tracer = get_active_tracer("agentlens.integrations.crewai")
        return self._tracer

    def _should_capture_content(self) -> bool:
        if self._capture_content is not None:
            return self._capture_content
        return is_content_capture_enabled()

    def step_callback(self, step_output: Any) -> None:
        """Invoked after each step taken by a CrewAI agent."""
        if is_instrumentation_suppressed():
            return

        try:
            # step_output is typically AgentAction or AgentFinish
            tool_name = getattr(step_output, "tool", None)
            tool_input = getattr(step_output, "tool_input", None)
            log_text = getattr(step_output, "log", None)

            span_type = "tool" if tool_name else "agent"
            span_name = f"crewai.step.{tool_name}" if tool_name else "crewai.step"

            attrs: dict[str, Any] = {
                "agentlens.span_type": span_type,
            }
            if tool_name:
                attrs["agentlens.tool.name"] = str(tool_name)

            if self._should_capture_content():
                if tool_input is not None:
                    attrs["agentlens.tool.input"] = str(tool_input)[:2000]
                if log_text is not None:
                    attrs["crewai.step.log"] = str(log_text)[:2000]

            span = self.tracer.start_span(span_name, attributes=attrs)
            span.set_status(Status(StatusCode.OK))
            span.end()
        except Exception as e:
            logger.debug("Failed handling CrewAI step_callback: %s", e)

    def task_callback(self, task_output: Any) -> None:
        """Invoked after a CrewAI task completes."""
        if is_instrumentation_suppressed():
            return

        try:
            task_desc = getattr(task_output, "description", None) or "crewai.task"
            span_name = f"task.{task_desc[:30]}"

            attrs: dict[str, Any] = {
                "agentlens.span_type": "task",
            }
            agent_role = getattr(task_output, "agent", None)
            if agent_role:
                attrs["agent.name"] = str(agent_role)

            if self._should_capture_content():
                raw_output = getattr(task_output, "raw", None) or getattr(task_output, "output", None)
                if raw_output is not None:
                    attrs["crewai.task.output"] = str(raw_output)[:2000]

            span = self.tracer.start_span(span_name, attributes=attrs)
            span.set_status(Status(StatusCode.OK))
            span.end()
        except Exception as e:
            logger.debug("Failed handling CrewAI task_callback: %s", e)


@contextmanager
def trace_crew(
    crew_name: str = "crewai.crew",
    tracer: otel_trace.Tracer | None = None,
    attributes: dict[str, Any] | None = None,
) -> Generator[otel_trace.Span, None, None]:
    """Context manager surrounding crew.kickoff() to establish the root trace span."""
    t = tracer or get_active_tracer("agentlens.integrations.crewai")
    attrs = dict(attributes or {})
    attrs["agentlens.span_type"] = "agent"
    attrs["crewai.crew.name"] = crew_name

    with t.start_as_current_span(f"crew.{crew_name}", attributes=attrs) as span:
        try:
            yield span
            span.set_status(Status(StatusCode.OK))
        except Exception as e:
            span.record_exception(e)
            span.set_status(Status(StatusCode.ERROR, str(e)))
            raise
