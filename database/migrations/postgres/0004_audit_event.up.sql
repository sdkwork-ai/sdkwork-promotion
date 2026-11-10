-- sdkwork:migration
-- id: 0004_audit_event
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Persist an append-only evidence row for every audited code access
--   (`codes.reveal` / `codes.export`).
--   A redeemable coupon code must never leave the service without evidence, so
--   the audited reveal/export operations write this row and read the code in one
--   transaction; if the write fails the operation fails and no code is served.
--   `actor_id` is the IAM-authenticated operator (`PromotionAdminScope`), never a
--   body/query field, and `aggregate_type`/`aggregate_id` identify exactly which
--   code (`promotion_code`) or pool (`promotion_code_batch`) was accessed.
--   `detail_json` is reserved for future structured context; phase 1 leaves it
--   NULL so the evidence row stays a fixed, auditable shape.
--   Rows are append-only: no update or delete path exists outside the
--   transaction that first records `subject_count`.
-- reversible: false
-- rollback: forward-fix (append-only audit evidence carries no backfill that
--   must be undone; dropping the table would destroy the audit trail)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 60s

BEGIN;

CREATE TABLE IF NOT EXISTS promotion_audit_event (
    id TEXT PRIMARY KEY,
    uuid VARCHAR(64) NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    organization_id TEXT NOT NULL DEFAULT '0',
    actor_id TEXT NOT NULL,
    action VARCHAR(64) NOT NULL,            -- codes.reveal | codes.export
    aggregate_type VARCHAR(32) NOT NULL,    -- promotion_code | promotion_code_batch
    aggregate_id TEXT NOT NULL,
    subject_count BIGINT NOT NULL DEFAULT 0,
    detail_json TEXT,
    trace_id VARCHAR(128) NOT NULL DEFAULT '',
    occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CONSTRAINT chk_pae_action CHECK (action IN ('codes.reveal', 'codes.export'))
);
CREATE INDEX IF NOT EXISTS idx_pae_tenant_time ON promotion_audit_event (tenant_id, organization_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_pae_aggregate  ON promotion_audit_event (tenant_id, aggregate_type, aggregate_id, occurred_at DESC);

COMMIT;
