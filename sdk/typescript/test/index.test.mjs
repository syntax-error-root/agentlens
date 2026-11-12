import test from "node:test";
import assert from "node:assert/strict";
import { AgentLens } from "../dist/index.js";

test("AgentLens SDK initializes with default configuration", () => {
  const client = new AgentLens({ apiKey: "agl_live_test_key" });
  const config = client.getConfig();
  assert.equal(config.apiKey, "agl_live_test_key");
  assert.equal(config.captureContent, false);
  assert.equal(config.endpoint, "http://localhost:4318/v1/traces");
});
