import {
  Agent,
  AgentVersion,
  ApiKey,
  Environment,
  OrgRole,
  Organization,
  Project,
  ProjectOverviewMetrics,
  Session,
  Span,
  SpanDetail,
  Trace,
  TraceQueryResult,
  User,
} from "@/types";

let csrfToken: string | null = null;

export function setCsrfToken(token: string | null) {
  csrfToken = token;
  if (typeof window !== "undefined") {
    if (token) {
      sessionStorage.setItem("agentlens_csrf", token);
    } else {
      sessionStorage.removeItem("agentlens_csrf");
    }
  }
}

export function getCsrfToken(): string | null {
  if (csrfToken) return csrfToken;
  if (typeof window !== "undefined") {
    csrfToken = sessionStorage.getItem("agentlens_csrf");
  }
  return csrfToken;
}

export class ApiError extends Error {
  status: number;
  data: unknown;

  constructor(status: number, message: string, data?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.data = data;
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const headers = new Headers(options.headers || {});
  headers.set("Accept", "application/json");

  const method = (options.method || "GET").toUpperCase();
  if (["POST", "PUT", "PATCH", "DELETE"].includes(method)) {
    const token = getCsrfToken();
    if (token) {
      headers.set("X-CSRF-Token", token);
    }
    if (options.body && typeof options.body === "string" && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }
  }

  const res = await fetch(path, {
    ...options,
    headers,
    credentials: "same-origin",
  });

  if (!res.ok) {
    let errorMsg = `Request failed with status ${res.status}`;
    let errorData = null;
    try {
      errorData = await res.json();
      if (errorData && typeof errorData === "object" && "error" in errorData) {
        errorMsg = String((errorData as { error: unknown }).error);
      }
    } catch {
      // Body not JSON
    }
    throw new ApiError(res.status, errorMsg, errorData);
  }

  if (res.status === 204) {
    return null as unknown as T;
  }

  return res.json() as Promise<T>;
}

export const api = {
  auth: {
    async getBootstrapStatus(): Promise<{ can_bootstrap: boolean }> {
      return request<{ can_bootstrap: boolean }>("/api/v1/auth/bootstrap/status");
    },
    async bootstrap(input: {
      email: string;
      password: string;
      organization_name: string;
      organization_slug: string;
      default_project_name?: string;
    }): Promise<{ user: User; csrf_token: string }> {
      const data = await request<{ user: User; csrf_token: string }>("/api/v1/auth/bootstrap", {
        method: "POST",
        body: JSON.stringify(input),
      });
      if (data.csrf_token) setCsrfToken(data.csrf_token);
      return data;
    },
    async login(input: {
      email: string;
      password: string;
    }): Promise<{ user: User; csrf_token: string }> {
      const data = await request<{ user: User; csrf_token: string }>("/api/v1/auth/login", {
        method: "POST",
        body: JSON.stringify(input),
      });
      if (data.csrf_token) setCsrfToken(data.csrf_token);
      return data;
    },
    async getMe(): Promise<{
      user: User;
      organizations: Array<{ organization: Organization; role: OrgRole }>;
    }> {
      return request<{
        user: User;
        organizations: Array<{ organization: Organization; role: OrgRole }>;
      }>("/api/v1/auth/me");
    },
    async logout(): Promise<void> {
      try {
        await request("/api/v1/auth/logout", { method: "POST" });
      } finally {
        setCsrfToken(null);
      }
    },
  },

  projects: {
    async list(organizationId?: string): Promise<Project[]> {
      const query = organizationId ? `?organization_id=${encodeURIComponent(organizationId)}` : "";
      return request<Project[]>(`/api/v1/projects${query}`);
    },
    async get(projectId: string): Promise<Project> {
      return request<Project>(`/api/v1/projects/${encodeURIComponent(projectId)}`);
    },
    async listEnvironments(projectId: string): Promise<Environment[]> {
      return request<Environment[]>(`/api/v1/projects/${encodeURIComponent(projectId)}/environments`);
    },
    async listApiKeys(projectId: string): Promise<ApiKey[]> {
      return request<ApiKey[]>(`/api/v1/projects/${encodeURIComponent(projectId)}/api-keys`);
    },
    async createApiKey(
      projectId: string,
      input: {
        environment_id: string;
        name: string;
        scopes?: string[];
        expires_in_days?: number;
      }
    ): Promise<{ api_key: ApiKey; secret: string }> {
      return request<{ api_key: ApiKey; secret: string }>(
        `/api/v1/projects/${encodeURIComponent(projectId)}/api-keys`,
        {
          method: "POST",
          body: JSON.stringify(input),
        }
      );
    },
    async revokeApiKey(keyId: string): Promise<void> {
      return request<void>(`/api/v1/api-keys/${encodeURIComponent(keyId)}`, {
        method: "DELETE",
      });
    },
  },

  overview: {
    async get(
      projectId: string,
      params?: {
        environment_id?: string;
        from?: string;
        to?: string;
      }
    ): Promise<ProjectOverviewMetrics> {
      const query = new URLSearchParams();
      if (params?.environment_id) query.set("environment_id", params.environment_id);
      if (params?.from) query.set("from", params.from);
      if (params?.to) query.set("to", params.to);
      const qs = query.toString() ? `?${query.toString()}` : "";
      return request<ProjectOverviewMetrics>(
        `/api/v1/projects/${encodeURIComponent(projectId)}/overview${qs}`
      );
    },
  },

  agents: {
    async list(projectId: string): Promise<Agent[]> {
      return request<Agent[]>(`/api/v1/projects/${encodeURIComponent(projectId)}/agents`);
    },
    async create(
      projectId: string,
      input: {
        name: string;
        framework?: string;
        language?: string;
        description?: string;
        metadata?: Record<string, unknown>;
      }
    ): Promise<Agent> {
      return request<Agent>(`/api/v1/projects/${encodeURIComponent(projectId)}/agents`, {
        method: "POST",
        body: JSON.stringify(input),
      });
    },
    async get(agentId: string): Promise<Agent> {
      return request<Agent>(`/api/v1/agents/${encodeURIComponent(agentId)}`);
    },
    async update(
      agentId: string,
      input: {
        framework?: string;
        language?: string;
        description?: string;
        metadata?: Record<string, unknown>;
      }
    ): Promise<Agent> {
      return request<Agent>(`/api/v1/agents/${encodeURIComponent(agentId)}`, {
        method: "PATCH",
        body: JSON.stringify(input),
      });
    },
    async listVersions(agentId: string): Promise<AgentVersion[]> {
      return request<AgentVersion[]>(`/api/v1/agents/${encodeURIComponent(agentId)}/versions`);
    },
    async listTraces(agentId: string, limit = 50): Promise<Trace[]> {
      return request<Trace[]>(
        `/api/v1/agents/${encodeURIComponent(agentId)}/traces?limit=${limit}`
      );
    },
  },

  traces: {
    async list(params: {
      project_id: string;
      environment_id?: string;
      agent_id?: string;
      status?: string;
      provider?: string;
      model?: string;
      span_type?: string;
      min_duration_ms?: number;
      max_duration_ms?: number;
      from?: string;
      to?: string;
      search_trace_id?: string;
      cursor?: string;
      limit?: number;
    }): Promise<TraceQueryResult> {
      const query = new URLSearchParams();
      query.set("project_id", params.project_id);
      if (params.environment_id) query.set("environment_id", params.environment_id);
      if (params.agent_id) query.set("agent_id", params.agent_id);
      if (params.status) query.set("status", params.status);
      if (params.provider) query.set("provider", params.provider);
      if (params.model) query.set("model", params.model);
      if (params.span_type) query.set("span_type", params.span_type);
      if (params.min_duration_ms !== undefined)
        query.set("min_duration_ms", params.min_duration_ms.toString());
      if (params.max_duration_ms !== undefined)
        query.set("max_duration_ms", params.max_duration_ms.toString());
      if (params.from) query.set("from", params.from);
      if (params.to) query.set("to", params.to);
      if (params.search_trace_id) query.set("search_trace_id", params.search_trace_id);
      if (params.cursor) query.set("cursor", params.cursor);
      if (params.limit) query.set("limit", params.limit.toString());

      return request<TraceQueryResult>(`/api/v1/traces?${query.toString()}`);
    },
    async get(traceRecordId: string): Promise<Trace> {
      return request<Trace>(`/api/v1/traces/${encodeURIComponent(traceRecordId)}`);
    },
    async getSpans(traceRecordId: string): Promise<Span[]> {
      return request<Span[]>(`/api/v1/traces/${encodeURIComponent(traceRecordId)}/spans`);
    },
  },

  spans: {
    async get(spanRecordId: string): Promise<SpanDetail> {
      return request<SpanDetail>(`/api/v1/spans/${encodeURIComponent(spanRecordId)}`);
    },
  },

  sessions: {
    async list(projectId: string, limit = 50): Promise<Session[]> {
      return request<Session[]>(
        `/api/v1/sessions?project_id=${encodeURIComponent(projectId)}&limit=${limit}`
      );
    },
    async get(sessionId: string): Promise<Session> {
      return request<Session>(`/api/v1/sessions/${encodeURIComponent(sessionId)}`);
    },
    async listTraces(sessionId: string, limit = 50): Promise<Trace[]> {
      return request<Trace[]>(
        `/api/v1/sessions/${encodeURIComponent(sessionId)}/traces?limit=${limit}`
      );
    },
  },
};
