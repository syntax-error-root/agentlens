/**
 * AgentLens TypeScript SDK
 * Monitor. Trace. Understand AI.
 */

export interface AgentLensConfig {
  apiKey: string;
  endpoint?: string;
  captureContent?: boolean;
}

export class AgentLens {
  private readonly config: AgentLensConfig;

  constructor(config: AgentLensConfig) {
    if (!config.apiKey) {
      throw new Error("AgentLens API key is required");
    }
    this.config = {
      endpoint: "http://localhost:4318/v1/traces",
      captureContent: false,
      ...config,
    };
  }

  public getConfig(): Readonly<AgentLensConfig> {
    return Object.freeze({ ...this.config });
  }

  public async flush(): Promise<void> {
    // Flush buffered spans to OTLP collector
  }
}
