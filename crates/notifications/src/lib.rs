#![forbid(unsafe_code)]

//! `agentlens-notifications` crate.
//! Multi-channel notification delivery with AES-256-GCM encryption and connection-time SSRF defenses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use rand::RngCore;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use agentlens_domain::{AlertEventStatus, NotificationChannel, NotificationChannelType};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Invalid key length: expected 32 bytes, got {0}")]
    InvalidKeyLength(usize),
    #[error("SSRF error: {0}")]
    Ssrf(#[from] SsrfError),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Delivery error ({status}): {body}")]
    DeliveryFailed { status: u16, body: String },
    #[error("Unsupported channel type: {0}")]
    UnsupportedChannel(String),
}

#[derive(Debug, Error)]
pub enum SsrfError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Disallowed scheme: {0}. Only http and https are permitted")]
    DisallowedScheme(String),
    #[error("Failed to resolve hostname: {0}")]
    DnsResolutionFailed(String),
    #[error("Connection blocked to private, link-local, loopback, or metadata address: {0}")]
    BlockedIp(IpAddr),
    #[error("No IP addresses found for host")]
    NoAddressesFound,
}

/// Checks if an IP address is private, loopback, link-local, multicast, or cloud metadata.
#[must_use]
pub fn is_ip_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_ipv4_blocked(v4),
        IpAddr::V6(v6) => is_ipv6_blocked(v6),
    }
}

fn is_ipv4_blocked(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 0.0.0.0/8 (Current network)
    if octets[0] == 0 {
        return true;
    }
    // 127.0.0.0/8 (Loopback)
    if octets[0] == 127 {
        return true;
    }
    // 10.0.0.0/8 (Private RFC 1918)
    if octets[0] == 10 {
        return true;
    }
    // 172.16.0.0/12 (Private RFC 1918)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }
    // 192.168.0.0/16 (Private RFC 1918)
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }
    // 169.254.0.0/16 (Link-local & AWS/GCP/Azure Cloud Metadata 169.254.169.254)
    if octets[0] == 169 && octets[1] == 254 {
        return true;
    }
    // 100.64.0.0/10 (Carrier-grade NAT RFC 6598)
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return true;
    }
    // 192.0.0.0/24 (IETF Protocol Assignments)
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return true;
    }
    // 192.0.2.0/24 (TEST-NET-1)
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 2 {
        return true;
    }
    // 198.51.100.0/24 (TEST-NET-2)
    if octets[0] == 198 && octets[1] == 51 && octets[2] == 100 {
        return true;
    }
    // 203.0.113.0/24 (TEST-NET-3)
    if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
        return true;
    }
    // 198.18.0.0/15 (Network benchmark tests)
    if octets[0] == 198 && (18..=19).contains(&octets[1]) {
        return true;
    }
    // 224.0.0.0/4 (Multicast)
    if octets[0] >= 224 && octets[0] <= 239 {
        return true;
    }
    // 240.0.0.0/4 (Reserved / Future use)
    if octets[0] >= 240 {
        return true;
    }
    // 255.255.255.255 (Broadcast)
    if ip == Ipv4Addr::BROADCAST {
        return true;
    }

    false
}

fn is_ipv6_blocked(ip: Ipv6Addr) -> bool {
    let octets = ip.octets();

    // ::1 (Loopback)
    if ip.is_loopback() {
        return true;
    }
    // :: (Unspecified)
    if ip.is_unspecified() {
        return true;
    }
    // fe80::/10 (Link-local)
    if (octets[0] == 0xfe) && ((octets[1] & 0xc0) == 0x80) {
        return true;
    }
    // fc00::/7 (Unique local address RFC 4193)
    if (octets[0] & 0xfe) == 0xfc {
        return true;
    }
    // ff00::/8 (Multicast)
    if ip.is_multicast() {
        return true;
    }
    // IPv4-mapped IPv6 (::ffff:0:0/96)
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_ipv4_blocked(v4);
    }
    // 2001:db8::/32 (Documentation)
    octets[0] == 0x20 && octets[1] == 0x01 && octets[2] == 0x0d && octets[3] == 0xb8
}

