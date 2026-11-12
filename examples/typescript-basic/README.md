# AgentLens TypeScript Basic Example

This example demonstrates instrumenting a multi-step autonomous AI agent using the `@agentlens/sdk` TypeScript package.

## Features Illustrated
- Global SDK initialization (`init`)
- Top-level agent workflow spans (`withAgentSpan`)
- Tool spans with input scrubbing (`withToolSpan`)
- LLM API call spans (`withLlmSpan`)
- Client-side privacy enforcement (`captureContent = false`)
- Non-blocking fail-open telemetry flush (`flush`, `shutdown`)

## Running the Example

```bash
# Install dependencies from root or inside directory
pnpm install

# Build and run
cd examples/typescript-basic
pnpm build
pnpm start
```
