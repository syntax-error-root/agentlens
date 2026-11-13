/**
 * AgentLens TypeScript SDK
 * Monitor. Trace. Understand AI.
 */

import {
  Context,
  Span,
  SpanStatusCode,
  Tracer,
  context,
  trace,
} from "@opentelemetry/api";
import {
  BasicTracerProvider,
  BatchSpanProcessor,
  ReadableSpan,
  Span as SdkSpan,
  SpanExporter,
  SpanProcessor,
} from "@opentelemetry/sdk-trace-base";
import { resourceFromAttributes } from "@opentelemetry/resources";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-proto";

export interface AgentLensConfig {
  apiKey?: string;
  endpoint?: string;
  environment?: string;
  serviceName?: string;
  agentName?: string;
  agentVersion?: string;
  captureContent?: boolean;
  redactedKeys?: string[];
  maxQueueSize?: number;
  exportTimeoutMs?: number;
  exporter?: SpanExporter;
}

export const CONTENT_KEYS: ReadonlySet<string> = new Set([
  "gen_ai.prompt",
  "gen_ai.completion",
  "gen_ai.input.messages",
  "gen_ai.output.messages",
  "llm.prompts",
  "llm.completions",
  "agentlens.tool.input",
  "agentlens.tool.output",
]);

export const SENSITIVE_KEY_SUBSTRINGS: readonly string[] = [
  "password",
  "secret",
  "api_key",
  "token",
  "auth",
];

