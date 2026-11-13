"use client";

import { useMemo, useState } from "react";
import { Span } from "@/types";
import {
  buildSpanTree,
  flattenSpanTree,
  getSpanTypeStyle,
  SpanTreeNode,
} from "@/lib/waterfall";
import {
  AlertCircle,
  ChevronDown,
  ChevronRight,
  Maximize2,
  Minimize2,
} from "lucide-react";

interface SpanWaterfallProps {
  spans: Span[];
  selectedSpan: Span | null;
  onSelectSpan: (span: Span) => void;
}

export function SpanWaterfall({
  spans,
  selectedSpan,
  onSelectSpan,
}: SpanWaterfallProps) {
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(new Set());

  const { roots, totalDurationMs, traceStartMs } = useMemo(() => {
    return buildSpanTree(spans);
  }, [spans]);

  const visibleNodes = useMemo(() => {
    return flattenSpanTree(roots, collapsedIds);
  }, [roots, collapsedIds]);

  const toggleCollapse = (spanId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setCollapsedIds((prev) => {
      const next = new Set(prev);
      if (next.has(spanId)) {
        next.delete(spanId);
      } else {
        next.add(spanId);
      }
      return next;
    });
  };

  const collapseAll = () => {
    const allParentIds = new Set<string>();
    function collect(node: SpanTreeNode) {
      if (node.children.length > 0) {
        allParentIds.add(node.span.span_id);
        node.children.forEach(collect);
      }
    }
    roots.forEach(collect);
    setCollapsedIds(allParentIds);
  };

  const expandAll = () => {
    setCollapsedIds(new Set());
  };

  if (!spans || spans.length === 0) {
    return (
      <div className="p-8 text-center text-xs text-neutral-500 border border-neutral-850 rounded-xl bg-neutral-900/30">
        No spans found for this trace.
      </div>
    );
  }

  return (
    <div className="border border-neutral-850 rounded-xl bg-neutral-900/40 overflow-hidden flex flex-col">
      {/* Top toolbar */}
      <div className="px-4 py-2.5 border-b border-neutral-850 bg-neutral-950/40 flex items-center justify-between text-xs">
        <div className="flex items-center gap-3">
          <span className="font-semibold text-neutral-300">
            Timeline Waterfall ({spans.length} spans)
          </span>
          <span className="text-neutral-500 font-mono">
            Total Duration:{" "}
            {totalDurationMs >= 1000
              ? `${(totalDurationMs / 1000).toFixed(2)}s`
              : `${totalDurationMs.toFixed(1)}ms`}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={expandAll}
            className="flex items-center gap-1 px-2 py-1 rounded text-[11px] text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800"
            title="Expand all nodes"
          >
            <Maximize2 size={12} />
            <span>Expand</span>
          </button>
          <button
            onClick={collapseAll}
            className="flex items-center gap-1 px-2 py-1 rounded text-[11px] text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800"
            title="Collapse all nodes"
          >
            <Minimize2 size={12} />
            <span>Collapse</span>
          </button>
        </div>
      </div>

      {/* Waterfall Grid Header: Tree column & Timeline ruler */}
      <div className="grid grid-cols-12 border-b border-neutral-850 bg-neutral-950/70 text-[11px] font-mono text-neutral-500 select-none">
        <div className="col-span-5 px-4 py-1.5 border-r border-neutral-850">
          Span Hierarchy
        </div>
        <div className="col-span-7 px-4 py-1.5 flex justify-between relative">
          <span>0ms</span>
          <span>{(totalDurationMs * 0.25).toFixed(0)}ms</span>
          <span>{(totalDurationMs * 0.5).toFixed(0)}ms</span>
          <span>{(totalDurationMs * 0.75).toFixed(0)}ms</span>
          <span>{totalDurationMs.toFixed(0)}ms</span>
        </div>
      </div>

      {/* Span Rows */}
      <div className="divide-y divide-neutral-850/60 font-sans">
        {visibleNodes.map((node) => {
          const { span, depth, children, offsetPercent, widthPercent } = node;
          const isSelected = selectedSpan?.id === span.id;
          const isCollapsed = collapsedIds.has(span.span_id);
          const hasChildren = children.length > 0;
          const typeStyle = getSpanTypeStyle(span.span_type);
          const isError = span.status === "error";

          const spanDurationMs =
            span.duration_ms ??
            (span.ended_at
              ? Math.max(
                  0,
                  new Date(span.ended_at).getTime() -
                    new Date(span.started_at).getTime()
                )
              : 0);

          return (
            <div
              key={span.id}
              onClick={() => onSelectSpan(span)}
              className={`grid grid-cols-12 items-center cursor-pointer text-xs transition-colors group ${
                isSelected
                  ? "bg-neutral-800/90 text-neutral-100"
                  : "hover:bg-neutral-850/50 text-neutral-300"
              }`}
            >
              {/* Left Column: Span Tree Node */}
              <div
                className="col-span-5 px-3 py-2 flex items-center gap-1.5 border-r border-neutral-850 overflow-hidden"
                style={{ paddingLeft: `${Math.max(12, depth * 16 + 12)}px` }}
              >
                {/* Expand / Collapse toggle */}
                {hasChildren ? (
                  <button
                    onClick={(e) => toggleCollapse(span.span_id, e)}
                    className="p-0.5 text-neutral-400 hover:text-neutral-100 rounded"
                    aria-label={isCollapsed ? "Expand" : "Collapse"}
                  >
                    {isCollapsed ? (
                      <ChevronRight size={13} />
                    ) : (
                      <ChevronDown size={13} />
                    )}
                  </button>
                ) : (
                  <span className="w-3.5 inline-block" />
                )}

                {/* Type Badge */}
                <span
                  className={`text-[9px] font-mono px-1.5 py-0.2 rounded border font-bold shrink-0 ${typeStyle.badgeBg}`}
                >
                  {typeStyle.label}
                </span>

                {/* Error Icon if failed */}
                {isError && (
                  <AlertCircle size={13} className="text-rose-400 shrink-0" />
                )}

                {/* Span Name */}
                <span
                  className="font-medium truncate max-w-[200px]"
                  title={span.name}
                >
                  {span.name}
                </span>

                {/* Model badge if present */}
                {span.model && (
                  <span className="text-[10px] text-neutral-500 font-mono shrink-0 hidden sm:inline">
                    ({span.model})
                  </span>
                )}

                {/* Duration */}
                <span className="ml-auto text-[10px] font-mono text-neutral-500 shrink-0 pl-2">
                  {spanDurationMs >= 1000
                    ? `${(spanDurationMs / 1000).toFixed(2)}s`
                    : `${spanDurationMs.toFixed(1)}ms`}
                </span>
              </div>

              {/* Right Column: Timeline Waterfall Bar */}
              <div className="col-span-7 px-4 py-2 relative flex items-center h-full">
                {/* Subtle vertical grid lines */}
                <div className="absolute inset-0 flex justify-between pointer-events-none opacity-10">
                  <div className="w-px bg-neutral-500 h-full" />
                  <div className="w-px bg-neutral-500 h-full" />
                  <div className="w-px bg-neutral-500 h-full" />
                  <div className="w-px bg-neutral-500 h-full" />
                  <div className="w-px bg-neutral-500 h-full" />
                </div>

                {/* Duration Bar */}
                <div
                  className={`h-4 rounded relative transition-all shadow-sm ${
                    isError ? "bg-rose-500" : typeStyle.barBg
                  }`}
                  style={{
                    marginLeft: `${offsetPercent}%`,
                    width: `${widthPercent}%`,
                  }}
                  title={`${span.name}: ${spanDurationMs.toFixed(1)}ms (${offsetPercent.toFixed(1)}% offset)`}
                >
                  {/* Internal duration label if wide enough */}
                  {widthPercent > 12 && (
                    <span className="absolute inset-0 flex items-center px-1.5 text-[9px] font-mono font-bold text-neutral-950 truncate select-none">
                      {spanDurationMs.toFixed(1)}ms
                    </span>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
