#![forbid(unsafe_code)]

//! `agentlens-privacy` crate.
//! Zero-trust content stripping, PII redaction, and bounded attribute length enforcement.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub const DEFAULT_MAX_STRING_LENGTH: usize = 16_384;
pub const LARGE_PAYLOAD_THRESHOLD_BYTES: usize = 64 * 1024; // 64 KiB

/// Evaluates whether a captured content payload exceeds the inline threshold and requires S3 offload.
#[must_use]
pub fn should_offload_to_s3(payload_len: usize) -> bool {
    payload_len > LARGE_PAYLOAD_THRESHOLD_BYTES
}

pub const STRIPPED_CONTENT_KEYS: &[&str] = &[
    "gen_ai.prompt",
    "gen_ai.completion",
    "gen_ai.input.messages",
    "gen_ai.output.messages",
    "llm.prompts",
    "llm.completions",
    "agentlens.tool.input",
    "agentlens.tool.output",
];

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("Invalid regex pattern in redaction rule: {0}")]
    InvalidPattern(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRule {
    pub pattern: String,
    pub replacement: String,
}

#[derive(Debug, Clone)]
pub struct PrivacyPolicy {
    pub capture_content: bool,
    pub max_string_length: usize,
    compiled_rules: Vec<(Regex, String)>,
}

impl PrivacyPolicy {
    pub fn new(capture_content: bool) -> Self {
        Self {
            capture_content,
            max_string_length: DEFAULT_MAX_STRING_LENGTH,
            compiled_rules: Self::default_redaction_rules(),
        }
    }

    pub fn with_rules(
        capture_content: bool,
        max_string_length: usize,
        rules: &[RedactionRule],
    ) -> Result<Self, PrivacyError> {
        let mut compiled = Vec::with_capacity(rules.len() + 3);
        // Include default PII patterns
        compiled.extend(Self::default_redaction_rules());

        for r in rules {
            let re =
                Regex::new(&r.pattern).map_err(|e| PrivacyError::InvalidPattern(e.to_string()))?;
            compiled.push((re, r.replacement.clone()));
        }

        Ok(Self {
            capture_content,
            max_string_length,
            compiled_rules: compiled,
        })
    }

    fn default_redaction_rules() -> Vec<(Regex, String)> {
        vec![
            // Bearer tokens and API keys
            (
                Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.]{12,}").unwrap(),
                "Bearer [REDACTED]".to_string(),
            ),
            // Emails
            (
                Regex::new(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+").unwrap(),
                "[EMAIL_REDACTED]".to_string(),
            ),
            // SSN
            (
                Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
                "[SSN_REDACTED]".to_string(),
            ),
        ]
    }

    /// Redacts and sanitizes an attributes JSON object in-place.
    pub fn sanitize_attributes(&self, attributes: &mut Value) {
        if let Value::Object(map) = attributes {
            self.sanitize_map(map);
        }
    }

    fn sanitize_map(&self, map: &mut Map<String, Value>) {
        if !self.capture_content {
            for key in STRIPPED_CONTENT_KEYS {
                if map.contains_key(*key) {
                    map.insert(
                        (*key).to_string(),
                        Value::String("[CONTENT_CAPTURE_DISABLED]".to_string()),
                    );
                }
            }
        }

        for (k, v) in map.iter_mut() {
            // Block sensitive key names directly
            let k_lower = k.to_ascii_lowercase();
            if k_lower.contains("password")
                || k_lower.contains("authorization")
                || k_lower.contains("secret_key")
            {
                *v = Value::String("[REDACTED_KEY]".to_string());
                continue;
            }

            self.sanitize_value(v);
        }
    }

    fn sanitize_value(&self, val: &mut Value) {
        match val {
            Value::String(s) => {
                let mut sanitized = s.clone();
                for (regex, replacement) in &self.compiled_rules {
                    if regex.is_match(&sanitized) {
                        sanitized = regex
                            .replace_all(&sanitized, replacement.as_str())
                            .to_string();
                    }
                }

                // Truncate if exceeds max_string_length
                if sanitized.len() > self.max_string_length {
                    sanitized.truncate(self.max_string_length);
                    sanitized.push_str("...[TRUNCATED]");
                }

                *s = sanitized;
            }
            Value::Array(arr) => {
                for item in arr {
                    self.sanitize_value(item);
                }
            }
            Value::Object(sub_map) => {
                self.sanitize_map(sub_map);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strips_content_when_capture_disabled() {
        let policy = PrivacyPolicy::new(false);
        let mut attrs = json!({
            "gen_ai.system": "openai",
            "gen_ai.prompt": "My private prompt here",
            "gen_ai.completion": "My sensitive answer",
            "custom.attribute": "ok value"
        });

        policy.sanitize_attributes(&mut attrs);
        assert_eq!(attrs["gen_ai.system"], "openai");
        assert_eq!(attrs["gen_ai.prompt"], "[CONTENT_CAPTURE_DISABLED]");
        assert_eq!(attrs["gen_ai.completion"], "[CONTENT_CAPTURE_DISABLED]");
        assert_eq!(attrs["custom.attribute"], "ok value");
    }

    #[test]
    fn test_preserves_content_when_capture_enabled() {
        let policy = PrivacyPolicy::new(true);
        let mut attrs = json!({
            "gen_ai.prompt": "Preserved prompt text",
            "gen_ai.completion": "Preserved response text"
        });

        policy.sanitize_attributes(&mut attrs);
        assert_eq!(attrs["gen_ai.prompt"], "Preserved prompt text");
        assert_eq!(attrs["gen_ai.completion"], "Preserved response text");
    }

    #[test]
    fn test_redacts_bearer_tokens_and_emails() {
        let policy = PrivacyPolicy::new(true);
        let mut attrs = json!({
            "auth_header": "Bearer secret_api_key_1234567890abcdef",
            "contact": "Contact us at user@example.com for info",
            "ssn": "Government id 123-45-6789"
        });

        policy.sanitize_attributes(&mut attrs);
        assert_eq!(attrs["auth_header"], "Bearer [REDACTED]");
        assert_eq!(attrs["contact"], "Contact us at [EMAIL_REDACTED] for info");
        assert_eq!(attrs["ssn"], "Government id [SSN_REDACTED]");
    }

    #[test]
    fn test_truncates_oversized_attribute_values() {
        let mut policy = PrivacyPolicy::new(true);
        policy.max_string_length = 20;

        let long_text = "A".repeat(100);
        let mut attrs = json!({ "payload": long_text });

        policy.sanitize_attributes(&mut attrs);
        let sanitized = attrs["payload"].as_str().unwrap();
        assert!(sanitized.starts_with("AAAAAAAAAAAAAAAAAAAA"));
        assert!(sanitized.ends_with("...[TRUNCATED]"));
    }
}
