"""AgentLens Python SDK — OpenTelemetry-native AI Agent Observability."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from contextlib import AbstractContextManager
from typing import Any, TypeVar

from opentelemetry import trace as otel_trace

from agentlens.client import AgentLens
from agentlens.config import AgentLensConfig
from agentlens.processor import AgentLensSpanProcessor

F = TypeVar("F", bound=Callable[..., Any])


def init(
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
) -> AgentLens:
    """Initialize the global AgentLens singleton client."""
    return AgentLens(
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


def get_instance() -> AgentLens:
    """Returns the global AgentLens instance, initializing from environment if needed."""
    return AgentLens.get_instance()


def trace(
    name: str,
    span_type: str = "custom",
    attributes: dict[str, Any] | None = None,
) -> AbstractContextManager[otel_trace.Span]:
    """Context manager for tracing custom operations."""
    return get_instance().trace(name=name, span_type=span_type, attributes=attributes)


def agent(
    name: str | None = None,
    version: str | None = None,
) -> Callable[[F], F]:
    """Decorator for instrumenting AI agent entrypoint functions."""
    return get_instance().agent(name=name, version=version)


def tool(
    name: str | None = None,
) -> Callable[[F], F]:
    """Decorator for instrumenting tool execution functions."""
    return get_instance().tool(name=name)


def llm_call(
    model: str,
    provider: str = "openai",
) -> Callable[[F], F]:
    """Decorator for instrumenting LLM API invocation functions."""
    return get_instance().llm_call(model=model, provider=provider)


def flush(timeout_seconds: float = 5.0) -> bool:
    """Flushes queued spans to the AgentLens collector."""
    return get_instance().flush(timeout_seconds=timeout_seconds)


def shutdown() -> None:
    """Shuts down the AgentLens exporter and span processors."""
    get_instance().shutdown()


__all__ = [
    "AgentLens",
    "AgentLensConfig",
    "AgentLensSpanProcessor",
    "agent",
    "flush",
    "get_instance",
    "init",
    "llm_call",
    "shutdown",
    "tool",
    "trace",
]
