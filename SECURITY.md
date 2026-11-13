# Security Policy

The AgentLens team takes security vulnerabilities seriously. We appreciate your efforts to responsibly disclose your findings.

---

## Supported Versions

Only the latest release receives security patches and proactive vulnerability remediations.

| Version | Supported |
| :--- | :--- |
| 1.0.x | :white_check_mark: |
| < 1.0 | :x: |

---

## Security Model & Hardening Controls

AgentLens incorporates defense-in-depth mechanisms by design:
- **Zero Raw Credential Storage:** Ingestion API keys are stored hashed with Argon2id; verification is constant-time.
- **Fail-Closed Redaction:** In-stream pattern redactor strips bearer tokens, private keys, passwords, and sensitive dictionary keys prior to disk persistence.
- **SSRF Mitigation:** All external notification dispatchers validate destination IP addresses at socket connect time, blocking internal networks, private subnets (RFC 1918), link-local addresses (RFC 3927), and cloud metadata endpoints (`169.254.169.254`).
- **Encrypted Channels:** Notification webhook URLs, authentication tokens, and bot secrets are encrypted at rest using AES-256-GCM.
- **Non-Root Containers:** Docker images run under non-root unprivileged service accounts (`UID:GID 65532:65532`) with read-only root filesystems and `no-new-privileges:true`.

---

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability in AgentLens, please report it through GitHub Private Vulnerability Reporting or email the security response team:

- **Email:** `default13789@gmail.com` (or open a GitHub Security Advisory)
- **Response Timeline:** We will acknowledge receipt within 24 hours and provide an initial assessment within 72 hours.
- **Coordination:** We coordinate fixes, CVE assignments, and public disclosures with reporting researchers before public release.
