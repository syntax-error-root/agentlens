export type OrgRole = "owner" | "admin" | "member" | "viewer";

export interface User {
  id: string;
  email: string;
  display_name: string | null;
  status: string;
  created_at: string;
}

export interface Organization {
  id: string;
  name: string;
  slug: string;
  created_at: string;
  updated_at: string;
}

export interface Project {
  id: string;
  organization_id: string;
  name: string;
  slug: string;
  description: string | null;
  capture_content_default: boolean;
  retention_days: number;
  created_at: string;
  updated_at: string;
}

export interface Environment {
  id: string;
  project_id: string;
  name: string;
  slug: string;
  description: string | null;
  created_at: string;
}

export interface ApiKey {
  id: string;
  organization_id: string;
  project_id: string;
  environment_id: string;
  name: string;
  key_prefix: string;
  scopes: string[];
  last_used_at: string | null;
  expires_at: string | null;
  revoked_at: string | null;
  created_at: string;
}

export interface Agent {
  id: string;
  organization_id: string;
  project_id: string;
  name: string;
  framework: string | null;
  language: string | null;
  description: string | null;
  metadata: Record<string, unknown>;
  last_seen_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface AgentVersion {
  id: string;
  organization_id: string;
  project_id: string;
  agent_id: string;
  version: string;
  commit_hash: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
}

export type TraceStatus = "unset" | "ok" | "error";

export type SpanType =
  | "agent"
  | "chain"
  | "llm"
  | "tool"
  | "retriever"
  | "router"
  | "guardrail"
  | "evaluator"
  | "custom";

export interface Trace {
  id: string;
  organization_id: string;
  project_id: string;
  environment_id: string;
  agent_id: string | null;
  session_id: string | null;
  trace_id: string;
  root_span_id: string | null;
  name: string;
  status: TraceStatus;
  started_at: string;
  ended_at: string | null;
  duration_ms: number | null;
  span_count: number;
  error_span_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cached_input_tokens: number;
  total_reasoning_tokens: number;
  estimated_cost: string | null;
  has_unpriced_usage: boolean;
  currency: string;
  resource_attributes: Record<string, unknown>;
  attributes: Record<string, unknown>;
  received_at: string;
  finalized_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Span {
  id: string;
  organization_id: string;
  project_id: string;
  trace_id: string;
  span_id: string;
  parent_span_id: string | null;
  agent_id: string | null;
  name: string;
  span_type: SpanType;
  status: TraceStatus;
  started_at: string;
  ended_at: string | null;
  duration_ms: number | null;
  provider: string | null;
  model: string | null;
  tool_name: string | null;
  attributes: Record<string, unknown>;
  resource_attributes: Record<string, unknown>;
  scope_attributes: Record<string, unknown>;
  payload_s3_key: string | null;
  error: Record<string, unknown> | null;
  received_at: string;
  created_at: string;
}

export interface SpanEvent {
  id: string;
  project_id: string;
  trace_id: string;
  span_id: string;
  event_index: number;
  event_name: string;
  occurred_at: string;
  attributes: Record<string, unknown>;
  created_at: string;
}

export interface SpanLink {
  id: string;
  project_id: string;
  trace_id: string;
  span_id: string;
  link_index: number;
  linked_trace_id: string;
  linked_span_id: string;
  attributes: Record<string, unknown>;
  created_at: string;
}

export interface TokenUsage {
  id: string;
  organization_id: string;
  project_id: string;
  trace_id: string;
  span_id: string;
  provider: string;
  model: string;
  input_tokens: number;
  output_tokens: number;
  cached_input_tokens: number;
  reasoning_tokens: number;
  cost_estimate: string | null;
  currency: string;
  pricing_version: string | null;
  input_rate_per_million: string | null;
  output_rate_per_million: string | null;
  cached_rate_per_million: string | null;
  usage_source: "provider_reported" | "estimated";
  recorded_at: string;
  created_at: string;
}

export interface SpanDetail {
  span: Span;
  events: SpanEvent[];
  links: SpanLink[];
  token_usage: TokenUsage | null;
}

export interface Session {
  id: string;
  organization_id: string;
  project_id: string;
  external_session_id: string;
  user_ref: string | null;
  metadata: Record<string, unknown>;
  started_at: string;
  last_seen_at: string;
  created_at: string;
}

export interface ProjectOverviewMetrics {
  active_agents: number;
  total_traces: number;
  error_traces: number;
  error_rate_pct: number;
  total_tokens: number;
  p50_duration_ms: number | null;
  p95_duration_ms: number | null;
  recent_traces: Trace[];
  recent_errors: Trace[];
}

export interface TraceQueryResult {
  items: Trace[];
  next_cursor: string | null;
  has_more: boolean;
}

export interface AnalyticsOverview {
  total_cost: string | null;
  has_unpriced_usage: boolean;
  total_tokens: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
  total_traces: number;
  total_errors: number;
  error_rate: number;
  p50_duration_ms: number | null;
  p95_duration_ms: number | null;
  p99_duration_ms: number | null;
}

export interface TokenUsageTimePoint {
  timestamp: string;
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  reasoning_tokens: number;
  total_tokens: number;
}

export interface TokenAnalyticsResult {
  time_series: TokenUsageTimePoint[];
  total_input_tokens: number;
  total_output_tokens: number;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
  total_tokens: number;
}

export interface CostTimePoint {
  date: string;
  cost: string | null;
  currency: string;
  unpriced_traces_count: number;
}

export interface CostAnalyticsResult {
  time_series: CostTimePoint[];
  total_estimated_cost: string | null;
  currency: string;
  unpriced_usage_count: number;
  has_unpriced_usage: boolean;
}

export interface ModelAnalyticsItem {
  provider: string;
  model: string;
  call_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cached_tokens: number;
  total_reasoning_tokens: number;
  total_tokens: number;
  estimated_cost: string | null;
  currency: string;
  has_unpriced: boolean;
}

export interface AgentAnalyticsItem {
  agent_id: string | null;
  agent_name: string;
  trace_count: number;
  error_count: number;
  error_rate: number;
  total_tokens: number;
  estimated_cost: string | null;
  p50_duration_ms: number | null;
  p95_duration_ms: number | null;
}

export interface ToolAnalyticsItem {
  tool_name: string;
  call_count: number;
  error_count: number;
  error_rate: number;
  p50_duration_ms: number | null;
  p95_duration_ms: number | null;
  p99_duration_ms: number | null;
}

export interface LatencyAnalyticsResult {
  p50_duration_ms: number | null;
  p95_duration_ms: number | null;
  p99_duration_ms: number | null;
  min_duration_ms: number | null;
  max_duration_ms: number | null;
  avg_duration_ms: number | null;
  total_count: number;
}

export interface ErrorTimePoint {
  timestamp: string;
  error_count: number;
  total_count: number;
  error_rate: number;
}

export interface TopErrorItem {
  error_type: string;
  count: number;
  sample_message: string | null;
  last_seen_at: string;
}

export interface ErrorAnalyticsResult {
  total_errors: number;
  total_traces: number;
  error_rate: number;
  time_series: ErrorTimePoint[];
  top_errors: TopErrorItem[];
}

export interface ModelPricing {
  id: string;
  provider: string;
  model_pattern: string;
  match_type: string;
  currency: string;
  input_per_million: string | null;
  output_per_million: string | null;
  cached_input_per_million: string | null;
  effective_from: string;
  effective_until: string | null;
  source_url: string | null;
  source_checked_at: string | null;
  version: string;
  created_at: string;
}
