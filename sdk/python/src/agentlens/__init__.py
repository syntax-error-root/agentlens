"""AgentLens Python SDK — AI Agent Observability."""

from dataclasses import dataclass


@dataclass(frozen=True)
class AgentLensConfig:
    api_key: str
    endpoint: str = "http://localhost:4318/v1/traces"
    capture_content: bool = False


class AgentLens:
    """AgentLens observability client."""

    def __init__(
        self, api_key: str, endpoint: str | None = None, capture_content: bool = False
    ) -> None:
        if not api_key:
            raise ValueError("AgentLens api_key is required")
        self.config = AgentLensConfig(
            api_key=api_key,
            endpoint=endpoint or "http://localhost:4318/v1/traces",
            capture_content=capture_content,
        )

    def flush(self, timeout_seconds: float = 5.0) -> None:
        """Flush buffered spans to the collector."""


__all__ = ["AgentLens", "AgentLensConfig"]
