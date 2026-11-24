/**
 * AgentLens TypeScript Basic Example
 * Demonstrates instrumenting an autonomous research agent with nested spans.
 */

import {
  init,
  withAgentSpan,
  withToolSpan,
  withLlmSpan,
  flush,
  shutdown,
} from "@agentlens/sdk";

// Initialize AgentLens client
init({
  apiKey: process.env.AGENTLENS_API_KEY || "agl_live_ts_example_0123456789",
  endpoint: process.env.AGENTLENS_OTLP_ENDPOINT || "http://localhost:4318",
  environment: process.env.APP_ENV || "development",
  serviceName: "market-researcher",
  agentName: "competitive_analyst",
  agentVersion: "1.0.0",
  captureContent: false, // Local privacy scrubbing enabled
});

async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function searchDatabase(competitor: string): Promise<string> {
  return withToolSpan(
    "search_competitor_db",
    async (span) => {
      span.setAttribute("db.system", "postgresql");
      await sleep(40);
      return `Competitor profile for ${competitor}: Pricing tier $49/mo, 15k users.`;
    },
    { input: { competitor } }
  );
}

async function fetchPublicMetrics(competitor: string): Promise<{ traffic: number; rating: number }> {
  return withToolSpan(
    "fetch_public_metrics",
    async (span) => {
      span.setAttribute("http.method", "GET");
      await sleep(30);
      return { traffic: 250000, rating: 4.6 };
    },
    { input: { competitor } }
  );
}

async function synthesizeReport(competitor: string, data: any): Promise<string> {
  return withLlmSpan(
    "generate_report",
    "claude-3-5-sonnet",
    async (span) => {
      span.setAttribute("gen_ai.usage.prompt_tokens", 520);
      span.setAttribute("gen_ai.usage.completion_tokens", 180);
      await sleep(60);
      return `Market Intelligence Report on ${competitor}: High traffic, competitive pricing at $49/mo.`;
    },
    { provider: "anthropic" }
  );
}

async function runResearchPipeline(targetCompany: string) {
  console.log(`[*] Executing research pipeline for: ${targetCompany}...`);

  return withAgentSpan(
    "competitive_analysis_workflow",
    async () => {
      // Step 1: Internal database search
      const dbProfile = await searchDatabase(targetCompany);
      console.log(`[+] Database profile: ${dbProfile}`);

      // Step 2: Public metrics fetch
      const metrics = await fetchPublicMetrics(targetCompany);
      console.log(`[+] Public metrics: ${JSON.stringify(metrics)}`);

      // Step 3: LLM synthesis
      const report = await synthesizeReport(targetCompany, { dbProfile, metrics });
      console.log(`[+] Executive report: ${report}`);

      return {
        target: targetCompany,
        profile: dbProfile,
        metrics,
        report,
      };
    },
    { version: "1.0.0" }
  );
}

async function main() {
  console.log("=== AgentLens TypeScript Basic Example ===");
  try {
    const outcome = await runResearchPipeline("CompetitorX");
    console.log("\nPipeline Output:");
    console.log(JSON.stringify(outcome, null, 2));
  } finally {
    console.log("\n[*] Flushing telemetry to AgentLens...");
    const flushed = await flush({ timeoutMs: 3000 });
    console.log(`[*] Flush result: ${flushed}`);
    await shutdown();
    console.log("[*] Telemetry shutdown complete.");
  }
}

main().catch((err) => {
  console.error("Pipeline failure:", err);
  process.exit(1);
});
