"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { api } from "@/lib/api";
import { Span, Trace } from "@/types";
import { SpanWaterfall } from "@/components/waterfall/SpanWaterfall";
import { SpanDetailDrawer } from "@/components/waterfall/SpanDetailDrawer";
import {
  AlertCircle,
  ArrowLeft,
  Clock,
  Coins,
  Copy,
  Layers,
  RefreshCw,
} from "lucide-react";

export default function TraceDetailPage() {
  const params = useParams();
  const traceIdParam = params.id as string;

  const [trace, setTrace] = useState<Trace | null>(null);
  const [spans, setSpans] = useState<Span[]>([]);
  const [selectedSpan, setSelectedSpan] = useState<Span | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const loadTraceData = useCallback(async () => {
    if (!traceIdParam) return;
    setLoading(true);
    setError(null);

    try {
      const [traceData, spansData] = await Promise.all([
        api.traces.get(traceIdParam),
        api.traces.getSpans(traceIdParam),
      ]);

      setTrace(traceData);
      setSpans(spansData);
      if (spansData.length > 0 && !selectedSpan) {
        // Select root span by default
        setSelectedSpan(spansData[0]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load trace details");
    } finally {
      setLoading(false);
    }
  }, [traceIdParam, selectedSpan]);

  useEffect(() => {
    loadTraceData();
  }, [loadTraceData]);

  const copyTraceId = () => {
    if (trace) {
      navigator.clipboard.writeText(trace.trace_id);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const formatDuration = (val: number | null): string => {
    if (val === null || val === undefined) return "—";
    if (val >= 1000) return `${(val / 1000).toFixed(2)}s`;
    return `${val.toFixed(1)}ms`;
  };

  if (loading && !trace) {
    return (
      <div className="flex h-64 items-center justify-center text-neutral-400">
        <div className="flex flex-col items-center gap-3">
          <div className="w-6 h-6 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs font-mono">Loading trace spans...</span>
        </div>
      </div>
    );
  }

  if (error || !trace) {
    return (
      <div className="space-y-4">
        <Link
          href="/traces"
          className="inline-flex items-center gap-1.5 text-xs text-neutral-400 hover:text-neutral-200"
        >
          <ArrowLeft size={14} />
          <span>Back to traces</span>
        </Link>
        <div className="p-4 rounded-xl bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error || "Trace not found"}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6 relative">
      {/* Header breadcrumb & actions */}
      <div className="flex items-center justify-between">
        <Link
          href="/traces"
          className="inline-flex items-center gap-1.5 text-xs text-neutral-400 hover:text-neutral-200 transition-colors"
        >
          <ArrowLeft size={14} />
          <span>Back to trace explorer</span>
        </Link>
        <button
          onClick={loadTraceData}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-800 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          <span>Reload</span>
        </button>
      </div>

      {/* Trace Overview Card */}
      <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-4 shadow-sm">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <span
                className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-bold ${
                  trace.status === "error"
                    ? "bg-rose-950/60 border-rose-800 text-rose-300"
                    : "bg-emerald-950/60 border-emerald-800 text-emerald-300"
                }`}
              >
                {trace.status}
              </span>
              <h1 className="text-lg font-bold text-neutral-100">{trace.name}</h1>
            </div>
            <div className="flex items-center gap-2 text-xs font-mono text-neutral-400">
              <span>{trace.trace_id}</span>
              <button
                onClick={copyTraceId}
                className="text-neutral-500 hover:text-neutral-300 p-0.5 rounded"
                title="Copy trace ID"
              >
                <Copy size={13} />
              </button>
              {copied && <span className="text-emerald-400 text-[10px]">Copied!</span>}
            </div>
          </div>

          {/* Quick Metrics */}
          <div className="flex items-center gap-4 text-xs font-mono bg-neutral-950/60 px-4 py-2.5 rounded-lg border border-neutral-850">
            <div>
              <span className="text-[10px] text-neutral-500 block uppercase">Duration</span>
              <span className="text-neutral-200 font-bold">
                {formatDuration(trace.duration_ms)}
              </span>
            </div>
            <div className="w-px h-6 bg-neutral-850" />
            <div>
              <span className="text-[10px] text-neutral-500 block uppercase">Spans</span>
              <span className="text-neutral-200 font-bold">
                {trace.span_count}
                {trace.error_span_count > 0 && (
                  <span className="text-rose-400 ml-1">({trace.error_span_count} err)</span>
                )}
              </span>
            </div>
            <div className="w-px h-6 bg-neutral-850" />
            <div>
              <span className="text-[10px] text-neutral-500 block uppercase">Tokens</span>
              <span className="text-neutral-200 font-bold">
                {(trace.total_input_tokens + trace.total_output_tokens).toLocaleString()}
              </span>
            </div>
            <div className="w-px h-6 bg-neutral-850" />
            <div>
              <span className="text-[10px] text-neutral-500 block uppercase">Cost</span>
              <span className="text-emerald-400 font-bold">
                {trace.estimated_cost ? `$${Number(trace.estimated_cost).toFixed(4)}` : "—"}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Interactive Span Waterfall */}
      <SpanWaterfall
        spans={spans}
        selectedSpan={selectedSpan}
        onSelectSpan={(span) => setSelectedSpan(span)}
      />

      {/* Span Detail Drawer */}
      <SpanDetailDrawer
        span={selectedSpan}
        onClose={() => setSelectedSpan(null)}
      />
    </div>
  );
}
