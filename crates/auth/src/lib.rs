#![forbid(unsafe_code)]

//! `agentlens-auth` crate.
//! Authentication primitives, Argon2id passwords, HMAC session tokens,
//! CSRF protection, constant-time API key verification, and OIDC interfaces.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Password hashing error: {0}")]
    PasswordHashing(String),
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Invalid session format or signature")]
    InvalidSession,
    #[error("Session has expired")]
    SessionExpired,
    #[error("CSRF token validation failed")]
    CsrfMismatch,
    #[error("Invalid API key")]
    InvalidApiKey,
    #[error("API key has been revoked")]
    ApiKeyRevoked,
    #[error("API key has expired")]
    ApiKeyExpired,
    #[error("OIDC error: {0}")]
    OidcError(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
}

// ---------------------------------------------------------------------------
// Password Hashing (Argon2id)
// ---------------------------------------------------------------------------

pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AuthError::PasswordHashing(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, AuthError> {
    let parsed_hash =
        PasswordHash::new(password_hash).map_err(|e| AuthError::PasswordHashing(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

// ---------------------------------------------------------------------------
// Scoped Ingestion API Keys
// ---------------------------------------------------------------------------

/// Generates a new API key pair: `(full_secret, prefix, sha256_hash)`.
/// Format: `agl_live_<32_base62_chars>`.
/// Prefix: `agl_live_<first_8_chars>`.
#[must_use]
pub fn generate_api_key() -> (String, String, Vec<u8>) {
    let mut rng = rand::thread_rng();
    let secret_entropy: String = (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..BASE62_CHARS.len());
            BASE62_CHARS[idx] as char
        })
        .collect();

    let full_secret = format!("agl_live_{secret_entropy}");
    let prefix = format!("agl_live_{}", &secret_entropy[..8]);
    let hash = hash_api_key(&full_secret);

    (full_secret, prefix, hash)
}

/// Computes SHA-256 hash digest of an API key secret.
#[must_use]
pub fn hash_api_key(secret: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().to_vec()
}

/// Verifies an API key secret against its stored SHA-256 hash in constant time.
#[must_use]
pub fn verify_api_key_hash(expected_hash: &[u8], candidate_secret: &str) -> bool {
    let candidate_hash = hash_api_key(candidate_secret);
    bool::from(candidate_hash.ct_eq(expected_hash))
}

// ---------------------------------------------------------------------------
// Session & CSRF Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPayload {
    pub user_id: Uuid,
    pub created_at: i64,
    pub expires_at: i64,
    pub csrf_token: String,
}

/// Generates a high-entropy CSRF token (32 base62 characters).
#[must_use]
pub fn generate_csrf_token() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..BASE62_CHARS.len());
            BASE62_CHARS[idx] as char
        })
        .collect()
}

/// Validates a CSRF token in constant time.
#[must_use]
pub fn verify_csrf_token(expected: &str, candidate: &str) -> bool {
    if expected.is_empty() || candidate.is_empty() {
        return false;
    }
    bool::from(expected.as_bytes().ct_eq(candidate.as_bytes()))
}

/// Encodes a session payload into a signed session cookie value.
/// Format: `<hex(json_payload)>.<hex(hmac_sha256)>`.
pub fn create_session_token(
    user_id: Uuid,
    duration_secs: i64,
    secret: &[u8],
) -> Result<(String, String), AuthError> {
    let now = Utc::now().timestamp();
    let csrf_token = generate_csrf_token();
    let payload = SessionPayload {
        user_id,
        created_at: now,
        expires_at: now + duration_secs,
        csrf_token: csrf_token.clone(),
    };

    let serialized = serde_json::to_vec(&payload)
        .map_err(|e| AuthError::PasswordHashing(format!("Serialization error: {e}")))?;
    let hex_payload = hex::encode(serialized);

    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| AuthError::InvalidSession)?;
    mac.update(hex_payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());

    let token = format!("{hex_payload}.{sig}");
    Ok((token, csrf_token))
}

/// Validates and decodes a signed session token.
pub fn verify_session_token(token: &str, secret: &[u8]) -> Result<SessionPayload, AuthError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return Err(AuthError::InvalidSession);
    }

    let hex_payload = parts[0];
    let expected_sig = parts[1];

    // 1. Verify signature in constant time
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| AuthError::InvalidSession)?;
    mac.update(hex_payload.as_bytes());
    let computed_sig = hex::encode(mac.finalize().into_bytes());

    if !bool::from(computed_sig.as_bytes().ct_eq(expected_sig.as_bytes())) {
        return Err(AuthError::InvalidSession);
    }

    // 2. Decode payload
    let raw_payload = hex::decode(hex_payload).map_err(|_| AuthError::InvalidSession)?;
    let payload: SessionPayload =
        serde_json::from_slice(&raw_payload).map_err(|_| AuthError::InvalidSession)?;

    // 3. Check expiration
    if Utc::now().timestamp() > payload.expires_at {
        return Err(AuthError::SessionExpired);
    }

    Ok(payload)
}

