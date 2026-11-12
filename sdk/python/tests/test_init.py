"""Tests for AgentLens Python SDK initialization."""

import pytest

from agentlens import AgentLens


def test_agentlens_init_defaults() -> None:
    client = AgentLens(api_key="agl_live_test_key")
    assert client.config.api_key == "agl_live_test_key"
    assert client.config.endpoint == "http://localhost:4318/v1/traces"
    assert client.config.capture_content is False


def test_agentlens_init_missing_key() -> None:
    with pytest.raises(ValueError, match="api_key is required"):
        AgentLens(api_key="")
