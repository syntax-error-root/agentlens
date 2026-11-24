"use client";

import { useEffect, useState } from "react";
import { Span, SpanDetail } from "@/types";
import { api } from "@/lib/api";
import { getSpanTypeStyle } from "@/lib/waterfall";
import {
  AlertTriangle,
  Coins,
  Copy,
  ExternalLink,
  Layers,
  ListTree,
  X,
} from "lucide-react";

interface SpanDetailDrawerProps {
  span: Span | null;
  onClose: () => void;
}

export function SpanDetailDrawer({ span, onClose }: SpanDetailDrawerProps) {
  const [detail, setDetail] = useState<SpanDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<"attributes" | "tokens" | "events" | "raw">("attributes");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!span) {
      setDetail(null);
      return;
    }

    let isMounted = true;
    setLoading(true);

    api.spans
      .get(span.id)
      .then((data) => {
        if (isMounted) setDetail(data);
      })
      .catch((err) => {
        console.error("Failed to load span details", err);
        if (isMounted) {
          setDetail({
            span,
            events: [],
            links: [],
            token_usage: null,
          });
        }
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [span]);

  if (!span) return null;

  const typeStyle = getSpanTypeStyle(span.span_type);

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-y-0 right-0 z-50 w-full max-w-xl bg-neutral-900 border-l border-neutral-800 shadow-2xl flex flex-col animate-in slide-in-from-right duration-200">
      {/* Drawer Header */}
      <div className="p-4 border-b border-neutral-800 flex items-start justify-between bg-neutral-950/40">
        <div className="space-y-1.5 min-w-0 pr-4">
          <div className="flex items-center gap-2 flex-wrap">
            <span
              className={`text-[10px] font-mono px-2 py-0.5 rounded border uppercase font-bold tracking-wider ${typeStyle.badgeBg}`}
            >
              {typeStyle.label}
            </span>
            <span
              className={`text-[10px] font-mono px-2 py-0.5 rounded border ${
                span.status === "error"
                  ? "bg-rose-950/60 border-rose-800/80 text-rose-300"
                  : "bg-emerald-950/60 border-emerald-800/80 text-emerald-300"
              }`}
            >
              {span.status.toUpperCase()}
            </span>
            {span.duration_ms !== null && (
              <span className="text-[11px] font-mono text-neutral-400">
                {span.duration_ms >= 1000
                  ? `${(span.duration_ms / 1000).toFixed(2)}s`
                  : `${span.duration_ms.toFixed(1)}ms`}
              </span>
            )}
          </div>
          <h2 className="text-base font-semibold text-neutral-100 truncate" title={span.name}>
            {span.name}
          </h2>
          <div className="flex items-center gap-3 text-[11px] font-mono text-neutral-500">
            <span>span: {span.span_id.slice(0, 8)}...</span>
            {span.parent_span_id && <span>parent: {span.parent_span_id.slice(0, 8)}...</span>}
          </div>
        </div>

        <button
          onClick={onClose}
          className="p-1 rounded-md text-neutral-400 hover:text-neutral-100 hover:bg-neutral-800"
          aria-label="Close drawer"
        >
          <X size={18} />
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-neutral-800 bg-neutral-950/20 px-4">
        <button
          onClick={() => setActiveTab("attributes")}
          className={`py-2 px-3 text-xs font-medium border-b-2 transition-colors ${
            activeTab === "attributes"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          Attributes
        </button>
        <button
          onClick={() => setActiveTab("tokens")}
          className={`py-2 px-3 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 ${
            activeTab === "tokens"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          <Coins size={13} />
          <span>Usage</span>
        </button>
        <button
          onClick={() => setActiveTab("events")}
          className={`py-2 px-3 text-xs font-medium border-b-2 transition-colors flex items-center gap-1.5 ${
            activeTab === "events"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          <ListTree size={13} />
          <span>Events ({detail?.events.length || 0})</span>
        </button>
        <button
          onClick={() => setActiveTab("raw")}
          className={`py-2 px-3 text-xs font-medium border-b-2 transition-colors ${
            activeTab === "raw"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          Raw JSON
        </button>
      </div>

      {/* Drawer Body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Error Callout if error */}
        {span.error && (
          <div className="p-3 rounded-lg bg-rose-950/30 border border-rose-800/60 text-xs text-rose-200 space-y-1">
            <div className="flex items-center gap-1.5 font-semibold text-rose-400">
              <AlertTriangle size={14} />
              <span>Execution Error</span>
            </div>
            <pre className="text-[11px] font-mono whitespace-pre-wrap break-all text-neutral-300">
              {JSON.stringify(span.error, null, 2)}
            </pre>
          </div>
        )}

        {/* Tab 1: Attributes */}
        {activeTab === "attributes" && (
          <div className="space-y-4">
            <div>
              <h3 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider mb-2">
                Span Attributes
              </h3>
              <div className="rounded-lg border border-neutral-800 overflow-hidden divide-y divide-neutral-800 bg-neutral-950/40 text-xs">
                {Object.entries(span.attributes || {}).length === 0 ? (
                  <div className="p-3 text-neutral-500 text-center">No attributes recorded</div>
                ) : (
                  Object.entries(span.attributes).map(([key, value]) => (
                    <div key={key} className="flex p-2.5 hover:bg-neutral-850/50">
                      <span className="w-1/3 font-mono text-neutral-400 break-all pr-2">{key}</span>
                      <span className="w-2/3 font-mono text-neutral-200 break-all">
                        {typeof value === "object" ? JSON.stringify(value) : String(value)}
                      </span>
                    </div>
                  ))
                )}
              </div>
            </div>

            {Object.keys(span.resource_attributes || {}).length > 0 && (
              <div>
                <h3 className="text-xs font-semibold text-neutral-400 uppercase tracking-wider mb-2">
                  Resource Attributes
                </h3>
                <div className="rounded-lg border border-neutral-800 overflow-hidden divide-y divide-neutral-800 bg-neutral-950/40 text-xs">
                  {Object.entries(span.resource_attributes).map(([key, value]) => (
                    <div key={key} className="flex p-2.5 hover:bg-neutral-850/50">
                      <span className="w-1/3 font-mono text-neutral-400 break-all pr-2">{key}</span>
                      <span className="w-2/3 font-mono text-neutral-200 break-all">
                        {typeof value === "object" ? JSON.stringify(value) : String(value)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Tab 2: Tokens & Pricing */}
        {activeTab === "tokens" && (
          <div className="space-y-4">
            {detail?.token_usage ? (
              <div className="space-y-3">
                <div className="grid grid-cols-2 gap-3 text-xs">
                  <div className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40">
                    <span className="text-neutral-500 block text-[10px] uppercase">Input Tokens</span>
                    <span className="text-base font-semibold font-mono text-neutral-100">
                      {detail.token_usage.input_tokens.toLocaleString()}
                    </span>
                  </div>
                  <div className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40">
                    <span className="text-neutral-500 block text-[10px] uppercase">Output Tokens</span>
                    <span className="text-base font-semibold font-mono text-neutral-100">
                      {detail.token_usage.output_tokens.toLocaleString()}
                    </span>
                  </div>
                  <div className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40">
                    <span className="text-neutral-500 block text-[10px] uppercase">Cached Tokens</span>
                    <span className="text-base font-semibold font-mono text-neutral-100">
                      {detail.token_usage.cached_input_tokens.toLocaleString()}
                    </span>
                  </div>
                  <div className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40">
                    <span className="text-neutral-500 block text-[10px] uppercase">Reasoning Tokens</span>
                    <span className="text-base font-semibold font-mono text-neutral-100">
                      {detail.token_usage.reasoning_tokens.toLocaleString()}
                    </span>
                  </div>
                </div>

                <div className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40 space-y-1 text-xs">
                  <div className="flex justify-between">
                    <span className="text-neutral-500">Provider & Model:</span>
                    <span className="font-mono text-neutral-200">
                      {detail.token_usage.provider} / {detail.token_usage.model}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-neutral-500">Estimated Cost:</span>
                    <span className="font-mono text-emerald-400 font-semibold">
                      {detail.token_usage.cost_estimate
                        ? `$${Number(detail.token_usage.cost_estimate).toFixed(6)} ${detail.token_usage.currency}`
                        : "Unpriced"}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-neutral-500">Source:</span>
                    <span className="font-mono text-neutral-400">
                      {detail.token_usage.usage_source}
                    </span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="p-6 text-center text-xs text-neutral-500 border border-neutral-800 rounded-lg">
                No token usage recorded for this span.
              </div>
            )}
          </div>
        )}

        {/* Tab 3: Events */}
        {activeTab === "events" && (
          <div className="space-y-3">
            {!detail?.events || detail.events.length === 0 ? (
              <div className="p-6 text-center text-xs text-neutral-500 border border-neutral-800 rounded-lg">
                No span events recorded.
              </div>
            ) : (
              detail.events.map((ev) => (
                <div
                  key={ev.id}
                  className="p-3 rounded-lg border border-neutral-800 bg-neutral-950/40 text-xs space-y-1"
                >
                  <div className="flex justify-between items-center">
                    <span className="font-semibold text-neutral-200 font-mono">{ev.event_name}</span>
                    <span className="text-[10px] text-neutral-500 font-mono">
                      {new Date(ev.occurred_at).toLocaleTimeString()}
                    </span>
                  </div>
                  {Object.keys(ev.attributes || {}).length > 0 && (
                    <pre className="text-[10px] font-mono text-neutral-400 bg-neutral-900 p-2 rounded overflow-x-auto">
                      {JSON.stringify(ev.attributes, null, 2)}
                    </pre>
                  )}
                </div>
              ))
            )}
          </div>
        )}

        {/* Tab 4: Raw JSON */}
        {activeTab === "raw" && (
          <div className="space-y-2">
            <div className="flex justify-end">
              <button
                onClick={() => copyToClipboard(JSON.stringify(detail?.span || span, null, 2))}
                className="flex items-center gap-1 px-2.5 py-1 text-xs rounded border border-neutral-800 text-neutral-400 hover:text-neutral-200"
              >
                <Copy size={12} />
                <span>{copied ? "Copied" : "Copy JSON"}</span>
              </button>
            </div>
            <pre className="p-3 rounded-lg bg-neutral-950 border border-neutral-800 text-[11px] font-mono text-neutral-300 overflow-x-auto whitespace-pre">
              {JSON.stringify(detail?.span || span, null, 2)}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
