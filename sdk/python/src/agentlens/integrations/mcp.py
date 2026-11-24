"""Model Context Protocol (MCP) instrumentation adapter for AgentLens observability."""

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

logger = logging.getLogger("agentlens.integrations.mcp")


class AgentLensMCPMiddleware:
    """Instrumentation adapter for Model Context Protocol (MCP) client and server operations."""

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
            self._tracer = get_active_tracer("agentlens.integrations.mcp")
        return self._tracer

    def _should_capture_content(self) -> bool:
        if self._capture_content is not None:
            return self._capture_content
        return is_content_capture_enabled()

    @contextmanager
    def trace_tool_call(
        self,
        tool_name: str,
        arguments: dict[str, Any] | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager for an MCP tools/call JSON-RPC operation."""
        if is_instrumentation_suppressed():
            yield otel_trace.get_current_span()
            return

        attrs: dict[str, Any] = {
            "agentlens.span_type": "tool",
            "agentlens.tool.name": tool_name,
            "mcp.method": "tools/call",
        }

        if arguments is not None:
            if self._should_capture_content():
                attrs["agentlens.tool.input"] = str(arguments)[:2000]
                attrs["mcp.tool.arguments"] = str(arguments)[:2000]
            else:
                attrs["agentlens.tool.input_keys"] = list(arguments.keys())

        with self.tracer.start_as_current_span(f"mcp.tool.{tool_name}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise

    @contextmanager
    def trace_resource_read(
        self,
        uri: str,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager for an MCP resources/read JSON-RPC operation."""
        if is_instrumentation_suppressed():
            yield otel_trace.get_current_span()
            return

        attrs: dict[str, Any] = {
            "agentlens.span_type": "resource",
            "mcp.resource.uri": uri,
            "mcp.method": "resources/read",
        }

        with self.tracer.start_as_current_span(f"mcp.resource.{uri[:30]}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise

    @contextmanager
    def trace_prompt_get(
        self,
        prompt_name: str,
        arguments: dict[str, Any] | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager for an MCP prompts/get JSON-RPC operation."""
        if is_instrumentation_suppressed():
            yield otel_trace.get_current_span()
            return

        attrs: dict[str, Any] = {
            "agentlens.span_type": "prompt",
            "mcp.prompt.name": prompt_name,
            "mcp.method": "prompts/get",
        }

        if arguments is not None:
            if self._should_capture_content():
                attrs["mcp.prompt.arguments"] = str(arguments)[:2000]
            else:
                attrs["mcp.prompt.argument_keys"] = list(arguments.keys())

        with self.tracer.start_as_current_span(f"mcp.prompt.{prompt_name}", attributes=attrs) as span:
            try:
                yield span
                span.set_status(Status(StatusCode.OK))
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise
