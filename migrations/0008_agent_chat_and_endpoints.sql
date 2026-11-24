-- 0008_agent_chat_and_endpoints.sql

-- 1. Extend agents table with invocation endpoint settings
ALTER TABLE agents
ADD COLUMN IF NOT EXISTS endpoint_url VARCHAR(1024),
ADD COLUMN IF NOT EXISTS auth_type VARCHAR(50) DEFAULT 'none',
ADD COLUMN IF NOT EXISTS auth_token TEXT,
ADD COLUMN IF NOT EXISTS default_model VARCHAR(255);

-- 2. Dedicated chat_sessions table
CREATE TABLE IF NOT EXISTS chat_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    agent_id UUID NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    title VARCHAR(255) NOT NULL DEFAULT 'New Conversation',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, agent_id)
        REFERENCES agents(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id, project_id)
        REFERENCES projects(organization_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_agent
    ON chat_sessions(project_id, agent_id, updated_at DESC);

-- 3. Dedicated chat_messages table
CREATE TABLE IF NOT EXISTS chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    project_id UUID NOT NULL,
    chat_session_id UUID NOT NULL,
    role VARCHAR(50) NOT NULL, -- 'user', 'assistant', 'system', 'tool'
    content TEXT NOT NULL,
    trace_id VARCHAR(64),
    token_count INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, chat_session_id)
        REFERENCES chat_sessions(project_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session
    ON chat_messages(project_id, chat_session_id, created_at ASC);
