"""Tests for AgentLens Python SDK initialization and configuration."""

import os
from unittest.mock import patch

from agentlens import AgentLens, AgentLensConfig


def test_agentlens_init_defaults() -> None:
    client = AgentLens(api_key="agl_live_test_key")
    assert client.config.api_key == "agl_live_test_key"
    assert client.config.endpoint == "http://localhost:4318"
    assert client.config.capture_content is False
    assert client.config.environment == "development"
    assert client.config.service_name == "agent"


def test_agentlens_init_from_env() -> None:
    env = {
        "AGENTLENS_API_KEY": "agl_live_env_key",
        "AGENTLENS_OTLP_ENDPOINT": "http://collector:4318",
        "APP_ENV": "staging",
        "OTEL_SERVICE_NAME": "research-service",
        "AGENTLENS_AGENT_NAME": "researcher",
        "AGENTLENS_AGENT_VERSION": "2.1.0",
    }
    with patch.dict(os.environ, env):
        config = AgentLensConfig.from_env()
        assert config.api_key == "agl_live_env_key"
        assert config.endpoint == "http://collector:4318"
        assert config.environment == "staging"
        assert config.service_name == "research-service"
        assert config.agent_name == "researcher"
        assert config.agent_version == "2.1.0"


def test_agentlens_init_missing_key() -> None:
    with patch.dict(os.environ, {}, clear=True):
        client = AgentLens(api_key="")
        # Empty api key defaults without raising or config holds empty string
        assert client.config.api_key == ""
