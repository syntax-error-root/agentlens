"""Custom OpenTelemetry SpanProcessor for AgentLens with client-side redaction and bounded queuing."""

from __future__ import annotations

import logging
import re
from collections.abc import Sequence
from typing import Any

from opentelemetry.context import Context
from opentelemetry.sdk.trace import ReadableSpan, Span, SpanProcessor
from opentelemetry.sdk.trace.export import BatchSpanProcessor, SpanExporter

logger = logging.getLogger("agentlens")

CONTENT_KEYS: frozenset[str] = frozenset(
    {
        "gen_ai.prompt",
        "gen_ai.completion",
        "gen_ai.input.messages",
        "gen_ai.output.messages",
        "llm.prompts",
        "llm.completions",
        "agentlens.tool.input",
        "agentlens.tool.output",
    }
)

BEARER_PATTERN = re.compile(r"Bearer\s+[a-zA-Z0-9_\-\.]+", re.IGNORECASE)
EMAIL_PATTERN = re.compile(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+")
API_KEY_PATTERN = re.compile(r"agl_live_[a-zA-Z0-9]{16,}")


class AgentLensSpanProcessor(SpanProcessor):
    """OpenTelemetry SpanProcessor enforcing AgentLens privacy scrubbing and drop tracking."""

    def __init__(
        self,
        exporter: SpanExporter,
        agent_name: str | None = None,
        agent_version: str | None = None,
        environment: str = "development",
        capture_content: bool = False,
        redacted_keys: Sequence[str] = (),
        max_queue_size: int = 2048,
        export_timeout_millis: int = 5000,
    ) -> None:
        self.agent_name = agent_name
        self.agent_version = agent_version
        self.environment = environment
        self.capture_content = capture_content
        self.redacted_keys = frozenset(k.lower() for k in redacted_keys)
        self._dropped_count = 0

        self.max_queue_size = max_queue_size
        self._batch_processor = BatchSpanProcessor(
            span_exporter=exporter,
            max_queue_size=max_queue_size,
            export_timeout_millis=export_timeout_millis,
        )

    @property
    def dropped_span_count(self) -> int:
        """Returns the total number of spans dropped due to queue saturation or failures."""
        return self._dropped_count

    def on_start(self, span: Span, parent_context: Context | None = None) -> None:
        """Injects agent metadata on span initialization."""
        try:
            span_attrs = span.attributes or {}
            if self.agent_name and "agent.name" not in span_attrs:
                span.set_attribute("agent.name", self.agent_name)
            if self.agent_version and "agent.version" not in span_attrs:
                span.set_attribute("agent.version", self.agent_version)
            if self.environment and "deployment.environment" not in span_attrs:
                span.set_attribute("deployment.environment", self.environment)
        except Exception as e:
            logger.debug("Failed injecting span attributes on_start: %s", e)

        self._batch_processor.on_start(span, parent_context)

    def on_end(self, span: ReadableSpan) -> None:
        """Sanitizes sensitive attributes locally before queuing to the exporter."""
        try:
            self._sanitize_span_attributes(span)
        except Exception as e:
            logger.debug("Failed sanitizing span attributes on_end: %s", e)

        # Forward to batch processor; track queue saturation
        try:
            # Check queue capacity
            queue = getattr(self._batch_processor, "queue", None)
            if queue is not None and getattr(queue, "qsize", lambda: 0)() >= self.max_queue_size:
                self._dropped_count += 1
            self._batch_processor.on_end(span)
        except Exception as e:
            self._dropped_count += 1
            logger.debug("Failed enqueuing span to batch processor: %s", e)

    def _sanitize_span_attributes(self, span: ReadableSpan) -> None:
        attrs = getattr(span, "_attributes", None)
        if attrs is None:
            return

        new_attrs: dict[str, Any] = {}
        for key, val in attrs.items():
            key_str = str(key)
            key_lower = key_str.lower()

            # 1. Strip raw content when capture_content is False
            if not self.capture_content and key_str in CONTENT_KEYS:
                continue

            # 2. Redact matching keys and secrets
            if key_lower in self.redacted_keys or any(
                sensitive in key_lower
                for sensitive in ("password", "secret", "api_key", "token", "auth")
            ):
                new_attrs[key_str] = "[REDACTED]"
            elif isinstance(val, str):
                redacted_val = BEARER_PATTERN.sub("Bearer [REDACTED]", val)
                redacted_val = EMAIL_PATTERN.sub("[REDACTED_EMAIL]", redacted_val)
                redacted_val = API_KEY_PATTERN.sub("[REDACTED_KEY]", redacted_val)
                new_attrs[key_str] = redacted_val
            else:
                new_attrs[key_str] = val

        span._attributes = new_attrs

    def shutdown(self) -> None:
        """Shuts down the underlying batch processor."""
        self._batch_processor.shutdown()  # type: ignore[no-untyped-call]

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        """Flushes buffered spans to the exporter."""
        return bool(self._batch_processor.force_flush(timeout_millis))
