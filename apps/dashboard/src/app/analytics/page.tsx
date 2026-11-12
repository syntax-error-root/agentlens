"use client";

import { useEffect, useState } from "react";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import {
  AgentAnalyticsItem,
  AnalyticsOverview,
  CostAnalyticsResult,
  ErrorAnalyticsResult,
  LatencyAnalyticsResult,
  ModelAnalyticsItem,
  TokenAnalyticsResult,
  ToolAnalyticsItem,
} from "@/types";
import {
  AlertTriangle,
  Coins,
  Cpu,
  Layers,
  Sparkles,
  Timer,
  Wrench,
  Zap,
} from "lucide-react";

export default function AnalyticsPage() {
  const { currentProject, currentEnvironment, timeRange } = useApp();

  const [overview, setOverview] = useState<AnalyticsOverview | null>(null);
  const [tokens, setTokens] = useState<TokenAnalyticsResult | null>(null);
  const [costs, setCosts] = useState<CostAnalyticsResult | null>(null);
  const [models, setModels] = useState<ModelAnalyticsItem[]>([]);
  const [agents, setAgents] = useState<AgentAnalyticsItem[]>([]);
  const [tools, setTools] = useState<ToolAnalyticsItem[]>([]);
  const [latency, setLatency] = useState<LatencyAnalyticsResult | null>(null);
  const [errors, setErrors] = useState<ErrorAnalyticsResult | null>(null);

  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  useEffect(() => {
    if (!currentProject) {
      setLoading(false);
      return;
    }

    let isMounted = true;
    setLoading(true);
    setFetchError(null);

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

    const params = {
      projectId: currentProject.id,
      environmentId: currentEnvironment?.id,
      from: fromDate.toISOString(),
      to: now.toISOString(),
    };

    Promise.all([
      api.analytics.getOverview(params),
      api.analytics.getTokens(params),
      api.analytics.getCosts(params),
      api.analytics.getModels(params),
      api.analytics.getAgents(params),
      api.analytics.getTools(params),
      api.analytics.getLatency(params),
      api.analytics.getErrors(params),
    ])
      .then(
        ([
          ovData,
          tokData,
          costData,
          modelData,
          agentData,
          toolData,
          latData,
          errData,
        ]) => {
          if (isMounted) {
            setOverview(ovData);
            setTokens(tokData);
            setCosts(costData);
            setModels(modelData);
            setAgents(agentData);
            setTools(toolData);
            setLatency(latData);
            setErrors(errData);
          }
        }
      )
      .catch((err) => {
        if (isMounted) {
          setFetchError(
            err instanceof Error ? err.message : "Failed to load analytics data"
          );
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
    if (val >= 1_000_000) return `${(val / 1_000_000).toFixed(2)}M`;
    if (val >= 1_000) return `${(val / 1_000).toFixed(1)}k`;
    return val.toString();
  };

  const formatCost = (val: string | null | undefined): string => {
    if (val === null || val === undefined) return "—";
    const num = parseFloat(val);
    if (isNaN(num)) return "—";
    if (num === 0) return "$0.00";
    if (num < 0.01) return `$${num.toFixed(6)}`;
    return `$${num.toFixed(4)}`;
  };

  const formatDuration = (val: number | null | undefined): string => {
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
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-neutral-850 pb-5">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-neutral-100 flex items-center gap-2.5">
            <Coins className="text-amber-400" size={22} />
            Analytics & Pricing
          </h1>
          <p className="text-xs text-neutral-400 mt-1">
            Financial token accounting, exact decimal model pricing, and continuous latency percentiles.
          </p>
        </div>
      </div>

      {fetchError && (
        <div className="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 text-xs flex items-center gap-2.5">
          <AlertTriangle size={16} className="shrink-0" />
          <span>{fetchError}</span>
        </div>
      )}

      {/* Unpriced Warning Alert */}
      {overview?.has_unpriced_usage && (
        <div className="p-4 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs flex items-start gap-3">
          <AlertTriangle size={18} className="text-amber-400 shrink-0 mt-0.5" />
          <div>
            <span className="font-semibold text-amber-200">
              Unpriced Model Usage Detected
            </span>
            <p className="mt-0.5 text-neutral-300 leading-relaxed">
              Traces in this scope contain LLM calls with models or providers that lack configured pricing rate cards.
              Reported USD costs are lower-bound totals and do not default to $0.00 for unpriced models.
            </p>
          </div>
        </div>
      )}

      {/* Overview Metric Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Cost Card */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/50 backdrop-blur-sm relative overflow-hidden">
          <div className="flex items-center justify-between text-neutral-400 text-xs mb-2">
            <span className="font-medium flex items-center gap-1.5">
              <Coins size={14} className="text-amber-400" />
              Estimated Cost
            </span>
            <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-400 border border-neutral-750">
              USD
            </span>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono tracking-tight text-neutral-100">
              {loading ? "..." : formatCost(overview?.total_cost)}
            </span>
          </div>
          <div className="mt-2 text-[11px] text-neutral-500 flex items-center gap-1">
            {overview?.has_unpriced_usage ? (
              <span className="text-amber-400 flex items-center gap-1">
                <AlertTriangle size={11} /> Lower bound (unpriced traces present)
              </span>
            ) : (
              <span>100% pricing card coverage</span>
            )}
          </div>
        </div>

        {/* Total Tokens Card */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/50 backdrop-blur-sm">
          <div className="flex items-center justify-between text-neutral-400 text-xs mb-2">
            <span className="font-medium flex items-center gap-1.5">
              <Sparkles size={14} className="text-emerald-400" />
              Total Tokens
            </span>
            <span className="text-[10px] font-mono text-neutral-500">
              {overview?.total_traces || 0} traces
            </span>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono tracking-tight text-neutral-100">
              {loading ? "..." : formatTokens(overview?.total_tokens || 0)}
            </span>
          </div>
          <div className="mt-2 text-[11px] text-neutral-400 font-mono flex items-center gap-2">
            <span>In: {formatTokens(overview?.total_input_tokens || 0)}</span>
            <span>•</span>
            <span>Out: {formatTokens(overview?.total_output_tokens || 0)}</span>
          </div>
        </div>

        {/* Cached & Reasoning Tokens Card */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/50 backdrop-blur-sm">
          <div className="flex items-center justify-between text-neutral-400 text-xs mb-2">
            <span className="font-medium flex items-center gap-1.5">
              <Zap size={14} className="text-cyan-400" />
              Cache & Reasoning
            </span>
            <span className="text-[10px] font-mono text-cyan-400/80">Optimization</span>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono tracking-tight text-neutral-100">
              {loading ? "..." : formatTokens(overview?.total_cached_tokens || 0)}
            </span>
            <span className="text-xs text-neutral-400 font-mono">cached</span>
          </div>
          <div className="mt-2 text-[11px] text-neutral-400 font-mono">
            Reasoning: {formatTokens(overview?.total_reasoning_tokens || 0)} tokens
          </div>
        </div>

        {/* Latency Percentiles Card */}
        <div className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/50 backdrop-blur-sm">
          <div className="flex items-center justify-between text-neutral-400 text-xs mb-2">
            <span className="font-medium flex items-center gap-1.5">
              <Timer size={14} className="text-purple-400" />
              Latency (P50 / P95 / P99)
            </span>
            <span className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-400 border border-neutral-750">
              MS
            </span>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-bold font-mono tracking-tight text-neutral-100">
              {loading ? "..." : formatDuration(overview?.p50_duration_ms)}
            </span>
            <span className="text-xs text-neutral-400 font-mono">P50</span>
          </div>
          <div className="mt-2 text-[11px] text-neutral-400 font-mono flex items-center gap-2">
            <span>P95: {formatDuration(overview?.p95_duration_ms)}</span>
            <span>•</span>
            <span>P99: {formatDuration(overview?.p99_duration_ms)}</span>
          </div>
        </div>
      </div>

      {/* Cost & Token Time Series */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Cost Time Series */}
        <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/30">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-sm font-semibold text-neutral-200 flex items-center gap-2">
              <Coins size={16} className="text-amber-400" />
              Daily Cost History (USD)
            </h3>
            <span className="text-xs font-mono text-neutral-400">
              Total: {formatCost(costs?.total_estimated_cost)}
            </span>
          </div>
          {costs?.time_series.length === 0 ? (
            <div className="h-44 flex items-center justify-center text-xs text-neutral-500 font-mono border border-dashed border-neutral-800 rounded-lg">
              No cost data for selected time window
            </div>
          ) : (
            <div className="space-y-2">
              {costs?.time_series.slice(-7).map((pt) => {
                const costNum = parseFloat(pt.cost || "0");
                const maxCost = Math.max(
                  ...costs.time_series.map((t) => parseFloat(t.cost || "0")),
                  0.001
                );
                const pct = Math.min(100, Math.max(5, (costNum / maxCost) * 100));
                return (
                  <div key={pt.date} className="flex items-center gap-3 text-xs">
                    <span className="w-20 font-mono text-neutral-400 text-[11px]">
                      {pt.date}
                    </span>
                    <div className="flex-1 h-5 bg-neutral-850 rounded overflow-hidden flex items-center px-2">
                      <div
                        className="h-full bg-amber-500/30 border-r-2 border-amber-400 transition-all"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                    <span className="w-24 text-right font-mono text-neutral-200">
                      {formatCost(pt.cost)}
                    </span>
                    {pt.unpriced_traces_count > 0 && (
                      <span className="text-[10px] text-amber-400 flex items-center gap-0.5" title={`${pt.unpriced_traces_count} unpriced traces`}>
                        <AlertTriangle size={11} />
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Token Usage Breakdown */}
        <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/30">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-sm font-semibold text-neutral-200 flex items-center gap-2">
              <Sparkles size={16} className="text-emerald-400" />
              Token Composition
            </h3>
            <span className="text-xs font-mono text-neutral-400">
              {formatTokens(tokens?.total_tokens || 0)} Total
            </span>
          </div>
          <div className="grid grid-cols-2 gap-3 mb-4">
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800">
              <div className="text-[11px] text-neutral-400">Standard Input</div>
              <div className="text-lg font-mono font-bold text-neutral-100">
                {formatTokens(tokens?.total_input_tokens || 0)}
              </div>
            </div>
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800">
              <div className="text-[11px] text-neutral-400">Output Generated</div>
              <div className="text-lg font-mono font-bold text-neutral-100">
                {formatTokens(tokens?.total_output_tokens || 0)}
              </div>
            </div>
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800">
              <div className="text-[11px] text-cyan-400">Cached Prompt Tokens</div>
              <div className="text-lg font-mono font-bold text-cyan-200">
                {formatTokens(tokens?.total_cached_tokens || 0)}
              </div>
            </div>
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800">
              <div className="text-[11px] text-purple-400">Reasoning / CoT Tokens</div>
              <div className="text-lg font-mono font-bold text-purple-200">
                {formatTokens(tokens?.total_reasoning_tokens || 0)}
              </div>
            </div>
          </div>
          <p className="text-[11px] text-neutral-500 leading-relaxed">
            Cached tokens are billed at lower prompt-cache rates where supported, avoiding double-counting with standard input.
          </p>
        </div>
      </div>

      {/* Model Breakdown Table */}
      <div className="rounded-xl border border-neutral-850 bg-neutral-900/40 overflow-hidden">
        <div className="px-5 py-4 border-b border-neutral-850 flex items-center justify-between">
          <div>
            <h3 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
              <Cpu size={16} className="text-indigo-400" />
              Model Cost & Volume Comparison
            </h3>
            <p className="text-[11px] text-neutral-400 mt-0.5">
              Aggregated usage and evaluated decimal costs per AI provider and model pattern.
            </p>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead className="bg-neutral-900/80 text-neutral-400 border-b border-neutral-850 uppercase text-[10px] tracking-wider">
              <tr>
                <th className="px-4 py-3">Provider / Model</th>
                <th className="px-4 py-3 text-right">Calls</th>
                <th className="px-4 py-3 text-right">Input</th>
                <th className="px-4 py-3 text-right">Output</th>
                <th className="px-4 py-3 text-right">Cached</th>
                <th className="px-4 py-3 text-right">Reasoning</th>
                <th className="px-4 py-3 text-right">Est. Cost (USD)</th>
                <th className="px-4 py-3 text-center">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-850/60 font-mono">
              {models.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-8 text-center text-neutral-500 font-sans">
                    No LLM calls recorded in selected period
                  </td>
                </tr>
              ) : (
                models.map((m, idx) => (
                  <tr key={`${m.provider}-${m.model}-${idx}`} className="hover:bg-neutral-850/30 transition-colors">
                    <td className="px-4 py-3">
                      <div className="flex flex-col">
                        <span className="font-semibold text-neutral-200">{m.model}</span>
                        <span className="text-[10px] text-neutral-500 lowercase">{m.provider}</span>
                      </div>
                    </td>
                    <td className="px-4 py-3 text-right text-neutral-300">
                      {m.call_count.toLocaleString()}
                    </td>
                    <td className="px-4 py-3 text-right text-neutral-400">
                      {formatTokens(m.total_input_tokens)}
                    </td>
                    <td className="px-4 py-3 text-right text-neutral-400">
                      {formatTokens(m.total_output_tokens)}
                    </td>
                    <td className="px-4 py-3 text-right text-cyan-400/90">
                      {m.total_cached_tokens > 0 ? formatTokens(m.total_cached_tokens) : "—"}
                    </td>
                    <td className="px-4 py-3 text-right text-purple-400/90">
                      {m.total_reasoning_tokens > 0 ? formatTokens(m.total_reasoning_tokens) : "—"}
                    </td>
                    <td className="px-4 py-3 text-right font-bold text-neutral-100">
                      {formatCost(m.estimated_cost)}
                    </td>
                    <td className="px-4 py-3 text-center">
                      {m.has_unpriced ? (
                        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-sans font-medium bg-amber-500/10 text-amber-400 border border-amber-500/30">
                          <AlertTriangle size={10} /> Unpriced
                        </span>
                      ) : (
                        <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-sans font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                          Priced
                        </span>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Tool Performance & Latency Details */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Tool Performance */}
        <div className="rounded-xl border border-neutral-850 bg-neutral-900/40 overflow-hidden">
          <div className="px-5 py-4 border-b border-neutral-850">
            <h3 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
              <Wrench size={16} className="text-cyan-400" />
              Tool Execution Performance
            </h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="bg-neutral-900/80 text-neutral-400 border-b border-neutral-850 uppercase text-[10px] tracking-wider">
                <tr>
                  <th className="px-4 py-2.5">Tool Name</th>
                  <th className="px-4 py-2.5 text-right">Calls</th>
                  <th className="px-4 py-2.5 text-right">Error %</th>
                  <th className="px-4 py-2.5 text-right">P50 (ms)</th>
                  <th className="px-4 py-2.5 text-right">P95 (ms)</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-neutral-850/60 font-mono">
                {tools.length === 0 ? (
                  <tr>
                    <td colSpan={5} className="px-4 py-6 text-center text-neutral-500 font-sans">
                      No tool spans recorded in this period
                    </td>
                  </tr>
                ) : (
                  tools.map((t) => (
                    <tr key={t.tool_name} className="hover:bg-neutral-850/30">
                      <td className="px-4 py-2.5 font-sans font-medium text-neutral-200">
                        {t.tool_name}
                      </td>
                      <td className="px-4 py-2.5 text-right text-neutral-300">
                        {t.call_count.toLocaleString()}
                      </td>
                      <td className="px-4 py-2.5 text-right">
                        <span className={t.error_rate > 0 ? "text-red-400 font-bold" : "text-neutral-500"}>
                          {(t.error_rate * 100).toFixed(1)}%
                        </span>
                      </td>
                      <td className="px-4 py-2.5 text-right text-neutral-400">
                        {formatDuration(t.p50_duration_ms)}
                      </td>
                      <td className="px-4 py-2.5 text-right text-neutral-400">
                        {formatDuration(t.p95_duration_ms)}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>

        {/* Latency Percentiles Distribution */}
        <div className="rounded-xl border border-neutral-850 bg-neutral-900/40 p-5">
          <h3 className="text-sm font-semibold text-neutral-100 flex items-center gap-2 mb-4">
            <Timer size={16} className="text-purple-400" />
            Continuous Latency Distribution
          </h3>
          <div className="grid grid-cols-3 gap-3 mb-5">
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800 text-center">
              <span className="text-[10px] uppercase font-mono text-neutral-500">P50 Median</span>
              <div className="text-lg font-mono font-bold text-neutral-100 mt-1">
                {formatDuration(latency?.p50_duration_ms)}
              </div>
            </div>
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800 text-center">
              <span className="text-[10px] uppercase font-mono text-neutral-500">P95 Tail</span>
              <div className="text-lg font-mono font-bold text-neutral-100 mt-1">
                {formatDuration(latency?.p95_duration_ms)}
              </div>
            </div>
            <div className="p-3 rounded-lg bg-neutral-900 border border-neutral-800 text-center">
              <span className="text-[10px] uppercase font-mono text-neutral-500">P99 Outlier</span>
              <div className="text-lg font-mono font-bold text-neutral-100 mt-1">
                {formatDuration(latency?.p99_duration_ms)}
              </div>
            </div>
          </div>

          <div className="space-y-2 text-xs border-t border-neutral-850 pt-4">
            <div className="flex justify-between text-neutral-400">
              <span>Fastest Trace (Min):</span>
              <span className="font-mono text-neutral-200">{formatDuration(latency?.min_duration_ms)}</span>
            </div>
            <div className="flex justify-between text-neutral-400">
              <span>Average Duration:</span>
              <span className="font-mono text-neutral-200">{formatDuration(latency?.avg_duration_ms)}</span>
            </div>
            <div className="flex justify-between text-neutral-400">
              <span>Slowest Trace (Max):</span>
              <span className="font-mono text-neutral-200">{formatDuration(latency?.max_duration_ms)}</span>
            </div>
            <div className="flex justify-between text-neutral-400">
              <span>Analyzed Traces:</span>
              <span className="font-mono text-neutral-200">{latency?.total_count || 0}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
