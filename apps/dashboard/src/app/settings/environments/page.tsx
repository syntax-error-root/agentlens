"use client";

import { useEffect, useState, useCallback } from "react";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { Environment } from "@/types";
import { AlertCircle, Layers, RefreshCw } from "lucide-react";

export default function EnvironmentsPage() {
  const { currentProject } = useApp();
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchEnvs = useCallback(async () => {
    if (!currentProject) return;
    setLoading(true);
    setError(null);

    try {
      const data = await api.projects.listEnvironments(currentProject.id);
      setEnvironments(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load environments");
    } finally {
      setLoading(false);
    }
  }, [currentProject]);

  useEffect(() => {
    fetchEnvs();
  }, [fetchEnvs]);

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
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-neutral-100">
            Project Environments
          </h1>
          <p className="text-xs text-neutral-400">
            Isolated deployment tiers for tracing and access control.
          </p>
        </div>
        <button
          onClick={fetchEnvs}
          disabled={loading}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-850 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
          <span>Refresh</span>
        </button>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Environments Table */}
      <div className="border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30">
        <table className="w-full text-left text-xs">
          <thead className="bg-neutral-950/80 border-b border-neutral-850 text-[11px] font-mono text-neutral-500 uppercase tracking-wider">
            <tr>
              <th className="py-2.5 px-4">Environment Name</th>
              <th className="py-2.5 px-4">Slug</th>
              <th className="py-2.5 px-4">Description</th>
              <th className="py-2.5 px-4">Created Date</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-neutral-850">
            {loading && environments.length === 0 ? (
              Array.from({ length: 3 }).map((_, i) => (
                <tr key={i} className="animate-pulse">
                  <td colSpan={4} className="py-3 px-4">
                    <div className="h-4 bg-neutral-850 rounded w-full" />
                  </td>
                </tr>
              ))
            ) : environments.length === 0 ? (
              <tr>
                <td colSpan={4} className="py-8 px-4 text-center text-neutral-500">
                  No environments configured for this project.
                </td>
              </tr>
            ) : (
              environments.map((env) => (
                <tr key={env.id} className="hover:bg-neutral-850/40 transition-colors">
                  <td className="py-3 px-4 font-semibold text-neutral-100 flex items-center gap-2">
                    <Layers size={14} className="text-emerald-400" />
                    <span>{env.name}</span>
                  </td>
                  <td className="py-3 px-4 font-mono text-neutral-400">
                    {env.slug}
                  </td>
                  <td className="py-3 px-4 text-neutral-400">
                    {env.description || "—"}
                  </td>
                  <td className="py-3 px-4 font-mono text-neutral-500">
                    {new Date(env.created_at).toLocaleDateString()}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