/// Builds `Set-Cookie` header value for session cookie.
#[must_use]
pub fn build_session_cookie(token: &str, max_age_secs: i64, is_production: bool) -> String {
    let secure_flag = if is_production { "; Secure" } else { "" };
    format!(
        "agentlens_session={token}; Path=/; Max-Age={max_age_secs}; HttpOnly; SameSite=Lax{secure_flag}"
    )
}

/// Builds `Set-Cookie` header value to clear session cookie.
#[must_use]
pub fn build_clear_cookie(is_production: bool) -> String {
    let secure_flag = if is_production { "; Secure" } else { "" };
    format!("agentlens_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{secure_flag}")
}

// ---------------------------------------------------------------------------
// OIDC Abstraction & PKCE
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcClaims {
    pub subject: String,
    pub issuer: String,
    pub email: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTokens {
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

/// Generates PKCE code verifier and code challenge (S256).
#[must_use]
pub fn generate_pkce() -> PkcePair {
    let mut rng = rand::thread_rng();
    let verifier: String = (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..BASE62_CHARS.len());
            BASE62_CHARS[idx] as char
        })
        .collect();

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = hex::encode(hasher.finalize());

    PkcePair {
        verifier,
        challenge,
    }
}

/// Generates an OIDC random state or nonce parameter.
#[must_use]
pub fn generate_oidc_param() -> String {
    let rng = rand::thread_rng();
    rng.sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

#[async_trait::async_trait]
pub trait OidcProvider: Send + Sync {
    fn authorization_url(&self, state: &str, nonce: &str, pkce_challenge: &str) -> String;
    async fn exchange_code(&self, code: &str, pkce_verifier: &str)
        -> Result<OidcTokens, AuthError>;
    async fn verify_id_token(&self, id_token: &str, nonce: &str) -> Result<OidcClaims, AuthError>;
}

/// Mock OIDC provider for tests and deterministic pipelines.
#[derive(Debug, Default, Clone)]
pub struct MockOidcProvider;

#[async_trait::async_trait]
impl OidcProvider for MockOidcProvider {
    fn authorization_url(&self, state: &str, nonce: &str, pkce_challenge: &str) -> String {
        format!("https://idp.example.com/oauth/authorize?state={state}&nonce={nonce}&code_challenge={pkce_challenge}")
    }

    async fn exchange_code(
        &self,
        code: &str,
        _pkce_verifier: &str,
    ) -> Result<OidcTokens, AuthError> {
        if code == "invalid_code" {
            return Err(AuthError::OidcError("Code invalid".to_string()));
        }
        Ok(OidcTokens {
            access_token: "mock_access_token".to_string(),
            id_token: "mock_id_token".to_string(),
            refresh_token: Some("mock_refresh_token".to_string()),
        })
    }

    async fn verify_id_token(&self, id_token: &str, _nonce: &str) -> Result<OidcClaims, AuthError> {
        if id_token == "invalid_token" {
            return Err(AuthError::OidcError("Token invalid".to_string()));
        }
        Ok(OidcClaims {
            subject: "sub_12345".to_string(),
            issuer: "https://idp.example.com".to_string(),
            email: "oidc_user@example.com".to_string(),
            display_name: Some("OIDC Test User".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_password_hashing() {
        let password = "SuperSecretPassword123!";
        let hash = hash_password(password).expect("hash password");
        assert!(verify_password(password, &hash).expect("verify password"));
        assert!(!verify_password("WrongPassword!", &hash).expect("verify wrong password"));
    }

    #[test]
    fn test_api_key_generation_and_verification() {
        let (secret, prefix, hash) = generate_api_key();
        assert!(secret.starts_with("agl_live_"));
        assert!(prefix.starts_with("agl_live_"));
        assert_eq!(hash.len(), 32);

        assert!(verify_api_key_hash(&hash, &secret));
        assert!(!verify_api_key_hash(&hash, "agl_live_invalid_candidate"));
    }

    #[test]
    fn test_session_lifecycle_and_csrf() {
        let secret = b"a_very_secure_and_long_session_secret_key_32bytes!";
        let user_id = Uuid::new_v4();

        let (token, csrf) = create_session_token(user_id, 3600, secret).expect("create session");
        assert!(verify_csrf_token(&csrf, &csrf));
        assert!(!verify_csrf_token(&csrf, "tampered_csrf_token"));

        let payload = verify_session_token(&token, secret).expect("verify session");
        assert_eq!(payload.user_id, user_id);
        assert_eq!(payload.csrf_token, csrf);

        // Tampered token fails
        let tampered = format!("{token}extra");
        assert!(verify_session_token(&tampered, secret).is_err());
    }

    #[test]
    fn test_pkce_generation() {
        let pkce = generate_pkce();
        assert_eq!(pkce.verifier.len(), 64);
        assert_eq!(pkce.challenge.len(), 64);
    }
}
