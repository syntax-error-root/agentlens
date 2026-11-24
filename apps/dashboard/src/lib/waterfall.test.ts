import test from "node:test";
import assert from "node:assert/strict";
import { buildSpanTree, flattenSpanTree } from "./waterfall.ts";
import type { Span } from "../types/index.ts";

function mockSpan(overrides: Partial<Span>): Span {
  return {
    id: "00000000-0000-0000-0000-000000000001",
    organization_id: "00000000-0000-0000-0000-000000000002",
    project_id: "00000000-0000-0000-0000-000000000003",
    trace_id: "4bf92f3577b34da6a3ce929d0e0e4736",
    span_id: "00f067aa0ba902b7",
    parent_span_id: null,
    agent_id: null,
    name: "root_span",
    span_type: "agent",
    status: "ok",
    started_at: "2026-11-13T00:00:00.000Z",
    ended_at: "2026-11-13T00:00:01.000Z",
    duration_ms: 1000,
    provider: null,
    model: null,
    tool_name: null,
    attributes: {},
    resource_attributes: {},
    scope_attributes: {},
    payload_s3_key: null,
    error: null,
    received_at: "2026-11-13T00:00:01.000Z",
    created_at: "2026-11-13T00:00:01.000Z",
    ...overrides,
  };
}

test("buildSpanTree: handles empty list", () => {
  const result = buildSpanTree([]);
  assert.equal(result.roots.length, 0);
  assert.equal(result.totalDurationMs, 0);
});

test("buildSpanTree: builds single root node with correct offset and width", () => {
  const span = mockSpan({ span_id: "span1", duration_ms: 1000 });
  const result = buildSpanTree([span]);
  assert.equal(result.roots.length, 1);
  assert.equal(result.roots[0].span.span_id, "span1");
  assert.equal(result.roots[0].depth, 0);
  assert.equal(result.roots[0].offsetPercent, 0);
  assert.equal(result.roots[0].widthPercent, 100);
});

test("buildSpanTree: builds nested parent-child tree hierarchy", () => {
  const root = mockSpan({
    span_id: "root",
    parent_span_id: null,
    started_at: "2026-11-13T00:00:00.000Z",
    duration_ms: 1000,
  });
  const child1 = mockSpan({
    span_id: "child1",
    parent_span_id: "root",
    started_at: "2026-11-13T00:00:00.100Z",
    duration_ms: 400,
  });
  const grandchild = mockSpan({
    span_id: "grandchild",
    parent_span_id: "child1",
    started_at: "2026-11-13T00:00:00.200Z",
    duration_ms: 200,
  });

  const result = buildSpanTree([root, child1, grandchild]);
  assert.equal(result.roots.length, 1);
  assert.equal(result.roots[0].children.length, 1);
  assert.equal(result.roots[0].children[0].span.span_id, "child1");
  assert.equal(result.roots[0].children[0].depth, 1);
  assert.equal(result.roots[0].children[0].children.length, 1);
  assert.equal(result.roots[0].children[0].children[0].span.span_id, "grandchild");
  assert.equal(result.roots[0].children[0].children[0].depth, 2);
});

test("buildSpanTree: safely recovers orphan child whose parent is missing/in-flight", () => {
  const orphanChild = mockSpan({
    span_id: "orphan1",
    parent_span_id: "missing_parent",
    started_at: "2026-11-13T00:00:00.000Z",
    duration_ms: 500,
  });
  const regularRoot = mockSpan({
    span_id: "root1",
    parent_span_id: null,
    started_at: "2026-11-13T00:00:00.100Z",
    duration_ms: 500,
  });

  const result = buildSpanTree([orphanChild, regularRoot]);
  assert.equal(result.roots.length, 2, "Both orphan and regular root must be preserved");
  assert.equal(result.roots[0].span.span_id, "orphan1");
  assert.equal(result.roots[1].span.span_id, "root1");
});

test("flattenSpanTree: respects collapsed nodes", () => {
  const root = mockSpan({ span_id: "root", parent_span_id: null });
  const child = mockSpan({ span_id: "child", parent_span_id: "root" });

  const tree = buildSpanTree([root, child]);
  const flatExpanded = flattenSpanTree(tree.roots, new Set());
  assert.equal(flatExpanded.length, 2);

  const flatCollapsed = flattenSpanTree(tree.roots, new Set(["root"]));
  assert.equal(flatCollapsed.length, 1);
  assert.equal(flatCollapsed[0].span.span_id, "root");
});
