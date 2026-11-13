# AgentLens LangGraph Integration

```bash
pip install "agentlens[langgraph]"
```

## Usage

```python
from langgraph.graph import StateGraph
from agentlens.integrations.langgraph import AgentLensLangGraphCallbackHandler

handler = AgentLensLangGraphCallbackHandler()

# Pass handler into invocation config
app = workflow.compile()
result = app.invoke({"messages": [...]}, config={"callbacks": [handler]})
```
