"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import { api } from "@/lib/api";
import { Agent, AgentVersion, Trace } from "@/types";
import {
  AlertCircle,
  ArrowLeft,
  Bot,
  Calendar,
  Code2,
  GitCommit,
  Layers,
  RefreshCw,
} from "lucide-react";

export default function AgentDetailPage() {
  const params = useParams();
  const agentId = params.id as string;

  const [agent, setAgent] = useState<Agent | null>(null);
  const [versions, setVersions] = useState<AgentVersion[]>([]);
  const [traces, setTraces] = useState<Trace[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"traces" | "versions">("traces");

  const loadData = useCallback(async () => {
    if (!agentId) return;
    setLoading(true);
    setError(null);

    try {
      const [agentData, versionsData, tracesData] = await Promise.all([
        api.agents.get(agentId),
        api.agents.listVersions(agentId),
        api.agents.listTraces(agentId),
      ]);

      setAgent(agentData);
      setVersions(versionsData);
      setTraces(tracesData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load agent");
    } finally {
      setLoading(false);
    }
  }, [agentId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const formatDuration = (val: number | null): string => {
    if (val === null || val === undefined) return "—";
    if (val >= 1000) return `${(val / 1000).toFixed(2)}s`;
    return `${val.toFixed(1)}ms`;
  };

  if (loading && !agent) {
    return (
      <div className="flex h-64 items-center justify-center text-neutral-400">
        <div className="flex flex-col items-center gap-3">
          <div className="w-6 h-6 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs font-mono">Loading agent...</span>
        </div>
      </div>
    );
  }

  if (error || !agent) {
    return (
      <div className="space-y-4">
        <Link
          href="/agents"
          className="inline-flex items-center gap-1.5 text-xs text-neutral-400 hover:text-neutral-200"
        >
          <ArrowLeft size={14} />
          <span>Back to agents</span>
        </Link>
        <div className="p-4 rounded-xl bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error || "Agent not found"}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header breadcrumb & reload */}
      <div className="flex items-center justify-between">
        <Link
          href="/agents"
          className="inline-flex items-center gap-1.5 text-xs text-neutral-400 hover:text-neutral-200 transition-colors"
        >
          <ArrowLeft size={14} />
          <span>Back to agent registry</span>
        </Link>
        <button
          onClick={loadData}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-800 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          <span>Reload</span>
        </button>
      </div>

      {/* Agent Summary Card */}
      <div className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-purple-950/70 border border-purple-800/60 flex items-center justify-center text-purple-400 shrink-0">
              <Bot size={20} />
            </div>
            <div>
              <h1 className="text-lg font-bold text-neutral-100">{agent.name}</h1>
              <p className="text-xs text-neutral-400">
                {agent.description || "No description provided."}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 text-xs font-mono text-neutral-400">
            {agent.framework && (
              <span className="px-2.5 py-1 rounded-lg border border-neutral-800 bg-neutral-950">
                {agent.framework}
              </span>
            )}
            <span className="px-2.5 py-1 rounded-lg border border-neutral-800 bg-neutral-950">
              {agent.language || "python"}
            </span>
            <span className="text-[11px] text-neutral-500">
              {agent.last_seen_at
                ? `Last seen ${new Date(agent.last_seen_at).toLocaleDateString()}`
                : "No telemetry recorded"}
            </span>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="border-b border-neutral-850 flex gap-4 text-xs">
        <button
          onClick={() => setActiveTab("traces")}
          className={`pb-2 font-medium border-b-2 transition-colors ${
            activeTab === "traces"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          Traces ({traces.length})
        </button>
        <button
          onClick={() => setActiveTab("versions")}
          className={`pb-2 font-medium border-b-2 transition-colors ${
            activeTab === "versions"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          Version History ({versions.length})
        </button>
      </div>

      {/* Tab 1: Traces */}
      {activeTab === "traces" && (
        <div className="border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30">
          <table className="w-full text-left text-xs">
            <thead className="bg-neutral-950/80 border-b border-neutral-850 text-[11px] font-mono text-neutral-500 uppercase tracking-wider">
              <tr>
                <th className="py-2.5 px-4">Trace Name</th>
                <th className="py-2.5 px-4">Trace ID</th>
                <th className="py-2.5 px-4">Status</th>
                <th className="py-2.5 px-4">Duration</th>
                <th className="py-2.5 px-4">Spans</th>
                <th className="py-2.5 px-4">Tokens</th>
                <th className="py-2.5 px-4">Timestamp</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-850">
              {traces.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-8 px-4 text-center text-neutral-500">
                    No traces recorded for this agent yet.
                  </td>
                </tr>
              ) : (
                traces.map((trace) => (
                  <tr key={trace.id} className="hover:bg-neutral-850/50 transition-colors">
                    <td className="py-3 px-4 font-medium text-neutral-200">
                      <Link
                        href={`/traces/${trace.trace_id}`}
                        className="hover:text-emerald-400"
                      >
                        {trace.name}
                      </Link>
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-400">
                      <Link
                        href={`/traces/${trace.trace_id}`}
                        className="hover:underline hover:text-emerald-400"
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
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-300">
                      {(
                        trace.total_input_tokens + trace.total_output_tokens
                      ).toLocaleString()}
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
      )}

      {/* Tab 2: Versions */}
      {activeTab === "versions" && (
        <div className="border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30">
          <table className="w-full text-left text-xs">
            <thead className="bg-neutral-950/80 border-b border-neutral-850 text-[11px] font-mono text-neutral-500 uppercase tracking-wider">
              <tr>
                <th className="py-2.5 px-4">Version</th>
                <th className="py-2.5 px-4">Commit Hash</th>
                <th className="py-2.5 px-4">Metadata</th>
                <th className="py-2.5 px-4">Recorded At</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-850">
              {versions.length === 0 ? (
                <tr>
                  <td colSpan={4} className="py-8 px-4 text-center text-neutral-500">
                    No explicit version records registered for this agent.
                  </td>
                </tr>
              ) : (
                versions.map((ver) => (
                  <tr key={ver.id} className="hover:bg-neutral-850/50 transition-colors">
                    <td className="py-3 px-4 font-mono font-semibold text-emerald-400">
                      {ver.version}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-400">
                      {ver.commit_hash ? ver.commit_hash.slice(0, 10) : "—"}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-400">
                      {JSON.stringify(ver.metadata)}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-500">
                      {new Date(ver.created_at).toLocaleString()}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
