-- 0005_alerts_notifications_metrics.sql

CREATE TABLE IF NOT EXISTS alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    metric VARCHAR(100) NOT NULL,
    operator VARCHAR(10) NOT NULL,
    threshold NUMERIC(24, 6) NOT NULL,
    window_seconds INTEGER NOT NULL,
    min_sample_count INTEGER NOT NULL DEFAULT 1,
    group_by JSONB NOT NULL DEFAULT '[]'::jsonb,
    filters JSONB NOT NULL DEFAULT '{}'::jsonb,
    cooldown_seconds INTEGER NOT NULL DEFAULT 300,
    severity VARCHAR(20) NOT NULL DEFAULT 'warning',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE,
    CHECK (operator IN ('gt', 'gte', 'lt', 'lte', 'eq')),
    CHECK (severity IN ('info', 'warning', 'critical')),
    CHECK (window_seconds >= 60 AND min_sample_count >= 1 AND cooldown_seconds >= 0)
);

CREATE TABLE IF NOT EXISTS alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    rule_id UUID NOT NULL,
    fingerprint VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'firing',
    severity VARCHAR(20) NOT NULL,
    summary TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    first_triggered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_triggered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, rule_id)
        REFERENCES alert_rules(project_id, id) ON DELETE CASCADE,
    CHECK (status IN ('firing', 'acknowledged', 'resolved')),
    CHECK (severity IN ('info', 'warning', 'critical'))
);

CREATE INDEX IF NOT EXISTS idx_alert_events_active
    ON alert_events(project_id, rule_id, fingerprint)
    WHERE status IN ('firing', 'acknowledged');

CREATE TABLE IF NOT EXISTS notification_channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID,
    channel_type VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    encrypted_config TEXT NOT NULL,
    key_version VARCHAR(32) NOT NULL DEFAULT 'v1',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (channel_type IN ('webhook', 'telegram', 'discord', 'slack', 'email'))
);

CREATE TABLE IF NOT EXISTS notification_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_event_id UUID NOT NULL REFERENCES alert_events(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    provider_message_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    CHECK (status IN ('pending', 'delivered', 'failed'))
);

CREATE TABLE IF NOT EXISTS daily_metrics (
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    environment_id UUID NOT NULL,
    metric_date DATE NOT NULL,
    dimension_type VARCHAR(50) NOT NULL,
    dimension_key VARCHAR(255) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    count_value BIGINT NOT NULL DEFAULT 0,
    sum_value NUMERIC(28, 12) NOT NULL DEFAULT 0,
    min_value NUMERIC(28, 12),
    max_value NUMERIC(28, 12),
    p50_value NUMERIC(28, 12),
    p95_value NUMERIC(28, 12),
    p99_value NUMERIC(28, 12),
    estimated_cost NUMERIC(24, 12),
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name),
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, environment_id)
        REFERENCES environments(project_id, id) ON DELETE CASCADE
);
