#![forbid(unsafe_code)]

//! `agentlens-database` crate.
//! PostgreSQL connection pools, SQLx migrations, transactions, and tenant repositories.

use agentlens_config::AppConfig;
use agentlens_domain::{
    decode_cursor, encode_cursor, Agent, AgentAnalyticsItem, AgentHealthReport, AgentHealthStatus,
    AgentVersion, AlertEvent, AlertEventStatus, AlertMetric, AlertOperator, AlertRule,
    AlertSeverity, AnalyticsFilters, AnalyticsOverview, ApiKey, AuditLog, CostAnalyticsResult,
    CostTimePoint, Environment, ErrorAnalyticsResult, ErrorTimePoint, LatencyAnalyticsResult,
    ModelAnalyticsItem, ModelPricing, NotificationChannel, NotificationChannelType,
    NotificationDelivery, NotificationDeliveryStatus, OrgRole, Organization, OrganizationMember,
    OutboxJob, Project, ProjectOverviewMetrics, Session, Span, SpanDetail, SpanEvent, SpanId,
    SpanLink, SpanType, TokenAnalyticsResult, TokenUsage, TokenUsageTimePoint, ToolAnalyticsItem,
    TopErrorItem, Trace, TraceId, TraceQueryFilters, TraceQueryResult, TraceStatus, UsageSource,
    User,
};
use agentlens_telemetry::NormalizedSpan;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;
use std::collections::HashMap;
use std::str::FromStr;
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
    #[error("Bootstrap has already been completed")]
    BootstrapAlreadyCompleted,
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

    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<Organization>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, slug, created_at, updated_at
            FROM organizations
            WHERE slug = $1
            "#,
        )
        .bind(slug)
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

    pub async fn update(&self, id: Uuid, name: &str) -> Result<Organization, DatabaseError> {
        let row = sqlx::query(
            r#"
            UPDATE organizations
            SET name = $2, updated_at = now()
            WHERE id = $1
            RETURNING id, name, slug, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .map(|r: PgRow| Organization {
            id: r.get("id"),
            name: r.get("name"),
            slug: r.get("slug"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?
        .ok_or_else(|| DatabaseError::NotFound(format!("Organization {id} not found")))?;

        Ok(row)
    }

    pub async fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Organization, OrgRole)>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.name, o.slug, o.created_at, o.updated_at, m.role
            FROM organizations o
            JOIN organization_members m ON o.id = m.organization_id
            WHERE m.user_id = $1
            ORDER BY o.name ASC
            "#,
        )
        .bind(user_id)
        .map(|r: PgRow| {
            let role_str: String = r.get("role");
            let role = OrgRole::from_str(&role_str).unwrap_or(OrgRole::Viewer);
            let org = Organization {
                id: r.get("id"),
                name: r.get("name"),
                slug: r.get("slug"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            };
            (org, role)
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Users Repository
// ---------------------------------------------------------------------------

pub struct UsersRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> UsersRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn count(&self) -> Result<i64, DatabaseError> {
        let row = sqlx::query(r#"SELECT COUNT(*) as count FROM users"#)
            .map(|r: PgRow| r.get::<i64, _>("count"))
            .fetch_one(self.pool)
            .await?;
        Ok(row)
    }

    pub async fn create(
        &self,
        email: &str,
        password_hash: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<User, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO users (email, password_hash, display_name, status)
            VALUES ($1, $2, $3, 'active')
            RETURNING id, email, password_hash, oidc_subject, oidc_issuer,
                      display_name, status, created_at, updated_at
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .map(|r: PgRow| User {
            id: r.get("id"),
            email: r.get("email"),
            password_hash: r.get("password_hash"),
            oidc_subject: r.get("oidc_subject"),
            oidc_issuer: r.get("oidc_issuer"),
            display_name: r.get("display_name"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, password_hash, oidc_subject, oidc_issuer,
                   display_name, status, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .map(|r: PgRow| User {
            id: r.get("id"),
            email: r.get("email"),
            password_hash: r.get("password_hash"),
            oidc_subject: r.get("oidc_subject"),
            oidc_issuer: r.get("oidc_issuer"),
            display_name: r.get("display_name"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, email, password_hash, oidc_subject, oidc_issuer,
                   display_name, status, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .map(|r: PgRow| User {
            id: r.get("id"),
            email: r.get("email"),
            password_hash: r.get("password_hash"),
            oidc_subject: r.get("oidc_subject"),
            oidc_issuer: r.get("oidc_issuer"),
            display_name: r.get("display_name"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Organization Members Repository
// ---------------------------------------------------------------------------

pub struct OrganizationMembersRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> OrganizationMembersRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn add(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: OrgRole,
    ) -> Result<OrganizationMember, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO organization_members (organization_id, user_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role
            RETURNING organization_id, user_id, role, created_at
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role.as_str())
        .map(|r: PgRow| {
            let role_str: String = r.get("role");
            OrganizationMember {
                organization_id: r.get("organization_id"),
                user_id: r.get("user_id"),
                role: OrgRole::from_str(&role_str).unwrap_or(OrgRole::Viewer),
                created_at: r.get("created_at"),
            }
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrgRole>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT role FROM organization_members
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .map(|r: PgRow| {
            let role_str: String = r.get("role");
            OrgRole::from_str(&role_str).unwrap_or(OrgRole::Viewer)
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_members(&self, org_id: Uuid) -> Result<Vec<(User, OrgRole)>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.email, u.password_hash, u.oidc_subject, u.oidc_issuer,
                   u.display_name, u.status, u.created_at, u.updated_at, m.role
            FROM users u
            JOIN organization_members m ON u.id = m.user_id
            WHERE m.organization_id = $1
            ORDER BY u.email ASC
            "#,
        )
        .bind(org_id)
        .map(|r: PgRow| {
            let role_str: String = r.get("role");
            let role = OrgRole::from_str(&role_str).unwrap_or(OrgRole::Viewer);
            let user = User {
                id: r.get("id"),
                email: r.get("email"),
                password_hash: r.get("password_hash"),
                oidc_subject: r.get("oidc_subject"),
                oidc_issuer: r.get("oidc_issuer"),
                display_name: r.get("display_name"),
                status: r.get("status"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            };
            (user, role)
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn update_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: OrgRole,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE organization_members
            SET role = $3
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role.as_str())
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove(&self, org_id: Uuid, user_id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            DELETE FROM organization_members
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .execute(self.pool)
        .await?;

        Ok(())
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

    pub async fn list_for_org(&self, org_id: Uuid) -> Result<Vec<Project>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, name, slug, description,
                   capture_content_default, redaction_rules, retention_days,
                   created_at, updated_at
            FROM projects
            WHERE organization_id = $1
            ORDER BY name ASC
            "#,
        )
        .bind(org_id)
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
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        name: &str,
        description: Option<&str>,
        retention_days: i32,
    ) -> Result<Project, DatabaseError> {
        let row = sqlx::query(
            r#"
            UPDATE projects
            SET name = $2, description = $3, retention_days = $4, updated_at = now()
            WHERE id = $1
            RETURNING id, organization_id, name, slug, description,
                      capture_content_default, redaction_rules, retention_days,
                      created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(name)
        .bind(description)
        .bind(retention_days)
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
        .await?
        .ok_or_else(|| DatabaseError::NotFound(format!("Project {project_id} not found")))?;

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
// Api Keys Repository
// ---------------------------------------------------------------------------

pub struct ApiKeysRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ApiKeysRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        environment_id: Uuid,
        name: &str,
        key_prefix: &str,
        key_hash: &[u8],
        scopes: serde_json::Value,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (
                organization_id, project_id, environment_id,
                name, key_prefix, key_hash, scopes, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, organization_id, project_id, environment_id,
                      name, key_prefix, key_hash, scopes,
                      last_used_at, expires_at, revoked_at, created_at
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(environment_id)
        .bind(name)
        .bind(key_prefix)
        .bind(key_hash)
        .bind(scopes)
        .bind(expires_at)
        .map(|r: PgRow| ApiKey {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            environment_id: r.get("environment_id"),
            name: r.get("name"),
            key_prefix: r.get("key_prefix"),
            key_hash: r.get("key_hash"),
            scopes: r.get("scopes"),
            last_used_at: r.get("last_used_at"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
            created_at: r.get("created_at"),
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn find_by_prefix(&self, prefix: &str) -> Result<Vec<ApiKey>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id,
                   name, key_prefix, key_hash, scopes,
                   last_used_at, expires_at, revoked_at, created_at
            FROM api_keys
            WHERE key_prefix = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(prefix)
        .map(|r: PgRow| ApiKey {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            environment_id: r.get("environment_id"),
            name: r.get("name"),
            key_prefix: r.get("key_prefix"),
            key_hash: r.get("key_hash"),
            scopes: r.get("scopes"),
            last_used_at: r.get("last_used_at"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<ApiKey>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id,
                   name, key_prefix, key_hash, scopes,
                   last_used_at, expires_at, revoked_at, created_at
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .map(|r: PgRow| ApiKey {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            environment_id: r.get("environment_id"),
            name: r.get("name"),
            key_prefix: r.get("key_prefix"),
            key_hash: r.get("key_hash"),
            scopes: r.get("scopes"),
            last_used_at: r.get("last_used_at"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
            created_at: r.get("created_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_for_project(&self, project_id: Uuid) -> Result<Vec<ApiKey>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id,
                   name, key_prefix, key_hash, scopes,
                   last_used_at, expires_at, revoked_at, created_at
            FROM api_keys
            WHERE project_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .map(|r: PgRow| ApiKey {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            environment_id: r.get("environment_id"),
            name: r.get("name"),
            key_prefix: r.get("key_prefix"),
            key_hash: r.get("key_hash"),
            scopes: r.get("scopes"),
            last_used_at: r.get("last_used_at"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn revoke(&self, id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn touch_last_used(&self, id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET last_used_at = now()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Audit Logs Repository
// ---------------------------------------------------------------------------

pub struct AuditLogsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> AuditLogsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record(
        &self,
        org_id: Uuid,
        actor_user_id: Option<Uuid>,
        actor_api_key_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<AuditLog, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                organization_id, actor_user_id, actor_api_key_id,
                action, resource_type, resource_id, ip_address, user_agent, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, organization_id, actor_user_id, actor_api_key_id,
                      action, resource_type, resource_id, ip_address, user_agent,
                      metadata, created_at
            "#,
        )
        .bind(org_id)
        .bind(actor_user_id)
        .bind(actor_api_key_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(ip_address)
        .bind(user_agent)
        .bind(metadata)
        .map(|r: PgRow| AuditLog {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            actor_user_id: r.get("actor_user_id"),
            actor_api_key_id: r.get("actor_api_key_id"),
            action: r.get("action"),
            resource_type: r.get("resource_type"),
            resource_id: r.get("resource_id"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
            metadata: r.get("metadata"),
            created_at: r.get("created_at"),
        })
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_for_org(
        &self,
        org_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditLog>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, actor_user_id, actor_api_key_id,
                   action, resource_type, resource_id, ip_address, user_agent,
                   metadata, created_at
            FROM audit_logs
            WHERE organization_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(limit)
        .map(|r: PgRow| AuditLog {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            actor_user_id: r.get("actor_user_id"),
            actor_api_key_id: r.get("actor_api_key_id"),
            action: r.get("action"),
            resource_type: r.get("resource_type"),
            resource_id: r.get("resource_id"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
            metadata: r.get("metadata"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Bootstrap Repository
// ---------------------------------------------------------------------------

pub struct BootstrapRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> BootstrapRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Atomically executes first-owner bootstrap guarded by a PostgreSQL advisory transaction lock.
    /// If any user exists, returns `DatabaseError::BootstrapAlreadyCompleted`.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        &self,
        email: &str,
        password_hash: &str,
        org_name: &str,
        org_slug: &str,
        default_project_name: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(User, Organization, Project, Vec<Environment>), DatabaseError> {
        let mut tx = self.pool.begin().await?;

        // 1. Acquire transaction advisory lock
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('agentlens_bootstrap'))")
            .execute(&mut *tx)
            .await?;

        // 2. Count existing users
        let user_count: i64 = sqlx::query("SELECT COUNT(*) as count FROM users")
            .map(|r: PgRow| r.get::<i64, _>("count"))
            .fetch_one(&mut *tx)
            .await?;

        if user_count > 0 {
            return Err(DatabaseError::BootstrapAlreadyCompleted);
        }

        // 3. Create user
        let user = sqlx::query(
            r#"
            INSERT INTO users (email, password_hash, status)
            VALUES ($1, $2, 'active')
            RETURNING id, email, password_hash, oidc_subject, oidc_issuer,
                      display_name, status, created_at, updated_at
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .map(|r: PgRow| User {
            id: r.get("id"),
            email: r.get("email"),
            password_hash: r.get("password_hash"),
            oidc_subject: r.get("oidc_subject"),
            oidc_issuer: r.get("oidc_issuer"),
            display_name: r.get("display_name"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(&mut *tx)
        .await?;

        // 4. Create initial organization
        let org = sqlx::query(
            r#"
            INSERT INTO organizations (name, slug)
            VALUES ($1, $2)
            RETURNING id, name, slug, created_at, updated_at
            "#,
        )
        .bind(org_name)
        .bind(org_slug)
        .map(|r: PgRow| Organization {
            id: r.get("id"),
            name: r.get("name"),
            slug: r.get("slug"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .fetch_one(&mut *tx)
        .await?;

        // 5. Add user as organization owner
        sqlx::query(
            r#"
            INSERT INTO organization_members (organization_id, user_id, role)
            VALUES ($1, $2, 'owner')
            "#,
        )
        .bind(org.id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;

        // 6. Create default project
        let project = sqlx::query(
            r#"
            INSERT INTO projects (organization_id, name, slug, description)
            VALUES ($1, $2, 'default', 'Default project created during bootstrap')
            RETURNING id, organization_id, name, slug, description,
                      capture_content_default, redaction_rules, retention_days,
                      created_at, updated_at
            "#,
        )
        .bind(org.id)
        .bind(default_project_name)
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

        // 7. Create default environments (production, staging, development)
        let env_specs = [
            ("production", true),
            ("staging", false),
            ("development", false),
        ];
        let mut envs = Vec::with_capacity(3);
        for (name, is_def) in env_specs {
            let env = sqlx::query(
                r#"
                INSERT INTO environments (organization_id, project_id, name, is_default)
                VALUES ($1, $2, $3, $4)
                RETURNING id, organization_id, project_id, name, is_default, created_at
                "#,
            )
            .bind(org.id)
            .bind(project.id)
            .bind(name)
            .bind(is_def)
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

        // 8. Record audit log
        sqlx::query(
            r#"
            INSERT INTO audit_logs (
                organization_id, actor_user_id, action, resource_type,
                resource_id, ip_address, user_agent, metadata
            ) VALUES ($1, $2, 'auth.bootstrap', 'organization', $3, $4, $5, '{}'::jsonb)
            "#,
        )
        .bind(org.id)
        .bind(user.id)
        .bind(org.id.to_string())
        .bind(ip_address)
        .bind(user_agent)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok((user, org, project, envs))
    }

    pub async fn can_bootstrap(&self) -> Result<bool, DatabaseError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(self.pool)
            .await?;
        Ok(count == 0)
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

    pub async fn list_by_project(&self, project_id: Uuid) -> Result<Vec<Agent>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, name, framework, language,
                   description, metadata, last_seen_at, created_at, updated_at
            FROM agents
            WHERE project_id = $1
            ORDER BY COALESCE(last_seen_at, created_at) DESC
            "#,
        )
        .bind(project_id)
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
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        name: &str,
        framework: Option<&str>,
        language: Option<&str>,
        description: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<Agent, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO agents (organization_id, project_id, name, framework, language, description, metadata, last_seen_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, now())
            RETURNING id, organization_id, project_id, name, framework, language,
                      description, metadata, last_seen_at, created_at, updated_at
            "#,
        )
        .bind(org_id)
        .bind(project_id)
        .bind(name)
        .bind(framework)
        .bind(language)
        .bind(description)
        .bind(metadata)
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

    pub async fn get_by_id_global(&self, agent_id: Uuid) -> Result<Option<Agent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, name, framework, language,
                   description, metadata, last_seen_at, created_at, updated_at
            FROM agents
            WHERE id = $1
            "#,
        )
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

    pub async fn update(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        framework: Option<&str>,
        language: Option<&str>,
        description: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Agent, DatabaseError> {
        let existing = self
            .get_by_id(project_id, agent_id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound("Agent not found".into()))?;

        let framework_val = framework.or(existing.framework.as_deref());
        let language_val = language.or(existing.language.as_deref());
        let description_val = description.or(existing.description.as_deref());
        let metadata_val = metadata.unwrap_or(existing.metadata);

        let row = sqlx::query(
            r#"
            UPDATE agents SET
                framework = $1,
                language = $2,
                description = $3,
                metadata = $4,
                updated_at = now()
            WHERE project_id = $5 AND id = $6
            RETURNING id, organization_id, project_id, name, framework, language,
                      description, metadata, last_seen_at, created_at, updated_at
            "#,
        )
        .bind(framework_val)
        .bind(language_val)
        .bind(description_val)
        .bind(metadata_val)
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
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_versions(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
    ) -> Result<Vec<AgentVersion>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, agent_id, version, configuration, created_at
            FROM agent_versions
            WHERE project_id = $1 AND agent_id = $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .bind(agent_id)
        .map(|r: PgRow| AgentVersion {
            id: r.get("id"),
            project_id: r.get("project_id"),
            agent_id: r.get("agent_id"),
            version: r.get("version"),
            configuration: r.get("configuration"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_traces(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Trace>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1 AND agent_id = $2
            ORDER BY started_at DESC
            LIMIT $3
            "#,
        )
        .bind(project_id)
        .bind(agent_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.pool)
        .await?;

        let mut traces = Vec::with_capacity(rows.len());
        for r in &rows {
            traces.push(parse_trace_row(r)?);
        }
        Ok(traces)
    }
}

// ---------------------------------------------------------------------------
// Sessions Repository
// ---------------------------------------------------------------------------

pub struct SessionsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> SessionsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn list_by_project(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Session>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, external_session_id, user_ref, metadata,
                   started_at, last_seen_at, created_at
            FROM sessions
            WHERE project_id = $1
            ORDER BY COALESCE(last_seen_at, created_at) DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(limit.clamp(1, 100))
        .map(|r: PgRow| Session {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            external_session_id: r.get("external_session_id"),
            user_ref: r.get("user_ref"),
            metadata: r.get("metadata"),
            started_at: r.get("started_at"),
            last_seen_at: r.get("last_seen_at"),
            created_at: r.get("created_at"),
        })
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<Session>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, external_session_id, user_ref, metadata,
                   started_at, last_seen_at, created_at
            FROM sessions
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(session_id)
        .map(|r: PgRow| Session {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            external_session_id: r.get("external_session_id"),
            user_ref: r.get("user_ref"),
            metadata: r.get("metadata"),
            started_at: r.get("started_at"),
            last_seen_at: r.get("last_seen_at"),
            created_at: r.get("created_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_by_id_global(
        &self,
        session_id: Uuid,
    ) -> Result<Option<Session>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, external_session_id, user_ref, metadata,
                   started_at, last_seen_at, created_at
            FROM sessions
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .map(|r: PgRow| Session {
            id: r.get("id"),
            organization_id: r.get("organization_id"),
            project_id: r.get("project_id"),
            external_session_id: r.get("external_session_id"),
            user_ref: r.get("user_ref"),
            metadata: r.get("metadata"),
            started_at: r.get("started_at"),
            last_seen_at: r.get("last_seen_at"),
            created_at: r.get("created_at"),
        })
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list_traces(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<Trace>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1 AND session_id = $2
            ORDER BY started_at DESC
            LIMIT $3
            "#,
        )
        .bind(project_id)
        .bind(session_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.pool)
        .await?;

        let mut traces = Vec::with_capacity(rows.len());
        for r in &rows {
            traces.push(parse_trace_row(r)?);
        }
        Ok(traces)
    }
}

pub fn parse_trace_row(r: &PgRow) -> Result<Trace, DatabaseError> {
    let tid_raw: String = r.get("trace_id");
    let parsed_trace_id =
        TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

    let root_sid_raw: Option<String> = r.get("root_span_id");
    let parsed_root_span_id = match root_sid_raw {
        Some(sid) => Some(SpanId::new(sid).map_err(|e| DatabaseError::NotFound(e.to_string()))?),
        None => None,
    };

    let status_raw: String = r.get("status");
    let status = status_raw
        .parse::<TraceStatus>()
        .unwrap_or(TraceStatus::Unset);

    Ok(Trace {
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
    })
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

    /// Ingests a batch of normalized spans inside a single atomic PostgreSQL transaction.
    /// Preserves LEAST(started_at) and GREATEST(ended_at), reconciles root span metadata,
    /// upserts agents/sessions/spans/events/links/usages, and writes outbox_jobs.
    #[allow(clippy::too_many_lines)]
    pub async fn persist_batch(
        &self,
        org_id: Uuid,
        project_id: Uuid,
        env_id: Uuid,
        spans: &[NormalizedSpan],
    ) -> Result<Vec<String>, DatabaseError> {
        let mut tx = self.pool.begin().await?;

        // 1. Upsert agents & sessions
        let mut agent_ids: HashMap<String, Uuid> = HashMap::new();
        for span in spans {
            if let Some(ref agent_name) = span.agent_name {
                if !agent_ids.contains_key(agent_name) {
                    let agent_id: Uuid = sqlx::query(
                        r#"
                        INSERT INTO agents (organization_id, project_id, name, last_seen_at)
                        VALUES ($1, $2, $3, now())
                        ON CONFLICT (project_id, name) DO UPDATE SET last_seen_at = now()
                        RETURNING id
                        "#,
                    )
                    .bind(org_id)
                    .bind(project_id)
                    .bind(agent_name)
                    .map(|r: PgRow| r.get("id"))
                    .fetch_one(&mut *tx)
                    .await?;

                    agent_ids.insert(agent_name.clone(), agent_id);

                    if let Some(ref ver) = span.agent_version {
                        sqlx::query(
                            r#"
                            INSERT INTO agent_versions (project_id, agent_id, version)
                            VALUES ($1, $2, $3)
                            ON CONFLICT (agent_id, version) DO NOTHING
                            "#,
                        )
                        .bind(project_id)
                        .bind(agent_id)
                        .bind(ver)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }

            if let Some(ref session_id) = span.session_id {
                sqlx::query(
                    r#"
                    INSERT INTO sessions (organization_id, project_id, external_session_id, last_seen_at)
                    VALUES ($1, $2, $3, now())
                    ON CONFLICT (project_id, external_session_id) DO UPDATE SET last_seen_at = now()
                    "#,
                )
                .bind(org_id)
                .bind(project_id)
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
            }
        }

        // 2. Group spans by trace_id and upsert traces
        let mut trace_groups: HashMap<String, Vec<&NormalizedSpan>> = HashMap::new();
        for span in spans {
            trace_groups
                .entry(span.trace_id.as_str().to_string())
                .or_default()
                .push(span);
        }

        let mut affected_trace_ids = Vec::with_capacity(trace_groups.len());

        for (trace_id_str, t_spans) in &trace_groups {
            affected_trace_ids.push(trace_id_str.clone());

            let min_start = t_spans.iter().map(|s| s.started_at).min().unwrap();
            let max_end = t_spans.iter().filter_map(|s| s.ended_at).max();
            let root_span = t_spans.iter().find(|s| s.parent_span_id.is_none());

            if let Some(root) = root_span {
                let root_sid = root.span_id.as_str();
                let status_str = root.status.as_str();
                let agent_id = root
                    .agent_name
                    .as_ref()
                    .and_then(|name| agent_ids.get(name))
                    .copied();

                sqlx::query(
                    r#"
                    INSERT INTO traces (
                        organization_id, project_id, environment_id, trace_id,
                        root_span_id, name, status, agent_id, started_at, ended_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    ON CONFLICT (project_id, trace_id) DO UPDATE SET
                        started_at = LEAST(traces.started_at, EXCLUDED.started_at),
                        ended_at = GREATEST(traces.ended_at, EXCLUDED.ended_at),
                        root_span_id = COALESCE(EXCLUDED.root_span_id, traces.root_span_id),
                        name = COALESCE(EXCLUDED.name, traces.name),
                        status = CASE WHEN EXCLUDED.status != 'unset' THEN EXCLUDED.status ELSE traces.status END,
                        agent_id = COALESCE(EXCLUDED.agent_id, traces.agent_id),
                        updated_at = now()
                    "#,
                )
                .bind(org_id)
                .bind(project_id)
                .bind(env_id)
                .bind(trace_id_str)
                .bind(root_sid)
                .bind(&root.name)
                .bind(status_str)
                .bind(agent_id)
                .bind(min_start)
                .bind(max_end)
                .execute(&mut *tx)
                .await?;
            } else {
                let first_name = &t_spans[0].name;
                sqlx::query(
                    r#"
                    INSERT INTO traces (
                        organization_id, project_id, environment_id, trace_id,
                        name, status, started_at, ended_at
                    ) VALUES ($1, $2, $3, $4, $5, 'unset', $6, $7)
                    ON CONFLICT (project_id, trace_id) DO UPDATE SET
                        started_at = LEAST(traces.started_at, EXCLUDED.started_at),
                        ended_at = GREATEST(traces.ended_at, EXCLUDED.ended_at),
                        name = COALESCE(traces.name, EXCLUDED.name),
                        updated_at = now()
                    "#,
                )
                .bind(org_id)
                .bind(project_id)
                .bind(env_id)
                .bind(trace_id_str)
                .bind(first_name)
                .bind(min_start)
                .bind(max_end)
                .execute(&mut *tx)
                .await?;
            }
        }

        // 3. Upsert spans, events, links, token usages
        for span in spans {
            let trace_id_str = span.trace_id.as_str();
            let span_id_str = span.span_id.as_str();
            let parent_sid_str = span.parent_span_id.as_ref().map(SpanId::as_str);
            let span_type_str = span.span_type.as_str();
            let status_str = span.status.as_str();
            let agent_id = span
                .agent_name
                .as_ref()
                .and_then(|name| agent_ids.get(name))
                .copied();

            sqlx::query(
                r#"
                INSERT INTO spans (
                    organization_id, project_id, trace_id, span_id, parent_span_id,
                    agent_id, name, span_type, status, started_at, ended_at, duration_ms,
                    provider, model, tool_name, attributes, resource_attributes,
                    scope_attributes, error
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                    $13, $14, $15, $16, $17, $18, $19
                )
                ON CONFLICT (project_id, trace_id, span_id) DO UPDATE SET
                    ended_at = COALESCE(EXCLUDED.ended_at, spans.ended_at),
                    duration_ms = COALESCE(EXCLUDED.duration_ms, spans.duration_ms),
                    status = CASE WHEN EXCLUDED.status != 'unset' THEN EXCLUDED.status ELSE spans.status END,
                    error = COALESCE(EXCLUDED.error, spans.error)
                "#,
            )
            .bind(org_id)
            .bind(project_id)
            .bind(trace_id_str)
            .bind(span_id_str)
            .bind(parent_sid_str)
            .bind(agent_id)
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
            .bind(&span.error)
            .execute(&mut *tx)
            .await?;

            for ev in &span.events {
                sqlx::query(
                    r#"
                    INSERT INTO span_events (
                        project_id, trace_id, span_id, event_index, event_name, occurred_at, attributes
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT (project_id, trace_id, span_id, event_index) DO NOTHING
                    "#,
                )
                .bind(project_id)
                .bind(trace_id_str)
                .bind(span_id_str)
                .bind(ev.event_index)
                .bind(&ev.event_name)
                .bind(ev.occurred_at)
                .bind(&ev.attributes)
                .execute(&mut *tx)
                .await?;
            }

            for link in &span.links {
                sqlx::query(
                    r#"
                    INSERT INTO span_links (
                        project_id, trace_id, span_id, link_index, linked_trace_id, linked_span_id, attributes
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                    ON CONFLICT (project_id, trace_id, span_id, link_index) DO NOTHING
                    "#,
                )
                .bind(project_id)
                .bind(trace_id_str)
                .bind(span_id_str)
                .bind(link.link_index)
                .bind(&link.linked_trace_id)
                .bind(&link.linked_span_id)
                .bind(&link.attributes)
                .execute(&mut *tx)
                .await?;
            }

            if let Some(ref u) = span.usage {
                sqlx::query(
                    r#"
                    INSERT INTO token_usages (
                        organization_id, project_id, trace_id, span_id, provider, model,
                        input_tokens, output_tokens, cached_input_tokens, reasoning_tokens,
                        usage_source, recorded_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'provider_reported', $11)
                    ON CONFLICT (project_id, trace_id, span_id) DO UPDATE SET
                        input_tokens = EXCLUDED.input_tokens,
                        output_tokens = EXCLUDED.output_tokens,
                        cached_input_tokens = EXCLUDED.cached_input_tokens,
                        reasoning_tokens = EXCLUDED.reasoning_tokens
                    "#,
                )
                .bind(org_id)
                .bind(project_id)
                .bind(trace_id_str)
                .bind(span_id_str)
                .bind(&u.provider)
                .bind(&u.model)
                .bind(u.input_tokens)
                .bind(u.output_tokens)
                .bind(u.cached_input_tokens)
                .bind(u.reasoning_tokens)
                .bind(span.started_at)
                .execute(&mut *tx)
                .await?;
            }
        }

        // 4. Insert transactional outbox jobs
        for trace_id_str in &affected_trace_ids {
            let dedup = format!("trace_ingested:{project_id}:{trace_id_str}");
            let payload = serde_json::json!({
                "organization_id": org_id,
                "project_id": project_id,
                "trace_id": trace_id_str
            });

            sqlx::query(
                r#"
                INSERT INTO outbox_jobs (
                    organization_id, project_id, job_type, dedup_key, payload, status, available_at
                ) VALUES ($1, $2, 'trace_ingested', $3, $4, 'pending', now())
                ON CONFLICT (dedup_key) DO UPDATE SET
                    status = 'pending',
                    attempts = 0,
                    available_at = now(),
                    completed_at = NULL,
                    last_error = NULL
                WHERE outbox_jobs.status IN ('completed', 'dead')
                "#,
            )
            .bind(org_id)
            .bind(project_id)
            .bind(&dedup)
            .bind(payload)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(affected_trace_ids)
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
            Some(ref r) => Ok(Some(parse_trace_row(r)?)),
            None => Ok(None),
        }
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Trace>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(ref r) => Ok(Some(parse_trace_row(r)?)),
            None => Ok(None),
        }
    }

    pub async fn get_by_id_or_trace_id(
        &self,
        project_id: Uuid,
        id_or_trace_id: &str,
    ) -> Result<Option<Trace>, DatabaseError> {
        if let Ok(record_id) = Uuid::parse_str(id_or_trace_id) {
            if let Some(trace) = self.get_by_id(project_id, record_id).await? {
                return Ok(Some(trace));
            }
        }
        if let Ok(tid) = TraceId::new(id_or_trace_id.to_string()) {
            return self.get_by_trace_id(project_id, &tid).await;
        }
        Ok(None)
    }

    pub async fn get_by_id_or_trace_id_global(
        &self,
        id_or_trace_id: &str,
    ) -> Result<Option<Trace>, DatabaseError> {
        if let Ok(record_id) = Uuid::parse_str(id_or_trace_id) {
            let row = sqlx::query(
                r#"
                SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                       trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                       span_count, error_span_count, total_input_tokens, total_output_tokens,
                       total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                       has_unpriced_usage, currency, resource_attributes, attributes,
                       received_at, finalized_at, created_at, updated_at
                FROM traces
                WHERE id = $1
                "#,
            )
            .bind(record_id)
            .fetch_optional(self.pool)
            .await?;

            if let Some(ref r) = row {
                return Ok(Some(parse_trace_row(r)?));
            }
        }

        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE trace_id = $1
            LIMIT 1
            "#,
        )
        .bind(id_or_trace_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(ref r) => Ok(Some(parse_trace_row(r)?)),
            None => Ok(None),
        }
    }

    pub async fn list_traces(
        &self,
        filters: &TraceQueryFilters,
    ) -> Result<TraceQueryResult, DatabaseError> {
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let cursor_pair = filters.cursor.as_deref().and_then(decode_cursor);

        let mut qb = sqlx::QueryBuilder::new(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = 
            "#,
        );
        qb.push_bind(filters.project_id);

        if let Some(env_id) = filters.environment_id {
            qb.push(" AND environment_id = ");
            qb.push_bind(env_id);
        }
        if let Some(agent_id) = filters.agent_id {
            qb.push(" AND agent_id = ");
            qb.push_bind(agent_id);
        }
        if let Some(ref status) = filters.status {
            qb.push(" AND status = ");
            qb.push_bind(status.to_lowercase());
        }
        if let Some(min_dur) = filters.min_duration_ms {
            qb.push(" AND duration_ms >= ");
            qb.push_bind(min_dur);
        }
        if let Some(max_dur) = filters.max_duration_ms {
            qb.push(" AND duration_ms <= ");
            qb.push_bind(max_dur);
        }
        if let Some(from_dt) = filters.from {
            qb.push(" AND started_at >= ");
            qb.push_bind(from_dt);
        }
        if let Some(to_dt) = filters.to {
            qb.push(" AND started_at <= ");
            qb.push_bind(to_dt);
        }
        if let Some(ref search_tid) = filters.search_trace_id {
            let clean = search_tid.trim().to_lowercase();
            if !clean.is_empty() {
                qb.push(" AND trace_id LIKE ");
                qb.push_bind(format!("{clean}%"));
            }
        }
        if let Some((cursor_ts, cursor_id)) = cursor_pair {
            qb.push(" AND (started_at < ");
            qb.push_bind(cursor_ts);
            qb.push(" OR (started_at = ");
            qb.push_bind(cursor_ts);
            qb.push(" AND id < ");
            qb.push_bind(cursor_id);
            qb.push("))");
        }

        qb.push(" ORDER BY started_at DESC, id DESC LIMIT ");
        qb.push_bind(limit + 1);

        let rows = qb.build().fetch_all(self.pool).await?;

        let has_more = rows.len() > limit as usize;
        let items_slice = if has_more {
            &rows[..limit as usize]
        } else {
            &rows[..]
        };

        let mut items = Vec::with_capacity(items_slice.len());
        for r in items_slice {
            items.push(parse_trace_row(r)?);
        }

        let next_cursor = if has_more {
            items
                .last()
                .map(|last| encode_cursor(last.started_at, last.id))
        } else {
            None
        };

        Ok(TraceQueryResult {
            items,
            next_cursor,
            has_more,
        })
    }

    pub async fn get_project_overview(
        &self,
        project_id: Uuid,
        environment_id: Option<Uuid>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<ProjectOverviewMetrics, DatabaseError> {
        let from_ts = from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
        let to_ts = to.unwrap_or_else(Utc::now);

        let active_agents: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM agents
            WHERE project_id = $1 AND last_seen_at >= now() - interval '24 hours'
            "#,
        )
        .bind(project_id)
        .fetch_one(self.pool)
        .await
        .unwrap_or(0);

        let summary_row = sqlx::query(
            r#"
            SELECT
                COUNT(*) AS total_traces,
                COUNT(*) FILTER (WHERE status = 'error' OR error_span_count > 0) AS error_traces,
                COALESCE(SUM(total_input_tokens), 0)::BIGINT AS input_tokens,
                COALESCE(SUM(total_output_tokens), 0)::BIGINT AS output_tokens,
                COALESCE(percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms), 0.0)::FLOAT8 AS p50_duration,
                COALESCE(percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms), 0.0)::FLOAT8 AS p95_duration
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND started_at >= $3 AND started_at <= $4
            "#,
        )
        .bind(project_id)
        .bind(environment_id)
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(self.pool)
        .await?;

        let total_traces: i64 = summary_row.get("total_traces");
        let error_traces: i64 = summary_row.get("error_traces");
        let input_tokens: i64 = summary_row.get("input_tokens");
        let output_tokens: i64 = summary_row.get("output_tokens");
        let p50: f64 = summary_row.get("p50_duration");
        let p95: f64 = summary_row.get("p95_duration");

        let error_rate = if total_traces > 0 {
            (error_traces as f64) / (total_traces as f64)
        } else {
            0.0
        };

        let recent_trace_rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
            ORDER BY started_at DESC
            LIMIT 5
            "#,
        )
        .bind(project_id)
        .bind(environment_id)
        .fetch_all(self.pool)
        .await?;

        let mut recent_traces = Vec::with_capacity(recent_trace_rows.len());
        for r in &recent_trace_rows {
            recent_traces.push(parse_trace_row(r)?);
        }

        let recent_error_rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, agent_id, session_id,
                   trace_id, root_span_id, name, status, started_at, ended_at, duration_ms,
                   span_count, error_span_count, total_input_tokens, total_output_tokens,
                   total_cached_input_tokens, total_reasoning_tokens, estimated_cost,
                   has_unpriced_usage, currency, resource_attributes, attributes,
                   received_at, finalized_at, created_at, updated_at
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND (status = 'error' OR error_span_count > 0)
            ORDER BY started_at DESC
            LIMIT 5
            "#,
        )
        .bind(project_id)
        .bind(environment_id)
        .fetch_all(self.pool)
        .await?;

        let mut recent_errors = Vec::with_capacity(recent_error_rows.len());
        for r in &recent_error_rows {
            recent_errors.push(parse_trace_row(r)?);
        }

        Ok(ProjectOverviewMetrics {
            active_agents_count: active_agents,
            total_traces_count: total_traces,
            error_rate,
            total_input_tokens: input_tokens,
            total_output_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            p50_duration_ms: Some(p50),
            p95_duration_ms: Some(p95),
            recent_traces,
            recent_errors,
        })
    }

    pub async fn finalize_trace(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
    ) -> Result<Option<Trace>, DatabaseError> {
        let trace_id_str = trace_id.as_str();

        let row = sqlx::query(
            r#"
            WITH span_summary AS (
                SELECT
                    MIN(started_at) AS min_started_at,
                    MAX(ended_at) AS max_ended_at,
                    EXTRACT(EPOCH FROM (MAX(ended_at) - MIN(started_at))) * 1000.0 AS duration_ms,
                    COUNT(*) AS total_spans,
                    COUNT(*) FILTER (WHERE status = 'error') AS total_error_spans
                FROM spans
                WHERE project_id = $1 AND trace_id = $2
            ),
            usage_summary AS (
                SELECT
                    COALESCE(SUM(input_tokens), 0) AS total_input,
                    COALESCE(SUM(output_tokens), 0) AS total_output,
                    COALESCE(SUM(cached_input_tokens), 0) AS total_cached_input,
                    COALESCE(SUM(reasoning_tokens), 0) AS total_reasoning,
                    SUM(cost_estimate) AS total_cost,
                    BOOL_OR(cost_estimate IS NULL) AS has_unpriced
                FROM token_usages
                WHERE project_id = $1 AND trace_id = $2
            ),
            root_info AS (
                SELECT
                    span_id AS root_span_id,
                    name AS root_name,
                    status AS root_status,
                    agent_id AS root_agent_id
                FROM spans
                WHERE project_id = $1 AND trace_id = $2 AND parent_span_id IS NULL
                LIMIT 1
            )
            UPDATE traces
            SET started_at = COALESCE(span_summary.min_started_at, traces.started_at),
                ended_at = COALESCE(span_summary.max_ended_at, traces.ended_at),
                duration_ms = CASE
                    WHEN span_summary.duration_ms IS NOT NULL AND span_summary.duration_ms >= 0
                    THEN span_summary.duration_ms::BIGINT
                    ELSE traces.duration_ms
                END,
                span_count = COALESCE(span_summary.total_spans, 0),
                error_span_count = COALESCE(span_summary.total_error_spans, 0),
                total_input_tokens = COALESCE(usage_summary.total_input, 0),
                total_output_tokens = COALESCE(usage_summary.total_output, 0),
                total_cached_input_tokens = COALESCE(usage_summary.total_cached_input, 0),
                total_reasoning_tokens = COALESCE(usage_summary.total_reasoning, 0),
                estimated_cost = usage_summary.total_cost,
                has_unpriced_usage = COALESCE(usage_summary.has_unpriced, FALSE),
                root_span_id = COALESCE(root_info.root_span_id, traces.root_span_id),
                name = COALESCE(root_info.root_name, traces.name),
                agent_id = COALESCE(root_info.root_agent_id, traces.agent_id),
                status = CASE
                    WHEN root_info.root_status IS NOT NULL AND root_info.root_status <> 'unset' THEN root_info.root_status
                    WHEN span_summary.total_error_spans > 0 THEN 'error'
                    ELSE traces.status
                END,
                finalized_at = now(),
                updated_at = now()
            FROM span_summary, usage_summary
            LEFT JOIN root_info ON TRUE
            WHERE traces.project_id = $1 AND traces.trace_id = $2
            RETURNING traces.id, traces.organization_id, traces.project_id, traces.environment_id,
                      traces.agent_id, traces.session_id, traces.trace_id, traces.root_span_id,
                      traces.name, traces.status, traces.started_at, traces.ended_at,
                      traces.duration_ms, traces.span_count, traces.error_span_count,
                      traces.total_input_tokens, traces.total_output_tokens,
                      traces.total_cached_input_tokens, traces.total_reasoning_tokens,
                      traces.estimated_cost, traces.has_unpriced_usage, traces.currency,
                      traces.resource_attributes, traces.attributes, traces.received_at,
                      traces.finalized_at, traces.created_at, traces.updated_at
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .fetch_optional(self.pool)
        .await?;

        let trace = match row {
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

                let t = Trace {
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
                };

                if let (Some(aid), Some(ended)) = (t.agent_id, t.ended_at) {
                    let _ = sqlx::query(
                        r#"
                        UPDATE agents
                        SET last_seen_at = GREATEST(COALESCE(last_seen_at, $2), $2),
                            updated_at = now()
                        WHERE project_id = $1 AND id = $3
                        "#,
                    )
                    .bind(t.project_id)
                    .bind(ended)
                    .bind(aid)
                    .execute(self.pool)
                    .await;
                }

                if let (Some(sid), Some(ended)) = (t.session_id, t.ended_at) {
                    let _ = sqlx::query(
                        r#"
                        UPDATE sessions
                        SET last_seen_at = GREATEST(COALESCE(last_seen_at, $2), $2)
                        WHERE project_id = $1 AND id = $3
                        "#,
                    )
                    .bind(t.project_id)
                    .bind(ended)
                    .bind(sid)
                    .execute(self.pool)
                    .await;
                }

                Some(t)
            }
            None => None,
        };

        Ok(trace)
    }
}

pub fn parse_span_row(r: &PgRow) -> Result<Span, DatabaseError> {
    let tid_raw: String = r.get("trace_id");
    let parsed_trace_id =
        TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

    let sid_raw: String = r.get("span_id");
    let parsed_span_id =
        SpanId::new(sid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

    let parent_sid_raw: Option<String> = r.get("parent_span_id");
    let parsed_parent_span_id = match parent_sid_raw {
        Some(sid) => Some(SpanId::new(sid).map_err(|e| DatabaseError::NotFound(e.to_string()))?),
        None => None,
    };

    let span_type_raw: String = r.get("span_type");
    let span_type = span_type_raw
        .parse::<SpanType>()
        .unwrap_or(SpanType::Custom);

    let status_raw: String = r.get("status");
    let status = status_raw
        .parse::<TraceStatus>()
        .unwrap_or(TraceStatus::Unset);

    Ok(Span {
        id: r.get("id"),
        organization_id: r.get("organization_id"),
        project_id: r.get("project_id"),
        trace_id: parsed_trace_id,
        span_id: parsed_span_id,
        parent_span_id: parsed_parent_span_id,
        agent_id: r.get("agent_id"),
        name: r.get("name"),
        span_type,
        status,
        started_at: r.get("started_at"),
        ended_at: r.get("ended_at"),
        duration_ms: r.get("duration_ms"),
        provider: r.get("provider"),
        model: r.get("model"),
        tool_name: r.get("tool_name"),
        attributes: r.get("attributes"),
        resource_attributes: r.get("resource_attributes"),
        scope_attributes: r.get("scope_attributes"),
        payload_s3_key: r.get("payload_s3_key"),
        error: r.get("error"),
        received_at: r.get("received_at"),
        created_at: r.get("created_at"),
    })
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

    pub async fn list_for_trace(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
    ) -> Result<Vec<Span>, DatabaseError> {
        let trace_id_str = trace_id.as_str();
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, trace_id, span_id, parent_span_id,
                   agent_id, name, span_type, status, started_at, ended_at, duration_ms,
                   provider, model, tool_name, attributes, resource_attributes,
                   scope_attributes, payload_s3_key, error, received_at, created_at
            FROM spans
            WHERE project_id = $1 AND trace_id = $2
            ORDER BY started_at ASC
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .fetch_all(self.pool)
        .await?;

        rows.iter().map(parse_span_row).collect()
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Span>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, trace_id, span_id, parent_span_id,
                   agent_id, name, span_type, status, started_at, ended_at, duration_ms,
                   provider, model, tool_name, attributes, resource_attributes,
                   scope_attributes, payload_s3_key, error, received_at, created_at
            FROM spans
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await?;

        row.as_ref().map(parse_span_row).transpose()
    }

    pub async fn list_events_for_span(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
        span_id: &SpanId,
    ) -> Result<Vec<SpanEvent>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, trace_id, span_id, event_index, event_name,
                   occurred_at, attributes, created_at
            FROM span_events
            WHERE project_id = $1 AND trace_id = $2 AND span_id = $3
            ORDER BY event_index ASC
            "#,
        )
        .bind(project_id)
        .bind(trace_id.as_str())
        .bind(span_id.as_str())
        .fetch_all(self.pool)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        for r in rows {
            let tid_raw: String = r.get("trace_id");
            let tid = TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
            let sid_raw: String = r.get("span_id");
            let sid = SpanId::new(sid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
            events.push(SpanEvent {
                id: r.get("id"),
                project_id: r.get("project_id"),
                trace_id: tid,
                span_id: sid,
                event_index: r.get("event_index"),
                event_name: r.get("event_name"),
                occurred_at: r.get("occurred_at"),
                attributes: r.get("attributes"),
                created_at: r.get("created_at"),
            });
        }
        Ok(events)
    }

    pub async fn list_links_for_span(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
        span_id: &SpanId,
    ) -> Result<Vec<SpanLink>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, trace_id, span_id, link_index, linked_trace_id,
                   linked_span_id, attributes, created_at
            FROM span_links
            WHERE project_id = $1 AND trace_id = $2 AND span_id = $3
            ORDER BY link_index ASC
            "#,
        )
        .bind(project_id)
        .bind(trace_id.as_str())
        .bind(span_id.as_str())
        .fetch_all(self.pool)
        .await?;

        let mut links = Vec::with_capacity(rows.len());
        for r in rows {
            let tid_raw: String = r.get("trace_id");
            let tid = TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
            let sid_raw: String = r.get("span_id");
            let sid = SpanId::new(sid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
            links.push(SpanLink {
                id: r.get("id"),
                project_id: r.get("project_id"),
                trace_id: tid,
                span_id: sid,
                link_index: r.get("link_index"),
                linked_trace_id: r.get("linked_trace_id"),
                linked_span_id: r.get("linked_span_id"),
                attributes: r.get("attributes"),
                created_at: r.get("created_at"),
            });
        }
        Ok(links)
    }

    pub async fn get_usage_for_span(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
        span_id: &SpanId,
    ) -> Result<Option<TokenUsage>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, trace_id, span_id, provider, model,
                   input_tokens, output_tokens, cached_input_tokens, reasoning_tokens,
                   cost_estimate, currency, pricing_version, input_rate_per_million,
                   output_rate_per_million, cached_rate_per_million, usage_source,
                   recorded_at, created_at
            FROM token_usages
            WHERE project_id = $1 AND trace_id = $2 AND span_id = $3
            "#,
        )
        .bind(project_id)
        .bind(trace_id.as_str())
        .bind(span_id.as_str())
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let tid_raw: String = r.get("trace_id");
                let tid =
                    TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
                let sid_raw: String = r.get("span_id");
                let sid =
                    SpanId::new(sid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;
                let usage_src_raw: String = r.get("usage_source");
                let usage_source = usage_src_raw
                    .parse::<UsageSource>()
                    .unwrap_or(UsageSource::Estimated);

                Ok(Some(TokenUsage {
                    id: r.get("id"),
                    organization_id: r.get("organization_id"),
                    project_id: r.get("project_id"),
                    trace_id: tid,
                    span_id: sid,
                    provider: r.get("provider"),
                    model: r.get("model"),
                    input_tokens: r.get("input_tokens"),
                    output_tokens: r.get("output_tokens"),
                    cached_input_tokens: r.get("cached_input_tokens"),
                    reasoning_tokens: r.get("reasoning_tokens"),
                    cost_estimate: r.get("cost_estimate"),
                    currency: r.get("currency"),
                    pricing_version: r.get("pricing_version"),
                    input_rate_per_million: r.get("input_rate_per_million"),
                    output_rate_per_million: r.get("output_rate_per_million"),
                    cached_rate_per_million: r.get("cached_rate_per_million"),
                    usage_source,
                    recorded_at: r.get("recorded_at"),
                    created_at: r.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_span_detail(
        &self,
        project_id: Uuid,
        id: Uuid,
    ) -> Result<Option<SpanDetail>, DatabaseError> {
        let span = match self.get_by_id(project_id, id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let events = self
            .list_events_for_span(project_id, &span.trace_id, &span.span_id)
            .await?;
        let links = self
            .list_links_for_span(project_id, &span.trace_id, &span.span_id)
            .await?;
        let token_usage = self
            .get_usage_for_span(project_id, &span.trace_id, &span.span_id)
            .await?;

        Ok(Some(SpanDetail {
            span,
            events,
            links,
            token_usage,
        }))
    }

    pub async fn get_by_id_global(&self, id: Uuid) -> Result<Option<Span>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, trace_id, span_id, parent_span_id,
                   agent_id, name, span_type, status, started_at, ended_at, duration_ms,
                   provider, model, tool_name, attributes, resource_attributes,
                   scope_attributes, payload_s3_key, error, received_at, created_at
            FROM spans
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?;

        row.as_ref().map(parse_span_row).transpose()
    }

    pub async fn get_span_detail_global(
        &self,
        id: Uuid,
    ) -> Result<Option<SpanDetail>, DatabaseError> {
        let span = match self.get_by_id_global(id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let events = self
            .list_events_for_span(span.project_id, &span.trace_id, &span.span_id)
            .await?;
        let links = self
            .list_links_for_span(span.project_id, &span.trace_id, &span.span_id)
            .await?;
        let token_usage = self
            .get_usage_for_span(span.project_id, &span.trace_id, &span.span_id)
            .await?;

        Ok(Some(SpanDetail {
            span,
            events,
            links,
            token_usage,
        }))
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

    pub async fn list_by_trace(
        &self,
        project_id: Uuid,
        trace_id: &TraceId,
    ) -> Result<Vec<TokenUsage>, DatabaseError> {
        let trace_id_str = trace_id.as_str();
        let rows = sqlx::query(
            r#"
            SELECT
                id, organization_id, project_id, trace_id, span_id, provider, model,
                input_tokens, output_tokens, cached_input_tokens, reasoning_tokens,
                cost_estimate, currency, pricing_version, input_rate_per_million,
                output_rate_per_million, cached_rate_per_million, usage_source,
                recorded_at, created_at
            FROM token_usages
            WHERE project_id = $1 AND trace_id = $2
            ORDER BY recorded_at ASC
            "#,
        )
        .bind(project_id)
        .bind(trace_id_str)
        .fetch_all(self.pool)
        .await?;

        let mut usages = Vec::with_capacity(rows.len());
        for r in rows {
            let tid: String = r.get("trace_id");
            let sid: String = r.get("span_id");
            let source_str: String = r.get("usage_source");
            usages.push(TokenUsage {
                id: r.get("id"),
                organization_id: r.get("organization_id"),
                project_id: r.get("project_id"),
                trace_id: TraceId::new(tid).map_err(|e| DatabaseError::NotFound(e.to_string()))?,
                span_id: SpanId::new(sid).map_err(|e| DatabaseError::NotFound(e.to_string()))?,
                provider: r.get("provider"),
                model: r.get("model"),
                input_tokens: r.get("input_tokens"),
                output_tokens: r.get("output_tokens"),
                cached_input_tokens: r.get("cached_input_tokens"),
                reasoning_tokens: r.get("reasoning_tokens"),
                cost_estimate: r.get("cost_estimate"),
                currency: r.get("currency"),
                pricing_version: r.get("pricing_version"),
                input_rate_per_million: r.get("input_rate_per_million"),
                output_rate_per_million: r.get("output_rate_per_million"),
                cached_rate_per_million: r.get("cached_rate_per_million"),
                usage_source: UsageSource::from_str(&source_str)
                    .map_err(|e| DatabaseError::NotFound(e.to_string()))?,
                recorded_at: r.get("recorded_at"),
                created_at: r.get("created_at"),
            });
        }
        Ok(usages)
    }

    pub async fn update_pricing_evaluation(
        &self,
        id: Uuid,
        cost_estimate: Option<Decimal>,
        pricing_version: Option<&str>,
        input_rate: Option<Decimal>,
        output_rate: Option<Decimal>,
        cached_rate: Option<Decimal>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE token_usages
            SET cost_estimate = $1,
                pricing_version = $2,
                input_rate_per_million = $3,
                output_rate_per_million = $4,
                cached_rate_per_million = $5
            WHERE id = $6
            "#,
        )
        .bind(cost_estimate)
        .bind(pricing_version)
        .bind(input_rate)
        .bind(output_rate)
        .bind(cached_rate)
        .bind(id)
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
        project_id: Option<Uuid>,
    ) -> Result<Vec<OutboxJob>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            WITH claimable AS (
                SELECT id
                FROM outbox_jobs
                WHERE ((status = 'pending' AND available_at <= now())
                   OR (status = 'processing' AND lease_expires_at <= now()))
                  AND ($4::UUID IS NULL OR project_id = $4)
                ORDER BY available_at ASC, id ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE outbox_jobs
            SET status = 'processing',
                locked_by = $2,
                locked_at = now(),
                lease_expires_at = now() + ($3 * interval '1 second'),
                attempts = attempts + 1
            FROM claimable
            WHERE outbox_jobs.id = claimable.id
            RETURNING outbox_jobs.id, outbox_jobs.organization_id, outbox_jobs.project_id,
                      outbox_jobs.job_type, outbox_jobs.dedup_key, outbox_jobs.payload,
                      outbox_jobs.status, outbox_jobs.attempts, outbox_jobs.max_attempts,
                      outbox_jobs.available_at, outbox_jobs.lease_expires_at,
                      outbox_jobs.locked_at, outbox_jobs.locked_by, outbox_jobs.last_error,
                      outbox_jobs.created_at, outbox_jobs.completed_at
            "#,
        )
        .bind(limit)
        .bind(worker_id)
        .bind(f64::from(lease_seconds))
        .bind(project_id)
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

    pub async fn mark_completed(&self, job_id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE outbox_jobs
            SET status = 'completed',
                completed_at = now(),
                locked_by = NULL,
                lease_expires_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_failed(
        &self,
        job_id: Uuid,
        attempts: i32,
        max_attempts: i32,
        last_error: &str,
        backoff_seconds: i64,
    ) -> Result<(), DatabaseError> {
        if attempts >= max_attempts {
            sqlx::query(
                r#"
                UPDATE outbox_jobs
                SET status = 'dead',
                    last_error = $2,
                    completed_at = now(),
                    locked_by = NULL,
                    lease_expires_at = NULL
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(last_error)
            .execute(self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE outbox_jobs
                SET status = 'pending',
                    last_error = $2,
                    locked_by = NULL,
                    locked_at = NULL,
                    lease_expires_at = NULL,
                    available_at = now() + ($3 * interval '1 second')
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(last_error)
            .bind(backoff_seconds as f64)
            .execute(self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn get_by_id(&self, job_id: Uuid) -> Result<Option<OutboxJob>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, job_type, dedup_key,
                   payload, status, attempts, max_attempts, available_at,
                   lease_expires_at, locked_at, locked_by, last_error,
                   created_at, completed_at
            FROM outbox_jobs
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(|r: PgRow| OutboxJob {
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
        }))
    }

    pub async fn get_pending_count(&self, project_id: Option<Uuid>) -> Result<i64, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM outbox_jobs
            WHERE status = 'pending' AND ($1::UUID IS NULL OR project_id = $1)
            "#,
        )
        .bind(project_id)
        .map(|r: PgRow| r.get::<i64, _>("count"))
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_oldest_pending_age_seconds(
        &self,
        project_id: Option<Uuid>,
    ) -> Result<f64, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT (COALESCE(EXTRACT(EPOCH FROM (now() - MIN(created_at))), 0.0))::FLOAT8 AS age
            FROM outbox_jobs
            WHERE status = 'pending' AND ($1::UUID IS NULL OR project_id = $1)
            "#,
        )
        .bind(project_id)
        .map(|r: PgRow| r.get::<f64, _>("age"))
        .fetch_one(self.pool)
        .await?;

        Ok(row)
    }
}

// ---------------------------------------------------------------------------
// Model Pricing Repository
// ---------------------------------------------------------------------------

pub struct ModelPricingRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ModelPricingRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<ModelPricing>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, provider, model_pattern, match_type, currency,
                input_per_million, output_per_million, cached_input_per_million,
                effective_from, effective_until, source_url, source_checked_at,
                version, created_at
            FROM model_pricing
            ORDER BY provider ASC, model_pattern ASC
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| ModelPricing {
                id: r.get("id"),
                provider: r.get("provider"),
                model_pattern: r.get("model_pattern"),
                match_type: r.get("match_type"),
                currency: r.get("currency"),
                input_per_million: r.get("input_per_million"),
                output_per_million: r.get("output_per_million"),
                cached_input_per_million: r.get("cached_input_per_million"),
                effective_from: r.get("effective_from"),
                effective_until: r.get("effective_until"),
                source_url: r.get("source_url"),
                source_checked_at: r.get("source_checked_at"),
                version: r.get("version"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    pub async fn create(&self, card: &ModelPricing) -> Result<ModelPricing, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO model_pricing (
                id, provider, model_pattern, match_type, currency,
                input_per_million, output_per_million, cached_input_per_million,
                effective_from, effective_until, source_url, source_checked_at,
                version
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            )
            RETURNING id, provider, model_pattern, match_type, currency,
                      input_per_million, output_per_million, cached_input_per_million,
                      effective_from, effective_until, source_url, source_checked_at,
                      version, created_at
            "#,
        )
        .bind(card.id)
        .bind(&card.provider)
        .bind(&card.model_pattern)
        .bind(&card.match_type)
        .bind(&card.currency)
        .bind(card.input_per_million)
        .bind(card.output_per_million)
        .bind(card.cached_input_per_million)
        .bind(card.effective_from)
        .bind(card.effective_until)
        .bind(&card.source_url)
        .bind(card.source_checked_at)
        .bind(&card.version)
        .fetch_one(self.pool)
        .await?;

        Ok(ModelPricing {
            id: row.get("id"),
            provider: row.get("provider"),
            model_pattern: row.get("model_pattern"),
            match_type: row.get("match_type"),
            currency: row.get("currency"),
            input_per_million: row.get("input_per_million"),
            output_per_million: row.get("output_per_million"),
            cached_input_per_million: row.get("cached_input_per_million"),
            effective_from: row.get("effective_from"),
            effective_until: row.get("effective_until"),
            source_url: row.get("source_url"),
            source_checked_at: row.get("source_checked_at"),
            version: row.get("version"),
            created_at: row.get("created_at"),
        })
    }
}

// ---------------------------------------------------------------------------
// Daily Metrics Repository
// ---------------------------------------------------------------------------

pub struct DailyMetricsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> DailyMetricsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_trace_metric(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        environment_id: Uuid,
        metric_date: chrono::NaiveDate,
        duration_ms: Option<i64>,
        cost: Option<Decimal>,
        input_tokens: i64,
        output_tokens: i64,
        is_error: bool,
    ) -> Result<(), DatabaseError> {
        let dur_dec = duration_ms.map(Decimal::from);
        sqlx::query(
            r#"
            INSERT INTO daily_metrics (
                organization_id, project_id, environment_id, metric_date,
                dimension_type, dimension_key, metric_name, count_value,
                sum_value, min_value, max_value, estimated_cost, updated_at
            ) VALUES (
                $1, $2, $3, $4, 'overview', 'total', 'traces', 1,
                COALESCE($5, 0), $5, $5, $6, now()
            )
            ON CONFLICT (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name)
            DO UPDATE SET
                count_value = daily_metrics.count_value + 1,
                sum_value = daily_metrics.sum_value + COALESCE(EXCLUDED.sum_value, 0),
                min_value = LEAST(daily_metrics.min_value, EXCLUDED.min_value),
                max_value = GREATEST(daily_metrics.max_value, EXCLUDED.max_value),
                estimated_cost = CASE
                    WHEN daily_metrics.estimated_cost IS NULL AND EXCLUDED.estimated_cost IS NULL THEN NULL
                    ELSE COALESCE(daily_metrics.estimated_cost, 0) + COALESCE(EXCLUDED.estimated_cost, 0)
                END,
                updated_at = now()
            "#,
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(environment_id)
        .bind(metric_date)
        .bind(dur_dec)
        .bind(cost)
        .execute(self.pool)
        .await?;

        if is_error {
            sqlx::query(
                r#"
                INSERT INTO daily_metrics (
                    organization_id, project_id, environment_id, metric_date,
                    dimension_type, dimension_key, metric_name, count_value, updated_at
                ) VALUES (
                    $1, $2, $3, $4, 'overview', 'total', 'errors', 1, now()
                )
                ON CONFLICT (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name)
                DO UPDATE SET
                    count_value = daily_metrics.count_value + 1,
                    updated_at = now()
                "#,
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(environment_id)
            .bind(metric_date)
            .execute(self.pool)
            .await?;
        }

        if input_tokens > 0 {
            sqlx::query(
                r#"
                INSERT INTO daily_metrics (
                    organization_id, project_id, environment_id, metric_date,
                    dimension_type, dimension_key, metric_name, sum_value, updated_at
                ) VALUES (
                    $1, $2, $3, $4, 'overview', 'total', 'input_tokens', $5, now()
                )
                ON CONFLICT (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name)
                DO UPDATE SET
                    sum_value = daily_metrics.sum_value + EXCLUDED.sum_value,
                    updated_at = now()
                "#,
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(environment_id)
            .bind(metric_date)
            .bind(Decimal::from(input_tokens))
            .execute(self.pool)
            .await?;
        }

        if output_tokens > 0 {
            sqlx::query(
                r#"
                INSERT INTO daily_metrics (
                    organization_id, project_id, environment_id, metric_date,
                    dimension_type, dimension_key, metric_name, sum_value, updated_at
                ) VALUES (
                    $1, $2, $3, $4, 'overview', 'total', 'output_tokens', $5, now()
                )
                ON CONFLICT (project_id, environment_id, metric_date, dimension_type, dimension_key, metric_name)
                DO UPDATE SET
                    sum_value = daily_metrics.sum_value + EXCLUDED.sum_value,
                    updated_at = now()
                "#,
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(environment_id)
            .bind(metric_date)
            .bind(Decimal::from(output_tokens))
            .execute(self.pool)
            .await?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Analytics Repository
// ---------------------------------------------------------------------------

pub struct AnalyticsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> AnalyticsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn get_overview(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<AnalyticsOverview, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::BIGINT AS total_traces,
                COUNT(*) FILTER (WHERE status = 'error')::BIGINT AS total_errors,
                COALESCE(SUM(total_input_tokens), 0)::BIGINT AS total_input_tokens,
                COALESCE(SUM(total_output_tokens), 0)::BIGINT AS total_output_tokens,
                COALESCE(SUM(total_cached_input_tokens), 0)::BIGINT AS total_cached_tokens,
                COALESCE(SUM(total_reasoning_tokens), 0)::BIGINT AS total_reasoning_tokens,
                SUM(estimated_cost) AS total_cost,
                BOOL_OR(has_unpriced_usage) AS has_unpriced_usage,
                percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p50_duration_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p95_duration_ms,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p99_duration_ms
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR started_at <= $4)
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_one(self.pool)
        .await?;

        let total_traces: i64 = row.get("total_traces");
        let total_errors: i64 = row.get("total_errors");
        let total_input_tokens: i64 = row.get("total_input_tokens");
        let total_output_tokens: i64 = row.get("total_output_tokens");
        let total_cached_tokens: i64 = row.get("total_cached_tokens");
        let total_reasoning_tokens: i64 = row.get("total_reasoning_tokens");
        let total_cost: Option<Decimal> = row.get("total_cost");
        let has_unpriced_usage: Option<bool> = row.get("has_unpriced_usage");
        let p50_duration_ms: Option<f64> = row.get("p50_duration_ms");
        let p95_duration_ms: Option<f64> = row.get("p95_duration_ms");
        let p99_duration_ms: Option<f64> = row.get("p99_duration_ms");

        let total_tokens = total_input_tokens + total_output_tokens;
        let error_rate = if total_traces > 0 {
            total_errors as f64 / total_traces as f64
        } else {
            0.0
        };

        Ok(AnalyticsOverview {
            total_cost,
            has_unpriced_usage: has_unpriced_usage.unwrap_or(false),
            total_tokens,
            total_input_tokens,
            total_output_tokens,
            total_cached_tokens,
            total_reasoning_tokens,
            total_traces,
            total_errors,
            error_rate,
            p50_duration_ms,
            p95_duration_ms,
            p99_duration_ms,
        })
    }

    pub async fn get_tokens(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<TokenAnalyticsResult, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                date_trunc('day', recorded_at) AS bucket_time,
                COALESCE(SUM(input_tokens), 0)::BIGINT AS input_tokens,
                COALESCE(SUM(output_tokens), 0)::BIGINT AS output_tokens,
                COALESCE(SUM(cached_input_tokens), 0)::BIGINT AS cached_tokens,
                COALESCE(SUM(reasoning_tokens), 0)::BIGINT AS reasoning_tokens
            FROM token_usages
            WHERE project_id = $1
              AND ($2::TIMESTAMPTZ IS NULL OR recorded_at >= $2)
              AND ($3::TIMESTAMPTZ IS NULL OR recorded_at <= $3)
            GROUP BY bucket_time
            ORDER BY bucket_time ASC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        let mut time_series = Vec::with_capacity(rows.len());
        let mut total_input = 0i64;
        let mut total_output = 0i64;
        let mut total_cached = 0i64;
        let mut total_reasoning = 0i64;

        for r in rows {
            let timestamp: DateTime<Utc> = r.get("bucket_time");
            let input_tokens: i64 = r.get("input_tokens");
            let output_tokens: i64 = r.get("output_tokens");
            let cached_tokens: i64 = r.get("cached_tokens");
            let reasoning_tokens: i64 = r.get("reasoning_tokens");
            let total = input_tokens + output_tokens;

            total_input += input_tokens;
            total_output += output_tokens;
            total_cached += cached_tokens;
            total_reasoning += reasoning_tokens;

            time_series.push(TokenUsageTimePoint {
                timestamp,
                input_tokens,
                output_tokens,
                cached_tokens,
                reasoning_tokens,
                total_tokens: total,
            });
        }

        let total_tokens = total_input + total_output;

        Ok(TokenAnalyticsResult {
            time_series,
            total_input_tokens: total_input,
            total_output_tokens: total_output,
            total_cached_tokens: total_cached,
            total_reasoning_tokens: total_reasoning,
            total_tokens,
        })
    }

    pub async fn get_costs(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<CostAnalyticsResult, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                TO_CHAR(started_at, 'YYYY-MM-DD') AS bucket_date,
                SUM(estimated_cost) AS daily_cost,
                COUNT(*) FILTER (WHERE has_unpriced_usage = TRUE)::BIGINT AS unpriced_traces_count
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR started_at <= $4)
            GROUP BY bucket_date
            ORDER BY bucket_date ASC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        let mut time_series = Vec::with_capacity(rows.len());
        let mut total_cost: Option<Decimal> = None;
        let mut total_unpriced_count = 0i64;

        for r in rows {
            let date: String = r.get("bucket_date");
            let daily_cost: Option<Decimal> = r.get("daily_cost");
            let unpriced_count: i64 = r.get("unpriced_traces_count");

            if let Some(c) = daily_cost {
                total_cost = Some(total_cost.unwrap_or(Decimal::ZERO) + c);
            }
            total_unpriced_count += unpriced_count;

            time_series.push(CostTimePoint {
                date,
                cost: daily_cost,
                currency: "USD".to_string(),
                unpriced_traces_count: unpriced_count,
            });
        }

        Ok(CostAnalyticsResult {
            time_series,
            total_estimated_cost: total_cost,
            currency: "USD".to_string(),
            unpriced_usage_count: total_unpriced_count,
            has_unpriced_usage: total_unpriced_count > 0,
        })
    }

    pub async fn get_models(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<ModelAnalyticsItem>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                provider,
                model,
                COUNT(*)::BIGINT AS call_count,
                COALESCE(SUM(input_tokens), 0)::BIGINT AS total_input_tokens,
                COALESCE(SUM(output_tokens), 0)::BIGINT AS total_output_tokens,
                COALESCE(SUM(cached_input_tokens), 0)::BIGINT AS total_cached_tokens,
                COALESCE(SUM(reasoning_tokens), 0)::BIGINT AS total_reasoning_tokens,
                SUM(cost_estimate) AS estimated_cost,
                COALESCE(MAX(currency), 'USD') AS currency,
                BOOL_OR(cost_estimate IS NULL) AS has_unpriced
            FROM token_usages
            WHERE project_id = $1
              AND ($2::TIMESTAMPTZ IS NULL OR recorded_at >= $2)
              AND ($3::TIMESTAMPTZ IS NULL OR recorded_at <= $3)
            GROUP BY provider, model
            ORDER BY (COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)) DESC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let input: i64 = r.get("total_input_tokens");
                let output: i64 = r.get("total_output_tokens");
                ModelAnalyticsItem {
                    provider: r.get("provider"),
                    model: r.get("model"),
                    call_count: r.get("call_count"),
                    total_input_tokens: input,
                    total_output_tokens: output,
                    total_cached_tokens: r.get("total_cached_tokens"),
                    total_reasoning_tokens: r.get("total_reasoning_tokens"),
                    total_tokens: input + output,
                    estimated_cost: r.get("estimated_cost"),
                    currency: r.get("currency"),
                    has_unpriced: r.get::<Option<bool>, _>("has_unpriced").unwrap_or(false),
                }
            })
            .collect())
    }

    pub async fn get_agents(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<AgentAnalyticsItem>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                t.agent_id,
                COALESCE(a.name, 'Unassigned') AS agent_name,
                COUNT(*)::BIGINT AS trace_count,
                COUNT(*) FILTER (WHERE t.status = 'error')::BIGINT AS error_count,
                COALESCE(SUM(t.total_input_tokens + t.total_output_tokens), 0)::BIGINT AS total_tokens,
                SUM(t.estimated_cost) AS estimated_cost,
                percentile_cont(0.50) WITHIN GROUP (ORDER BY t.duration_ms)::FLOAT8 AS p50_duration_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY t.duration_ms)::FLOAT8 AS p95_duration_ms
            FROM traces t
            LEFT JOIN agents a ON a.id = t.agent_id AND a.project_id = t.project_id
            WHERE t.project_id = $1
              AND ($2::UUID IS NULL OR t.environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR t.started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR t.started_at <= $4)
            GROUP BY t.agent_id, a.name
            ORDER BY trace_count DESC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let trace_count: i64 = r.get("trace_count");
                let error_count: i64 = r.get("error_count");
                let error_rate = if trace_count > 0 {
                    error_count as f64 / trace_count as f64
                } else {
                    0.0
                };
                AgentAnalyticsItem {
                    agent_id: r.get("agent_id"),
                    agent_name: r.get("agent_name"),
                    trace_count,
                    error_count,
                    error_rate,
                    total_tokens: r.get("total_tokens"),
                    estimated_cost: r.get("estimated_cost"),
                    p50_duration_ms: r.get("p50_duration_ms"),
                    p95_duration_ms: r.get("p95_duration_ms"),
                }
            })
            .collect())
    }

    pub async fn get_tools(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<Vec<ToolAnalyticsItem>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT
                name AS tool_name,
                COUNT(*)::BIGINT AS call_count,
                COUNT(*) FILTER (WHERE status = 'error')::BIGINT AS error_count,
                percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p50_duration_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p95_duration_ms,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p99_duration_ms
            FROM spans
            WHERE project_id = $1
              AND span_type = 'tool'
              AND ($2::TIMESTAMPTZ IS NULL OR started_at >= $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at <= $3)
            GROUP BY name
            ORDER BY call_count DESC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let call_count: i64 = r.get("call_count");
                let error_count: i64 = r.get("error_count");
                let error_rate = if call_count > 0 {
                    error_count as f64 / call_count as f64
                } else {
                    0.0
                };
                ToolAnalyticsItem {
                    tool_name: r.get("tool_name"),
                    call_count,
                    error_count,
                    error_rate,
                    p50_duration_ms: r.get("p50_duration_ms"),
                    p95_duration_ms: r.get("p95_duration_ms"),
                    p99_duration_ms: r.get("p99_duration_ms"),
                }
            })
            .collect())
    }

    pub async fn get_latency(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<LatencyAnalyticsResult, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT
                percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p50_duration_ms,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p95_duration_ms,
                percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms)::FLOAT8 AS p99_duration_ms,
                MIN(duration_ms)::FLOAT8 AS min_duration_ms,
                MAX(duration_ms)::FLOAT8 AS max_duration_ms,
                AVG(duration_ms)::FLOAT8 AS avg_duration_ms,
                COUNT(*)::BIGINT AS total_count
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR started_at <= $4)
              AND duration_ms IS NOT NULL
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_one(self.pool)
        .await?;

        Ok(LatencyAnalyticsResult {
            p50_duration_ms: row.get("p50_duration_ms"),
            p95_duration_ms: row.get("p95_duration_ms"),
            p99_duration_ms: row.get("p99_duration_ms"),
            min_duration_ms: row.get("min_duration_ms"),
            max_duration_ms: row.get("max_duration_ms"),
            avg_duration_ms: row.get("avg_duration_ms"),
            total_count: row.get("total_count"),
        })
    }

    pub async fn get_errors(
        &self,
        filters: &AnalyticsFilters,
    ) -> Result<ErrorAnalyticsResult, DatabaseError> {
        let overview_row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::BIGINT AS total_traces,
                COUNT(*) FILTER (WHERE status = 'error')::BIGINT AS total_errors
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR started_at <= $4)
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_one(self.pool)
        .await?;

        let total_traces: i64 = overview_row.get("total_traces");
        let total_errors: i64 = overview_row.get("total_errors");
        let error_rate = if total_traces > 0 {
            total_errors as f64 / total_traces as f64
        } else {
            0.0
        };

        let time_series_rows = sqlx::query(
            r#"
            SELECT
                date_trunc('day', started_at) AS bucket_time,
                COUNT(*) FILTER (WHERE status = 'error')::BIGINT AS error_count,
                COUNT(*)::BIGINT AS total_count
            FROM traces
            WHERE project_id = $1
              AND ($2::UUID IS NULL OR environment_id = $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at >= $3)
              AND ($4::TIMESTAMPTZ IS NULL OR started_at <= $4)
            GROUP BY bucket_time
            ORDER BY bucket_time ASC
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.environment_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        let time_series = time_series_rows
            .into_iter()
            .map(|r| {
                let timestamp: DateTime<Utc> = r.get("bucket_time");
                let errs: i64 = r.get("error_count");
                let totals: i64 = r.get("total_count");
                let rate = if totals > 0 {
                    errs as f64 / totals as f64
                } else {
                    0.0
                };
                ErrorTimePoint {
                    timestamp,
                    error_count: errs,
                    total_count: totals,
                    error_rate: rate,
                }
            })
            .collect();

        let top_errors_rows = sqlx::query(
            r#"
            SELECT
                COALESCE(NULLIF(status_message, ''), 'UnknownError') AS error_type,
                COUNT(*)::BIGINT AS error_count,
                MAX(status_message) AS sample_message,
                MAX(started_at) AS last_seen_at
            FROM spans
            WHERE project_id = $1
              AND status = 'error'
              AND ($2::TIMESTAMPTZ IS NULL OR started_at >= $2)
              AND ($3::TIMESTAMPTZ IS NULL OR started_at <= $3)
            GROUP BY error_type
            ORDER BY error_count DESC
            LIMIT 10
            "#,
        )
        .bind(filters.project_id)
        .bind(filters.from)
        .bind(filters.to)
        .fetch_all(self.pool)
        .await?;

        let top_errors = top_errors_rows
            .into_iter()
            .map(|r| TopErrorItem {
                error_type: r.get("error_type"),
                count: r.get("error_count"),
                sample_message: r.get("sample_message"),
                last_seen_at: r.get("last_seen_at"),
            })
            .collect();

        Ok(ErrorAnalyticsResult {
            total_errors,
            total_traces,
            error_rate,
            time_series,
            top_errors,
        })
    }
}

pub struct AlertRulesRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AlertRulesRepository<'a> {
    #[must_use]
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        environment_id: Option<Uuid>,
        name: &str,
        description: Option<&str>,
        enabled: bool,
        metric: AlertMetric,
        operator: AlertOperator,
        threshold: Decimal,
        window_seconds: i32,
        min_sample_count: i32,
        group_by: serde_json::Value,
        filters: serde_json::Value,
        cooldown_seconds: i32,
        severity: AlertSeverity,
        created_by: Option<Uuid>,
    ) -> Result<AlertRule, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO alert_rules (
                organization_id, project_id, environment_id, name, description,
                enabled, metric, operator, threshold, window_seconds, min_sample_count,
                group_by, filters, cooldown_seconds, severity, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING id, organization_id, project_id, environment_id, name, description,
                      enabled, metric, operator, threshold, window_seconds, min_sample_count,
                      group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at
            "#,
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(environment_id)
        .bind(name)
        .bind(description)
        .bind(enabled)
        .bind(metric.as_str())
        .bind(operator.as_str())
        .bind(threshold)
        .bind(window_seconds)
        .bind(min_sample_count)
        .bind(group_by)
        .bind(filters)
        .bind(cooldown_seconds)
        .bind(severity.as_str())
        .bind(created_by)
        .fetch_one(self.pool)
        .await?;

        map_alert_rule_row(&row)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        rule_id: Uuid,
    ) -> Result<Option<AlertRule>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, name, description,
                   enabled, metric, operator, threshold, window_seconds, min_sample_count,
                   group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at
            FROM alert_rules
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(rule_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_rule_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_project(&self, project_id: Uuid) -> Result<Vec<AlertRule>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, name, description,
                   enabled, metric, operator, threshold, window_seconds, min_sample_count,
                   group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at
            FROM alert_rules
            WHERE project_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_alert_rule_row(&row)?);
        }
        Ok(out)
    }

    pub async fn list_all_enabled(&self) -> Result<Vec<AlertRule>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, environment_id, name, description,
                   enabled, metric, operator, threshold, window_seconds, min_sample_count,
                   group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at
            FROM alert_rules
            WHERE enabled = TRUE
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_alert_rule_row(&row)?);
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        project_id: Uuid,
        rule_id: Uuid,
        name: Option<&str>,
        description: Option<Option<&str>>,
        enabled: Option<bool>,
        metric: Option<AlertMetric>,
        operator: Option<AlertOperator>,
        threshold: Option<Decimal>,
        window_seconds: Option<i32>,
        min_sample_count: Option<i32>,
        cooldown_seconds: Option<i32>,
        severity: Option<AlertSeverity>,
    ) -> Result<Option<AlertRule>, DatabaseError> {
        let existing = match self.get_by_id(project_id, rule_id).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        let new_name = name.unwrap_or(&existing.name);
        let new_description = match description {
            Some(d) => d.map(|s| s.to_string()),
            None => existing.description,
        };
        let new_enabled = enabled.unwrap_or(existing.enabled);
        let new_metric = metric.unwrap_or(existing.metric);
        let new_operator = operator.unwrap_or(existing.operator);
        let new_threshold = threshold.unwrap_or(existing.threshold);
        let new_window = window_seconds.unwrap_or(existing.window_seconds);
        let new_min_samples = min_sample_count.unwrap_or(existing.min_sample_count);
        let new_cooldown = cooldown_seconds.unwrap_or(existing.cooldown_seconds);
        let new_severity = severity.unwrap_or(existing.severity);

        let row = sqlx::query(
            r#"
            UPDATE alert_rules
            SET name = $1, description = $2, enabled = $3, metric = $4,
                operator = $5, threshold = $6, window_seconds = $7,
                min_sample_count = $8, cooldown_seconds = $9, severity = $10,
                updated_at = now()
            WHERE project_id = $11 AND id = $12
            RETURNING id, organization_id, project_id, environment_id, name, description,
                      enabled, metric, operator, threshold, window_seconds, min_sample_count,
                      group_by, filters, cooldown_seconds, severity, created_by, created_at, updated_at
            "#,
        )
        .bind(new_name)
        .bind(new_description)
        .bind(new_enabled)
        .bind(new_metric.as_str())
        .bind(new_operator.as_str())
        .bind(new_threshold)
        .bind(new_window)
        .bind(new_min_samples)
        .bind(new_cooldown)
        .bind(new_severity.as_str())
        .bind(project_id)
        .bind(rule_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_rule_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn delete(&self, project_id: Uuid, rule_id: Uuid) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            r#"
            DELETE FROM alert_rules
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(rule_id)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

fn map_alert_rule_row(row: &PgRow) -> Result<AlertRule, DatabaseError> {
    let metric_str: String = row.get("metric");
    let operator_str: String = row.get("operator");
    let severity_str: String = row.get("severity");

    let metric = AlertMetric::from_str(&metric_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid metric: {e}")))?;
    let operator = AlertOperator::from_str(&operator_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid operator: {e}")))?;
    let severity = AlertSeverity::from_str(&severity_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid severity: {e}")))?;

    Ok(AlertRule {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        project_id: row.get("project_id"),
        environment_id: row.get("environment_id"),
        name: row.get("name"),
        description: row.get("description"),
        enabled: row.get("enabled"),
        metric,
        operator,
        threshold: row.get("threshold"),
        window_seconds: row.get("window_seconds"),
        min_sample_count: row.get("min_sample_count"),
        group_by: row.get("group_by"),
        filters: row.get("filters"),
        cooldown_seconds: row.get("cooldown_seconds"),
        severity,
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub struct AlertEventsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AlertEventsRepository<'a> {
    #[must_use]
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        rule_id: Uuid,
        fingerprint: &str,
        severity: AlertSeverity,
        summary: &str,
        details: serde_json::Value,
    ) -> Result<AlertEvent, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO alert_events (
                organization_id, project_id, rule_id, fingerprint, status,
                severity, summary, details, first_triggered_at, last_triggered_at
            )
            VALUES ($1, $2, $3, $4, 'firing', $5, $6, $7, now(), now())
            RETURNING id, organization_id, project_id, rule_id, fingerprint, status,
                      severity, summary, details, first_triggered_at, last_triggered_at,
                      resolved_at, acknowledged_at, acknowledged_by, created_at
            "#,
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(rule_id)
        .bind(fingerprint)
        .bind(severity.as_str())
        .bind(summary)
        .bind(details)
        .fetch_one(self.pool)
        .await?;

        map_alert_event_row(&row)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        event_id: Uuid,
    ) -> Result<Option<AlertEvent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, rule_id, fingerprint, status,
                   severity, summary, details, first_triggered_at, last_triggered_at,
                   resolved_at, acknowledged_at, acknowledged_by, created_at
            FROM alert_events
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(event_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_event_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_project(
        &self,
        project_id: Uuid,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AlertEvent>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, rule_id, fingerprint, status,
                   severity, summary, details, first_triggered_at, last_triggered_at,
                   resolved_at, acknowledged_at, acknowledged_by, created_at
            FROM alert_events
            WHERE project_id = $1
              AND ($2::VARCHAR IS NULL OR status = $2)
            ORDER BY last_triggered_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(project_id)
        .bind(status_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_alert_event_row(&row)?);
        }
        Ok(out)
    }

    pub async fn find_active_by_fingerprint(
        &self,
        project_id: Uuid,
        rule_id: Uuid,
        fingerprint: &str,
    ) -> Result<Option<AlertEvent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, rule_id, fingerprint, status,
                   severity, summary, details, first_triggered_at, last_triggered_at,
                   resolved_at, acknowledged_at, acknowledged_by, created_at
            FROM alert_events
            WHERE project_id = $1 AND rule_id = $2 AND fingerprint = $3
              AND status IN ('firing', 'acknowledged')
            ORDER BY last_triggered_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(rule_id)
        .bind(fingerprint)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_event_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn find_last_resolved_by_fingerprint(
        &self,
        project_id: Uuid,
        rule_id: Uuid,
        fingerprint: &str,
    ) -> Result<Option<AlertEvent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, rule_id, fingerprint, status,
                   severity, summary, details, first_triggered_at, last_triggered_at,
                   resolved_at, acknowledged_at, acknowledged_by, created_at
            FROM alert_events
            WHERE project_id = $1 AND rule_id = $2 AND fingerprint = $3
              AND status = 'resolved'
            ORDER BY resolved_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(rule_id)
        .bind(fingerprint)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_event_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn update_triggered(
        &self,
        event_id: Uuid,
        details: serde_json::Value,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE alert_events
            SET last_triggered_at = now(), details = $1
            WHERE id = $2
            "#,
        )
        .bind(details)
        .bind(event_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn acknowledge(
        &self,
        project_id: Uuid,
        event_id: Uuid,
        user_id: Option<Uuid>,
    ) -> Result<Option<AlertEvent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            UPDATE alert_events
            SET status = 'acknowledged', acknowledged_at = now(), acknowledged_by = $1
            WHERE project_id = $2 AND id = $3 AND status = 'firing'
            RETURNING id, organization_id, project_id, rule_id, fingerprint, status,
                      severity, summary, details, first_triggered_at, last_triggered_at,
                      resolved_at, acknowledged_at, acknowledged_by, created_at
            "#,
        )
        .bind(user_id)
        .bind(project_id)
        .bind(event_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_event_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn resolve(&self, event_id: Uuid) -> Result<Option<AlertEvent>, DatabaseError> {
        let row = sqlx::query(
            r#"
            UPDATE alert_events
            SET status = 'resolved', resolved_at = now()
            WHERE id = $1 AND status IN ('firing', 'acknowledged')
            RETURNING id, organization_id, project_id, rule_id, fingerprint, status,
                      severity, summary, details, first_triggered_at, last_triggered_at,
                      resolved_at, acknowledged_at, acknowledged_by, created_at
            "#,
        )
        .bind(event_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_alert_event_row(&r)?)),
            None => Ok(None),
        }
    }
}

fn map_alert_event_row(row: &PgRow) -> Result<AlertEvent, DatabaseError> {
    let status_str: String = row.get("status");
    let severity_str: String = row.get("severity");

    let status = AlertEventStatus::from_str(&status_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid status: {e}")))?;
    let severity = AlertSeverity::from_str(&severity_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid severity: {e}")))?;

    Ok(AlertEvent {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        project_id: row.get("project_id"),
        rule_id: row.get("rule_id"),
        fingerprint: row.get("fingerprint"),
        status,
        severity,
        summary: row.get("summary"),
        details: row.get("details"),
        first_triggered_at: row.get("first_triggered_at"),
        last_triggered_at: row.get("last_triggered_at"),
        resolved_at: row.get("resolved_at"),
        acknowledged_at: row.get("acknowledged_at"),
        acknowledged_by: row.get("acknowledged_by"),
        created_at: row.get("created_at"),
    })
}

pub struct NotificationChannelsRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> NotificationChannelsRepository<'a> {
    #[must_use]
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        organization_id: Uuid,
        project_id: Option<Uuid>,
        channel_type: NotificationChannelType,
        name: &str,
        encrypted_config: &str,
        key_version: &str,
    ) -> Result<NotificationChannel, DatabaseError> {
        let row = sqlx::query(
            r#"
            INSERT INTO notification_channels (
                organization_id, project_id, channel_type, name,
                encrypted_config, key_version, enabled
            )
            VALUES ($1, $2, $3, $4, $5, $6, TRUE)
            RETURNING id, organization_id, project_id, channel_type, name,
                      encrypted_config, key_version, enabled, created_at, updated_at
            "#,
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(channel_type.as_str())
        .bind(name)
        .bind(encrypted_config)
        .bind(key_version)
        .fetch_one(self.pool)
        .await?;

        map_notification_channel_row(&row)
    }

    pub async fn get_by_id(
        &self,
        project_id: Uuid,
        channel_id: Uuid,
    ) -> Result<Option<NotificationChannel>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, channel_type, name,
                   encrypted_config, key_version, enabled, created_at, updated_at
            FROM notification_channels
            WHERE id = $1 AND (project_id = $2 OR project_id IS NULL)
            "#,
        )
        .bind(channel_id)
        .bind(project_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_notification_channel_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<NotificationChannel>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, channel_type, name,
                   encrypted_config, key_version, enabled, created_at, updated_at
            FROM notification_channels
            WHERE project_id = $1 OR project_id IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_notification_channel_row(&row)?);
        }
        Ok(out)
    }

    pub async fn list_enabled_for_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<NotificationChannel>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, project_id, channel_type, name,
                   encrypted_config, key_version, enabled, created_at, updated_at
            FROM notification_channels
            WHERE (project_id = $1 OR project_id IS NULL) AND enabled = TRUE
            ORDER BY created_at ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_notification_channel_row(&row)?);
        }
        Ok(out)
    }

    pub async fn update(
        &self,
        project_id: Uuid,
        channel_id: Uuid,
        name: Option<&str>,
        encrypted_config: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<Option<NotificationChannel>, DatabaseError> {
        let existing = match self.get_by_id(project_id, channel_id).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        let new_name = name.unwrap_or(&existing.name);
        let new_config = encrypted_config.unwrap_or(&existing.encrypted_config);
        let new_enabled = enabled.unwrap_or(existing.enabled);

        let row = sqlx::query(
            r#"
            UPDATE notification_channels
            SET name = $1, encrypted_config = $2, enabled = $3, updated_at = now()
            WHERE id = $4 AND (project_id = $5 OR project_id IS NULL)
            RETURNING id, organization_id, project_id, channel_type, name,
                      encrypted_config, key_version, enabled, created_at, updated_at
            "#,
        )
        .bind(new_name)
        .bind(new_config)
        .bind(new_enabled)
        .bind(channel_id)
        .bind(project_id)
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(map_notification_channel_row(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn delete(&self, project_id: Uuid, channel_id: Uuid) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            r#"
            DELETE FROM notification_channels
            WHERE id = $1 AND (project_id = $2 OR project_id IS NULL)
            "#,
        )
        .bind(channel_id)
        .bind(project_id)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

fn map_notification_channel_row(row: &PgRow) -> Result<NotificationChannel, DatabaseError> {
    let type_str: String = row.get("channel_type");
    let channel_type = NotificationChannelType::from_str(&type_str)
        .map_err(|e| DatabaseError::NotFound(format!("Invalid channel type: {e}")))?;

    Ok(NotificationChannel {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        project_id: row.get("project_id"),
        channel_type,
        name: row.get("name"),
        encrypted_config: row.get("encrypted_config"),
        key_version: row.get("key_version"),
        enabled: row.get("enabled"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub struct NotificationDeliveriesRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> NotificationDeliveriesRepository<'a> {
    #[must_use]
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_batch(&self, deliveries: &[(Uuid, Uuid)]) -> Result<usize, DatabaseError> {
        if deliveries.is_empty() {
            return Ok(0);
        }

        let mut count = 0;
        for (event_id, channel_id) in deliveries {
            sqlx::query(
                r#"
                INSERT INTO notification_deliveries (
                    alert_event_id, channel_id, status, attempts, next_attempt_at
                )
                VALUES ($1, $2, 'pending', 0, now())
                "#,
            )
            .bind(event_id)
            .bind(channel_id)
            .execute(self.pool)
            .await?;
            count += 1;
        }

        Ok(count)
    }

    pub async fn list_pending(
        &self,
        limit: i64,
    ) -> Result<Vec<(NotificationDelivery, NotificationChannel, AlertEvent)>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                d.id as delivery_id, d.alert_event_id, d.channel_id, d.status as delivery_status,
                d.attempts, d.next_attempt_at, d.last_error, d.provider_message_id,
                d.created_at as delivery_created_at, d.delivered_at,
                c.id as ch_id, c.organization_id as ch_org_id, c.project_id as ch_proj_id,
                c.channel_type, c.name as ch_name, c.encrypted_config, c.key_version,
                c.enabled as ch_enabled, c.created_at as ch_created_at, c.updated_at as ch_updated_at,
                e.id as ev_id, e.organization_id as ev_org_id, e.project_id as ev_proj_id,
                e.rule_id, e.fingerprint, e.status as ev_status, e.severity, e.summary,
                e.details, e.first_triggered_at, e.last_triggered_at, e.resolved_at,
                e.acknowledged_at, e.acknowledged_by, e.created_at as ev_created_at
            FROM notification_deliveries d
            JOIN notification_channels c ON c.id = d.channel_id
            JOIN alert_events e ON e.id = d.alert_event_id
            WHERE d.status = 'pending' AND d.next_attempt_at <= now()
            ORDER BY d.created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let deliv_status_str: String = r.get("delivery_status");
            let deliv_status = NotificationDeliveryStatus::from_str(&deliv_status_str)
                .map_err(|e| DatabaseError::NotFound(format!("Invalid delivery status: {e}")))?;

            let delivery = NotificationDelivery {
                id: r.get("delivery_id"),
                alert_event_id: r.get("alert_event_id"),
                channel_id: r.get("channel_id"),
                status: deliv_status,
                attempts: r.get("attempts"),
                next_attempt_at: r.get("next_attempt_at"),
                last_error: r.get("last_error"),
                provider_message_id: r.get("provider_message_id"),
                created_at: r.get("delivery_created_at"),
                delivered_at: r.get("delivered_at"),
            };

            let ch_type_str: String = r.get("channel_type");
            let channel_type = NotificationChannelType::from_str(&ch_type_str)
                .map_err(|e| DatabaseError::NotFound(format!("Invalid channel type: {e}")))?;

            let channel = NotificationChannel {
                id: r.get("ch_id"),
                organization_id: r.get("ch_org_id"),
                project_id: r.get("ch_proj_id"),
                channel_type,
                name: r.get("ch_name"),
                encrypted_config: r.get("encrypted_config"),
                key_version: r.get("key_version"),
                enabled: r.get("ch_enabled"),
                created_at: r.get("ch_created_at"),
                updated_at: r.get("ch_updated_at"),
            };

            let ev_status_str: String = r.get("ev_status");
            let ev_status = AlertEventStatus::from_str(&ev_status_str)
                .map_err(|e| DatabaseError::NotFound(format!("Invalid event status: {e}")))?;
            let ev_sev_str: String = r.get("severity");
            let ev_sev = AlertSeverity::from_str(&ev_sev_str)
                .map_err(|e| DatabaseError::NotFound(format!("Invalid severity: {e}")))?;

            let event = AlertEvent {
                id: r.get("ev_id"),
                organization_id: r.get("ev_org_id"),
                project_id: r.get("ev_proj_id"),
                rule_id: r.get("rule_id"),
                fingerprint: r.get("fingerprint"),
                status: ev_status,
                severity: ev_sev,
                summary: r.get("summary"),
                details: r.get("details"),
                first_triggered_at: r.get("first_triggered_at"),
                last_triggered_at: r.get("last_triggered_at"),
                resolved_at: r.get("resolved_at"),
                acknowledged_at: r.get("acknowledged_at"),
                acknowledged_by: r.get("acknowledged_by"),
                created_at: r.get("ev_created_at"),
            };

            out.push((delivery, channel, event));
        }

        Ok(out)
    }

    pub async fn mark_delivered(
        &self,
        delivery_id: Uuid,
        provider_message_id: Option<&str>,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            UPDATE notification_deliveries
            SET status = 'delivered', delivered_at = now(), provider_message_id = $1
            WHERE id = $2
            "#,
        )
        .bind(provider_message_id)
        .bind(delivery_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_failed(
        &self,
        delivery_id: Uuid,
        error: &str,
        next_attempt_at: DateTime<Utc>,
        attempts: i32,
    ) -> Result<(), DatabaseError> {
        let final_status = if attempts >= 5 { "failed" } else { "pending" };

        sqlx::query(
            r#"
            UPDATE notification_deliveries
            SET status = $1, last_error = $2, next_attempt_at = $3, attempts = $4
            WHERE id = $5
            "#,
        )
        .bind(final_status)
        .bind(error)
        .bind(next_attempt_at)
        .bind(attempts)
        .bind(delivery_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

pub struct AgentHealthRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> AgentHealthRepository<'a> {
    #[must_use]
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn compute_health(
        &self,
        project_id: Uuid,
        agent_id: Uuid,
        window: &str,
    ) -> Result<AgentHealthReport, DatabaseError> {
        let window_seconds: i64 = match window {
            "1h" => 3600,
            "7d" => 604_800,
            _ => 86400, // default "24h"
        };

        // Query agent last_seen_at
        let agent_row = sqlx::query(
            r#"
            SELECT last_seen_at
            FROM agents
            WHERE project_id = $1 AND id = $2
            "#,
        )
        .bind(project_id)
        .bind(agent_id)
        .fetch_optional(self.pool)
        .await?;

        let last_seen_at: Option<DateTime<Utc>> = agent_row.and_then(|r| r.get("last_seen_at"));

        // Determine freshness status
        let now = Utc::now();
        let freshness_status = match last_seen_at {
            None => AgentHealthStatus::NeverSeen,
            Some(last_seen) => {
                let diff_secs = (now - last_seen).num_seconds();
                if diff_secs <= 600 {
                    AgentHealthStatus::Active
                } else if diff_secs <= 7200 {
                    AgentHealthStatus::Idle
                } else {
                    AgentHealthStatus::Stale
                }
            }
        };

        // Query trace metrics over window
        let stats_row = sqlx::query(
            r#"
            SELECT 
                COUNT(*)::BIGINT as total_count,
                COUNT(*) FILTER (WHERE status = 'ok')::BIGINT as success_count,
                COUNT(*) FILTER (WHERE status = 'error')::BIGINT as error_count,
                COUNT(*) FILTER (WHERE status = 'timeout' OR error_type ILIKE '%timeout%')::BIGINT as timeout_count,
                percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms) as p50_duration,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms) as p95_duration
            FROM traces
            WHERE project_id = $1 AND agent_id = $2
              AND started_at >= now() - ($3 || ' seconds')::INTERVAL
            "#,
        )
        .bind(project_id)
        .bind(agent_id)
        .bind(window_seconds)
        .fetch_one(self.pool)
        .await?;

        let total_count: i64 = stats_row.get("total_count");
        let success_count: i64 = stats_row.get("success_count");
        let error_count: i64 = stats_row.get("error_count");
        let timeout_count: i64 = stats_row.get("timeout_count");
        let p50_duration: Option<f64> = stats_row.get("p50_duration");
        let p95_duration: Option<f64> = stats_row.get("p95_duration");

        let (success_rate, error_rate, timeout_rate) = if total_count > 0 {
            let tc = total_count as f64;
            (
                (success_count as f64 / tc * 1000.0).round() / 1000.0,
                (error_count as f64 / tc * 1000.0).round() / 1000.0,
                (timeout_count as f64 / tc * 1000.0).round() / 1000.0,
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let evaluated_thresholds = serde_json::json!({
            "error_rate_threshold": 0.05,
            "p95_latency_threshold_ms": 3000,
            "inactivity_threshold_seconds": 3600
        });

        let status_explanation = if total_count == 0 {
            match freshness_status {
                AgentHealthStatus::NeverSeen => {
                    "No telemetry has ever been recorded for this agent.".to_string()
                }
                _ => format!(
                    "No traces executed within the requested {window} window. Freshness: {:?}.",
                    freshness_status
                ),
            }
        } else {
            format!(
                "Executed {total_count} traces with {:.1}% success rate and {:.1}% error rate. P95 latency is {}ms.",
                success_rate * 100.0,
                error_rate * 100.0,
                p95_duration.map_or("N/A".to_string(), |v| format!("{v:.0}"))
            )
        };

        Ok(AgentHealthReport {
            agent_id,
            window: window.to_string(),
            last_seen_at,
            freshness_status,
            execution_count: total_count,
            success_rate,
            error_rate,
            timeout_rate,
            p50_latency_ms: p50_duration,
            p95_latency_ms: p95_duration,
            status_explanation,
            evaluated_thresholds,
        })
    }
}
