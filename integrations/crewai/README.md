# AgentLens CrewAI Integration

```bash
pip install "agentlens[crewai]"
```

## Usage

```python
from crewai import Agent, Crew, Task
from agentlens.integrations.crewai import AgentLensCrewAICallback, trace_crew

callback = AgentLensCrewAICallback()

agent = Agent(
    role="Researcher",
    goal="Discover insights",
    backstory="Expert researcher",
    step_callback=callback.step_callback,
)

task = Task(
    description="Research AI observability",
    expected_output="Summary report",
    agent=agent,
    callback=callback.task_callback,
)

crew = Crew(agents=[agent], tasks=[task])

# Trace entire crew execution
with trace_crew("market-research"):
    result = crew.kickoff()
```
