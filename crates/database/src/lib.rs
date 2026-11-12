#![forbid(unsafe_code)]

//! `agentlens-database` crate.
//! PostgreSQL connection pools, SQLx migrations, transactions, and tenant repositories.

use agentlens_config::AppConfig;
use agentlens_domain::{
    Agent, ApiKey, AuditLog, Environment, OrgRole, Organization, OrganizationMember, OutboxJob,
    Project, Span, SpanId, SpanType, TokenUsage, Trace, TraceId, TraceStatus, User,
};
use agentlens_telemetry::NormalizedSpan;
use chrono::{DateTime, Utc};
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
                ON CONFLICT (dedup_key) DO NOTHING
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
                    COALESCE(SUM(cost_estimate), 0.0) AS total_cost
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

        let mut spans = Vec::with_capacity(rows.len());
        for r in rows {
            let tid_raw: String = r.get("trace_id");
            let parsed_trace_id =
                TraceId::new(tid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

            let sid_raw: String = r.get("span_id");
            let parsed_span_id =
                SpanId::new(sid_raw).map_err(|e| DatabaseError::NotFound(e.to_string()))?;

            let parent_sid_raw: Option<String> = r.get("parent_span_id");
            let parsed_parent_span_id = match parent_sid_raw {
                Some(sid) => {
                    Some(SpanId::new(sid).map_err(|e| DatabaseError::NotFound(e.to_string()))?)
                }
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

            spans.push(Span {
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
            });
        }

        Ok(spans)
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
