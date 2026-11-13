"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { Session, Trace } from "@/types";
import {
  AlertCircle,
  Clock,
  ExternalLink,
  MessagesSquare,
  RefreshCw,
  User,
} from "lucide-react";

export default function SessionsPage() {
  const { currentProject } = useApp();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [selectedSession, setSelectedSession] = useState<Session | null>(null);
  const [sessionTraces, setSessionTraces] = useState<Trace[]>([]);
  const [loading, setLoading] = useState(true);
  const [tracesLoading, setTracesLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSelectSession = useCallback(async (sess: Session) => {
    setSelectedSession(sess);
    setTracesLoading(true);

    try {
      const data = await api.sessions.listTraces(sess.id);
      setSessionTraces(data);
    } catch (err) {
      console.error("Failed to load session traces", err);
      setSessionTraces([]);
    } finally {
      setTracesLoading(false);
    }
  }, []);

  const fetchSessions = useCallback(async () => {
    if (!currentProject) return;
    setLoading(true);
    setError(null);

    try {
      const data = await api.sessions.list(currentProject.id);
      setSessions(data);
      if (data.length > 0 && !selectedSession) {
        handleSelectSession(data[0]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load sessions");
    } finally {
      setLoading(false);
    }
  }, [currentProject, selectedSession, handleSelectSession]);

  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  const formatDuration = (val: number | null): string => {
    if (val === null || val === undefined) return "—";
    if (val >= 1000) return `${(val / 1000).toFixed(2)}s`;
    return `${val.toFixed(1)}ms`;
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
            User Sessions
          </h1>
          <p className="text-xs text-neutral-400">
            Multi-turn conversation sessions and aggregated agent interactions.
          </p>
        </div>
        <button
          onClick={fetchSessions}
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

      {/* Two Column Layout: Sessions List on Left, Session Traces on Right */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* Left Column: Sessions List */}
        <div className="lg:col-span-5 border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30 flex flex-col h-[650px]">
          <div className="p-3.5 border-b border-neutral-850 bg-neutral-950/60 text-xs font-semibold text-neutral-300">
            Recorded Sessions ({sessions.length})
          </div>

          <div className="flex-1 overflow-y-auto divide-y divide-neutral-850">
            {loading && sessions.length === 0 ? (
              Array.from({ length: 4 }).map((_, i) => (
                <div key={i} className="p-4 animate-pulse space-y-2">
                  <div className="h-3.5 bg-neutral-850 rounded w-2/3" />
                  <div className="h-2.5 bg-neutral-850 rounded w-1/3" />
                </div>
              ))
            ) : sessions.length === 0 ? (
              <div className="p-8 text-center text-xs text-neutral-500">
                No sessions found for this project.
              </div>
            ) : (
              sessions.map((sess) => {
                const isSelected = selectedSession?.id === sess.id;
                return (
                  <div
                    key={sess.id}
                    onClick={() => handleSelectSession(sess)}
                    className={`p-3.5 cursor-pointer text-xs transition-colors space-y-1.5 ${
                      isSelected
                        ? "bg-neutral-850 border-l-2 border-emerald-500"
                        : "hover:bg-neutral-850/40"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="font-mono font-medium text-neutral-200 truncate pr-2">
                        {sess.external_session_id}
                      </div>
                      <span className="text-[10px] text-neutral-500 font-mono shrink-0">
                        {new Date(sess.last_seen_at).toLocaleDateString()}
                      </span>
                    </div>

                    <div className="flex items-center gap-3 text-[11px] text-neutral-400">
                      {sess.user_ref && (
                        <div className="flex items-center gap-1">
                          <User size={11} className="text-neutral-500" />
                          <span>{sess.user_ref}</span>
                        </div>
                      )}
                      <div className="flex items-center gap-1 text-neutral-500">
                        <Clock size={11} />
                        <span>Started {new Date(sess.started_at).toLocaleTimeString()}</span>
                      </div>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Right Column: Traces inside Selected Session */}
        <div className="lg:col-span-7 border border-neutral-850 rounded-xl overflow-hidden bg-neutral-900/30 flex flex-col h-[650px]">
          <div className="p-3.5 border-b border-neutral-850 bg-neutral-950/60 flex items-center justify-between text-xs">
            <span className="font-semibold text-neutral-300">
              Session Traces ({sessionTraces.length})
            </span>
            {selectedSession && (
              <span className="font-mono text-[11px] text-neutral-500">
                session: {selectedSession.external_session_id}
              </span>
            )}
          </div>

          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {tracesLoading ? (
              <div className="flex h-32 items-center justify-center text-neutral-500 text-xs">
                Loading session traces...
              </div>
            ) : !selectedSession ? (
              <div className="flex h-32 items-center justify-center text-neutral-500 text-xs">
                Select a session from the left to inspect its traces.
              </div>
            ) : sessionTraces.length === 0 ? (
              <div className="flex h-32 items-center justify-center text-neutral-500 text-xs">
                No traces recorded under this session.
              </div>
            ) : (
              sessionTraces.map((trace) => (
                <Link
                  key={trace.id}
                  href={`/traces/${trace.trace_id}`}
                  className="p-3.5 rounded-lg border border-neutral-800 bg-neutral-950/60 hover:bg-neutral-850/60 hover:border-neutral-700 transition-colors flex items-center justify-between text-xs group block"
                >
                  <div className="space-y-1">
                    <div className="font-medium text-neutral-200 group-hover:text-emerald-400 transition-colors">
                      {trace.name}
                    </div>
                    <div className="text-[10px] font-mono text-neutral-500 flex items-center gap-2">
                      <span>{trace.trace_id.slice(0, 16)}...</span>
                      <span>•</span>
                      <span>{trace.span_count} spans</span>
                      <span>•</span>
                      <span>{formatDuration(trace.duration_ms)}</span>
                    </div>
                  </div>

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
                    <ExternalLink size={13} className="text-neutral-500 group-hover:text-neutral-300" />
                  </div>
                </Link>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
