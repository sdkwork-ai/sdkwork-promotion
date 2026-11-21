-- sdkwork:migration
-- id: 0003_coupon_redemption_evidence
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Persist code-redemption evidence and repair three schema gaps the
--   redemption path depends on.
--   1. `promotion_code_redemption_attempt` records every redemption attempt,
--      success and failure, so support can answer "why did this code not work"
--      without reading application logs. Attempts are append-only evidence: no
--      update or delete path exists. `submitted_code_hash`/`submitted_code_suffix`
--      are the only representations of the submitted value, so the table never
--      stores a redeemable secret.
--   2. `promotion_coupon_ledger_entry` never had `organization_id`, so admin
--      scoping had to join through `promotion_coupon_stock`. The column is added
--      with the platform sentinel and backfilled from the owning stock so admin
--      lists can scope directly. The join is kept working by the backfill: rows
--      whose stock is gone stay '0' and remain visible to the platform scope.
--   3. Redemption matches with `UPPER(BTRIM(promotion_code)) = UPPER(BTRIM($3))`,
--      which cannot use `uk_promotion_code_value` and degrades to a sequential
--      scan on the largest promotion table. This migration originally added a
--      non-unique functional index over that exact normalized expression. That
--      index is superseded by the `0007_code_hash` unique hash index: the
--      normalized value is hashed once at write time and the hash is indexed,
--      so no separate expression index is created here anymore (0007 drops it
--      where an earlier run already built it).
-- reversible: false
-- rollback: forward-fix (an append-only evidence table carries no backfill that
--   must be undone; dropping the ledger `organization_id` column would lose the
--   backfilled scope, and the normalized-value index this migration once added
--   is superseded by the 0007 unique hash index)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 120s

BEGIN;

CREATE TABLE IF NOT EXISTS promotion_code_redemption_attempt (
    id TEXT PRIMARY KEY,
    uuid VARCHAR(64) NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    organization_id TEXT NOT NULL DEFAULT '0',
    code_id TEXT,
    stock_id TEXT,
    offer_id TEXT,
    user_coupon_id TEXT,
    order_id TEXT,
    submitted_code_hash VARCHAR(128) NOT NULL,
    submitted_code_suffix VARCHAR(8) NOT NULL,
    subject_type VARCHAR(32) NOT NULL,
    subject_id TEXT NOT NULL,
    channel VARCHAR(32) NOT NULL DEFAULT 'code',
    outcome VARCHAR(16) NOT NULL,
    failure_code VARCHAR(64),
    failure_detail VARCHAR(512),
    benefit_kind VARCHAR(32),
    granted_amount TEXT,
    currency_code VARCHAR(10),
    request_no VARCHAR(128) NOT NULL,
    idempotency_key VARCHAR(200) NOT NULL,
    trace_id VARCHAR(128) NOT NULL DEFAULT '',
    occurred_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CONSTRAINT uk_pca_no UNIQUE (tenant_id, id),
    CONSTRAINT chk_pca_outcome CHECK (outcome IN ('succeeded', 'failed')),
    CONSTRAINT chk_pca_failure CHECK ((outcome = 'succeeded' AND failure_code IS NULL) OR (outcome = 'failed' AND failure_code IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_pca_code    ON promotion_code_redemption_attempt (tenant_id, code_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_pca_hash    ON promotion_code_redemption_attempt (tenant_id, submitted_code_hash, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_pca_subject ON promotion_code_redemption_attempt (tenant_id, subject_type, subject_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_pca_failure ON promotion_code_redemption_attempt (tenant_id, outcome, failure_code, occurred_at DESC);

ALTER TABLE promotion_coupon_ledger_entry ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_coupon_ledger_entry l SET organization_id = s.organization_id
  FROM promotion_coupon_stock s WHERE s.tenant_id = l.tenant_id AND s.id = l.stock_id AND l.organization_id = '0';
CREATE INDEX IF NOT EXISTS idx_promotion_coupon_ledger_entry_org_created
  ON promotion_coupon_ledger_entry (tenant_id, organization_id, created_at DESC);

-- Superseded by `0007_code_hash`: `uk_promotion_code_hash` is the unique index
-- over the normalized-code identity and answers the same equality lookup. The
-- non-unique expression index is intentionally not created here; databases that
-- already ran the original version of this migration drop it in 0007.

COMMIT;