/// Validates target URL against SSRF rules.
/// Resolves hostname and verifies that all returned IPs are public/routable.
pub async fn validate_url_for_ssrf(
    url_str: &str,
    allow_loopback: bool,
) -> Result<reqwest::Url, SsrfError> {
    let parsed = reqwest::Url::parse(url_str).map_err(|e| SsrfError::InvalidUrl(e.to_string()))?;

    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(SsrfError::DisallowedScheme(other.to_string())),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| SsrfError::InvalidUrl("Missing host in URL".into()))?;

    // Check if host is direct IP
    if let Ok(ip) = host.parse::<IpAddr>() {
        if allow_loopback && ip.is_loopback() {
            return Ok(parsed);
        }
        if is_ip_blocked(ip) {
            return Err(SsrfError::BlockedIp(ip));
        }
        return Ok(parsed);
    }

    // Reject localhost explicitly if loopback is not allowed
    if !allow_loopback && (host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost")) {
        return Err(SsrfError::BlockedIp(IpAddr::V4(Ipv4Addr::new(
            127, 0, 0, 1,
        ))));
    }

    // Resolve hostname via DNS
    let port = parsed.port_or_known_default().unwrap_or(80);
    let socket_addrs = tokio::net::lookup_host(format!("{host}:{port}"))
        .await
        .map_err(|e| SsrfError::DnsResolutionFailed(e.to_string()))?;

    let mut found = false;
    for addr in socket_addrs {
        found = true;
        let ip = addr.ip();
        if allow_loopback && ip.is_loopback() {
            continue;
        }
        if is_ip_blocked(ip) {
            return Err(SsrfError::BlockedIp(ip));
        }
    }

    if !found {
        return Err(SsrfError::NoAddressesFound);
    }

    Ok(parsed)
}

/// Constructs an SSRF-safe HTTP client with redirects strictly disabled and bounded timeouts.
#[must_use]
pub fn create_ssrf_safe_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none()) // Mandatory: prevent 3xx redirects to internal metadata or private IPs
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Normalizes encryption key to 32 bytes (via SHA-256 if needed).
fn normalize_key(key: &[u8]) -> [u8; 32] {
    if key.len() == 32 {
        let mut k = [0u8; 32];
        k.copy_from_slice(key);
        k
    } else {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.finalize().into()
    }
}

/// Encrypts configuration plaintext using AES-256-GCM.
/// Output format: `v1:<base64(nonce_12_bytes)>:<base64(ciphertext_with_tag)>`
pub fn encrypt_channel_config(key: &[u8], plaintext: &str) -> Result<String, NotificationError> {
    let key_bytes = normalize_key(key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| NotificationError::Encryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| NotificationError::Encryption(e.to_string()))?;

    let encoded_nonce = BASE64.encode(nonce_bytes);
    let encoded_ciphertext = BASE64.encode(ciphertext);

    Ok(format!("v1:{encoded_nonce}:{encoded_ciphertext}"))
}

