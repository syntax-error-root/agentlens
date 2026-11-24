"""LangGraph and LangChain callback handler for AgentLens observability."""

from __future__ import annotations

import logging
import threading
from typing import Any
from uuid import UUID

from opentelemetry import context as otel_context
from opentelemetry import trace as otel_trace
from opentelemetry.context import Context
from opentelemetry.trace import Status, StatusCode

from agentlens.integrations.base import (
    get_active_tracer,
    is_content_capture_enabled,
    is_instrumentation_suppressed,
)

logger = logging.getLogger("agentlens.integrations.langgraph")

try:
    from langchain_core.callbacks import BaseCallbackHandler  # type: ignore[import-not-found]
except ImportError:  # pragma: no cover
    # Fallback duck-typed base callback handler when langchain-core is not installed
    class BaseCallbackHandler:  # type: ignore[no-redef]
        """Duck-typed BaseCallbackHandler fallback."""

        def __init__(self) -> None:
            pass


class AgentLensLangGraphCallbackHandler(BaseCallbackHandler):  # type: ignore[misc]
    """Callback handler capturing LangGraph transitions, model invocations, and tool runs.

    Preserves parent-child OpenTelemetry span context across asynchronous graph branches
    using run_id mappings.
    """

    def __init__(
        self,
        tracer: otel_trace.Tracer | None = None,
        capture_content: bool | None = None,
    ) -> None:
        super().__init__()
        self._tracer = tracer
        self._capture_content = capture_content
        self._lock = threading.Lock()
        # Maps run_id to (Span, ContextToken)
        self._runs: dict[str, tuple[otel_trace.Span, Any]] = {}

    @property
    def tracer(self) -> otel_trace.Tracer:
        if self._tracer is None:
            self._tracer = get_active_tracer("agentlens.integrations.langgraph")
        return self._tracer

    def _should_capture_content(self) -> bool:
        if self._capture_content is not None:
            return self._capture_content
        return is_content_capture_enabled()

    def _get_parent_context(self, parent_run_id: UUID | str | None) -> Context | None:
        if parent_run_id is None:
            return None
        key = str(parent_run_id)
        with self._lock:
            parent_entry = self._runs.get(key)
        if parent_entry is not None:
            parent_span, _ = parent_entry
            return otel_trace.set_span_in_context(parent_span)
        return None

    def on_chain_start(
        self,
        serialized: dict[str, Any] | None,
        inputs: dict[str, Any] | None,
        *,
        run_id: UUID | str,
        parent_run_id: UUID | str | None = None,
        tags: list[str] | None = None,
        metadata: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        """Captures graph node transitions, sub-graphs, and chain executions."""
        if is_instrumentation_suppressed():
            return

        run_key = str(run_id)
        name = "langgraph.node"
        if serialized and "name" in serialized:
            name = str(serialized["name"])
        elif metadata and "langgraph_node" in metadata:
            name = str(metadata["langgraph_node"])
        elif tags:
            name = tags[0]

        is_agent = (metadata and ("langgraph_node" in metadata or "agent" in metadata.get("graph_step", ""))) or (tags and any("agent" in t.lower() for t in tags))
        span_type = "agent" if is_agent else "chain"

        attrs: dict[str, Any] = {
            "agentlens.span_type": span_type,
            "langgraph.run_id": run_key,
        }
        if metadata:
            if "langgraph_node" in metadata:
                attrs["langgraph.node.name"] = str(metadata["langgraph_node"])
            if "langgraph_step" in metadata:
                attrs["langgraph.step"] = int(metadata["langgraph_step"])

        if self._should_capture_content() and inputs:
            attrs["langgraph.node.input"] = str(inputs)[:2000]

        parent_ctx = self._get_parent_context(parent_run_id)
        span = self.tracer.start_span(name, context=parent_ctx, attributes=attrs)
        ctx_token = otel_context.attach(otel_trace.set_span_in_context(span))

        with self._lock:
            self._runs[run_key] = (span, ctx_token)

    def on_chain_end(
        self,
        outputs: dict[str, Any] | None,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Ends chain/graph execution span and attaches output if capture_content is enabled."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            if self._should_capture_content() and outputs:
                span.set_attribute("langgraph.node.output", str(outputs)[:2000])
            span.set_status(Status(StatusCode.OK))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)

    def on_chain_error(
        self,
        error: BaseException,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Records chain error and marks span failed."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            span.record_exception(error)
            span.set_status(Status(StatusCode.ERROR, str(error)))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)

    def on_llm_start(
        self,
        serialized: dict[str, Any] | None,
        prompts: list[str],
        *,
        run_id: UUID | str,
        parent_run_id: UUID | str | None = None,
        tags: list[str] | None = None,
        metadata: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        """Captures model invocations within LangGraph chains."""
        if is_instrumentation_suppressed():
            return

        run_key = str(run_id)
        model = "unknown-model"
        if serialized and "name" in serialized:
            model = str(serialized["name"])
        elif metadata and "model_name" in metadata:
            model = str(metadata["model_name"])
        elif kwargs.get("invocation_params"):
            inv_params = kwargs["invocation_params"]
            model = str(inv_params.get("model_name") or inv_params.get("model") or model)

        attrs: dict[str, Any] = {
            "agentlens.span_type": "llm",
            "gen_ai.request.model": model,
            "langgraph.run_id": run_key,
        }
        if self._should_capture_content() and prompts:
            attrs["gen_ai.prompt"] = prompts[0][:2000]

        parent_ctx = self._get_parent_context(parent_run_id)
        span = self.tracer.start_span(f"llm.{model}", context=parent_ctx, attributes=attrs)
        ctx_token = otel_context.attach(otel_trace.set_span_in_context(span))

        with self._lock:
            self._runs[run_key] = (span, ctx_token)

    def on_llm_end(
        self,
        response: Any,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Extracts token usages and completions from LLM response."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            llm_output = getattr(response, "llm_output", None) or {}
            token_usage = llm_output.get("token_usage") or {}
            if token_usage:
                if "prompt_tokens" in token_usage:
                    span.set_attribute("gen_ai.usage.input_tokens", int(token_usage["prompt_tokens"]))
                if "completion_tokens" in token_usage:
                    span.set_attribute(
                        "gen_ai.usage.output_tokens", int(token_usage["completion_tokens"])
                    )

            if self._should_capture_content():
                generations = getattr(response, "generations", [])
                if generations and generations[0] and hasattr(generations[0][0], "text"):
                    span.set_attribute("gen_ai.completion", str(generations[0][0].text)[:2000])

            span.set_status(Status(StatusCode.OK))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)

    def on_llm_error(
        self,
        error: BaseException,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Handles LLM invocation errors."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            span.record_exception(error)
            span.set_status(Status(StatusCode.ERROR, str(error)))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)

    def on_tool_start(
        self,
        serialized: dict[str, Any] | None,
        input_str: str,
        *,
        run_id: UUID | str,
        parent_run_id: UUID | str | None = None,
        **kwargs: Any,
    ) -> None:
        """Captures tool invocations in LangGraph nodes."""
        if is_instrumentation_suppressed():
            return

        run_key = str(run_id)
        tool_name = "unknown-tool"
        if serialized and "name" in serialized:
            tool_name = str(serialized["name"])
        elif kwargs.get("name"):
            tool_name = str(kwargs["name"])

        attrs: dict[str, Any] = {
            "agentlens.span_type": "tool",
            "agentlens.tool.name": tool_name,
            "langgraph.run_id": run_key,
        }
        if self._should_capture_content() and input_str:
            attrs["agentlens.tool.input"] = input_str[:2000]

        parent_ctx = self._get_parent_context(parent_run_id)
        span = self.tracer.start_span(f"tool.{tool_name}", context=parent_ctx, attributes=attrs)
        ctx_token = otel_context.attach(otel_trace.set_span_in_context(span))

        with self._lock:
            self._runs[run_key] = (span, ctx_token)

    def on_tool_end(
        self,
        output: Any,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Ends tool execution span."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            if self._should_capture_content() and output is not None:
                span.set_attribute("agentlens.tool.output", str(output)[:2000])
            span.set_status(Status(StatusCode.OK))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)

    def on_tool_error(
        self,
        error: BaseException,
        *,
        run_id: UUID | str,
        **kwargs: Any,
    ) -> None:
        """Records tool execution failure."""
        run_key = str(run_id)
        with self._lock:
            entry = self._runs.pop(run_key, None)
        if entry is None:
            return

        span, ctx_token = entry
        try:
            span.record_exception(error)
            span.set_status(Status(StatusCode.ERROR, str(error)))
        finally:
            span.end()
            if ctx_token is not None:
                otel_context.detach(ctx_token)
