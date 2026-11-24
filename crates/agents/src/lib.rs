#![forbid(unsafe_code)]

//! `agentlens-agents` crate.
//! Agent registration, version tracking, and conversation session lifecycle.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub name: String,
    pub framework: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
}

pub fn validate_agent_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Agent name cannot be empty");
    }
    if trimmed.len() > 255 {
        return Err("Agent name exceeds maximum length of 255 characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_agent_name() {
        assert!(validate_agent_name("CodeReviewAgent").is_ok());
        assert!(validate_agent_name("").is_err());
        assert!(validate_agent_name("   ").is_err());
        assert!(validate_agent_name(&"x".repeat(256)).is_err());
    }
}