const BEARER_PATTERN = /Bearer\s+[a-zA-Z0-9_\-\.]+/gi;
const EMAIL_PATTERN = /[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+/g;
const API_KEY_PATTERN = /agl_live_[a-zA-Z0-9]{16,}/g;

export class AgentLensSpanProcessor implements SpanProcessor {
  private readonly batchProcessor: BatchSpanProcessor;
  private droppedCount: number = 0;
  private readonly agentName?: string;
  private readonly agentVersion?: string;
  private readonly environment: string;
  private readonly captureContent: boolean;
  private readonly redactedKeys: Set<string>;
  private readonly maxQueueSize: number;

  constructor(
    exporter: SpanExporter,
    options: {
      agentName?: string;
      agentVersion?: string;
      environment?: string;
      captureContent?: boolean;
      redactedKeys?: string[];
      maxQueueSize?: number;
      exportTimeoutMillis?: number;
    } = {}
  ) {
    this.agentName = options.agentName;
    this.agentVersion = options.agentVersion;
    this.environment = options.environment || "development";
    this.captureContent = options.captureContent ?? false;
    this.redactedKeys = new Set((options.redactedKeys || []).map((k) => k.toLowerCase()));
    this.maxQueueSize = options.maxQueueSize ?? 2048;

    this.batchProcessor = new BatchSpanProcessor(exporter, {
      maxQueueSize: this.maxQueueSize,
      exportTimeoutMillis: options.exportTimeoutMillis ?? 5000,
    });
  }

  public get droppedSpanCount(): number {
    const internalDropped = (this.batchProcessor as any)._droppedSpansCount || 0;
    return this.droppedCount + internalDropped;
  }

  public onStart(span: SdkSpan, parentContext: Context): void {
    try {
      if (this.agentName) {
        span.setAttribute("agent.name", this.agentName);
      }
      if (this.agentVersion) {
        span.setAttribute("agent.version", this.agentVersion);
      }
      if (this.environment) {
        span.setAttribute("deployment.environment", this.environment);
      }
    } catch {
      // Fail-open
    }
    this.batchProcessor.onStart(span, parentContext);
  }

  public onEnd(span: ReadableSpan): void {
    try {
      this.sanitizeSpanAttributes(span);
    } catch {
      // Fail-open
    }

    try {
      const queue = (this.batchProcessor as any)._finishedSpans;
      if (Array.isArray(queue) && queue.length >= this.maxQueueSize) {
        this.droppedCount++;
      }
      this.batchProcessor.onEnd(span);
    } catch {
      this.droppedCount++;
    }
  }

  private sanitizeSpanAttributes(span: ReadableSpan): void {
    const attrs = span.attributes;
    if (!attrs) return;

    for (const key of Object.keys(attrs)) {
      const keyLower = key.toLowerCase();

      // 1. Strip raw content when captureContent is false
      if (!this.captureContent && CONTENT_KEYS.has(key)) {
        delete attrs[key];
        continue;
      }

      // 2. Redact sensitive keys and secrets
      if (
        this.redactedKeys.has(keyLower) ||
        SENSITIVE_KEY_SUBSTRINGS.some((sub) => keyLower.includes(sub))
      ) {
        attrs[key] = "[REDACTED]";
      } else if (typeof attrs[key] === "string") {
        let val = attrs[key] as string;
        val = val.replace(BEARER_PATTERN, "Bearer [REDACTED]");
        val = val.replace(EMAIL_PATTERN, "[REDACTED_EMAIL]");
        val = val.replace(API_KEY_PATTERN, "[REDACTED_KEY]");
        attrs[key] = val;
      }
    }
  }

  public async forceFlush(): Promise<void> {
    try {
      await this.batchProcessor.forceFlush();
    } catch {
      // Fail-open
    }
  }

  public async shutdown(): Promise<void> {
    try {
      await this.batchProcessor.shutdown();
    } catch {
      // Fail-open
    }
  }
}

export class AgentLens {
  private static instance: AgentLens | null = null;
  private readonly config: Required<
    Omit<AgentLensConfig, "agentName" | "agentVersion" | "exporter">
  > & {
    agentName?: string;
    agentVersion?: string;
  };
  private readonly provider: BasicTracerProvider;
  private readonly processor: AgentLensSpanProcessor;
  private readonly tracer: Tracer;

  constructor(config: Partial<AgentLensConfig> = {}) {
    const env = typeof process !== "undefined" && process.env ? process.env : {};

    const rawEndpoint = (
      config.endpoint ||
      env.AGENTLENS_OTLP_ENDPOINT ||
      env.AGENTLENS_ENDPOINT ||
      "http://localhost:4318"
    ).replace(/\/+$/, "");

    this.config = {
      apiKey: config.apiKey ?? env.AGENTLENS_API_KEY ?? "",
      endpoint: rawEndpoint,
      environment: config.environment ?? env.APP_ENV ?? "development",
      serviceName: config.serviceName ?? env.OTEL_SERVICE_NAME ?? "agent",
      agentName: config.agentName ?? env.AGENTLENS_AGENT_NAME,
      agentVersion: config.agentVersion ?? env.AGENTLENS_AGENT_VERSION,
      captureContent: config.captureContent ?? false,
      redactedKeys: config.redactedKeys ?? [],
      maxQueueSize: config.maxQueueSize ?? 2048,
      exportTimeoutMs: config.exportTimeoutMs ?? 5000,
    };

    const resourceAttrs: Record<string, string> = {
      "service.name": this.config.serviceName,
      "deployment.environment": this.config.environment,
    };
    if (this.config.agentName) {
      resourceAttrs["agent.name"] = this.config.agentName;
    }
    if (this.config.agentVersion) {
      resourceAttrs["agent.version"] = this.config.agentVersion;
    }
    const resource = resourceFromAttributes(resourceAttrs);

    let exporter: SpanExporter;
    if (config.exporter) {
      exporter = config.exporter;
    } else {
      const exportUrl = this.config.endpoint.endsWith("/v1/traces")
        ? this.config.endpoint
        : `${this.config.endpoint}/v1/traces`;
      const headers: Record<string, string> = {};
      if (this.config.apiKey) {
        headers["Authorization"] = `Bearer ${this.config.apiKey}`;
      }
      exporter = new OTLPTraceExporter({
        url: exportUrl,
        headers,
        timeoutMillis: this.config.exportTimeoutMs,
      });
    }

    this.processor = new AgentLensSpanProcessor(exporter, {
      agentName: this.config.agentName,
      agentVersion: this.config.agentVersion,
      environment: this.config.environment,
      captureContent: this.config.captureContent,
      redactedKeys: this.config.redactedKeys,
      maxQueueSize: this.config.maxQueueSize,
      exportTimeoutMillis: this.config.exportTimeoutMs,
    });

    this.provider = new BasicTracerProvider({
      resource,
      spanProcessors: [this.processor],
    });

    this.tracer = this.provider.getTracer("agentlens", "0.1.0");

    AgentLens.instance = this;
  }

  public static getInstance(): AgentLens {
    if (!AgentLens.instance) {
      AgentLens.instance = new AgentLens();
    }
    return AgentLens.instance;
  }

  public getConfig(): Readonly<AgentLensConfig> {
    return Object.freeze({ ...this.config });
  }

  public getDroppedSpanCount(): number {
    return this.processor.droppedSpanCount;
  }

  public async flush(options: { timeoutMs?: number } = {}): Promise<boolean> {
    try {
      await this.processor.forceFlush();
      return true;
    } catch {
      return false;
    }
  }

  public async shutdown(): Promise<void> {
    try {
      await this.processor.shutdown();
    } catch {
      // Fail-open
    }
  }

  public async withSpan<T>(
    name: string,
    fn: (span: Span) => Promise<T> | T,
    options: {
      spanType?: string;
      attributes?: Record<string, any>;
    } = {}
  ): Promise<T> {
    const spanType = options.spanType || "custom";
    const attrs = { ...(options.attributes || {}), "agentlens.span_type": spanType };
    const span = this.tracer.startSpan(name, { attributes: attrs });

    return context.with(trace.setSpan(context.active(), span), async () => {
      try {
        const result = await fn(span);
        return result;
      } catch (err: any) {
        span.recordException(err);
        span.setStatus({
          code: SpanStatusCode.ERROR,
          message: err?.message || String(err),
        });
        throw err;
      } finally {
        span.end();
      }
    });
  }

  public async withAgentSpan<T>(
    name: string,
    fn: (span: Span) => Promise<T> | T,
    options: {
      version?: string;
      attributes?: Record<string, any>;
    } = {}
  ): Promise<T> {
    const attrs: Record<string, any> = {
      "agentlens.span_type": "agent",
      "agent.name": name,
      ...(options.attributes || {}),
    };
    if (options.version) {
      attrs["agent.version"] = options.version;
    }
    return this.withSpan(name, fn, { spanType: "agent", attributes: attrs });
  }

  public async withToolSpan<T>(
    name: string,
    fn: (span: Span) => Promise<T> | T,
    options: {
      input?: any;
      attributes?: Record<string, any>;
    } = {}
  ): Promise<T> {
    const attrs: Record<string, any> = {
      "agentlens.span_type": "tool",
      "tool.name": name,
      "gen_ai.tool.name": name,
      ...(options.attributes || {}),
    };
    if (this.config.captureContent && options.input !== undefined) {
      attrs["agentlens.tool.input"] =
        typeof options.input === "string" ? options.input : JSON.stringify(options.input);
    }

    return this.withSpan(
      `tool:${name}`,
      async (span) => {
        const result = await fn(span);
        if (this.config.captureContent && result !== undefined) {
          span.setAttribute(
            "agentlens.tool.output",
            typeof result === "string" ? result : JSON.stringify(result)
          );
        }
        return result;
      },
      { spanType: "tool", attributes: attrs }
    );
  }

  public async withLlmSpan<T>(
    name: string,
    model: string,
    fn: (span: Span) => Promise<T> | T,
    options: {
      provider?: string;
      attributes?: Record<string, any>;
    } = {}
  ): Promise<T> {
    const attrs: Record<string, any> = {
      "agentlens.span_type": "llm",
      "gen_ai.system": options.provider || "openai",
      "gen_ai.request.model": model,
      ...(options.attributes || {}),
    };
    return this.withSpan(`llm:${model}`, fn, { spanType: "llm", attributes: attrs });
  }
}

// Module-level ergonomic helpers delegating to default instance
export function init(config?: Partial<AgentLensConfig>): AgentLens {
  return new AgentLens(config);
}

export function getInstance(): AgentLens {
  return AgentLens.getInstance();
}

export function withSpan<T>(
  name: string,
  fn: (span: Span) => Promise<T> | T,
  options?: { spanType?: string; attributes?: Record<string, any> }
): Promise<T> {
  return getInstance().withSpan(name, fn, options);
}

export function withAgentSpan<T>(
  name: string,
  fn: (span: Span) => Promise<T> | T,
  options?: { version?: string; attributes?: Record<string, any> }
): Promise<T> {
  return getInstance().withAgentSpan(name, fn, options);
}

export function withToolSpan<T>(
  name: string,
  fn: (span: Span) => Promise<T> | T,
  options?: { input?: any; attributes?: Record<string, any> }
): Promise<T> {
  return getInstance().withToolSpan(name, fn, options);
}

export function withLlmSpan<T>(
  name: string,
  model: string,
  fn: (span: Span) => Promise<T> | T,
  options?: { provider?: string; attributes?: Record<string, any> }
): Promise<T> {
  return getInstance().withLlmSpan(name, model, fn, options);
}

export function flush(options?: { timeoutMs?: number }): Promise<boolean> {
  return getInstance().flush(options);
}

export function shutdown(): Promise<void> {
  return getInstance().shutdown();
}
