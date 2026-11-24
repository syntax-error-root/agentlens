"use client";

import { useEffect, useState, useCallback } from "react";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { ApiKey, Environment } from "@/types";
import {
  AlertCircle,
  AlertTriangle,
  Check,
  Copy,
  Key,
  Plus,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";

export default function ApiKeysPage() {
  const { currentProject, environments } = useApp();
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Create Key Modal
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [keyName, setKeyName] = useState("");
  const [envId, setEnvId] = useState("");
  const [expiresInDays, setExpiresInDays] = useState<number | "">("");
  const [creating, setCreating] = useState(false);

  // Plaintext secret revealed once
  const [newSecret, setNewSecret] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const fetchKeys = useCallback(async () => {
    if (!currentProject) return;
    setLoading(true);
    setError(null);

    try {
      const data = await api.projects.listApiKeys(currentProject.id);
      setApiKeys(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load API keys");
    } finally {
      setLoading(false);
    }
  }, [currentProject]);

  useEffect(() => {
    fetchKeys();
    if (environments.length > 0 && !envId) {
      setEnvId(environments[0].id);
    }
  }, [fetchKeys, environments, envId]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!currentProject || !envId) return;

    setCreating(true);
    setError(null);

    try {
      const res = await api.projects.createApiKey(currentProject.id, {
        environment_id: envId,
        name: keyName,
        expires_in_days: expiresInDays ? Number(expiresInDays) : undefined,
      });

      setNewSecret(res.secret);
      setIsCreateOpen(false);
      setKeyName("");
      setExpiresInDays("");
      fetchKeys();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create API key");
    } finally {
      setCreating(false);
    }
  };

  const handleRevoke = async (keyId: string) => {
    if (!confirm("Are you sure you want to revoke this API key? This action is immediate and cannot be undone.")) {
      return;
    }

    try {
      await api.projects.revokeApiKey(keyId);
      fetchKeys();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to revoke API key");
    }
  };

  const copySecret = () => {
    if (newSecret) {
      navigator.clipboard.writeText(newSecret);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
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
            Ingestion API Keys
          </h1>
          <p className="text-xs text-neutral-400">
            Scoped credentials for OpenTelemetry OTLP trace ingestion (<code className="text-neutral-300">agl_live_...</code>).
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={fetchKeys}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-850 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
            <span>Refresh</span>
          </button>
          <button
            onClick={() => setIsCreateOpen(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-neutral-950 font-semibold text-xs transition-colors"
          >
            <Plus size={14} />
            <span>Create API Key</span>
          </button>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
          <AlertCircle size={16} className="shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Secret Reveal Modal (shown once on creation) */}
      {newSecret && (
        <div className="p-4 rounded-xl border border-emerald-850/80 bg-emerald-950/30 text-xs space-y-3">
          <div className="flex items-center gap-2 text-emerald-400 font-semibold">
            <Key size={16} />
            <span>API Key Created — Copy Plaintext Secret</span>
          </div>
          <p className="text-neutral-300 text-xs">
            Make sure to copy your API key now. You won&apos;t be able to see it again!
          </p>
          <div className="flex items-center gap-2 bg-neutral-950 border border-emerald-800/60 p-2.5 rounded-lg">
            <code className="flex-1 font-mono text-emerald-300 text-xs select-all break-all">
              {newSecret}
            </code>
            <button
              onClick={copySecret}
              className="flex items-center gap-1 px-3 py-1.5 rounded bg-emerald-500 hover:bg-emerald-400 text-neutral-950 font-semibold text-xs transition-colors shrink-0"
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
              <span>{copied ? "Copied" : "Copy"}</span>
            </button>
          </div>
          <div className="flex justify-end">
            <button
              onClick={() => setNewSecret(null)}
              className="text-neutral-400 hover:text-neutral-200 text-xs underline"
            >
              I have stored this key safely
            </button>
          </div>
        </div>
      )}

      {/* API Keys Table */}
      <div className="border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30">
        <table className="w-full text-left text-xs">
          <thead className="bg-neutral-950/80 border-b border-neutral-850 text-[11px] font-mono text-neutral-500 uppercase tracking-wider">
            <tr>
              <th className="py-2.5 px-4">Name</th>
              <th className="py-2.5 px-4">Prefix</th>
              <th className="py-2.5 px-4">Environment</th>
              <th className="py-2.5 px-4">Status</th>
              <th className="py-2.5 px-4">Last Used</th>
              <th className="py-2.5 px-4">Expires</th>
              <th className="py-2.5 px-4 text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-neutral-850">
            {loading && apiKeys.length === 0 ? (
              Array.from({ length: 3 }).map((_, i) => (
                <tr key={i} className="animate-pulse">
                  <td colSpan={7} className="py-3 px-4">
                    <div className="h-4 bg-neutral-850 rounded w-full" />
                  </td>
                </tr>
              ))
            ) : apiKeys.length === 0 ? (
              <tr>
                <td colSpan={7} className="py-8 px-4 text-center text-neutral-500">
                  No API keys generated for this project yet.
                </td>
              </tr>
            ) : (
              apiKeys.map((k) => {
                const isRevoked = k.revoked_at !== null;
                const env = environments.find((e) => e.id === k.environment_id);
                return (
                  <tr key={k.id} className="hover:bg-neutral-850/40 transition-colors">
                    <td className="py-3 px-4 font-medium text-neutral-200">
                      {k.name}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-400">
                      {k.key_prefix}...
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-300">
                      {env ? env.name : "Default"}
                    </td>
                    <td className="py-3 px-4">
                      <span
                        className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-bold ${
                          isRevoked
                            ? "bg-rose-950/60 border-rose-800 text-rose-300"
                            : "bg-emerald-950/60 border-emerald-800 text-emerald-300"
                        }`}
                      >
                        {isRevoked ? "Revoked" : "Active"}
                      </span>
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-500">
                      {k.last_used_at
                        ? new Date(k.last_used_at).toLocaleDateString()
                        : "Never"}
                    </td>
                    <td className="py-3 px-4 font-mono text-neutral-500">
                      {k.expires_at
                        ? new Date(k.expires_at).toLocaleDateString()
                        : "Never"}
                    </td>
                    <td className="py-3 px-4 text-right">
                      {!isRevoked && (
                        <button
                          onClick={() => handleRevoke(k.id)}
                          className="p-1 rounded text-neutral-500 hover:text-rose-400 hover:bg-neutral-800 transition-colors"
                          title="Revoke key"
                        >
                          <Trash2 size={14} />
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {/* Create Key Modal */}
      {isCreateOpen && (
        <div className="fixed inset-0 z-50 bg-neutral-950/80 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150">
          <div className="w-full max-w-md bg-neutral-900 border border-neutral-800 rounded-xl p-5 space-y-4 shadow-2xl">
            <div className="flex items-center justify-between border-b border-neutral-800 pb-3">
              <h3 className="text-sm font-semibold text-neutral-100">
                Create Ingestion Key
              </h3>
              <button
                onClick={() => setIsCreateOpen(false)}
                className="text-neutral-500 hover:text-neutral-300"
              >
                <X size={16} />
              </button>
            </div>

            <form onSubmit={handleCreate} className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-neutral-300 mb-1">
                  Key Name *
                </label>
                <input
                  type="text"
                  required
                  value={keyName}
                  onChange={(e) => setKeyName(e.target.value)}
                  placeholder="e.g. production-backend"
                  className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-neutral-300 mb-1">
                  Environment *
                </label>
                <select
                  required
                  value={envId}
                  onChange={(e) => setEnvId(e.target.value)}
                  className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500 cursor-pointer"
                >
                  {environments.map((env) => (
                    <option key={env.id} value={env.id}>
                      {env.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-xs font-medium text-neutral-300 mb-1">
                  Expires in (Days, Optional)
                </label>
                <input
                  type="number"
                  min={1}
                  value={expiresInDays}
                  onChange={(e) =>
                    setExpiresInDays(e.target.value ? Number(e.target.value) : "")
                  }
                  placeholder="e.g. 90 (leave empty for never)"
                  className="w-full px-3 py-1.5 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500"
                />
              </div>

              <div className="pt-2 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setIsCreateOpen(false)}
                  className="px-3 py-1.5 rounded-lg border border-neutral-800 text-xs text-neutral-400 hover:bg-neutral-850"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={creating}
                  className="px-4 py-1.5 rounded-lg bg-emerald-500 text-neutral-950 font-semibold text-xs hover:bg-emerald-400 disabled:opacity-50"
                >
                  {creating ? "Creating..." : "Create Key"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
