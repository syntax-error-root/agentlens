"""LiteLLM custom callback logger for AgentLens observability."""

from __future__ import annotations

import logging
from typing import Any

from opentelemetry import trace as otel_trace
from opentelemetry.trace import Status, StatusCode

from agentlens.integrations.base import (
    get_active_tracer,
    is_content_capture_enabled,
    is_instrumentation_suppressed,
)

logger = logging.getLogger("agentlens.integrations.litellm")


class AgentLensLiteLLMLogger:
    """LiteLLM custom logger adapter for AgentLens.

    Captures LLM requests, response tokens (prompt, completion, cached, reasoning),
    latency, and model parameters into OpenTelemetry spans.
    """

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
            self._tracer = get_active_tracer("agentlens.integrations.litellm")
        return self._tracer

    def _should_capture_content(self) -> bool:
        if self._capture_content is not None:
            return self._capture_content
        return is_content_capture_enabled()

    def log_success_event(
        self,
        kwargs: dict[str, Any],
        response_obj: Any,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Invoked on successful LiteLLM completion."""
        if is_instrumentation_suppressed():
            return

        try:
            model = kwargs.get("model") or "unknown-model"
            if isinstance(response_obj, dict):
                response_model = response_obj.get("model", model)
            else:
                response_model = getattr(response_obj, "model", model)

            custom_llm_provider = kwargs.get("custom_llm_provider") or "openai"

            attrs: dict[str, Any] = {
                "agentlens.span_type": "llm",
                "gen_ai.system": str(custom_llm_provider),
                "gen_ai.request.model": str(model),
                "gen_ai.response.model": str(response_model),
            }

            # Extract usage metrics
            usage = None
            if isinstance(response_obj, dict):
                usage = response_obj.get("usage")
            elif hasattr(response_obj, "usage"):
                usage = response_obj.usage

            if usage is not None:
                if isinstance(usage, dict):
                    prompt_tokens = usage.get("prompt_tokens")
                    completion_tokens = usage.get("completion_tokens")
                    cached_tokens = usage.get("prompt_tokens_details", {}).get("cached_tokens")
                    reasoning_tokens = usage.get("completion_tokens_details", {}).get(
                        "reasoning_tokens"
                    )
                else:
                    prompt_tokens = getattr(usage, "prompt_tokens", None)
                    completion_tokens = getattr(usage, "completion_tokens", None)
                    cached_tokens = getattr(
                        getattr(usage, "prompt_tokens_details", None), "cached_tokens", None
                    )
                    reasoning_tokens = getattr(
                        getattr(usage, "completion_tokens_details", None), "reasoning_tokens", None
                    )

                if prompt_tokens is not None:
                    attrs["gen_ai.usage.input_tokens"] = int(prompt_tokens)
                if completion_tokens is not None:
                    attrs["gen_ai.usage.output_tokens"] = int(completion_tokens)
                if cached_tokens is not None:
                    attrs["gen_ai.usage.cached_tokens"] = int(cached_tokens)
                if reasoning_tokens is not None:
                    attrs["gen_ai.usage.reasoning_tokens"] = int(reasoning_tokens)

            # Enforce privacy defaults
            if self._should_capture_content():
                messages = kwargs.get("messages")
                if messages:
                    attrs["gen_ai.prompt"] = str(messages)[:2000]
                if isinstance(response_obj, dict):
                    choices = response_obj.get("choices")
                    if choices and len(choices) > 0:
                        attrs["gen_ai.completion"] = str(choices[0].get("message", {}).get("content", ""))[:2000]
                elif hasattr(response_obj, "choices"):
                    choices = getattr(response_obj, "choices", [])
                    if choices and len(choices) > 0:
                        msg = getattr(choices[0], "message", None)
                        if msg and hasattr(msg, "content"):
                            attrs["gen_ai.completion"] = str(msg.content)[:2000]

            # Start and immediately close span representing completed call
            span = self.tracer.start_span(f"llm.{model}", attributes=attrs)
            span.set_status(Status(StatusCode.OK))
            span.end()
        except Exception as e:
            logger.debug("Failed recording LiteLLM success event: %s", e)

    def log_failure_event(
        self,
        kwargs: dict[str, Any],
        exception: BaseException,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Invoked on failed LiteLLM completion."""
        if is_instrumentation_suppressed():
            return

        try:
            model = kwargs.get("model") or "unknown-model"
            attrs: dict[str, Any] = {
                "agentlens.span_type": "llm",
                "gen_ai.request.model": str(model),
            }
            span = self.tracer.start_span(f"llm.{model}", attributes=attrs)
            span.record_exception(exception)
            span.set_status(Status(StatusCode.ERROR, str(exception)))
            span.end()
        except Exception as e:
            logger.debug("Failed recording LiteLLM failure event: %s", e)

    async def async_log_success_event(
        self,
        kwargs: dict[str, Any],
        response_obj: Any,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Async compatibility wrapper."""
        self.log_success_event(kwargs, response_obj, start_time, end_time)

    async def async_log_failure_event(
        self,
        kwargs: dict[str, Any],
        exception: BaseException,
        start_time: Any,
        end_time: Any,
    ) -> None:
        """Async compatibility wrapper."""
        self.log_failure_event(kwargs, exception, start_time, end_time)


def instrument_litellm(
    litellm_module: Any | None = None,
    capture_content: bool | None = None,
) -> AgentLensLiteLLMLogger:
    """Registers the AgentLensLiteLLMLogger into LiteLLM's global callback lists.

    If litellm_module is not provided, dynamically imports 'litellm'.
    """
    if litellm_module is None:
        import litellm  # type: ignore[import-not-found]

        litellm_module = litellm

    logger_instance = AgentLensLiteLLMLogger(capture_content=capture_content)
    if not hasattr(litellm_module, "callbacks"):
        litellm_module.callbacks = []
    litellm_module.callbacks.append(logger_instance)
    return logger_instance
