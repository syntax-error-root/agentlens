-- 0004_traces_spans_and_usage.sql

CREATE TABLE IF NOT EXISTS traces (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    agent_id UUID,
    session_id UUID,
    trace_id VARCHAR(32) NOT NULL,
    root_span_id VARCHAR(16),
    name VARCHAR(255),
    status VARCHAR(20) NOT NULL DEFAULT 'unset',
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_ms BIGINT,
    span_count INTEGER NOT NULL DEFAULT 0,
    error_span_count INTEGER NOT NULL DEFAULT 0,
    total_input_tokens BIGINT NOT NULL DEFAULT 0,
    total_output_tokens BIGINT NOT NULL DEFAULT 0,
    total_cached_input_tokens BIGINT NOT NULL DEFAULT 0,
    total_reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    estimated_cost NUMERIC(24, 12),
    has_unpriced_usage BOOLEAN NOT NULL DEFAULT FALSE,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    resource_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finalized_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE SET NULL,
    FOREIGN KEY (project_id, session_id)
        REFERENCES sessions(project_id, id) ON DELETE SET NULL,
    CHECK (trace_id ~ '^[0-9a-f]{32}$' AND trace_id <> '00000000000000000000000000000000'),
    CHECK (root_span_id IS NULL OR (root_span_id ~ '^[0-9a-f]{16}$' AND root_span_id <> '0000000000000000')),
    CHECK (status IN ('unset', 'ok', 'error')),
    CHECK (total_input_tokens >= 0 AND total_output_tokens >= 0),
    CHECK (total_cached_input_tokens >= 0 AND total_reasoning_tokens >= 0),
    CHECK (duration_ms IS NULL OR duration_ms >= 0)
);

CREATE TABLE IF NOT EXISTS spans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    parent_span_id VARCHAR(16),
    agent_id UUID,
    name VARCHAR(255) NOT NULL,
    span_type VARCHAR(50) NOT NULL DEFAULT 'custom',
    status VARCHAR(20) NOT NULL DEFAULT 'unset',
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_ms BIGINT,
    provider VARCHAR(100),
    model VARCHAR(255),
    tool_name VARCHAR(255),
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    resource_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    scope_attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    payload_s3_key TEXT,
    error JSONB,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id),
    FOREIGN KEY (project_id, trace_id)
        REFERENCES traces(project_id, trace_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE SET NULL,
    CHECK (trace_id ~ '^[0-9a-f]{32}$' AND trace_id <> '00000000000000000000000000000000'),
    CHECK (span_id ~ '^[0-9a-f]{16}$' AND span_id <> '0000000000000000'),
    CHECK (parent_span_id IS NULL OR (parent_span_id ~ '^[0-9a-f]{16}$' AND parent_span_id <> '0000000000000000')),
    CHECK (span_type IN ('agent', 'llm', 'tool', 'retrieval', 'chain', 'http', 'database', 'custom')),
    CHECK (status IN ('unset', 'ok', 'error')),
    CHECK (duration_ms IS NULL OR duration_ms >= 0)
);

CREATE INDEX IF NOT EXISTS idx_traces_project_started
    ON traces(project_id, started_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_traces_project_env_started
    ON traces(project_id, environment_id, started_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_traces_project_agent_started
    ON traces(project_id, agent_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_project_status_started
    ON traces(project_id, status, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_spans_project_trace
    ON spans(project_id, trace_id, started_at);
CREATE INDEX IF NOT EXISTS idx_spans_project_parent
    ON spans(project_id, trace_id, parent_span_id);
CREATE INDEX IF NOT EXISTS idx_spans_project_started
    ON spans(project_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_spans_project_tool
    ON spans(project_id, tool_name, started_at DESC)
    WHERE span_type = 'tool';

CREATE TABLE IF NOT EXISTS span_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    event_index INTEGER NOT NULL,
    event_name VARCHAR(255) NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id, event_index),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS span_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    link_index INTEGER NOT NULL,
    linked_trace_id VARCHAR(32) NOT NULL,
    linked_span_id VARCHAR(16) NOT NULL,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id, link_index),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS token_usages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    trace_id VARCHAR(32) NOT NULL,
    span_id VARCHAR(16) NOT NULL,
    provider VARCHAR(100) NOT NULL,
    model VARCHAR(255) NOT NULL,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cached_input_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    cost_estimate NUMERIC(24, 12),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    pricing_version VARCHAR(100),
    input_rate_per_million NUMERIC(24, 12),
    output_rate_per_million NUMERIC(24, 12),
    cached_rate_per_million NUMERIC(24, 12),
    usage_source VARCHAR(30) NOT NULL DEFAULT 'provider_reported',
    recorded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, trace_id, span_id),
    FOREIGN KEY (project_id, trace_id, span_id)
        REFERENCES spans(project_id, trace_id, span_id) ON DELETE CASCADE,
    CHECK (input_tokens >= 0 AND output_tokens >= 0),
    CHECK (cached_input_tokens >= 0 AND reasoning_tokens >= 0),
    CHECK (usage_source IN ('provider_reported', 'estimated'))
);

CREATE INDEX IF NOT EXISTS idx_token_usage_project_recorded
    ON token_usages(project_id, recorded_at DESC);

CREATE TABLE IF NOT EXISTS model_pricing (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider VARCHAR(100) NOT NULL,
    model_pattern VARCHAR(255) NOT NULL,
    match_type VARCHAR(20) NOT NULL DEFAULT 'exact',
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    input_per_million NUMERIC(24, 12),
    output_per_million NUMERIC(24, 12),
    cached_input_per_million NUMERIC(24, 12),
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    source_url TEXT,
    source_checked_at TIMESTAMPTZ,
    version VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, model_pattern, version),
    CHECK (match_type IN ('exact', 'prefix')),
    CHECK (input_per_million IS NULL OR input_per_million >= 0),
    CHECK (output_per_million IS NULL OR output_per_million >= 0),
    CHECK (cached_input_per_million IS NULL OR cached_input_per_million >= 0)
);

CREATE TABLE IF NOT EXISTS outbox_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID,
    job_type VARCHAR(100) NOT NULL,
    dedup_key VARCHAR(255),
    payload JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 10,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_expires_at TIMESTAMPTZ,
    locked_at TIMESTAMPTZ,
    locked_by VARCHAR(255),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'processing', 'completed', 'dead')),
    CHECK (attempts >= 0 AND max_attempts > 0)
);

CREATE INDEX IF NOT EXISTS idx_outbox_claimable
    ON outbox_jobs(available_at, created_at)
    WHERE status IN ('pending', 'processing');
