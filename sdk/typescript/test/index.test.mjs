import test from "node:test";
import assert from "node:assert/strict";
import {
  AgentLens,
  AgentLensSpanProcessor,
  withSpan,
  withAgentSpan,
  withToolSpan,
  withLlmSpan,
} from "../dist/index.js";
import {
  BasicTracerProvider,
  InMemorySpanExporter,
} from "@opentelemetry/sdk-trace-base";

test("AgentLens SDK initializes with default configuration", () => {
  const client = new AgentLens({ apiKey: "agl_live_test_key" });
  const config = client.getConfig();
  assert.equal(config.apiKey, "agl_live_test_key");
  assert.equal(config.captureContent, false);
  assert.equal(config.endpoint, "http://localhost:4318");
  assert.equal(config.environment, "development");
  assert.equal(config.serviceName, "agent");
});

test("AgentLensSpanProcessor injects agent metadata", async () => {
  const exporter = new InMemorySpanExporter();
  const processor = new AgentLensSpanProcessor(exporter, {
    agentName: "qa-agent",
    agentVersion: "2.0.0",
    environment: "staging",
  });

  const provider = new BasicTracerProvider({
    spanProcessors: [processor],
  });
  const tracer = provider.getTracer("test-tracer");

  const span = tracer.startSpan("test_step");
  span.end();

  await processor.forceFlush();
  const spans = exporter.getFinishedSpans();
  assert.equal(spans.length, 1);
  assert.equal(spans[0].attributes["agent.name"], "qa-agent");
  assert.equal(spans[0].attributes["agent.version"], "2.0.0");
  assert.equal(spans[0].attributes["deployment.environment"], "staging");
});

test("AgentLensSpanProcessor scrubs content keys when captureContent is false", async () => {
  const exporter = new InMemorySpanExporter();
  const processor = new AgentLensSpanProcessor(exporter, {
    captureContent: false,
  });

  const provider = new BasicTracerProvider({
    spanProcessors: [processor],
  });
  const tracer = provider.getTracer("test-tracer");

  const span = tracer.startSpan("llm_call");
  span.setAttribute("gen_ai.prompt", "Confidential user query");
  span.setAttribute("gen_ai.completion", "Confidential AI response");
  span.setAttribute("agentlens.tool.input", "{ account: 12345 }");
  span.setAttribute("agentlens.tool.output", "{ balance: 99999 }");
  span.setAttribute("safe_count", 10);
  span.end();

  await processor.forceFlush();
  const spans = exporter.getFinishedSpans();
  assert.equal(spans.length, 1);
  const attrs = spans[0].attributes;

  assert.equal(attrs["gen_ai.prompt"], undefined);
  assert.equal(attrs["gen_ai.completion"], undefined);
  assert.equal(attrs["agentlens.tool.input"], undefined);
  assert.equal(attrs["agentlens.tool.output"], undefined);
  assert.equal(attrs["safe_count"], 10);
});

test("AgentLensSpanProcessor preserves content keys when captureContent is true", async () => {
  const exporter = new InMemorySpanExporter();
  const processor = new AgentLensSpanProcessor(exporter, {
    captureContent: true,
  });

  const provider = new BasicTracerProvider({
    spanProcessors: [processor],
  });
  const tracer = provider.getTracer("test-tracer");

  const span = tracer.startSpan("llm_call");
  span.setAttribute("gen_ai.prompt", "Allowed query");
  span.setAttribute("gen_ai.completion", "Allowed response");
  span.end();

  await processor.forceFlush();
  const spans = exporter.getFinishedSpans();
  assert.equal(spans.length, 1);
  assert.equal(spans[0].attributes["gen_ai.prompt"], "Allowed query");
  assert.equal(spans[0].attributes["gen_ai.completion"], "Allowed response");
});

test("AgentLensSpanProcessor redacts secrets, tokens, and PII patterns", async () => {
  const exporter = new InMemorySpanExporter();
  const processor = new AgentLensSpanProcessor(exporter, {
    redactedKeys: ["ssn", "passport_no"],
  });

  const provider = new BasicTracerProvider({
    spanProcessors: [processor],
  });
  const tracer = provider.getTracer("test-tracer");

  const span = tracer.startSpan("sec_test");
  span.setAttribute("database_password", "rootpass123");
  span.setAttribute("user_session_token", "jwt-secret-xyz");
  span.setAttribute("ssn", "000-11-2222");
  span.setAttribute("passport_no", "P891234");
  span.setAttribute("custom_header", "Authorization: Bearer secret-token-value");
  span.setAttribute("user_email", "admin@company.com");
  span.setAttribute("diagnostic_log", "Leaked agl_live_0123456789abcdef0123 in log");
  span.setAttribute("normal_attr", "harmless text");
  span.end();

  await processor.forceFlush();
  const spans = exporter.getFinishedSpans();
  assert.equal(spans.length, 1);
  const attrs = spans[0].attributes;

  assert.equal(attrs["database_password"], "[REDACTED]");
  assert.equal(attrs["user_session_token"], "[REDACTED]");
  assert.equal(attrs["ssn"], "[REDACTED]");
  assert.equal(attrs["passport_no"], "[REDACTED]");
  assert.equal(attrs["custom_header"], "Authorization: Bearer [REDACTED]");
  assert.equal(attrs["user_email"], "[REDACTED_EMAIL]");
  assert.equal(attrs["diagnostic_log"], "Leaked [REDACTED_KEY] in log");
  assert.equal(attrs["normal_attr"], "harmless text");
});

test("AgentLens client helpers create nested agent, tool, and llm spans", async () => {
  const exporter = new InMemorySpanExporter();
  const client = new AgentLens({
    apiKey: "agl_live_helper_test",
    exporter,
    agentName: "researcher",
    agentVersion: "1.0.0",
  });

  const outcome = await client.withAgentSpan("research_workflow", async () => {
    const toolRes = await client.withToolSpan("web_search", async () => {
      return "search results";
    }, { input: "latest rust news" });

    const llmRes = await client.withLlmSpan("gpt-summarize", "gpt-4o", async () => {
      return `Summary of ${toolRes}`;
    }, { provider: "openai" });

    return { toolRes, llmRes };
  }, { version: "1.0.1" });

  assert.equal(outcome.toolRes, "search results");
  assert.equal(outcome.llmRes, "Summary of search results");

  await client.flush();
  const spans = exporter.getFinishedSpans();
  assert.equal(spans.length, 3);
  const spanNames = spans.map((s) => s.name);
  assert.ok(spanNames.includes("research_workflow"));
  assert.ok(spanNames.includes("tool:web_search"));
  assert.ok(spanNames.includes("llm:gpt-4o"));
});

test("AgentLens fail-open guarantee on unreachable collector", async () => {
  const client = new AgentLens({
    apiKey: "agl_live_unreachable",
    endpoint: "http://127.0.0.1:59998",
    exportTimeoutMs: 500,
  });

  // Calling withSpan must not crash even if collector is offline
  const result = await client.withSpan("safe_execution", async () => {
    return "resilient return value";
  });
  assert.equal(result, "resilient return value");

  // Flush must not throw
  const flushed = await client.flush({ timeoutMs: 500 });
  assert.equal(typeof flushed, "boolean");

  await client.shutdown();
});
