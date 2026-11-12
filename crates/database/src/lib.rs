#![forbid(unsafe_code)]

//! `agentlens-database` crate.
//! PostgreSQL connection pools, SQLx migrations, transactions, and tenant repositories.

use agentlens_config::AppConfig;
use agentlens_domain::{
    Agent, Environment, Organization, OutboxJob, Project, Span, SpanId, SpanType, TokenUsage,
    Trace, TraceId, TraceStatus, UsageSource,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLx database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Entity not found: {0}")]
    NotFound(String),
}

pub type DbPool = PgPool;

/// Create a PostgreSQL connection pool with standard production timeouts.
pub async fn create_pool(config: &AppConfig) -> Result<DbPool, DatabaseError> {
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .connect(&config.database.url)
        .await?;

    Ok(pool)
}

/// Run pending migrations from the repository root `migrations/` directory.
pub async fn run_migrations(pool: &DbPool) -> Result<(), DatabaseError> {
    sqlx::migrate!("../../migrations").run(pool).await?;
    Ok(())
}

/// Check database readiness by querying `SELECT 1`.
pub async fn check_readiness(pool: &DbPool) -> Result<(), DatabaseError> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Organizations Repository
// ---------------------------------------------------------------------------

pub struct OrganizationsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> OrganizationsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, name: &str, slug: &str) -> Result<Organization, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO organizations (name, slug)
            VALUES ($1, $2)
            RETURNING id, name, slug, created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(slug)
        .map(|r: PgRow| Organization {
            id: r.get("id"),
            name: r.get("name"),
            slug: r.get("slug"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Organization>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, slug, created_at, updated_at
            FROM organizations
            WHERE id = $1
            "#,
        )
        .bind(id)
        .map(|r: PgRow| Organization {
            id: r.get("id"),
            name: r.get("name"),
            slug: r.get("slug"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Projects Repository
// ---------------------------------------------------------------------------

pub struct ProjectsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ProjectsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Creates a project and automatically initializes production, staging, and development environments in one tx.
    pub async fn create(
        &self,
        org_id: Uuid,
        name: &str,
        slug: &str,
        description: Option<&str>,
    ) -> Result<(Project, Vec<Environment>), DatabaseError> {
        let mut tx = self.pool.begin().await?;

        let project = sqlx::query(
            r#"
            INSERT INTO projects (organization_id, name, slug, description)
            VALUES ($1, $2, $3, $4)
            RETURNING id, organization_id, name, slug, description,
                      capture_content_default, redaction_rules, retention_days,
                      created_at, updated_at
            "#,
        )
        .bind(org_id)
        .bind(name)
        .bind(slug)
        .bind(description)
        .map(|r: PgRow| Project {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            name: r.get("name"),
            slug: r.get("slug"),
            description: r.get("description"),
            capture_content_default: r.get("capture_content_default"),
            redaction_rules: r.get("redaction_rules"),
            retention_days: r.get("retention_days"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(&mut *tx)
        .await?;

        let env_names = [
            ("production", true),
            ("staging", false),
            ("development", false),
        ];

        let mut envs = Vec::with_capacity(3);
        for (env_name, is_default) in env_names {
            let env = sqlx::query(
                r#"
                INSERT INTO environments (organization_id, project_id, name, is_default)
                VALUES ($1, $2, $3, $4)
                RETURNING id, organization_id, project_id, name, is_default, created_at
                "#,
            )
            .bind(org_id)
            .bind(project.id)
            .bind(env_name)
            .bind(is_default)
            .map(|r: PgRow| Environment {
                id: r.get("id"),
                organization_id: r.get("organization_id"),
                project_id: r.get("project_id"),
                name: r.get("name"),
                is_default: r.get("is_default"),
                created_at: r.get("created_at"),
            })
            .fetch_one(&mut *tx)
            .await?;

            envs.push(env);
        }

        tx.commit().await?;
        Ok((project, envs))
    }

    pub async fn get_by_id(&self, project_id: Uuid) -> Result<Option<Project>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, name, slug, description,
                   capture_content_default, redaction_rules, retention_days,
                   created_at, updated_at
            FROM projects
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .map(|r: PgRow| Project {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            name: r.get("name"),
            slug: r.get("slug"),
            description: r.get("description"),
            capture_content_default: r.get("capture_content_default"),
            redaction_rules: r.get("redaction_rules"),
            retention_days: r.get("retention_days"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Environments Repository
// ---------------------------------------------------------------------------

pub struct EnvironmentsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> EnvironmentsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn list_for_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Environment>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, name, is_default, created_at
            FROM environments
            WHERE project_id = $1
            ORDER BY is_default DESC, name ASC
            "#,
        )
        .bind(project_id)
        .map(|r: PgRow| Environment {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            is_default: r.get("is_default"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        env_id: Uuid,
    ) -> Result<Option<Environment>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, name, is_default, created_at
            FROM environments
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(env_id)
        .map(|r: PgRow| Environment {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            is_default: r.get("is_default"),
            created_at: r.get("created_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Agents Repository
// ---------------------------------------------------------------------------

pub struct AgentsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> AgentsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        name: &str,
        framework: Option<&str>,
        language: Option<&str>,
    ) -> Result<Agent, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO agents (organization_id, project_id, name, framework, language, last_seen_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (project_id, name) DO UPDATE SET
                framework = COALESCE(EXCLUDED.framework, agents.framework),
                language = COALESCE(EXCLUDED.language, agents.language),
                last_seen_at = now(),
                updated_at = now()
            RETURNING id, organization_id, project_id, name, framework, language,
                      description, metadata, last_seen_at, created_at, updated_at
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(name)
        .bind(framework)
        .bind(language)
        .map(|r: PgRow| Agent {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            framework: r.get("framework"),
            language: r.get("language"),
            description: r.get("description"),
            metadata: r.get("metadata"),
            last_seen_at: r.get("last_seen_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> Result<Option<Agent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, name, framework, language,
                   description, metadata, last_seen_at, created_at, updated_at
            FROM agents
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(agent_id)
        .map(|r: PgRow| Agent {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            framework: r.get("framework"),
            language: r.get("language"),
            description: r.get("description"),
            metadata: r.get("metadata"),
            last_seen_at: r.get("last_seen_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Traces Repository (Provisional Upsert & Root Span Reconciliation)
// ---------------------------------------------------------------------------

pub struct TracesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> TracesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Upserts a provisional trace row when child spans arrive before the root span.
    /// Preserves LEAST(started_at) and GREATEST(ended_at) across batches.
    pub async fn upsert_provisional(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        env_id: Uuid,
        trace_id: &TraceId,
        started_at: DateTime<Utc>,
        ended_at: Option<DateTime<Utc>>,
    ) -> Result<(), DatabaseError> {
        let trace_id_str = trace_id.as_str();

        sqlx::query(
            r#"
            INSERT INTO traces (
                organization_id, project_id, environment_id, trace_id, started_at, ended_at, status
            ) VALUES ($1, $2, $3, $4, $5, $6, 'unset')
            ON CONFLICT (project_id, trace_id) DO UPDATE SET
                started_at = LEAST(traces.started_at, EXCLUDED.started_at),
                ended_at = GREATEST(traces.ended_at, EXCLUDED.ended_at),
                updated_at = now()
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(env_id)
        .bind(trace_id_str)
        .bind(started_at)
        .bind(ended_at)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Reconciles trace metadata when the root span arrives.
    pub async fn reconcile_root_span(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
        root_span_id: &SpanId,
        name: &str,
        status: TraceStatus,
        agent_id: Option<Uuid>,
    ) -> Result<(), DatabaseError> {
        let trace_id_str = trace_id.as_str();
        let root_span_id_str = root_span_id.as_str();
        let status_str = status.as_str();

        sqlx::query(
            r#"
            UPDATE traces SET
                root_span_id = $3,
                name = $4,
                status = $5,
                agent_id = $6,
                updated_at = now()
            WHERE project_id = $1 AND trace_id = $2
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .bind(root_span_id_str)
        .bind(name)
        .bind(status_str)
        .bind(agent_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_by_trace_id(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
    ) -> Result<Option<Trace>, DatabaseError> {
        let trace_id_str = trace_id.as_str();
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1 AND trace_id = $2
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let tid_raw: String = r.get("trace_id");
                let parsed_trace_id =
                    TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

                let root_sid_raw: Option<String> = r.get("root_span_id");
                let parsed_root_span_id = match root_sid_raw {
                    Some(sid) => {
                        Some(SpanId::new(sid).map_err(|e| DatabaseError::NotFound(e.to_string()))?)
                    }
                    None => None,
                };

                let status_raw: String = r.get("status");
                let status = status_raw
                    .parse::<TraceStatus>()
                    .unwrap_or(TraceStatus::Unset);

                Ok(Some(Trace {
                    id: r.get("id"),
                    organization_id: r.get("organization_id"),
                    project_id: r.get("project_id"),
                    environment_id: r.get("environment_id"),
                    agent_id: r.get("agent_id"),
                    session_id: r.get("session_id"),
                    trace_id: parsed_trace_id,
                    root_span_id: parsed_root_span_id,
                    name: r.get("name"),
                    status,
                    started_at: r.get("started_at"),
                    ended_at: r.get("ended_at"),
                    duration_ms: r.get("duration_ms"),
                    span_count: r.get("span_count"),
                    error_span_count: r.get("error_span_count"),
                    total_input_tokens: r.get("total_input_tokens"),
                    total_output_tokens: r.get("total_output_tokens"),
                    total_cached_input_tokens: r.get("total_cached_input_tokens"),
                    total_reasoning_tokens: r.get("total_reasoning_tokens"),
                    estimated_cost: r.get("estimated_cost"),
                    has_unpriced_usage: r.get("has_unpriced_usage"),
                    currency: r.get("currency"),
                    resource_attributes: r.get("resource_attributes"),
                    attributes: r.get("attributes"),
                    received_at: r.get("received_at"),
                    finalized_at: r.get("finalized_at"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Spans Repository
// ---------------------------------------------------------------------------

pub struct SpansRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> SpansRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn insert_span(&self, span: &Span) -> Result<(), DatabaseError> {
        let trace_id_str = span.trace_id.as_str();
        let span_id_str = span.span_id.as_str();
        let parent_span_id_str = span.parent_span_id.as_ref().map(SpanId::as_str);
        let span_type_str = span.span_type.as_str();
        let status_str = span.status.as_str();

        sqlx::query(
            r#"
            INSERT INTO spans (
                id, organization_id, project_id, trace_id, span_id, parent_span_id,
                agent_id, name, span_type, status, started_at, ended_at, duration_ms,
                provider, model, tool_name, attributes, resource_attributes,
                scope_attributes, payload_s3_key, error
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19, $20, $21
            )
            ON CONFLICT (project_id, trace_id, span_id) DO UPDATE SET
                status = EXCLUDED.status,
                ended_at = EXCLUDED.ended_at,
                duration_ms = EXCLUDED.duration_ms,
                attributes = EXCLUDED.attributes
            "#,
        )
        .bind(span.id)
        .bind(span.organization_id)
        .bind(span.project_id)
        .bind(trace_id_str)
        .bind(span_id_str)
        .bind(parent_span_id_str)
        .bind(span.agent_id)
        .bind(&span.name)
        .bind(span_type_str)
        .bind(status_str)
        .bind(span.started_at)
        .bind(span.ended_at)
        .bind(span.duration_ms)
        .bind(&span.provider)
        .bind(&span.model)
        .bind(&span.tool_name)
        .bind(&span.attributes)
        .bind(&span.resource_attributes)
        .bind(&span.scope_attributes)
        .bind(&span.payload_s3_key)
        .bind(&span.error)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn count_spans_for_trace(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
    ) -> Result<i64, DatabaseError> {
        let trace_id_str = trace_id.as_str();
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM spans
            WHERE project_id = $1 AND trace_id = $2
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .map(|r: PgRow| r.get::<i64, _>("count"))
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Token Usages Repository
// ---------------------------------------------------------------------------

pub struct TokenUsagesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> TokenUsagesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn upsert_usage(&self, usage: &TokenUsage) -> Result<(), DatabaseError> {
        let trace_id_str = usage.trace_id.as_str();
        let span_id_str = usage.span_id.as_str();
        let usage_source_str = usage.usage_source.as_str();

        sqlx::query(
            r#"
            INSERT INTO token_usages (
                id, organization_id, project_id, trace_id, span_id, provider, model,
                input_tokens, output_tokens, cached_input_tokens, reasoning_tokens,
                cost_estimate, currency, pricing_version, input_rate_per_million,
                output_rate_per_million, cached_rate_per_million, usage_source, recorded_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                $14, $15, $16, $17, $18, $19
            )
            ON CONFLICT (project_id, trace_id, span_id) DO UPDATE SET
                input_tokens = EXCLUDED.input_tokens,
                output_tokens = EXCLUDED.output_tokens,
                cached_input_tokens = EXCLUDED.cached_input_tokens,
                reasoning_tokens = EXCLUDED.reasoning_tokens,
                cost_estimate = EXCLUDED.cost_estimate
            "#,
        )
        .bind(usage.id)
        .bind(usage.organization_id)
        .bind(usage.project_id)
        .bind(trace_id_str)
        .bind(span_id_str)
        .bind(&usage.provider)
        .bind(&usage.model)
        .bind(usage.input_tokens)
        .bind(usage.output_tokens)
        .bind(usage.cached_input_tokens)
        .bind(usage.reasoning_tokens)
        .bind(usage.cost_estimate)
        .bind(&usage.currency)
        .bind(&usage.pricing_version)
        .bind(usage.input_rate_per_million)
        .bind(usage.output_rate_per_million)
        .bind(usage.cached_rate_per_million)
        .bind(usage_source_str)
        .bind(usage.recorded_at)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Outbox Repository
// ---------------------------------------------------------------------------

pub struct OutboxRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> OutboxRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn enqueue(&self, job: &OutboxJob) -> Result<Uuid, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO outbox_jobs (
                id, organization_id, project_id, job_type, dedup_key, payload,
                status, max_attempts, available_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(job.id)
        .bind(job.organization_id)
        .bind(job.project_id)
        .bind(&job.job_type)
        .bind(&job.dedup_key)
        .bind(&job.payload)
        .bind(&job.status)
        .bind(job.max_attempts)
        .bind(job.available_at)
        .map(|r: PgRow| r.get::<Uuid, _>("id"))
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn claim_jobs(
        &self,
        worker_id: &str,
        limit: i64,
        lease_seconds: i32,
    ) -> Result<Vec<OutboxJob>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            UPDATE outbox_jobs
            SET status = 'processing',
                locked_at = now(),
                locked_by = $1,
                lease_expires_at = now() + ($3 * interval '1 second'),
                attempts = attempts + 1
            WHERE id IN (
                SELECT id FROM outbox_jobs
                WHERE (status = 'pending' AND available_at <= now())
                   OR (status = 'processing' AND lease_expires_at < now())
                ORDER BY available_at ASC, created_at ASC
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, organization_id, project_id, job_type, dedup_key,
                      payload, status, attempts, max_attempts, available_at,
                      lease_expires_at, locked_at, locked_by, last_error,
                      created_at, completed_at
            "#,
        )
        .bind(worker_id)
        .bind(limit)
        .bind(f64::from(lease_seconds))
        .map(|r: PgRow| OutboxJob {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            job_type: r.get("job_type"),
            dedup_key: r.get("dedup_key"),
            payload: r.get("payload"),
            status: r.get("status"),
            attempts: r.get("attempts"),
            max_attempts: r.get("max_attempts"),
            available_at: r.get("available_at"),
            lease_expires_at: r.get("lease_expires_at"),
            locked_at: r.get("locked_at"),
            locked_by: r.get("locked_by"),
            last_error: r.get("last_error"),
            created_at: r.get("created_at"),
            completed_at: r.get("completed_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}