/// Decrypts AES-256-GCM encrypted channel configuration.
pub fn decrypt_channel_config(key: &[u8], encrypted: &str) -> Result<String, NotificationError> {
    let parts: Vec<&str> = encrypted.split(':').collect();
    if parts.len() != 3 {
        return Err(NotificationError::Decryption(
            "Invalid encrypted payload structure".into(),
        ));
    }

    if parts[0] != "v1" {
        return Err(NotificationError::Decryption(format!(
            "Unsupported encryption version: {}",
            parts[0]
        )));
    }

    let nonce_bytes = BASE64
        .decode(parts[1])
        .map_err(|e| NotificationError::Decryption(format!("Invalid nonce base64: {e}")))?;

    if nonce_bytes.len() != 12 {
        return Err(NotificationError::Decryption(format!(
            "Invalid nonce length: expected 12, got {}",
            nonce_bytes.len()
        )));
    }

    let ciphertext = BASE64
        .decode(parts[2])
        .map_err(|e| NotificationError::Decryption(format!("Invalid ciphertext base64: {e}")))?;

    let key_bytes = normalize_key(key);
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| NotificationError::Decryption(e.to_string()))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).map_err(|e| {
        NotificationError::Decryption(format!("Decryption authentication failed: {e}"))
    })?;

    String::from_utf8(decrypted)
        .map_err(|e| NotificationError::Decryption(format!("Decrypted bytes not valid UTF-8: {e}")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    pub event_id: uuid::Uuid,
    pub rule_id: uuid::Uuid,
    pub rule_name: String,
    pub severity: String,
    pub status: AlertEventStatus,
    pub summary: String,
    pub details: serde_json::Value,
    pub triggered_at: chrono::DateTime<Utc>,
    pub project_name: Option<String>,
}

/// Masks sensitive credentials for API responses.
#[must_use]
pub fn mask_credential_string(s: &str) -> String {
    if s.len() <= 8 {
        "********".to_string()
    } else {
        let prefix = &s[..4];
        let suffix = &s[s.len() - 4..];
        format!("{prefix}****{suffix}")
    }
}

/// Produces safe, masked JSON for public channel listings.
#[must_use]
pub fn mask_channel_config(
    channel_type: NotificationChannelType,
    config: &serde_json::Value,
) -> serde_json::Value {
    let mut masked = config.clone();
    match channel_type {
        NotificationChannelType::Webhook => {
            if let Some(secret) = masked.get_mut("secret") {
                if let Some(s) = secret.as_str() {
                    *secret = serde_json::Value::String(mask_credential_string(s));
                }
            }
        }
        NotificationChannelType::Telegram => {
            if let Some(bot_token) = masked.get_mut("bot_token") {
                if let Some(s) = bot_token.as_str() {
                    *bot_token = serde_json::Value::String(mask_credential_string(s));
                }
            }
        }
        NotificationChannelType::Discord | NotificationChannelType::Slack => {
            if let Some(url) = masked.get_mut("webhook_url") {
                if let Some(s) = url.as_str() {
                    *url = serde_json::Value::String(mask_credential_string(s));
                }
            }
        }
        NotificationChannelType::Email => {}
    }
    masked
}

/// Delivers a notification across the configured channel adapter with SSRF protections.
pub async fn deliver_notification(
    channel: &NotificationChannel,
    encryption_key: &[u8],
    payload: &NotificationPayload,
    allow_loopback: bool,
) -> Result<Option<String>, NotificationError> {
    let decrypted_json = decrypt_channel_config(encryption_key, &channel.encrypted_config)?;
    let client = create_ssrf_safe_http_client();

    match channel.channel_type {
        NotificationChannelType::Webhook => {
            let config: WebhookConfig = serde_json::from_str(&decrypted_json)?;
            let safe_url = validate_url_for_ssrf(&config.url, allow_loopback).await?;

            let body_bytes = serde_json::to_vec(payload)?;
            let mut req = client
                .post(safe_url)
                .header("Content-Type", "application/json")
                .header(
                    "X-AgentLens-Event",
                    if payload.status == AlertEventStatus::Resolved {
                        "alert.resolved"
                    } else {
                        "alert.firing"
                    },
                );

            if let Some(secret) = &config.secret {
                let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(secret.as_bytes())
                    .map_err(|e| NotificationError::Encryption(e.to_string()))?;
                mac.update(&body_bytes);
                let signature = hex::encode(mac.finalize().into_bytes());
                req = req.header("X-AgentLens-Signature", format!("sha256={signature}"));
            }

            let resp = req.body(body_bytes).send().await?;
            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                return Err(NotificationError::DeliveryFailed { status, body: text });
            }

            Ok(None)
        }
        NotificationChannelType::Telegram => {
            let config: TelegramConfig = serde_json::from_str(&decrypted_json)?;
            let tg_url = format!(
                "https://api.telegram.org/bot{}/sendMessage",
                config.bot_token
            );
            let safe_url = validate_url_for_ssrf(&tg_url, allow_loopback).await?;

            let icon = match payload.status {
                AlertEventStatus::Resolved => "✅ RESOLVED",
                AlertEventStatus::Firing => "🚨 FIRING",
                AlertEventStatus::Acknowledged => "👀 ACKNOWLEDGED",
            };

            let text = format!(
                "*{icon}* [{severity}]\n*Rule:* {rule}\n*Summary:* {summary}\n*Time:* {time}",
                severity = payload.severity.to_uppercase(),
                rule = payload.rule_name,
                summary = payload.summary,
                time = payload.triggered_at.to_rfc3339()
            );

            let body = serde_json::json!({
                "chat_id": config.chat_id,
                "text": text,
                "parse_mode": "Markdown"
            });

            let resp = client.post(safe_url).json(&body).send().await?;
            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let body_text = resp.text().await.unwrap_or_default();
                return Err(NotificationError::DeliveryFailed {
                    status,
                    body: body_text,
                });
            }

            Ok(None)
        }
        NotificationChannelType::Discord => {
            let config: DiscordConfig = serde_json::from_str(&decrypted_json)?;
            let safe_url = validate_url_for_ssrf(&config.webhook_url, allow_loopback).await?;

            let (color, title_prefix) = match payload.status {
                AlertEventStatus::Resolved => (0x2ecc71, "✅ [RESOLVED]"),
                AlertEventStatus::Firing => (0xe74c3c, "🚨 [FIRING]"),
                AlertEventStatus::Acknowledged => (0xf39c12, "👀 [ACKNOWLEDGED]"),
            };

            let body = serde_json::json!({
                "embeds": [{
                    "title": format!("{title_prefix} {}", payload.rule_name),
                    "description": payload.summary,
                    "color": color,
                    "fields": [
                        { "name": "Severity", "value": payload.severity, "inline": true },
                        { "name": "Status", "value": payload.status.as_str(), "inline": true },
                        { "name": "Triggered At", "value": payload.triggered_at.to_rfc3339(), "inline": false }
                    ],
                    "footer": { "text": "AgentLens Observability Platform" }
                }]
            });

            let resp = client.post(safe_url).json(&body).send().await?;
            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let body_text = resp.text().await.unwrap_or_default();
                return Err(NotificationError::DeliveryFailed {
                    status,
                    body: body_text,
                });
            }

            Ok(None)
        }
        NotificationChannelType::Slack => {
            let config: SlackConfig = serde_json::from_str(&decrypted_json)?;
            let safe_url = validate_url_for_ssrf(&config.webhook_url, allow_loopback).await?;

            let icon = match payload.status {
                AlertEventStatus::Resolved => ":white_check_mark:",
                AlertEventStatus::Firing => ":rotating_light:",
                AlertEventStatus::Acknowledged => ":eyes:",
            };

            let body = serde_json::json!({
                "blocks": [
                    {
                        "type": "header",
                        "text": {
                            "type": "plain_text",
                            "text": format!("{icon} Alert {}: {}", payload.status.as_str().to_uppercase(), payload.rule_name)
                        }
                    },
                    {
                        "type": "section",
                        "fields": [
                            { "type": "mrkdwn", "text": format!("*Severity:*\n{}", payload.severity) },
                            { "type": "mrkdwn", "text": format!("*Status:*\n{}", payload.status.as_str()) }
                        ]
                    },
                    {
                        "type": "section",
                        "text": { "type": "mrkdwn", "text": format!("*Summary:*\n{}", payload.summary) }
                    }
                ]
            });

            let resp = client.post(safe_url).json(&body).send().await?;
            if !resp.status().is_success() {
                let status = resp.status().as_u16();
                let body_text = resp.text().await.unwrap_or_default();
                return Err(NotificationError::DeliveryFailed {
                    status,
                    body: body_text,
                });
            }

            Ok(None)
        }
        NotificationChannelType::Email => {
            Err(NotificationError::UnsupportedChannel("email".into()))
        }
    }
}

