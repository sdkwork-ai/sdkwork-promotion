-- sdkwork:migration
-- id: 0006_budget_and_settlement
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Cap monetary give-away and give `promotion_discount_application` a
--   settlement lifecycle.
--   Before this migration an applied coupon stayed `applied` forever (nothing
--   ever wrote `settled`/`released`) and no column capped how much money a
--   coupon stock could hand out.
--   1. `promotion_budget_account` is the per-stock hard cap. The check
--      constraint is the invariant every mutation must preserve:
--      `available + reserved + used <= total`. A reservation moves money from
--      `available` to `reserved`; a settlement moves it from `reserved` to
--      `used`; a release moves it back from `reserved` to `available`.
--   2. `promotion_budget_ledger_entry` is the append-only evidence trail for
--      every one of those moves. `amount_minor` is always positive and
--      `direction` carries the sign; the `_after_minor` columns are the account
--      state immediately after the move (all three must sum to <= the account
--      total, so a ledger row is self-checking).
--   3. `promotion_discount_application` gains the budget link and the
--      reservation deadline the stale-reservation sweep filters on. The
--      partial index is scoped to `status = 'applied'` because only live
--      reservations are ever swept.
--   4. `promotion_coupon_stock.budget_amount_minor` is the optional cap an
--      operator sets at stock creation (NULL = no cap, existing rows unchanged).
--   5. The audit vocabulary is widened with the settlement lifecycle and the
--      system sweep action.
--   Money unit rule for every new column: `*_minor BIGINT` holds integer minor
--   units. Legacy decimal-major strings (`promotion_offer_version.discount_value`,
--   `minimum_amount`) are converted at the service boundary; no float and no
--   implicit division by 100 is ever stored here.
--   The migration is idempotent: tables/indexes/columns use IF NOT EXISTS and
--   the action constraint is dropped before it is re-added.
-- reversible: false
-- rollback: forward-fix (the new tables/columns are additive and carry no
--   backfill that must be undone; dropping the ledger would destroy budget
--   evidence)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 60s

BEGIN;

-- 1. Budget account (hard cap) and 2. append-only budget ledger.
CREATE TABLE IF NOT EXISTS promotion_budget_account (
    id TEXT PRIMARY KEY,
    uuid VARCHAR(64) NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    organization_id TEXT NOT NULL DEFAULT '0',
    budget_no VARCHAR(128) NOT NULL,
    offer_id TEXT NOT NULL,
    offer_version_id TEXT,
    stock_id TEXT,
    currency_code VARCHAR(3) NOT NULL,
    total_amount_minor BIGINT NOT NULL,          -- hard cap, > 0
    available_amount_minor BIGINT NOT NULL,      -- never-negative pool
    reserved_amount_minor BIGINT NOT NULL DEFAULT 0,
    used_amount_minor BIGINT NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    version BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CONSTRAINT uk_pba_no UNIQUE (tenant_id, budget_no),
    CONSTRAINT uk_pba_stock UNIQUE (tenant_id, stock_id),
    CONSTRAINT chk_pba_amounts CHECK (
        total_amount_minor > 0 AND available_amount_minor >= 0
        AND reserved_amount_minor >= 0 AND used_amount_minor >= 0
        AND available_amount_minor + reserved_amount_minor + used_amount_minor <= total_amount_minor)
);
CREATE TABLE IF NOT EXISTS promotion_budget_ledger_entry (
    id TEXT PRIMARY KEY,
    uuid VARCHAR(64) NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL,
    organization_id TEXT NOT NULL DEFAULT '0',
    budget_account_id TEXT NOT NULL,
    application_id TEXT,
    direction VARCHAR(16) NOT NULL,              -- reserve | settle | release
    amount_minor BIGINT NOT NULL,                -- always positive; direction carries the sign
    available_after_minor BIGINT NOT NULL,
    reserved_after_minor BIGINT NOT NULL,
    used_after_minor BIGINT NOT NULL,
    currency_code VARCHAR(3) NOT NULL,
    request_no VARCHAR(128) NOT NULL,
    idempotency_key VARCHAR(200) NOT NULL,
    occurred_at TEXT NOT NULL,
    CONSTRAINT uk_pble_idem UNIQUE (tenant_id, idempotency_key),
    CONSTRAINT chk_pble_direction CHECK (direction IN ('reserve','settle','release'))
);
CREATE INDEX IF NOT EXISTS idx_pble_account ON promotion_budget_ledger_entry (tenant_id, budget_account_id, occurred_at DESC);

-- 3. Reservation link + deadline on the application row.
ALTER TABLE promotion_discount_application ADD COLUMN IF NOT EXISTS budget_account_id TEXT;
ALTER TABLE promotion_discount_application ADD COLUMN IF NOT EXISTS reservation_expires_at TEXT;
CREATE INDEX IF NOT EXISTS idx_promotion_discount_application_status_expires
  ON promotion_discount_application (tenant_id, status, reservation_expires_at)
  WHERE status = 'applied';   -- partial index for the stale-reservation sweep

-- 4. Optional per-stock cap chosen at creation (NULL = no cap).
ALTER TABLE promotion_coupon_stock ADD COLUMN IF NOT EXISTS budget_amount_minor BIGINT;

-- 5. Audit vocabulary.
ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;
ALTER TABLE promotion_audit_event ADD CONSTRAINT chk_pae_action
  CHECK (action IN ('codes.reveal','codes.export','codes.disable','codes.batch_disable',
                    'user_coupons.void','user_coupons.extend','lifecycle.expire',
                    'discount_applications.settle','discount_applications.release',
                    'budget.release_expired'));

COMMIT;
