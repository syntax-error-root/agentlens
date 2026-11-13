# AgentLens LiteLLM Integration

```bash
pip install "agentlens[litellm]"
```

## Usage

```python
import litellm
from agentlens.integrations.litellm import instrument_litellm

# Automatically registers AgentLens logger into litellm.callbacks
instrument_litellm(litellm)

response = litellm.completion(
    model="gpt-4o",
    messages=[{"role": "user", "content": "Hello!"}],
)
```
