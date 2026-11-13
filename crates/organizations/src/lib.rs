#![forbid(unsafe_code)]

//! `agentlens-organizations` crate.
//! Organization boundaries, user memberships, and RBAC enforcement.

use agentlens_database::DatabaseError;
use agentlens_domain::OrgRole;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum OrgError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Invalid organization slug: must be 2-50 lowercase alphanumeric or hyphens: {0}")]
    InvalidSlug(String),
    #[error("Invalid organization name: cannot be empty: {0}")]
    InvalidName(String),
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("Entity not found: {0}")]
    NotFound(String),
    #[error("Cannot remove or demote the last organization owner")]
    CannotRemoveLastOwner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrgInput {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrgInput {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddMemberInput {
    pub user_id: Uuid,
    pub role: OrgRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemberRoleInput {
    pub role: OrgRole,
}

/// Validates an organization or project slug.
pub fn validate_slug(slug: &str) -> Result<(), OrgError> {
    if slug.len() < 2 || slug.len() > 50 {
        return Err(OrgError::InvalidSlug(slug.to_string()));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(OrgError::InvalidSlug(slug.to_string()));
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(OrgError::InvalidSlug(slug.to_string()));
    }
    Ok(())
}

/// Enforces that the actor holds the `Owner` role.
pub fn require_owner(role: OrgRole) -> Result<(), OrgError> {
    if role.can_manage_organization() {
        Ok(())
    } else {
        Err(OrgError::PermissionDenied(
            "This action requires the 'owner' role".to_string(),
        ))
    }
}

/// Enforces that the actor holds at least `Admin` or `Owner`.
pub fn require_admin_or_owner(role: OrgRole) -> Result<(), OrgError> {
    if role.can_manage_projects() {
        Ok(())
    } else {
        Err(OrgError::PermissionDenied(
            "This action requires at least 'admin' role".to_string(),
        ))
    }
}

/// Enforces that the actor is not a `Viewer` (any mutating operation).
pub fn require_mutation(role: OrgRole) -> Result<(), OrgError> {
    if role.can_mutate() {
        Ok(())
    } else {
        Err(OrgError::PermissionDenied(
            "Viewer role is strictly read-only and cannot mutate resources".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_validation() {
        assert!(validate_slug("acme-corp").is_ok());
        assert!(validate_slug("default").is_ok());
        assert!(validate_slug("a").is_err());
        assert!(validate_slug("-leading").is_err());
        assert!(validate_slug("trailing-").is_err());
        assert!(validate_slug("UPPERCASE").is_err());
        assert!(validate_slug("invalid_underscore").is_err());
    }

    #[test]
    fn test_rbac_hierarchy() {
        assert!(require_owner(OrgRole::Owner).is_ok());
        assert!(require_owner(OrgRole::Admin).is_err());
        assert!(require_owner(OrgRole::Member).is_err());
        assert!(require_owner(OrgRole::Viewer).is_err());

        assert!(require_admin_or_owner(OrgRole::Owner).is_ok());
        assert!(require_admin_or_owner(OrgRole::Admin).is_ok());
        assert!(require_admin_or_owner(OrgRole::Member).is_err());
        assert!(require_admin_or_owner(OrgRole::Viewer).is_err());

        assert!(require_mutation(OrgRole::Owner).is_ok());
        assert!(require_mutation(OrgRole::Admin).is_ok());
        assert!(require_mutation(OrgRole::Member).is_ok());
        assert!(require_mutation(OrgRole::Viewer).is_err());
    }
}
