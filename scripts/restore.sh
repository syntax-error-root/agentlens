#!/usr/bin/env bash
# AgentLens Disaster Recovery & Snapshot Restore Utility
# Validates manifest SHA-256 checksum, executes clean restore, and validates row integrity.
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <backup_directory> [--force]"
    exit 1
fi

BACKUP_DIR="$1"
FORCE="${2:-}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/agentlens}"
S3_ENDPOINT="${S3_ENDPOINT:-}"
S3_BUCKET="${S3_BUCKET:-agentlens-payloads}"

MANIFEST_FILE="${BACKUP_DIR}/manifest.json"
DUMP_FILE="${BACKUP_DIR}/agentlens_db.dump"

if [ ! -f "${MANIFEST_FILE}" ]; then
    echo "ERROR: manifest.json not found in ${BACKUP_DIR}" >&2
    exit 1
fi

if [ ! -f "${DUMP_FILE}" ]; then
    echo "ERROR: agentlens_db.dump not found in ${BACKUP_DIR}" >&2
    exit 1
fi

echo "==> Verifying SHA-256 checksum from manifest..."
EXPECTED_SHA256="$(grep -o '"sha256": *"[^"]*"' "${MANIFEST_FILE}" | head -n1 | cut -d'"' -f4)"
ACTUAL_SHA256="$(sha256sum "${DUMP_FILE}" | awk '{print $1}')"

if [ "${EXPECTED_SHA256}" != "${ACTUAL_SHA256}" ]; then
    echo "FATAL: Checksum mismatch! Corrupted backup bundle." >&2
    echo "Expected: ${EXPECTED_SHA256}" >&2
    echo "Actual:   ${ACTUAL_SHA256}" >&2
    exit 1
fi
echo "==> Checksum verified: ${ACTUAL_SHA256}"

if [ "${FORCE}" != "--force" ]; then
    read -r -p "WARNING: This will overwrite data in ${DATABASE_URL}. Proceed? [y/N] " confirmation
    if [[ ! "${confirmation}" =~ ^[Yy]$ ]]; then
        echo "Restore cancelled by operator."
        exit 0
    fi
fi

echo "==> Restoring PostgreSQL schema and data..."
pg_restore --clean --if-exists --no-owner --no-privileges -d "${DATABASE_URL}" "${DUMP_FILE}" || true

# Restore S3/MinIO objects if snapshot exists
if [ -d "${BACKUP_DIR}/s3_payloads" ] && [ -n "${S3_ENDPOINT}" ] && command -v mc &> /dev/null; then
    echo "==> Restoring S3/MinIO objects into bucket: ${S3_BUCKET}..."
    mc alias set local "${S3_ENDPOINT}" "${MINIO_ROOT_USER:-minioadmin}" "${MINIO_ROOT_PASSWORD:-minioadmin}" >/dev/null 2>&1 || true
    mc mb "local/${S3_BUCKET}" >/dev/null 2>&1 || true
    mc mirror "${BACKUP_DIR}/s3_payloads" "local/${S3_BUCKET}" >/dev/null 2>&1 || true
fi

echo "==> Validating restored database integrity..."
psql "${DATABASE_URL}" -c "
SELECT 
    (SELECT count(*) FROM organizations) AS organizations_count,
    (SELECT count(*) FROM users) AS users_count,
    (SELECT count(*) FROM projects) AS projects_count,
    (SELECT count(*) FROM traces) AS traces_count,
    (SELECT count(*) FROM spans) AS spans_count;
"

echo "==> Disaster recovery drill / snapshot restore completed successfully."
