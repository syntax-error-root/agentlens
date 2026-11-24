"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { ProjectOverviewMetrics } from "@/types";
import {
  Activity,
  AlertCircle,
  Bot,
  Clock,
  Coins,
  ExternalLink,
  Flame,
  Terminal,
} from "lucide-react";

export default function OverviewPage() {
  const { currentProject, currentEnvironment, timeRange } = useApp();
  const [metrics, setMetrics] = useState<ProjectOverviewMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentProject) {
      setLoading(false);
      return;
    }

    let isMounted = true;
    setLoading(true);
    setError(null);

    // Calculate time range boundary
    const now = new Date();
    let fromDate = new Date();
    switch (timeRange) {
      case "1h":
        fromDate = new Date(now.getTime() - 60 * 60 * 1000);
        break;
      case "6h":
        fromDate = new Date(now.getTime() - 6 * 60 * 60 * 1000);
        break;
      case "24h":
        fromDate = new Date(now.getTime() - 24 * 60 * 60 * 1000);
        break;
      case "7d":
        fromDate = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
        break;
      case "30d":
        fromDate = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);
        break;
    }

    api.overview
      .get(currentProject.id, {
        environment_id: currentEnvironment?.id,
        from: fromDate.toISOString(),
        to: now.toISOString(),
      })
      .then((data) => {
        if (isMounted) setMetrics(data);
      })
      .catch((err) => {
        if (isMounted) {
          setError(err instanceof Error ? err.message : "Failed to load overview");
        }
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [currentProject, currentEnvironment, timeRange]);

  const formatTokens = (val: number): string => {
    if (val >= 1_000_000) return `${(val / 1_000_000).toFixed(1)}M`;
    if (val >= 1_000) return `${(val / 1_000).toFixed(1)}k`;
    return val.toString();
  };

  const formatDuration = (val: number | null): string => {
    if (val === null || val === undefined) return "—";
    if (val >= 1000) return `${(val / 1000).toFixed(2)}s`;
    return `${val.toFixed(0)}ms`;
  };

  if (!currentProject) {
    return (
      <div className="p-8 text-center text-xs text-neutral-500 border border-neutral-850 rounded-xl bg-neutral-900/30">
        No active project selected.
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Page Title */}
      <div>
        <h1 className="text-xl font-bold tracking-tight text-neutral-100">
          Project Overview
        </h1>
        <p className="text-xs text-neutral-400">
          Live telemetry metrics and trace execution health for{" "}
          <span className="text-neutral-200 font-semibold">{currentProject.name}</span>
          {currentEnvironment && (
            <span> ({currentEnvironment.name})</span>
          )}
        </p>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Metrics Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        {/* Metric 1: Active Agents */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-2">
          <div className="flex items-center justify-between text-neutral-400">
            <span className="text-xs font-medium uppercase tracking-wider">Active Agents</span>
            <Bot size={16} className="text-purple-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {loading ? "..." : metrics?.active_agents ?? 0}
          </div>
        </div>

        {/* Metric 2: Total Traces */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-2">
          <div className="flex items-center justify-between text-neutral-400">
            <span className="text-xs font-medium uppercase tracking-wider">Total Traces</span>
            <Activity size={16} className="text-blue-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {loading ? "..." : (metrics?.total_traces ?? 0).toLocaleString()}
          </div>
        </div>

        {/* Metric 3: Error Rate */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-2">
          <div className="flex items-center justify-between text-neutral-400">
            <span className="text-xs font-medium uppercase tracking-wider">Error Rate</span>
            <Flame size={16} className="text-rose-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {loading ? "..." : `${(metrics?.error_rate_pct ?? 0).toFixed(1)}%`}
          </div>
        </div>

        {/* Metric 4: Total Tokens */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-2">
          <div className="flex items-center justify-between text-neutral-400">
            <span className="text-xs font-medium uppercase tracking-wider">Total Tokens</span>
            <Coins size={16} className="text-amber-400" />
          </div>
          <div className="text-2xl font-bold font-mono text-neutral-100">
            {loading ? "..." : formatTokens(metrics?.total_tokens ?? 0)}
          </div>
        </div>

        {/* Metric 5: Latency P50 / P95 */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-2">
          <div className="flex items-center justify-between text-neutral-400">
            <span className="text-xs font-medium uppercase tracking-wider">P50 / P95</span>
            <Clock size={16} className="text-emerald-400" />
          </div>
          <div className="text-lg font-bold font-mono text-neutral-100">
            {loading
              ? "..."
              : `${formatDuration(metrics?.p50_duration_ms ?? null)} / ${formatDuration(
                  metrics?.p95_duration_ms ?? null
                )}`}
          </div>
        </div>
      </div>

      {/* Main Content: Recent Traces & Quickstart if empty */}
      {metrics && metrics.total_traces === 0 ? (
        /* Empty State with Quickstart */
        <div className="p-8 rounded-xl border border-neutral-850 bg-neutral-900/30 text-center space-y-4">
          <div className="w-12 h-12 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 flex items-center justify-center mx-auto">
            <Terminal size={24} />
          </div>
          <div className="space-y-1">
            <h3 className="text-base font-semibold text-neutral-200">
              No Traces Ingested Yet
            </h3>
            <p className="text-xs text-neutral-400 max-w-md mx-auto">
              Instrument your AI agents using the AgentLens Python or TypeScript SDK to view live execution traces.
            </p>
          </div>

          <div className="max-w-xl mx-auto text-left rounded-lg border border-neutral-800 bg-neutral-950 p-4 font-mono text-xs text-neutral-300 space-y-2">
            <div className="text-neutral-500 text-[10px] uppercase font-bold tracking-wider">
              Python Quickstart
            </div>
            <pre className="text-emerald-400 overflow-x-auto whitespace-pre">
              pip install agentlens
            </pre>
            <pre className="overflow-x-auto whitespace-pre text-neutral-300">
{`from agentlens import AgentLens

al = AgentLens(
    endpoint="http://localhost:4318",
    api_key="agl_live_<your_key>",
    agent_name="support-agent"
)

with al.trace("user_query") as t:
    # Your agent code here...
    t.log_event("query_processed")`}
            </pre>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Recent Traces Table */}
          <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/30 space-y-4">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-neutral-200">Recent Traces</h2>
              <Link
                href="/traces"
                className="text-xs text-emerald-400 hover:text-emerald-300 flex items-center gap-1 font-medium"
              >
                <span>View all</span>
                <ExternalLink size={12} />
              </Link>
            </div>

            <div className="divide-y divide-neutral-850 border border-neutral-850 rounded-lg overflow-hidden bg-neutral-950/40 text-xs font-sans">
              {(metrics?.recent_traces || []).length === 0 ? (
                <div className="p-4 text-center text-neutral-500">No traces in selected window</div>
              ) : (
                metrics?.recent_traces.map((trace) => (
                  <Link
                    key={trace.id}
                    href={`/traces/${trace.trace_id}`}
                    className="flex items-center justify-between p-3 hover:bg-neutral-850/50 transition-colors"
                  >
                    <div className="space-y-0.5 truncate pr-3">
                      <div className="font-medium text-neutral-200 truncate">{trace.name}</div>
                      <div className="text-[10px] text-neutral-500 font-mono">
                        {trace.trace_id.slice(0, 12)}... • {trace.span_count} spans
                      </div>
                    </div>
                    <div className="text-right shrink-0">
                      <span
                        className={`text-[10px] font-mono px-1.5 py-0.5 rounded border uppercase ${
                          trace.status === "error"
                            ? "bg-rose-950/60 border-rose-800 text-rose-300"
                            : "bg-emerald-950/60 border-emerald-800 text-emerald-300"
                        }`}
                      >
                        {trace.status}
                      </span>
                      <div className="text-[10px] font-mono text-neutral-500 mt-1">
                        {formatDuration(trace.duration_ms)}
                      </div>
                    </div>
                  </Link>
                ))
              )}
            </div>
          </div>

          {/* Recent Errors Table */}
          <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/30 space-y-4">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-neutral-200">Recent Errors</h2>
              <span className="text-xs text-neutral-500">
                {(metrics?.recent_errors || []).length} captured
              </span>
            </div>

            <div className="divide-y divide-neutral-850 border border-neutral-850 rounded-lg overflow-hidden bg-neutral-950/40 text-xs font-sans">
              {(metrics?.recent_errors || []).length === 0 ? (
                <div className="p-4 text-center text-neutral-500">Zero errors in selected window</div>
              ) : (
                metrics?.recent_errors.map((trace) => (
                  <Link
                    key={trace.id}
                    href={`/traces/${trace.trace_id}`}
                    className="flex items-center justify-between p-3 hover:bg-neutral-850/50 transition-colors"
                  >
                    <div className="space-y-0.5 truncate pr-3">
                      <div className="font-medium text-rose-300 truncate">{trace.name}</div>
                      <div className="text-[10px] text-neutral-500 font-mono">
                        {trace.trace_id.slice(0, 12)}... • {trace.error_span_count} errors
                      </div>
                    </div>
                    <div className="text-right shrink-0 text-[10px] font-mono text-neutral-500">
                      {new Date(trace.started_at).toLocaleTimeString()}
                    </div>
                  </Link>
                ))
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
