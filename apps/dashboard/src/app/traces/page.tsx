"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { Trace, TraceQueryResult, TraceStatus, SpanType } from "@/types";
import {
  Activity,
  AlertCircle,
  ChevronLeft,
  ChevronRight,
  Filter,
  RefreshCw,
  Search,
} from "lucide-react";

export default function TracesExplorerPage() {
  const { currentProject, currentEnvironment } = useApp();

  const [traces, setTraces] = useState<Trace[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [searchTraceId, setSearchTraceId] = useState("");
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [providerFilter, setProviderFilter] = useState<string>("");
  const [spanTypeFilter, setSpanTypeFilter] = useState<string>("all");
  const [minDuration, setMinDuration] = useState<string>("");
  const [maxDuration, setMaxDuration] = useState<string>("");

  // Pagination
  const [cursor, setCursor] = useState<string | undefined>(undefined);
  const [cursorHistory, setCursorHistory] = useState<string[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);

  const fetchTraces = useCallback(
    async (targetCursor?: string) => {
      if (!currentProject) return;

      setLoading(true);
      setError(null);

      try {
        const res: TraceQueryResult = await api.traces.list({
          project_id: currentProject.id,
          environment_id: currentEnvironment?.id,
          status: statusFilter !== "all" ? statusFilter : undefined,
          provider: providerFilter.trim() || undefined,
          span_type: spanTypeFilter !== "all" ? spanTypeFilter : undefined,
          min_duration_ms: minDuration ? Number(minDuration) : undefined,
          max_duration_ms: maxDuration ? Number(maxDuration) : undefined,
          search_trace_id: searchTraceId.trim() || undefined,
          cursor: targetCursor,
          limit: 25,
        });

        setTraces(res.items);
        setNextCursor(res.next_cursor);
        setHasMore(res.has_more);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load traces");
      } finally {
        setLoading(false);
      }
    },
    [
      currentProject,
      currentEnvironment,
      statusFilter,
      providerFilter,
      spanTypeFilter,
      minDuration,
      maxDuration,
      searchTraceId,
    ]
  );

  useEffect(() => {
    setCursor(undefined);
    setCursorHistory([]);
    fetchTraces(undefined);
  }, [fetchTraces]);

  const handleNextPage = () => {
    if (nextCursor) {
      setCursorHistory((prev) => [...prev, cursor || ""]);
      setCursor(nextCursor);
      fetchTraces(nextCursor);
    }
  };

  const handlePrevPage = () => {
    if (cursorHistory.length > 0) {
      const prevHistory = [...cursorHistory];
      const prevCursor = prevHistory.pop();
      setCursorHistory(prevHistory);
      setCursor(prevCursor || undefined);
      fetchTraces(prevCursor || undefined);
    }
  };

  const formatDuration = (val: number | null): string => {
    if (val === null || val === undefined) return "—";
    if (val >= 1000) return `${(val / 1000).toFixed(2)}s`;
    return `${val.toFixed(1)}ms`;
  };

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-neutral-100">
            Trace Explorer
          </h1>
          <p className="text-xs text-neutral-400">
            Search, filter, and inspect OpenTelemetry traces with sub-millisecond precision.
          </p>
        </div>
        <button
          onClick={() => fetchTraces(cursor)}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-800 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          <span>Refresh</span>
        </button>
      </div>

      {/* Filter Bar */}
      <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-3">
        <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-neutral-400">
          <Filter size={14} />
          <span>Filters</span>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-6 gap-3">
          {/* Search Trace ID */}
          <div className="lg:col-span-2 relative">
            <Search
              size={14}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-neutral-500"
            />
            <input
              type="text"
              value={searchTraceId}
              onChange={(e) => setSearchTraceId(e.target.value)}
              placeholder="Search trace ID (hex)..."
              className="w-full pl-8 pr-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 font-mono text-neutral-200 placeholder:text-neutral-600 focus:outline-none focus:border-emerald-500"
            />
          </div>

          {/* Status Filter */}
          <div>
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-200 focus:outline-none focus:border-emerald-500 cursor-pointer"
            >
              <option value="all">All Statuses</option>
              <option value="ok">Success (OK)</option>
              <option value="error">Error Only</option>
            </select>
          </div>

          {/* Span Type Filter */}
          <div>
            <select
              value={spanTypeFilter}
              onChange={(e) => setSpanTypeFilter(e.target.value)}
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-200 focus:outline-none focus:border-emerald-500 cursor-pointer"
            >
              <option value="all">All Types</option>
              <option value="agent">Agent</option>
              <option value="chain">Chain</option>
              <option value="llm">LLM</option>
              <option value="tool">Tool</option>
              <option value="retriever">Retriever</option>
              <option value="router">Router</option>
              <option value="guardrail">Guardrail</option>
            </select>
          </div>

          {/* Min Duration */}
          <div>
            <input
              type="number"
              value={minDuration}
              onChange={(e) => setMinDuration(e.target.value)}
              placeholder="Min ms"
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-200 placeholder:text-neutral-600 focus:outline-none focus:border-emerald-500"
            />
          </div>

          {/* Max Duration */}
          <div>
            <input
              type="number"
              value={maxDuration}
              onChange={(e) => setMaxDuration(e.target.value)}
              placeholder="Max ms"
              className="w-full px-2.5 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-200 placeholder:text-neutral-600 focus:outline-none focus:border-emerald-500"
            />
          </div>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Traces Table */}
      <div className="border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-neutral-950/80 border-b border-neutral-850 text-[11px] font-mono text-neutral-500 uppercase tracking-wider">
              <tr>
                <th className="py-2.5 px-4">Trace Name</th>
                <th className="py-2.5 px-4">Trace ID</th>
                <th className="py-2.5 px-4">Status</th>
                <th className="py-2.5 px-4">Duration</th>
                <th className="py-2.5 px-4">Spans</th>
                <th className="py-2.5 px-4">Tokens</th>
                <th className="py-2.5 px-4">Cost</th>
                <th className="py-2.5 px-4">Timestamp</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-850">
              {loading && traces.length === 0 ? (
                // Skeletons
                Array.from({ length: 5 }).map((_, i) => (
                  <tr key={i} className="animate-pulse">
                    <td colSpan={8} className="py-3 px-4">
                      <div className="h-4 bg-neutral-850 rounded w-full" />
                    </td>
                  </tr>
                ))
              ) : traces.length === 0 ? (
                <tr>
                  <td colSpan={8} className="py-12 px-4 text-center text-neutral-500">
                    No traces matching the specified criteria found.
                  </td>
                </tr>
              ) : (
                traces.map((trace) => (
                  <tr
                    key={trace.id}
                    className="hover:bg-neutral-850/50 transition-colors group"
                  >
                    <td className="py-3 px-4 font-medium text-neutral-200">
                      <Link
                        href={`/traces/${trace.trace_id}`}
                        className="hover:text-emerald-400 flex items-center gap-1.5"
                      >
                        <span>{trace.name}</span>
                      </Link>
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-400">
                      <Link
                        href={`/traces/${trace.trace_id}`}
                        className="hover:underline hover:text-emerald-400"
                        title={trace.trace_id}
                      >
                        {trace.trace_id.slice(0, 16)}...
                      </Link>
                    </td>
                    <td className="py-3 px-4">
                      <span
                        className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-bold ${
                          trace.status === "error"
                            ? "bg-rose-950/60 border-rose-800 text-rose-300"
                            : "bg-emerald-950/60 border-emerald-800 text-emerald-300"
                        }`}
                      >
                        {trace.status}
                      </span>
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-300">
                      {formatDuration(trace.duration_ms)}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-300">
                      {trace.span_count}
                      {trace.error_span_count > 0 && (
                        <span className="text-rose-400 font-bold ml-1">
                          ({trace.error_span_count} err)
                        </span>
                      )}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-300">
                      {(
                        trace.total_input_tokens + trace.total_output_tokens
                      ).toLocaleString()}
                    </td>
                    <td className="py-3 px-4 font-mono text-emerald-400">
                      {trace.estimated_cost
                        ? `$${Number(trace.estimated_cost).toFixed(4)}`
                        : "—"}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-500 whitespace-nowrap">
                      {new Date(trace.started_at).toLocaleString()}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Keyset Cursor Pagination Footer */}
        <div className="p-3 border-t border-neutral-850 bg-neutral-950/40 flex items-center justify-between text-xs text-neutral-500">
          <span>Showing {traces.length} traces</span>
          <div className="flex items-center gap-2">
            <button
              onClick={handlePrevPage}
              disabled={cursorHistory.length === 0 || loading}
              className="flex items-center gap-1 px-3 py-1.5 rounded-lg border border-neutral-850 bg-neutral-900 text-neutral-300 hover:bg-neutral-850 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              <ChevronLeft size={14} />
              <span>Previous</span>
            </button>
            <button
              onClick={handleNextPage}
              disabled={!hasMore || !nextCursor || loading}
              className="flex items-center gap-1 px-3 py-1.5 rounded-lg border border-neutral-850 bg-neutral-900 text-neutral-300 hover:bg-neutral-850 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
              <span>Next</span>
              <ChevronRight size={14} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
