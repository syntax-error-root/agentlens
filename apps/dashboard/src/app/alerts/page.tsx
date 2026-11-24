"use client";

import { useEffect, useState, useCallback } from "react";
import { useApp } from "@/context/AppContext";
import { api } from "@/lib/api";
import { AlertEvent, AlertRule, NotificationChannel } from "@/types";
import {
  AlertCircle,
  AlertTriangle,
  Bell,
  CheckCircle,
  Eye,
  Plus,
  RefreshCw,
  Send,
  ShieldAlert,
  Trash2,
  X,
} from "lucide-react";

export default function AlertsPage() {
  const { currentProject } = useApp();

  const [activeTab, setActiveTab] = useState<"events" | "rules" | "channels">("events");
  const [events, setEvents] = useState<AlertEvent[]>([]);
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [channels, setChannels] = useState<NotificationChannel[]>([]);

  const [eventFilter, setEventFilter] = useState<string>("all");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Modals
  const [showCreateRule, setShowCreateRule] = useState(false);
  const [showCreateChannel, setShowCreateChannel] = useState(false);

  // Testing feedback
  const [testingChannelId, setTestingChannelId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{ id: string; success: boolean; msg: string } | null>(null);

  // Form states - Create Rule
  const [ruleName, setRuleName] = useState("");
  const [ruleDesc, setRuleDesc] = useState("");
  const [ruleMetric, setRuleMetric] = useState("agent_error_rate");
  const [ruleOperator, setRuleOperator] = useState("gt");
  const [ruleThreshold, setRuleThreshold] = useState("0.05");
  const [ruleWindow, setRuleWindow] = useState("300");
  const [ruleMinSamples, setRuleMinSamples] = useState("5");
  const [ruleCooldown, setRuleCooldown] = useState("300");
  const [ruleSeverity, setRuleSeverity] = useState("warning");
  const [submittingRule, setSubmittingRule] = useState(false);

  // Form states - Create Channel
  const [channelName, setChannelName] = useState("");
  const [channelType, setChannelType] = useState<"webhook" | "slack" | "discord" | "telegram">("slack");
  const [webhookUrl, setWebhookUrl] = useState("");
  const [signingSecret, setSigningSecret] = useState("");
  const [telegramBotToken, setTelegramBotToken] = useState("");
  const [telegramChatId, setTelegramChatId] = useState("");
  const [submittingChannel, setSubmittingChannel] = useState(false);

  const loadData = useCallback(async () => {
    if (!currentProject) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const [eventsData, rulesData, channelsData] = await Promise.all([
        api.alerts.listEvents(currentProject.id, eventFilter === "all" ? undefined : eventFilter),
        api.alerts.listRules(currentProject.id),
        api.notificationChannels.list(currentProject.id),
      ]);

      setEvents(eventsData);
      setRules(rulesData);
      setChannels(channelsData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load alerts data");
    } finally {
      setLoading(false);
    }
  }, [currentProject, eventFilter]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleAcknowledge = async (eventId: string) => {
    try {
      await api.alerts.acknowledgeEvent(eventId);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to acknowledge event");
    }
  };

  const handleDeleteRule = async (ruleId: string) => {
    if (!confirm("Are you sure you want to delete this alert rule?")) return;
    try {
      await api.alerts.deleteRule(ruleId);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to delete rule");
    }
  };

  const handleDeleteChannel = async (channelId: string) => {
    if (!confirm("Are you sure you want to delete this notification channel?")) return;
    try {
      await api.notificationChannels.delete(channelId);
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to delete channel");
    }
  };

  const handleTestChannel = async (channelId: string) => {
    setTestingChannelId(channelId);
    setTestResult(null);
    try {
      const res = await api.notificationChannels.test(channelId);
      setTestResult({
        id: channelId,
        success: res.success,
        msg: res.message || res.error || (res.success ? "Test delivered successfully" : "Test failed"),
      });
    } catch (err) {
      setTestResult({
        id: channelId,
        success: false,
        msg: err instanceof Error ? err.message : "Test request failed",
      });
    } finally {
      setTestingChannelId(null);
    }
  };

  const handleCreateRuleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!currentProject) return;

    setSubmittingRule(true);
    try {
      await api.alerts.createRule(currentProject.id, {
        name: ruleName,
        description: ruleDesc || undefined,
        metric: ruleMetric,
        operator: ruleOperator,
        threshold: parseFloat(ruleThreshold),
        window_seconds: parseInt(ruleWindow, 10),
        min_sample_count: parseInt(ruleMinSamples, 10),
        cooldown_seconds: parseInt(ruleCooldown, 10),
        severity: ruleSeverity,
      });

      setShowCreateRule(false);
      setRuleName("");
      setRuleDesc("");
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to create alert rule");
    } finally {
      setSubmittingRule(false);
    }
  };

  const handleCreateChannelSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!currentProject) return;

    setSubmittingChannel(true);
    try {
      let config: Record<string, unknown> = {};
      if (channelType === "webhook") {
        config = { url: webhookUrl, secret: signingSecret || undefined };
      } else if (channelType === "slack" || channelType === "discord") {
        config = { webhook_url: webhookUrl };
      } else if (channelType === "telegram") {
        config = { bot_token: telegramBotToken, chat_id: telegramChatId };
      }

      await api.notificationChannels.create(currentProject.id, {
        name: channelName,
        channel_type: channelType,
        config,
      });

      setShowCreateChannel(false);
      setChannelName("");
      setWebhookUrl("");
      setSigningSecret("");
      setTelegramBotToken("");
      setTelegramChatId("");
      await loadData();
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to create notification channel");
    } finally {
      setSubmittingChannel(false);
    }
  };

  if (!currentProject) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-neutral-400 gap-2">
        <ShieldAlert size={28} className="text-neutral-500" />
        <span className="text-sm font-medium">Please select a project to manage alerts</span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-neutral-100 flex items-center gap-2">
            <ShieldAlert size={20} className="text-rose-500" />
            Alerts & Notifications
          </h1>
          <p className="text-xs text-neutral-400">
            Real-time incident state machine, threshold rules, and multichannel notifications
          </p>
        </div>

        <div className="flex items-center gap-2">
          {activeTab === "rules" && (
            <button
              onClick={() => setShowCreateRule(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs transition-colors shadow-sm"
            >
              <Plus size={14} />
              <span>New Alert Rule</span>
            </button>
          )}

          {activeTab === "channels" && (
            <button
              onClick={() => setShowCreateChannel(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium text-xs transition-colors shadow-sm"
            >
              <Plus size={14} />
              <span>Add Channel</span>
            </button>
          )}

          <button
            onClick={loadData}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-neutral-800 bg-neutral-900 text-xs text-neutral-300 hover:text-neutral-100 hover:bg-neutral-850 transition-colors disabled:opacity-50"
          >
            <RefreshCw size={13} className={loading ? "animate-spin" : ""} />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg border border-red-500/30 bg-red-500/10 text-red-400 text-xs flex items-center gap-2">
          <AlertCircle size={15} />
          <span>{error}</span>
        </div>
      )}

      {/* Tabs */}
      <div className="border-b border-neutral-850 flex gap-4 text-xs font-medium">
        <button
          onClick={() => setActiveTab("events")}
          className={`pb-2.5 transition-colors border-b-2 flex items-center gap-1.5 ${
            activeTab === "events"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          <ShieldAlert size={14} />
          <span>Alert Events ({events.length})</span>
        </button>

        <button
          onClick={() => setActiveTab("rules")}
          className={`pb-2.5 transition-colors border-b-2 flex items-center gap-1.5 ${
            activeTab === "rules"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          <AlertTriangle size={14} />
          <span>Alert Rules ({rules.length})</span>
        </button>

        <button
          onClick={() => setActiveTab("channels")}
          className={`pb-2.5 transition-colors border-b-2 flex items-center gap-1.5 ${
            activeTab === "channels"
              ? "border-emerald-500 text-emerald-400 font-semibold"
              : "border-transparent text-neutral-400 hover:text-neutral-200"
          }`}
        >
          <Bell size={14} />
          <span>Notification Channels ({channels.length})</span>
        </button>
      </div>

      {/* TAB 1: Alert Events */}
      {activeTab === "events" && (
        <div className="space-y-4">
          <div className="flex items-center gap-1.5 bg-neutral-900/60 p-1 rounded-lg border border-neutral-850 w-fit text-[11px] font-mono">
            {["all", "firing", "acknowledged", "resolved"].map((st) => (
              <button
                key={st}
                onClick={() => setEventFilter(st)}
                className={`px-2.5 py-1 rounded capitalize transition-colors ${
                  eventFilter === st
                    ? "bg-neutral-800 text-neutral-100 font-semibold shadow-sm"
                    : "text-neutral-400 hover:text-neutral-200"
                }`}
              >
                {st}
              </button>
            ))}
          </div>

          {events.length === 0 ? (
            <div className="p-8 text-center border border-dashed border-neutral-850 rounded-xl text-neutral-500 text-xs font-mono">
              No alert events found for this filter.
            </div>
          ) : (
            <div className="divide-y divide-neutral-850 border border-neutral-850 rounded-xl bg-neutral-900/30 overflow-hidden">
              {events.map((ev) => (
                <div key={ev.id} className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 hover:bg-neutral-900/60 transition-colors">
                  <div className="space-y-1.5">
                    <div className="flex items-center gap-2">
                      <span
                        className={`px-2 py-0.5 text-[10px] font-mono rounded font-bold uppercase ${
                          ev.status === "firing"
                            ? "bg-rose-500/20 text-rose-400 border border-rose-500/40 animate-pulse"
                            : ev.status === "acknowledged"
                            ? "bg-amber-500/20 text-amber-400 border border-amber-500/40"
                            : "bg-emerald-500/20 text-emerald-400 border border-emerald-500/40"
                        }`}
                      >
                        {ev.status}
                      </span>

                      <span
                        className={`text-[10px] font-mono px-1.5 py-0.5 rounded uppercase font-semibold ${
                          ev.severity === "critical"
                            ? "bg-red-950 text-red-300 border border-red-800"
                            : ev.severity === "warning"
                            ? "bg-amber-950 text-amber-300 border border-amber-800"
                            : "bg-blue-950 text-blue-300 border border-blue-800"
                        }`}
                      >
                        {ev.severity}
                      </span>

                      <span className="text-[11px] font-mono text-neutral-500">
                        {new Date(ev.last_triggered_at).toLocaleString()}
                      </span>
                    </div>

                    <div className="text-sm font-semibold text-neutral-200">{ev.summary}</div>

                    <div className="text-xs text-neutral-400 font-mono">
                      Fingerprint: <span className="text-neutral-500">{ev.fingerprint.slice(0, 16)}...</span>
                      {ev.acknowledged_at && (
                        <span className="ml-3 text-neutral-500">
                          Acknowledged: {new Date(ev.acknowledged_at).toLocaleTimeString()}
                        </span>
                      )}
                      {ev.resolved_at && (
                        <span className="ml-3 text-emerald-500/80">
                          Resolved: {new Date(ev.resolved_at).toLocaleTimeString()}
                        </span>
                      )}
                    </div>
                  </div>

                  {ev.status === "firing" && (
                    <button
                      onClick={() => handleAcknowledge(ev.id)}
                      className="px-3 py-1.5 rounded-lg border border-amber-500/40 bg-amber-500/10 text-amber-300 hover:bg-amber-500/20 text-xs font-medium transition-colors flex items-center gap-1.5 shrink-0"
                    >
                      <Eye size={13} />
                      <span>Acknowledge</span>
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* TAB 2: Alert Rules */}
      {activeTab === "rules" && (
        <div className="space-y-4">
          {rules.length === 0 ? (
            <div className="p-8 text-center border border-dashed border-neutral-850 rounded-xl text-neutral-500 text-xs font-mono">
              No alert rules configured yet. Click &ldquo;New Alert Rule&rdquo; above to create one.
            </div>
          ) : (
            <div className="overflow-x-auto border border-neutral-850 rounded-xl bg-neutral-900/30">
              <table className="w-full text-left text-xs text-neutral-300 font-mono">
                <thead className="bg-neutral-950/70 border-b border-neutral-850 text-[11px] text-neutral-400 uppercase">
                  <tr>
                    <th className="px-4 py-3 font-semibold">Rule Name</th>
                    <th className="px-4 py-3 font-semibold">Condition</th>
                    <th className="px-4 py-3 font-semibold">Window</th>
                    <th className="px-4 py-3 font-semibold">Min Samples</th>
                    <th className="px-4 py-3 font-semibold">Cooldown</th>
                    <th className="px-4 py-3 font-semibold">Severity</th>
                    <th className="px-4 py-3 font-semibold text-right">Actions</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-neutral-850/60">
                  {rules.map((r) => (
                    <tr key={r.id} className="hover:bg-neutral-850/30 transition-colors">
                      <td className="px-4 py-3 font-sans font-medium text-neutral-100">
                        {r.name}
                        {r.description && (
                          <div className="text-[11px] text-neutral-500 font-normal">{r.description}</div>
                        )}
                      </td>
                      <td className="px-4 py-3 text-neutral-200">
                        <span className="text-emerald-400 font-semibold">{r.metric}</span>{" "}
                        <span className="text-neutral-400">{r.operator}</span> {r.threshold}
                      </td>
                      <td className="px-4 py-3 text-neutral-400">{r.window_seconds}s</td>
                      <td className="px-4 py-3 text-neutral-400">{r.min_sample_count}</td>
                      <td className="px-4 py-3 text-neutral-400">{r.cooldown_seconds}s</td>
                      <td className="px-4 py-3">
                        <span
                          className={`text-[10px] px-2 py-0.5 rounded font-bold uppercase ${
                            r.severity === "critical"
                              ? "bg-rose-500/10 text-rose-400 border border-rose-500/30"
                              : r.severity === "warning"
                              ? "bg-amber-500/10 text-amber-400 border border-amber-500/30"
                              : "bg-blue-500/10 text-blue-400 border border-blue-500/30"
                          }`}
                        >
                          {r.severity}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <button
                          onClick={() => handleDeleteRule(r.id)}
                          className="p-1.5 text-neutral-500 hover:text-red-400 transition-colors"
                          title="Delete rule"
                        >
                          <Trash2 size={14} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* TAB 3: Notification Channels */}
      {activeTab === "channels" && (
        <div className="space-y-4">
          {channels.length === 0 ? (
            <div className="p-8 text-center border border-dashed border-neutral-850 rounded-xl text-neutral-500 text-xs font-mono">
              No notification channels configured yet. Click &ldquo;Add Channel&rdquo; above.
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {channels.map((ch) => (
                <div key={ch.id} className="p-4 rounded-xl border border-neutral-850 bg-neutral-900/40 space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="p-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-300">
                        <Bell size={16} />
                      </span>
                      <div>
                        <div className="text-sm font-semibold text-neutral-100">{ch.name}</div>
                        <span className="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-neutral-800 text-neutral-400">
                          {ch.channel_type}
                        </span>
                      </div>
                    </div>

                    <button
                      onClick={() => handleDeleteChannel(ch.id)}
                      className="p-1.5 text-neutral-500 hover:text-red-400 transition-colors"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>

                  <div className="p-2.5 rounded-lg bg-neutral-950 border border-neutral-850 font-mono text-[11px] text-neutral-400 break-all">
                    {ch.channel_type === "telegram" ? (
                      <div>Chat ID: {String(ch.config?.chat_id || "******")}</div>
                    ) : (
                      <div>Target: {String(ch.config?.webhook_url || ch.config?.url || "https://...****")}</div>
                    )}
                  </div>

                  {testResult && testResult.id === ch.id && (
                    <div
                      className={`p-2 rounded text-[11px] font-mono flex items-center gap-1.5 ${
                        testResult.success
                          ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/30"
                          : "bg-red-500/10 text-red-400 border border-red-500/30"
                      }`}
                    >
                      {testResult.success ? <CheckCircle size={13} /> : <AlertCircle size={13} />}
                      <span className="truncate">{testResult.msg}</span>
                    </div>
                  )}

                  <div className="pt-1 flex items-center justify-between border-t border-neutral-850/60">
                    <span className="text-[10px] font-mono text-neutral-500">SSRF Guard Protected</span>
                    <button
                      onClick={() => handleTestChannel(ch.id)}
                      disabled={testingChannelId === ch.id}
                      className="flex items-center gap-1 px-2.5 py-1 rounded bg-neutral-850 hover:bg-neutral-800 text-neutral-200 text-xs font-medium transition-colors disabled:opacity-50"
                    >
                      <Send size={12} className={testingChannelId === ch.id ? "animate-spin" : ""} />
                      <span>{testingChannelId === ch.id ? "Testing..." : "Send Test"}</span>
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* CREATE ALERT RULE MODAL */}
      {showCreateRule && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
          <div className="w-full max-w-lg rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-2xl space-y-4">
            <div className="flex items-center justify-between border-b border-neutral-800 pb-3">
              <h2 className="text-base font-bold text-neutral-100 flex items-center gap-2">
                <AlertTriangle size={18} className="text-amber-400" />
                Create Alert Rule
              </h2>
              <button
                onClick={() => setShowCreateRule(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X size={18} />
              </button>
            </div>

            <form onSubmit={handleCreateRuleSubmit} className="space-y-4 text-xs font-mono">
              <div>
                <label className="block text-neutral-400 mb-1">Rule Name</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. High Agent Error Rate"
                  value={ruleName}
                  onChange={(e) => setRuleName(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500 font-sans"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-400 mb-1">Metric</label>
                  <select
                    value={ruleMetric}
                    onChange={(e) => setRuleMetric(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  >
                    <option value="agent_error_rate">agent_error_rate</option>
                    <option value="agent_p95_latency_ms">agent_p95_latency_ms</option>
                    <option value="agent_inactivity_seconds">agent_inactivity_seconds</option>
                    <option value="llm_error_rate">llm_error_rate</option>
                    <option value="tool_failure_rate">tool_failure_rate</option>
                    <option value="token_usage_total">token_usage_total</option>
                    <option value="estimated_cost_usd">estimated_cost_usd</option>
                  </select>
                </div>

                <div>
                  <label className="block text-neutral-400 mb-1">Operator</label>
                  <select
                    value={ruleOperator}
                    onChange={(e) => setRuleOperator(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  >
                    <option value="gt">&gt; (greater than)</option>
                    <option value="gte">&gt;= (greater than or equal)</option>
                    <option value="lt">&lt; (less than)</option>
                    <option value="lte">&lt;= (less than or equal)</option>
                    <option value="eq">== (equal)</option>
                  </select>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="block text-neutral-400 mb-1">Threshold</label>
                  <input
                    type="number"
                    step="any"
                    required
                    value={ruleThreshold}
                    onChange={(e) => setRuleThreshold(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
                <div>
                  <label className="block text-neutral-400 mb-1">Window (sec)</label>
                  <select
                    value={ruleWindow}
                    onChange={(e) => setRuleWindow(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  >
                    <option value="60">60s (1 min)</option>
                    <option value="300">300s (5 min)</option>
                    <option value="900">900s (15 min)</option>
                    <option value="3600">3600s (1 hr)</option>
                  </select>
                </div>
                <div>
                  <label className="block text-neutral-400 mb-1">Min Samples</label>
                  <input
                    type="number"
                    min="1"
                    required
                    value={ruleMinSamples}
                    onChange={(e) => setRuleMinSamples(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-neutral-400 mb-1">Cooldown (sec)</label>
                  <input
                    type="number"
                    min="0"
                    required
                    value={ruleCooldown}
                    onChange={(e) => setRuleCooldown(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
                <div>
                  <label className="block text-neutral-400 mb-1">Severity</label>
                  <select
                    value={ruleSeverity}
                    onChange={(e) => setRuleSeverity(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  >
                    <option value="info">Info</option>
                    <option value="warning">Warning</option>
                    <option value="critical">Critical</option>
                  </select>
                </div>
              </div>

              <div className="flex justify-end gap-2 pt-2 border-t border-neutral-800 font-sans">
                <button
                  type="button"
                  onClick={() => setShowCreateRule(false)}
                  className="px-3 py-1.5 rounded-lg border border-neutral-800 hover:bg-neutral-850 text-neutral-300"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={submittingRule}
                  className="px-4 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium disabled:opacity-50"
                >
                  {submittingRule ? "Creating..." : "Create Rule"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* CREATE NOTIFICATION CHANNEL MODAL */}
      {showCreateChannel && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
          <div className="w-full max-w-lg rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-2xl space-y-4">
            <div className="flex items-center justify-between border-b border-neutral-800 pb-3">
              <h2 className="text-base font-bold text-neutral-100 flex items-center gap-2">
                <Bell size={18} className="text-emerald-400" />
                Add Notification Channel
              </h2>
              <button
                onClick={() => setShowCreateChannel(false)}
                className="text-neutral-400 hover:text-neutral-200"
              >
                <X size={18} />
              </button>
            </div>

            <form onSubmit={handleCreateChannelSubmit} className="space-y-4 text-xs font-mono">
              <div>
                <label className="block text-neutral-400 mb-1">Channel Name</label>
                <input
                  type="text"
                  required
                  placeholder="e.g. SRE Slack Alerts"
                  value={channelName}
                  onChange={(e) => setChannelName(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500 font-sans"
                />
              </div>

              <div>
                <label className="block text-neutral-400 mb-1">Channel Type</label>
                <select
                  value={channelType}
                  onChange={(e) => setChannelType(e.target.value as any)}
                  className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500 uppercase"
                >
                  <option value="slack">Slack Incoming Webhook</option>
                  <option value="discord">Discord Webhook</option>
                  <option value="telegram">Telegram Bot</option>
                  <option value="webhook">Generic HTTPS Webhook</option>
                </select>
              </div>

              {(channelType === "slack" || channelType === "discord" || channelType === "webhook") && (
                <div>
                  <label className="block text-neutral-400 mb-1">Webhook URL</label>
                  <input
                    type="url"
                    required
                    placeholder="https://hooks.slack.com/services/..."
                    value={webhookUrl}
                    onChange={(e) => setWebhookUrl(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
              )}

              {channelType === "webhook" && (
                <div>
                  <label className="block text-neutral-400 mb-1">Signing Secret (Optional HMAC-SHA256)</label>
                  <input
                    type="password"
                    placeholder="Shared secret for X-AgentLens-Signature"
                    value={signingSecret}
                    onChange={(e) => setSigningSecret(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                  />
                </div>
              )}

              {channelType === "telegram" && (
                <>
                  <div>
                    <label className="block text-neutral-400 mb-1">Telegram Bot Token</label>
                    <input
                      type="password"
                      required
                      placeholder="123456789:ABCdefGhIJKlmNoPQRsTUVwxyZ"
                      value={telegramBotToken}
                      onChange={(e) => setTelegramBotToken(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                    />
                  </div>
                  <div>
                    <label className="block text-neutral-400 mb-1">Chat ID</label>
                    <input
                      type="text"
                      required
                      placeholder="-1001234567890"
                      value={telegramChatId}
                      onChange={(e) => setTelegramChatId(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg bg-neutral-950 border border-neutral-800 text-neutral-100 focus:outline-none focus:border-emerald-500"
                    />
                  </div>
                </>
              )}

              <div className="p-2.5 rounded-lg bg-neutral-950 border border-neutral-800 text-[11px] text-neutral-400 space-y-1">
                <span className="font-semibold text-neutral-300">Security Guarantee:</span>
                <div>All channel credentials are encrypted with AES-256-GCM before storage. Target destinations are rigorously validated at connect-time against SSRF attacks.</div>
              </div>

              <div className="flex justify-end gap-2 pt-2 border-t border-neutral-800 font-sans">
                <button
                  type="button"
                  onClick={() => setShowCreateChannel(false)}
                  className="px-3 py-1.5 rounded-lg border border-neutral-800 hover:bg-neutral-850 text-neutral-300"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={submittingChannel}
                  className="px-4 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium disabled:opacity-50"
                >
                  {submittingChannel ? "Saving..." : "Save Channel"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
