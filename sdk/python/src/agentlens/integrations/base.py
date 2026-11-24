"""Common base utilities, context managers, and suppression tokens for AgentLens integrations."""

from __future__ import annotations

import contextvars
from collections.abc import Generator
from contextlib import contextmanager
from typing import TYPE_CHECKING, Any

from opentelemetry import trace as otel_trace

if TYPE_CHECKING:
    from agentlens.client import AgentLens

# Context variable to prevent recursion and duplicate span creation when multiple
# integrations are active simultaneously in the same call stack.
_SUPPRESS_INSTRUMENTATION: contextvars.ContextVar[bool] = contextvars.ContextVar(
    "agentlens_suppress_instrumentation",
    default=False,
)


@contextmanager
def suppress_instrumentation() -> Generator[None, None, None]:
    """Context manager to temporarily suppress AgentLens integration hooks.

    Prevents infinite recursion and duplicate span emissions during re-entrant calls.
    """
    token = _SUPPRESS_INSTRUMENTATION.set(True)
    try:
        yield
    finally:
        _SUPPRESS_INSTRUMENTATION.reset(token)


def is_instrumentation_suppressed() -> bool:
    """Returns True if AgentLens instrumentation is currently suppressed in the active context."""
    return _SUPPRESS_INSTRUMENTATION.get()


def get_active_client() -> AgentLens:
    """Retrieves the active AgentLens client singleton instance."""
    from agentlens.client import AgentLens

    return AgentLens.get_instance()


def get_active_tracer(name: str = "agentlens.integrations", version: str = "0.1.0") -> otel_trace.Tracer:
    """Returns an OpenTelemetry tracer bound to the active AgentLens TracerProvider."""
    client = get_active_client()
    return client.tracer_provider.get_tracer(name, version)


def is_content_capture_enabled(client: AgentLens | None = None) -> bool:
    """Checks whether content capture (prompts, completions, tool payloads) is enabled."""
    c = client or get_active_client()
    return bool(c.config.capture_content)


def truncate_or_redact(
    value: Any,
    capture_content: bool,
    max_length: int = 1000,
) -> str | None:
    """Safely formats an attribute value obeying the capture_content privacy setting."""
    if not capture_content:
        return None
    if value is None:
        return None
    val_str = str(value)
    if len(val_str) > max_length:
        return val_str[:max_length] + " [TRUNCATED]"
    return val_str
