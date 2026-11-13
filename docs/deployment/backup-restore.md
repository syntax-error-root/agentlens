# AgentLens Backup & Disaster Recovery Runbook

This guide documents the procedures for capturing verified backups, executing disaster recovery drills, and restoring AgentLens state across PostgreSQL and S3/MinIO payload object storage.

## Architecture & Target Objectives

- **Recovery Point Objective (RPO):** < 1 hour (periodic hourly snapshots via cron/systemd).
- **Recovery Time Objective (RTO):** < 15 minutes (automated checksum validation and `pg_restore`).
- **Data Layers:**
  1. **PostgreSQL 16:** Authoritative source for organizations, projects, API keys, traces, spans, daily aggregations, alert rules, and audit logs.
  2. **Object Storage (S3 / MinIO):** Offloaded large span payloads (> 64 KiB) indexed by trace and span ID.

---

## 1. Creating a Snapshot

Execute the automated backup utility:

```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/agentlens" \
S3_ENDPOINT="http://localhost:9000" \
S3_BUCKET="agentlens-payloads" \
BACKUP_DIR="./backups" \
./scripts/backup.sh
```

### Backup Artifacts Produced

Each backup creates an isolated timestamped directory (e.g. `./backups/20261113T160000Z/`):
- `agentlens_db.dump`: PostgreSQL database dump in custom binary format (`pg_dump -Fc`).
- `s3_payloads/`: Mirrored S3 payload bucket hierarchy.
- `manifest.json`: Metadata manifest containing timestamp, version, payload byte size, and SHA-256 integrity checksums.

Example `manifest.json`:
```json
{
  "timestamp": "20261113T160000Z",
  "version": "v1.0.0",
  "database_dump": {
    "filename": "agentlens_db.dump",
    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "size_bytes": 10485760
  },
  "object_storage": {
    "bucket": "agentlens-payloads",
    "status": "completed"
  }
}
```

---

## 2. Restoring from Snapshot

The restore script cryptographically validates the SHA-256 hash against `manifest.json` before performing any database write operations.

### Interactive Restore

```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/agentlens" \
./scripts/restore.sh ./backups/20261113T160000Z
```

### Automated / Drill Restore (`--force`)

```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/agentlens" \
./scripts/restore.sh ./backups/20261113T160000Z --force
```

---

## 3. Disaster Recovery Drill Verification Checklist

To certify operational readiness for SOC2 / ISO27001 compliance, perform the following drill steps:

1. **Simulate Disaster:** Spin up a pristine container instance of PostgreSQL and MinIO on a non-production test port.
2. **Execute Restore:** Run `./scripts/restore.sh <snapshot_dir> --force`.
3. **Verify Counts:** Check that row count outputs match expected operational numbers.
4. **Health Check:** Point `agentlens-api` at the restored database and verify `GET /health` returns HTTP 200 with status `"healthy"`.
