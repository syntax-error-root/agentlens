"""AgentLens SDK configuration."""

from __future__ import annotations

import os
from collections.abc import Sequence
from dataclasses import dataclass, field


@dataclass(frozen=True)
class AgentLensConfig:
    api_key: str
    endpoint: str = "http://localhost:4318"
    environment: str = "development"
    service_name: str = "agent"
    agent_name: str | None = None
    agent_version: str | None = None
    capture_content: bool = False
    redacted_keys: Sequence[str] = field(default_factory=tuple)
    max_queue_size: int = 2048
    export_timeout_millis: int = 5000

    @classmethod
    def from_env(
        cls,
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
    ) -> AgentLensConfig:
        resolved_key: str = (
            api_key if api_key is not None else (os.getenv("AGENTLENS_API_KEY") or "")
        )
        resolved_endpoint = (
            endpoint
            or os.getenv("AGENTLENS_OTLP_ENDPOINT")
            or os.getenv("AGENTLENS_ENDPOINT")
            or "http://localhost:4318"
        )
        resolved_env = environment or os.getenv("APP_ENV") or "development"
        resolved_service = service_name or os.getenv("OTEL_SERVICE_NAME") or "agent"

        return cls(
            api_key=resolved_key,
            endpoint=resolved_endpoint.rstrip("/"),
            environment=resolved_env,
            service_name=resolved_service,
            agent_name=agent_name or os.getenv("AGENTLENS_AGENT_NAME"),
            agent_version=agent_version or os.getenv("AGENTLENS_AGENT_VERSION"),
            capture_content=capture_content,
            redacted_keys=tuple(redacted_keys),
            max_queue_size=max_queue_size,
            export_timeout_millis=export_timeout_millis,
        )
