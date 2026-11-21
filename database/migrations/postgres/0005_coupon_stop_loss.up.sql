-- sdkwork:migration
-- id: 0005_coupon_stop_loss
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Stop-loss controls for leaked codes and expired wallet coupons.
--   1. Widens `promotion_audit_event.action` to the full operator vocabulary:
--      the pre-existing audited access actions (`codes.reveal` / `codes.export`)
--      plus the new stop-loss actions (`codes.disable`, `codes.batch_disable`,
--      `user_coupons.void`, `user_coupons.extend`) and the system lifecycle
--      sweep action (`lifecycle.expire`).
--   2. Adds the void evidence columns. A disabled code and a voided wallet
--      coupon keep WHY and WHEN they were stopped, so a later dispute can be
--      answered from the row itself instead of a mutable status only.
--   3. Adds the sweep indexes. The expiry sweep filters `status` plus the
--      instant-parsed `expires_at` window with no tenant predicate; the
--      existing indexes all lead with `tenant_id` (or `owner_user_id`), so none
--      of them can serve that scan.
--   The migration is idempotent: the action constraint is dropped before it is
--   re-added, and the columns/indexes use IF NOT EXISTS.
-- reversible: false
-- rollback: forward-fix (widening a closed vocabulary and adding nullable
--   evidence columns carries no backfill that must be undone)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 60s

BEGIN;

-- 1. Audit vocabulary.
ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;
ALTER TABLE promotion_audit_event ADD CONSTRAINT chk_pae_action
  CHECK (action IN ('codes.reveal','codes.export','codes.disable','codes.batch_disable',
                    'user_coupons.void','user_coupons.extend','lifecycle.expire'));

-- 2. Void evidence columns.
ALTER TABLE promotion_user_coupon ADD COLUMN IF NOT EXISTS voided_at TEXT;
ALTER TABLE promotion_user_coupon ADD COLUMN IF NOT EXISTS void_reason VARCHAR(256);
ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS voided_at TEXT;
ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS void_reason VARCHAR(256);

-- 3. Sweep indexes (status + expiry window, without a tenant lead column).
CREATE INDEX IF NOT EXISTS idx_promotion_user_coupon_status_expires
  ON promotion_user_coupon (tenant_id, status, expires_at);
CREATE INDEX IF NOT EXISTS idx_promotion_code_status_expires
  ON promotion_code (tenant_id, status, expires_at);

COMMIT;
