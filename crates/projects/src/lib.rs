#![forbid(unsafe_code)]

//! `agentlens-projects` crate.
//! Project and environment boundaries, and scoped API key lifecycle policies.

use agentlens_database::DatabaseError;
use agentlens_domain::OrgRole;
use agentlens_organizations::{require_admin_or_owner, validate_slug, OrgError};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Invalid project slug: {0}")]
    InvalidSlug(String),
    #[error("Invalid project name: cannot be empty")]
    InvalidName,
    #[error("Invalid retention days: must be greater than zero")]
    InvalidRetentionDays,
    #[error("Invalid API key configuration: {0}")]
    InvalidApiKeyConfig(String),
    #[error("Organization error: {0}")]
    Org(#[from] OrgError),
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("Entity not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectInput {
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub retention_days: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyInput {
    pub environment_id: Uuid,
    pub name: String,
    pub scopes: Option<Vec<String>>,
    pub expires_in_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreatedOutput {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub secret: String,
    pub scopes: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub fn validate_project_input(
    input: &CreateProjectInput,
    role: OrgRole,
) -> Result<(), ProjectError> {
    require_admin_or_owner(role).map_err(|e| ProjectError::PermissionDenied(e.to_string()))?;
    if input.name.trim().is_empty() {
        return Err(ProjectError::InvalidName);
    }
    validate_slug(&input.slug).map_err(|e| ProjectError::InvalidSlug(e.to_string()))?;
    Ok(())
}

pub fn validate_api_key_input(
    input: &CreateApiKeyInput,
    role: OrgRole,
) -> Result<(), ProjectError> {
    require_admin_or_owner(role).map_err(|e| ProjectError::PermissionDenied(e.to_string()))?;
    if input.name.trim().is_empty() {
        return Err(ProjectError::InvalidApiKeyConfig(
            "Key name cannot be empty".to_string(),
        ));
    }
    if let Some(days) = input.expires_in_days {
        if days <= 0 {
            return Err(ProjectError::InvalidApiKeyConfig(
                "Expiration days must be > 0".to_string(),
            ));
        }
    }
    Ok(())
}

/// Calculate expiration timestamp from optional relative days.
#[must_use]
pub fn compute_key_expiration(expires_in_days: Option<i64>) -> Option<DateTime<Utc>> {
    expires_in_days.map(|days| Utc::now() + Duration::days(days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_project_input() {
        let valid = CreateProjectInput {
            organization_id: Uuid::new_v4(),
            name: "My Project".to_string(),
            slug: "my-project".to_string(),
            description: None,
        };
        assert!(validate_project_input(&valid, OrgRole::Owner).is_ok());
        assert!(validate_project_input(&valid, OrgRole::Admin).is_ok());
        assert!(validate_project_input(&valid, OrgRole::Member).is_err());
        assert!(validate_project_input(&valid, OrgRole::Viewer).is_err());
    }

    #[test]
    fn test_compute_key_expiration() {
        assert!(compute_key_expiration(None).is_none());
        let expires = compute_key_expiration(Some(30)).unwrap();
        assert!(expires > Utc::now());
    }
}
