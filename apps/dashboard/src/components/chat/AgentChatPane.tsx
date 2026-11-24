"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import { api } from "@/lib/api";
import { Agent, ChatMessage, ChatSession } from "@/types";
import {
  AlertCircle,
  Bot,
  CornerDownLeft,
  Loader2,
  Plus,
  RefreshCw,
  Send,
  Server,
  Settings,
  Sparkles,
  User,
} from "lucide-react";

interface AgentChatPaneProps {
  agent: Agent;
  onAgentUpdated?: (updated: Agent) => void;
}

export function AgentChatPane({ agent, onAgentUpdated }: AgentChatPaneProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputText, setInputText] = useState("");
  const [streamingContent, setStreamingContent] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Endpoint configuration modal / state
  const [showConfig, setShowConfig] = useState(false);
  const [endpointUrl, setEndpointUrl] = useState(agent.endpoint_url || "");
  const [authToken, setAuthToken] = useState(agent.auth_token || "");
  const [defaultModel, setDefaultModel] = useState(agent.default_model || "gpt-4o-mini");
  const [savingConfig, setSavingConfig] = useState(false);
  const [configSuccess, setConfigSuccess] = useState(false);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages, streamingContent]);

  // Load chat sessions for this agent
  const fetchSessions = useCallback(async () => {
    try {
      const data = await api.chat.listSessions(agent.id);
      setSessions(data);
      if (data.length > 0 && !activeSessionId) {
        setActiveSessionId(data[0].id);
      }
    } catch (err) {
      console.error("Failed loading chat sessions", err);
    }
  }, [agent.id, activeSessionId]);

  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  // Load messages whenever active session changes
  const fetchMessages = useCallback(async (sid: string) => {
    setLoadingHistory(true);
    setError(null);
    try {
      const msgs = await api.chat.listMessages(sid);
      setMessages(msgs);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load messages");
    } finally {
      setLoadingHistory(false);
    }
  }, []);

  useEffect(() => {
    if (activeSessionId) {
      fetchMessages(activeSessionId);
    } else {
      setMessages([]);
    }
  }, [activeSessionId, fetchMessages]);

  const handleNewSession = async () => {
    try {
      const newSess = await api.chat.createSession(agent.id);
      setSessions((prev) => [newSess, ...prev]);
      setActiveSessionId(newSess.id);
      setMessages([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create new session");
    }
  };

  const handleSaveEndpoint = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingConfig(true);
    setError(null);
    setConfigSuccess(false);

    try {
      const updated = await api.agents.update(agent.id, {
        endpoint_url: endpointUrl.trim(),
        auth_token: authToken.trim() || undefined,
        default_model: defaultModel.trim() || undefined,
      });

      if (onAgentUpdated) {
        onAgentUpdated(updated);
      }
      setConfigSuccess(true);
      setTimeout(() => setConfigSuccess(false), 2500);
      setShowConfig(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update endpoint");
    } finally {
      setSavingConfig(false);
    }
  };

  const handleSendMessage = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const prompt = inputText.trim();
    if (!prompt || isStreaming) return;

    if (!agent.endpoint_url) {
      setShowConfig(true);
      setError("Please configure an agent endpoint URL first.");
      return;
    }

    setInputText("");
    setError(null);
    setIsStreaming(true);
    setStreamingContent("");

    // Optimistically append user message to local state
    const optimisticMsg: ChatMessage = {
      id: "temp-user-" + Date.now(),
      organization_id: agent.organization_id,
      project_id: agent.project_id,
      chat_session_id: activeSessionId || "",
      role: "user",
      content: prompt,
      trace_id: null,
      token_count: null,
      created_at: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, optimisticMsg]);

    let accumulatedTokens = "";

    await api.chat.sendStream(
      agent.id,
      {
        content: prompt,
        session_id: activeSessionId || undefined,
        model: agent.default_model || undefined,
      },
      (meta) => {
        if (!activeSessionId) {
          setActiveSessionId(meta.session_id);
          fetchSessions();
        }
      },
      (token) => {
        accumulatedTokens += token;
        setStreamingContent(accumulatedTokens);
      },
      () => {
        setIsStreaming(false);
        if (accumulatedTokens) {
          const assistantMsg: ChatMessage = {
            id: "temp-assistant-" + Date.now(),
            organization_id: agent.organization_id,
            project_id: agent.project_id,
            chat_session_id: activeSessionId || "",
            role: "assistant",
            content: accumulatedTokens,
            trace_id: null,
            token_count: null,
            created_at: new Date().toISOString(),
          };
          setMessages((prev) => [...prev, assistantMsg]);
        }
        setStreamingContent("");
      },
      (err) => {
        setIsStreaming(false);
        setError(err.message);
        setStreamingContent("");
      }
    );
  };

  return (
    <div className="flex h-[680px] border border-neutral-850 rounded-xl bg-neutral-950/60 overflow-hidden shadow-2xl">
      {/* Session Threads Sidebar */}
      <div className="w-64 border-r border-neutral-850 bg-neutral-900/30 flex flex-col justify-between shrink-0">
        <div className="p-3 border-b border-neutral-850 flex items-center justify-between">
          <span className="text-xs font-semibold text-neutral-300">Conversations</span>
          <button
            onClick={handleNewSession}
            className="p-1 rounded-md text-neutral-400 hover:text-neutral-100 hover:bg-neutral-800 transition"
            title="New Chat Session"
          >
            <Plus size={15} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {sessions.length === 0 ? (
            <div className="p-4 text-center text-[11px] text-neutral-500">
              No conversations yet. Start chatting below!
            </div>
          ) : (
            sessions.map((sess) => (
              <button
                key={sess.id}
                onClick={() => setActiveSessionId(sess.id)}
                className={`w-full text-left px-3 py-2 rounded-lg text-xs truncate transition flex items-center justify-between ${
                  activeSessionId === sess.id
                    ? "bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 font-medium"
                    : "text-neutral-400 hover:bg-neutral-850 hover:text-neutral-200"
                }`}
              >
                <span className="truncate">{sess.title}</span>
              </button>
            ))
          )}
        </div>

        {/* Endpoint Config trigger */}
        <div className="p-3 border-t border-neutral-850 bg-neutral-900/50">
          <button
            onClick={() => setShowConfig(true)}
            className="w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 transition border border-neutral-800"
          >
            <div className="flex items-center gap-2 truncate">
              <Server size={14} className={agent.endpoint_url ? "text-emerald-400" : "text-amber-400"} />
              <span className="truncate">{agent.endpoint_url ? "Endpoint Connected" : "Set Endpoint"}</span>
            </div>
            <Settings size={13} />
          </button>
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1 flex flex-col justify-between bg-neutral-950">
        {/* Chat Header */}
        <div className="h-12 border-b border-neutral-850 px-4 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span className="text-xs font-medium text-neutral-200">{agent.name}</span>
            <span className="text-[10px] text-neutral-500 font-mono px-1.5 py-0.5 rounded bg-neutral-900 border border-neutral-800">
              {agent.default_model || "OpenAI-compatible"}
            </span>
          </div>

          <div className="flex items-center gap-2">
            {activeSessionId && (
              <button
                onClick={() => fetchMessages(activeSessionId)}
                className="p-1 rounded text-neutral-400 hover:text-neutral-200 hover:bg-neutral-850 transition"
                title="Refresh messages"
              >
                <RefreshCw size={13} />
              </button>
            )}
          </div>
        </div>

        {/* Message Thread */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {error && (
            <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
              <AlertCircle size={15} className="shrink-0" />
              <span>{error}</span>
            </div>
          )}

          {loadingHistory ? (
            <div className="flex h-32 items-center justify-center text-xs text-neutral-500 gap-2">
              <Loader2 size={16} className="animate-spin text-emerald-500" />
              <span>Loading conversation...</span>
            </div>
          ) : messages.length === 0 && !streamingContent ? (
            <div className="h-full flex flex-col items-center justify-center text-center p-8 space-y-3">
              <div className="w-12 h-12 rounded-2xl bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
                <Sparkles size={22} />
              </div>
              <h3 className="text-sm font-medium text-neutral-200">Start an Interactive Session</h3>
              <p className="text-xs text-neutral-500 max-w-sm">
                Directly prompt <span className="text-neutral-300 font-semibold">{agent.name}</span> through its configured endpoint. Responses stream live and persist to project history.
              </p>
            </div>
          ) : (
            messages.map((m) => (
              <div
                key={m.id}
                className={`flex gap-3 text-xs leading-relaxed ${
                  m.role === "user" ? "justify-end" : "justify-start"
                }`}
              >
                {m.role !== "user" && (
                  <div className="w-6 h-6 rounded-md bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 shrink-0 mt-0.5">
                    <Bot size={13} />
                  </div>
                )}
                <div
                  className={`max-w-[75%] rounded-xl px-3.5 py-2.5 whitespace-pre-wrap ${
                    m.role === "user"
                      ? "bg-emerald-600 text-white rounded-br-none shadow-md"
                      : "bg-neutral-900 border border-neutral-800 text-neutral-200 rounded-bl-none shadow-sm"
                  }`}
                >
                  {m.content}
                </div>
                {m.role === "user" && (
                  <div className="w-6 h-6 rounded-md bg-neutral-800 border border-neutral-700 flex items-center justify-center text-neutral-300 shrink-0 mt-0.5">
                    <User size={13} />
                  </div>
                )}
              </div>
            ))
          )}

          {/* Streaming Assistant Turn */}
          {isStreaming && streamingContent && (
            <div className="flex gap-3 text-xs leading-relaxed justify-start">
              <div className="w-6 h-6 rounded-md bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 shrink-0 mt-0.5">
                <Bot size={13} />
              </div>
              <div className="max-w-[75%] rounded-xl px-3.5 py-2.5 bg-neutral-900 border border-neutral-800 text-neutral-200 rounded-bl-none shadow-sm whitespace-pre-wrap">
                {streamingContent}
                <span className="inline-block w-1.5 h-3 ml-1 bg-emerald-400 animate-pulse align-middle" />
              </div>
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>

        {/* Input Bar */}
        <form onSubmit={handleSendMessage} className="p-3 border-t border-neutral-850 bg-neutral-900/40">
          <div className="relative flex items-center">
            <textarea
              rows={2}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleSendMessage();
                }
              }}
              placeholder={`Message ${agent.name}... (Press Enter to send, Shift+Enter for newline)`}
              disabled={isStreaming}
              className="w-full bg-neutral-950 border border-neutral-800 rounded-xl px-4 py-2.5 text-xs text-neutral-100 placeholder-neutral-500 focus:outline-none focus:border-emerald-500/60 focus:ring-1 focus:ring-emerald-500/40 resize-none pr-12 transition"
            />
            <button
              type="submit"
              disabled={!inputText.trim() || isStreaming}
              className="absolute right-2.5 p-2 rounded-lg bg-emerald-600 text-white hover:bg-emerald-500 disabled:opacity-40 disabled:hover:bg-emerald-600 transition"
            >
              {isStreaming ? <Loader2 size={14} className="animate-spin" /> : <Send size={14} />}
            </button>
          </div>
        </form>
      </div>

      {/* Endpoint Configuration Modal */}
      {showConfig && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4">
          <div className="w-full max-w-md bg-neutral-900 border border-neutral-800 rounded-xl p-5 shadow-2xl space-y-4">
            <div className="flex items-center justify-between border-b border-neutral-800 pb-3">
              <h3 className="text-sm font-semibold text-neutral-100 flex items-center gap-2">
                <Server size={16} className="text-emerald-400" />
                <span>Configure Agent Endpoint</span>
              </h3>
              <button
                onClick={() => setShowConfig(false)}
                className="text-neutral-400 hover:text-neutral-200 text-xs"
              >
                Cancel
              </button>
            </div>

            <form onSubmit={handleSaveEndpoint} className="space-y-3.5 text-xs">
              <div>
                <label className="block text-neutral-300 font-medium mb-1">
                  Endpoint URL (OpenAI-compatible)
                </label>
                <input
                  type="url"
                  required
                  placeholder="https://api.myagent.internal/v1/chat/completions"
                  value={endpointUrl}
                  onChange={(e) => setEndpointUrl(e.target.value)}
                  className="w-full bg-neutral-950 border border-neutral-800 rounded-lg px-3 py-2 text-neutral-100 focus:outline-none focus:border-emerald-500 font-mono text-[11px]"
                />
                <p className="text-[10px] text-neutral-500 mt-1">
                  AgentLens SSRF filters protect private subnets unless loopback override is enabled.
                </p>
              </div>

              <div>
                <label className="block text-neutral-300 font-medium mb-1">
                  Bearer Auth Token (Optional)
                </label>
                <input
                  type="password"
                  placeholder="sk-..."
                  value={authToken}
                  onChange={(e) => setAuthToken(e.target.value)}
                  className="w-full bg-neutral-950 border border-neutral-800 rounded-lg px-3 py-2 text-neutral-100 focus:outline-none focus:border-emerald-500 font-mono text-[11px]"
                />
              </div>

              <div>
                <label className="block text-neutral-300 font-medium mb-1">
                  Default Model Identifier
                </label>
                <input
                  type="text"
                  placeholder="gpt-4o-mini"
                  value={defaultModel}
                  onChange={(e) => setDefaultModel(e.target.value)}
                  className="w-full bg-neutral-950 border border-neutral-800 rounded-lg px-3 py-2 text-neutral-100 focus:outline-none focus:border-emerald-500 font-mono text-[11px]"
                />
              </div>

              {configSuccess && (
                <div className="p-2 rounded bg-emerald-950/40 border border-emerald-800 text-[11px] text-emerald-300">
                  Endpoint settings saved successfully.
                </div>
              )}

              <div className="flex items-center justify-end gap-2 pt-2 border-t border-neutral-800">
                <button
                  type="button"
                  onClick={() => setShowConfig(false)}
                  className="px-3 py-1.5 rounded-lg border border-neutral-800 text-neutral-400 hover:text-neutral-200 transition"
                >
                  Close
                </button>
                <button
                  type="submit"
                  disabled={savingConfig}
                  className="px-4 py-1.5 rounded-lg bg-emerald-600 text-white font-medium hover:bg-emerald-500 disabled:opacity-50 transition flex items-center gap-1.5"
                >
                  {savingConfig && <Loader2 size={12} className="animate-spin" />}
                  <span>Save Configuration</span>
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
