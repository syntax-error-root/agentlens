import type { Span } from "../types/index.ts";

export interface SpanTreeNode {
  span: Span;
  children: SpanTreeNode[];
  depth: number;
  offsetPercent: number;
  widthPercent: number;
}

/**
 * Reconstructs a hierarchical tree from a flat list of spans.
 * Gracefully handles missing/in-flight parent spans by attaching orphans to the root level.
 */
export function buildSpanTree(spans: Span[]): {
  roots: SpanTreeNode[];
  totalDurationMs: number;
  traceStartMs: number;
} {
  if (!spans || spans.length === 0) {
    return { roots: [], totalDurationMs: 0, traceStartMs: 0 };
  }

  // Find trace boundary timestamps
  let traceStartMs = Number.MAX_SAFE_INTEGER;
  let traceEndMs = 0;

  for (const span of spans) {
    const start = new Date(span.started_at).getTime();
    const end = span.ended_at
      ? new Date(span.ended_at).getTime()
      : span.duration_ms
      ? start + span.duration_ms
      : start;

    if (start < traceStartMs) traceStartMs = start;
    if (end > traceEndMs) traceEndMs = end;
  }

  if (traceStartMs === Number.MAX_SAFE_INTEGER) {
    traceStartMs = 0;
    traceEndMs = 0;
  }

  const rawDuration = traceEndMs - traceStartMs;
  const totalDurationMs = rawDuration > 0 ? rawDuration : 1;

  // Build map of span_id -> Span
  const spanMap = new Map<string, Span>();
  const childrenMap = new Map<string, Span[]>();

  for (const span of spans) {
    spanMap.set(span.span_id, span);
  }

  const rootSpans: Span[] = [];

  for (const span of spans) {
    if (!span.parent_span_id || !spanMap.has(span.parent_span_id)) {
      // Root span or orphan span with missing in-flight parent
      rootSpans.push(span);
    } else {
      const list = childrenMap.get(span.parent_span_id) || [];
      list.push(span);
      childrenMap.set(span.parent_span_id, list);
    }
  }

  // Sort root spans by started_at
  rootSpans.sort(
    (a, b) => new Date(a.started_at).getTime() - new Date(b.started_at).getTime()
  );

  function createNode(span: Span, depth: number): SpanTreeNode {
    const startMs = new Date(span.started_at).getTime();
    const duration =
      span.duration_ms ??
      (span.ended_at ? Math.max(0, new Date(span.ended_at).getTime() - startMs) : 0);

    const offsetPercent = Math.max(
      0,
      Math.min(100, ((startMs - traceStartMs) / totalDurationMs) * 100)
    );

    // Minimum width of 0.5% so sub-millisecond spans remain visible and interactive
    const rawWidth = (duration / totalDurationMs) * 100;
    const widthPercent = Math.max(0.5, Math.min(100 - offsetPercent, rawWidth));

    const childrenSpans = childrenMap.get(span.span_id) || [];
    childrenSpans.sort(
      (a, b) =>
        new Date(a.started_at).getTime() - new Date(b.started_at).getTime()
    );

    const children = childrenSpans.map((child) => createNode(child, depth + 1));

    return {
      span,
      children,
      depth,
      offsetPercent,
      widthPercent,
    };
  }

  const roots = rootSpans.map((span) => createNode(span, 0));

  return { roots, totalDurationMs, traceStartMs };
}

/**
 * Flattens a tree into a sequence for rendering, taking expanded state into account.
 */
export function flattenSpanTree(
  nodes: SpanTreeNode[],
  collapsedIds: Set<string>
): SpanTreeNode[] {
  const result: SpanTreeNode[] = [];

  function traverse(node: SpanTreeNode) {
    result.push(node);
    if (!collapsedIds.has(node.span.span_id)) {
      for (const child of node.children) {
        traverse(child);
      }
    }
  }

  for (const node of nodes) {
    traverse(node);
  }

  return result;
}

/**
 * Maps span type to visual styling metadata (colors and labels).
 */
export function getSpanTypeStyle(type: string): {
  label: string;
  badgeBg: string;
  barBg: string;
  textColor: string;
} {
  switch (type) {
    case "agent":
      return {
        label: "AGENT",
        badgeBg: "bg-purple-950/70 border-purple-800/60 text-purple-300",
        barBg: "bg-purple-500",
        textColor: "text-purple-400",
      };
    case "chain":
      return {
        label: "CHAIN",
        badgeBg: "bg-indigo-950/70 border-indigo-800/60 text-indigo-300",
        barBg: "bg-indigo-500",
        textColor: "text-indigo-400",
      };
    case "llm":
      return {
        label: "LLM",
        badgeBg: "bg-emerald-950/70 border-emerald-800/60 text-emerald-300",
        barBg: "bg-emerald-500",
        textColor: "text-emerald-400",
      };
    case "tool":
      return {
        label: "TOOL",
        badgeBg: "bg-amber-950/70 border-amber-800/60 text-amber-300",
        barBg: "bg-amber-500",
        textColor: "text-amber-400",
      };
    case "retriever":
      return {
        label: "RETRIEVER",
        badgeBg: "bg-cyan-950/70 border-cyan-800/60 text-cyan-300",
        barBg: "bg-cyan-500",
        textColor: "text-cyan-400",
      };
    case "router":
      return {
        label: "ROUTER",
        badgeBg: "bg-blue-950/70 border-blue-800/60 text-blue-300",
        barBg: "bg-blue-500",
        textColor: "text-blue-400",
      };
    case "guardrail":
      return {
        label: "GUARDRAIL",
        badgeBg: "bg-rose-950/70 border-rose-800/60 text-rose-300",
        barBg: "bg-rose-500",
        textColor: "text-rose-400",
      };
    case "evaluator":
      return {
        label: "EVALUATOR",
        badgeBg: "bg-fuchsia-950/70 border-fuchsia-800/60 text-fuchsia-300",
        barBg: "bg-fuchsia-500",
        textColor: "text-fuchsia-400",
      };
    default:
      return {
        label: "CUSTOM",
        badgeBg: "bg-neutral-900 border-neutral-700 text-neutral-300",
        barBg: "bg-neutral-500",
        textColor: "text-neutral-400",
      };
  }
}
