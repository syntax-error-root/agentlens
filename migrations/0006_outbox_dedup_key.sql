-- 0006_outbox_dedup_key.sql
-- Enforce uniqueness on outbox_jobs dedup_key for idempotent ON CONFLICT (dedup_key) DO NOTHING

CREATE UNIQUE INDEX IF NOT EXISTS idx_outbox_dedup_key
    ON outbox_jobs(dedup_key);
