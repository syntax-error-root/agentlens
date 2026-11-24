"""Main AgentLens client, decorators, and context managers."""

from __future__ import annotations

import functools
import logging
from collections.abc import Callable, Generator, Sequence
from contextlib import contextmanager
from typing import Any, TypeVar, cast

from opentelemetry import trace as otel_trace
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import SpanExporter
from opentelemetry.trace import Status, StatusCode

from agentlens.config import AgentLensConfig
from agentlens.processor import AgentLensSpanProcessor

logger = logging.getLogger("agentlens")

F = TypeVar("F", bound=Callable[..., Any])


class AgentLens:
    """AgentLens observability client for instrumenting AI agents."""

    _instance: AgentLens | None = None

    def __init__(
        self,
        api_key: str | None = None,
        endpoint: str | None = None,
        environment: str | None = None,
        service_name: str | None = None,
        agent_name: str | None = None,
        agent_version: str | None = None,
        capture_content: bool = False,
        redacted_keys: Sequence[str] = (),
        max_queue_size: int = 2048,
        export_timeout_millis: int = 5000,
        exporter: SpanExporter | None = None,
    ) -> None:
        self.config = AgentLensConfig.from_env(
            api_key=api_key,
            endpoint=endpoint,
            environment=environment,
            service_name=service_name,
            agent_name=agent_name,
            agent_version=agent_version,
            capture_content=capture_content,
            redacted_keys=redacted_keys,
            max_queue_size=max_queue_size,
            export_timeout_millis=export_timeout_millis,
        )

        # Build Resource
        resource_attributes: dict[str, Any] = {
            "service.name": self.config.service_name,
            "deployment.environment": self.config.environment,
        }
        if self.config.agent_name:
            resource_attributes["agent.name"] = self.config.agent_name
        if self.config.agent_version:
            resource_attributes["agent.version"] = self.config.agent_version

        resource = Resource.create(resource_attributes)
        self.tracer_provider = TracerProvider(resource=resource)

        if exporter is not None:
            self.exporter = exporter
        else:
            headers: dict[str, str] = {}
            if self.config.api_key:
                headers["Authorization"] = f"Bearer {self.config.api_key}"

            raw_endpoint = self.config.endpoint.rstrip("/")
            export_endpoint = (
                raw_endpoint if raw_endpoint.endswith("/v1/traces") else f"{raw_endpoint}/v1/traces"
            )
            self.exporter = OTLPSpanExporter(
                endpoint=export_endpoint,
                headers=headers,
                timeout=self.config.export_timeout_millis // 1000,
            )

        # Build Span Processor
        self.processor = AgentLensSpanProcessor(
            exporter=self.exporter,
            agent_name=self.config.agent_name,
            agent_version=self.config.agent_version,
            environment=self.config.environment,
            capture_content=self.config.capture_content,
            redacted_keys=self.config.redacted_keys,
            max_queue_size=self.config.max_queue_size,
            export_timeout_millis=self.config.export_timeout_millis,
        )
        self.tracer_provider.add_span_processor(self.processor)
        self.tracer = self.tracer_provider.get_tracer("agentlens", "0.1.0")

        AgentLens._instance = self

    @classmethod
    def get_instance(cls) -> AgentLens:
        """Returns the current active AgentLens singleton instance."""
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    @property
    def dropped_span_count(self) -> int:
        """Returns the number of spans dropped due to capacity or export errors."""
        return self.processor.dropped_span_count

    def flush(self, timeout_seconds: float = 5.0) -> bool:
        """Flushes buffered spans to the AgentLens collector."""
        try:
            return self.processor.force_flush(int(timeout_seconds * 1000))
        except Exception as e:
            logger.debug("Failed to flush spans: %s", e)
            return False

    def shutdown(self) -> None:
        """Flushes remaining spans and terminates worker threads."""
        try:
            self.processor.shutdown()
        except Exception as e:
            logger.debug("Error during AgentLens shutdown: %s", e)

    @contextmanager
    def trace(
        self,
        name: str,
        span_type: str = "custom",
        attributes: dict[str, Any] | None = None,
    ) -> Generator[otel_trace.Span, None, None]:
        """Context manager creating a custom trace span."""
        attrs = dict(attributes or {})
        attrs["agentlens.span_type"] = span_type

        with self.tracer.start_as_current_span(name, attributes=attrs) as span:
            try:
                yield span
            except Exception as e:
                span.record_exception(e)
                span.set_status(Status(StatusCode.ERROR, str(e)))
                raise

    def agent(
        self,
        name: str | None = None,
        version: str | None = None,
    ) -> Callable[[F], F]:
        """Decorator for instrumenting agent entry-point functions."""

        def decorator(func: F) -> F:
            span_name = name or func.__name__

            @functools.wraps(func)
            def wrapper(*args: Any, **kwargs: Any) -> Any:
                attrs: dict[str, Any] = {
                    "agentlens.span_type": "agent",
                    "agent.name": name or span_name,
                }
                if version:
                    attrs["agent.version"] = version

                with self.trace(span_name, span_type="agent", attributes=attrs):
                    return func(*args, **kwargs)

            return cast(F, wrapper)

        return decorator

    def tool(
        self,
        name: str | None = None,
    ) -> Callable[[F], F]:
        """Decorator for instrumenting tool execution functions."""

        def decorator(func: F) -> F:
            tool_name = name or func.__name__

            @functools.wraps(func)
            def wrapper(*args: Any, **kwargs: Any) -> Any:
                attrs: dict[str, Any] = {
                    "agentlens.span_type": "tool",
                    "tool.name": tool_name,
                    "gen_ai.tool.name": tool_name,
                }
                if self.config.capture_content:
                    attrs["agentlens.tool.input"] = str(args)

                with self.trace(f"tool:{tool_name}", span_type="tool", attributes=attrs) as span:
                    result = func(*args, **kwargs)
                    if self.config.capture_content:
                        span.set_attribute("agentlens.tool.output", str(result))
                    return result

            return cast(F, wrapper)

        return decorator

    def llm_call(
        self,
        model: str,
        provider: str = "openai",
    ) -> Callable[[F], F]:
        """Decorator for instrumenting LLM call functions."""

        def decorator(func: F) -> F:
            @functools.wraps(func)
            def wrapper(*args: Any, **kwargs: Any) -> Any:
                attrs: dict[str, Any] = {
                    "agentlens.span_type": "llm",
                    "gen_ai.system": provider,
                    "gen_ai.request.model": model,
                }
                with self.trace(f"llm:{model}", span_type="llm", attributes=attrs):
                    return func(*args, **kwargs)

            return cast(F, wrapper)

        return decorator