/// Sends a test notification to verify channel configuration.
pub async fn send_test_notification(
    channel: &NotificationChannel,
    encryption_key: &[u8],
    allow_loopback: bool,
) -> Result<(), NotificationError> {
    let dummy_payload = NotificationPayload {
        event_id: uuid::Uuid::new_v4(),
        rule_id: uuid::Uuid::new_v4(),
        rule_name: "AgentLens Test Notification".into(),
        severity: "info".into(),
        status: AlertEventStatus::Firing,
        summary:
            "This is a test notification confirming your channel configuration works properly."
                .into(),
        details: serde_json::json!({ "test": true }),
        triggered_at: Utc::now(),
        project_name: Some("Test Project".into()),
    };

    deliver_notification(channel, encryption_key, &dummy_payload, allow_loopback).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &[u8] = b"01234567890123456789012345678901";

    #[test]
    fn test_aes_gcm_roundtrip() {
        let plaintext = r#"{"webhook_url":"https://example.com/alerts","secret":"supersecret"}"#;
        let encrypted =
            encrypt_channel_config(TEST_KEY, plaintext).expect("Encryption must succeed");
        assert!(encrypted.starts_with("v1:"));

        let decrypted =
            decrypt_channel_config(TEST_KEY, &encrypted).expect("Decryption must succeed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aes_gcm_tampered_fails() {
        let plaintext = "test_payload";
        let encrypted = encrypt_channel_config(TEST_KEY, plaintext).unwrap();
        let tampered = format!("{encrypted}bad");
        assert!(decrypt_channel_config(TEST_KEY, &tampered).is_err());
    }

    #[test]
    fn test_is_ip_blocked_rfc1918_and_special() {
        // Loopback
        assert!(is_ip_blocked("127.0.0.1".parse().unwrap()));
        assert!(is_ip_blocked("127.255.255.255".parse().unwrap()));
        assert!(is_ip_blocked("::1".parse().unwrap()));

        // RFC 1918 Private
        assert!(is_ip_blocked("10.0.0.1".parse().unwrap()));
        assert!(is_ip_blocked("172.16.0.50".parse().unwrap()));
        assert!(is_ip_blocked("172.31.255.255".parse().unwrap()));
        assert!(is_ip_blocked("192.168.1.1".parse().unwrap()));

        // Cloud Metadata & Link-local
        assert!(is_ip_blocked("169.254.169.254".parse().unwrap()));
        assert!(is_ip_blocked("fe80::1".parse().unwrap()));

        // CGNAT
        assert!(is_ip_blocked("100.64.0.1".parse().unwrap()));
        assert!(is_ip_blocked("100.127.255.255".parse().unwrap()));

        // Unspecified / Broadcast
        assert!(is_ip_blocked("0.0.0.0".parse().unwrap()));
        assert!(is_ip_blocked("255.255.255.255".parse().unwrap()));

        // Public routable IPs (must NOT be blocked)
        assert!(!is_ip_blocked("8.8.8.8".parse().unwrap()));
        assert!(!is_ip_blocked("1.1.1.1".parse().unwrap()));
        assert!(!is_ip_blocked("140.82.121.4".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_ssrf_rejects_loopback_and_metadata_urls() {
        // Direct IP loopback
        assert!(validate_url_for_ssrf("http://127.0.0.1/webhook", false)
            .await
            .is_err());
        assert!(validate_url_for_ssrf("http://localhost/webhook", false)
            .await
            .is_err());
        // Cloud metadata
        assert!(
            validate_url_for_ssrf("http://169.254.169.254/latest/meta-data/", false)
                .await
                .is_err()
        );
        // Private 10.x
        assert!(validate_url_for_ssrf("http://10.0.0.1:8080/hook", false)
            .await
            .is_err());
        // Disallowed scheme
        assert!(validate_url_for_ssrf("file:///etc/passwd", false)
            .await
            .is_err());
        assert!(validate_url_for_ssrf("gopher://127.0.0.1:70", false)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_ssrf_allows_loopback_when_explicitly_flagged() {
        assert!(validate_url_for_ssrf("http://127.0.0.1:8080/webhook", true)
            .await
            .is_ok());
    }

    #[test]
    fn test_mask_credential_string() {
        assert_eq!(mask_credential_string("1234567890"), "1234****7890");
        assert_eq!(mask_credential_string("short"), "********");
    }
}
