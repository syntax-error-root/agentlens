"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { Agent } from "@/types";
import {
  AlertCircle,
  Bot,
  Calendar,
  Code2,
  ExternalLink,
  Plus,
  RefreshCw,
  X,
} from "lucide-react";

export default function AgentsPage() {
  const { currentProject } = useApp();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // New agent modal
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [framework, setFramework] = useState("");
  const [language, setLanguage] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);

  const fetchAgents = useCallback(async () => {
    if (!currentProject) return;
    setLoading(true);
    setError(null);

    try {
      const data = await api.agents.list(currentProject.id);
      setAgents(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load agents");
    } finally {
      setLoading(false);
    }
  }, [currentProject]);

  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  const handleCreateAgent = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!currentProject) return;
    setSaving(true);
    setError(null);

    try {
      await api.agents.create(currentProject.id, {
        name,
        framework: framework || undefined,
        language: language || undefined,
        description: description || undefined,
      });

      setIsModalOpen(false);
      setName("");
      setFramework("");
      setLanguage("");
      setDescription("");
      fetchAgents();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create agent");
    } finally {
      setSaving(false);
    }
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
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-neutral-100">
            Agent Registry
          </h1>
          <p className="text-xs text-neutral-400">
            Registered AI agents, models, frameworks, and deployment versions.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={fetchAgents}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-850 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
            <span>Refresh</span>
          </button>
          <button
            onClick={() => setIsModalOpen(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-neutral-950 font-semibold text-xs transition-colors"
          >
            <Plus size={14} />
            <span>Register Agent</span>
          </button>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Agents Grid */}
      {loading && agents.length === 0 ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/30 animate-pulse h-36"
            />
          ))}
        </div>
      ) : agents.length === 0 ? (
        <div className="p-12 text-center rounded-xl border border-neutral-850 bg-neutral-900/30 space-y-3">
          <Bot size={32} className="mx-auto text-neutral-600" />
          <h3 className="text-sm font-semibold text-neutral-300">
            No Agents Registered Yet
          </h3>
          <p className="text-xs text-neutral-500 max-w-sm mx-auto">
            Agents are automatically registered when traces are received, or you can register them manually.
          </p>
          <button
            onClick={() => setIsModalOpen(true)}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500 text-neutral-950 font-semibold text-xs"
          >
            <Plus size={13} />
            <span>Register First Agent</span>
          </button>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {agents.map((agent) => (
            <Link
              key={agent.id}
              href={`/agents/${agent.id}`}
              className="p-5 rounded-xl border border-neutral-850 bg-neutral-900/40 hover:bg-neutral-850/50 hover:border-neutral-700 transition-all flex flex-col justify-between group space-y-4"
            >
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <div className="w-7 h-7 rounded-lg bg-purple-950/70 border border-purple-800/60 flex items-center justify-center text-purple-400">
                      <Bot size={15} />
                    </div>
                    <span className="font-semibold text-sm text-neutral-100 group-hover:text-emerald-400 transition-colors">
                      {agent.name}
                    </span>
                  </div>
                  {agent.framework && (
                    <span className="text-[10px] font-mono px-2 py-0.5 rounded border border-neutral-800 bg-neutral-950 text-neutral-400">
                      {agent.framework}
                    </span>
                  )}
                </div>

                <p className="text-xs text-neutral-400 line-clamp-2">
                  {agent.description || "No description provided."}
                </p>
              </div>

              <div className="pt-3 border-t border-neutral-850/80 flex items-center justify-between text-[11px] font-mono text-neutral-500">
                <div className="flex items-center gap-1">
                  <Code2 size={13} />
                  <span>{agent.language || "python"}</span>
                </div>
                <div>
                  {agent.last_seen_at
                    ? `Seen ${new Date(agent.last_seen_at).toLocaleDateString()}`
                    : "Never executed"}
                </div>
              </div>
            </Link>
          ))}
        </div>
      )}

      {/* Registration Modal */}
      {isModalOpen && (
        <div className="fixed inset-0 z-50 bg-neutral-950/80 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150">
          <div className="w-full max-w-md bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-neutral-800 pb-3">
              <h3 className="text-sm font-semibold text-neutral-100">
                Register New Agent
              </h3>
              <button
                onClick={() => setIsModalOpen(false)}
                className="text-neutral-500 hover:text-neutral-300"
              >
                <X size={16} />
              </button>
            </div>

            <form onSubmit={handleCreateAgent} className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-neutral-300 mb-1">
                  Agent Name *
                </label>
                <input
                  type="text"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. customer-support-agent"
                  className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-neutral-300 mb-1">
                    Framework
                  </label>
                  <input
                    type="text"
                    value={framework}
                    onChange={(e) => setFramework(e.target.value)}
                    placeholder="LangGraph / CrewAI"
                    className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-neutral-300 mb-1">
                    Language
                  </label>
                  <input
                    type="text"
                    value={language}
                    onChange={(e) => setLanguage(e.target.value)}
                    placeholder="python / typescript"
                    className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-neutral-300 mb-1">
                  Description
                </label>
                <textarea
                  rows={3}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Autonomous customer resolution agent..."
                  className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                />
              </div>

              <div className="pt-2 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setIsModalOpen(false)}
                  className="px-3 py-1.5 rounded-lg border border-neutral-800 text-xs text-neutral-400 hover:bg-neutral-850"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="px-4 py-1.5 rounded-lg bg-emerald-500 text-neutral-950 font-semibold text-xs hover:bg-emerald-400 disabled:opacity-50"
                >
                  {saving ? "Registering..." : "Register"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
