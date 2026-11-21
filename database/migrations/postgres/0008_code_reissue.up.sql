-- sdkwork:migration
-- id: 0008_code_reissue
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Let an operator replace one unused promotion code with a freshly
--   issued one when the customer lost it or the value leaked before use.
--   1. `replaced_by_code_id` (on the old row) and `reissued_from_code_id` (on the
--      new row) are the lineage pair. Both are nullable TEXT: existing codes
--      were never reissued, and a replacement's predecessor is always another
--      `promotion_code.id`.
--   2. `idx_promotion_code_reissued_from` answers the lineage walk that starts
--      from a code and finds the code it replaced (support investigation). It
--      leads with `tenant_id`, matching every other promotion index.
--   3. `chk_pae_action` gains `codes.reissue`: the replacement is one audited
--      operator mutation that touches two codes, recorded with
--      `subject_count = 2` and both ids in `detail_json`.
--   The new columns are additive and the constraint is dropped before it is
--   re-added, so re-running this migration is safe. No table is added, so the
--   contract registries (`database/contract/schema.yaml`,
--   `database/contract/table-registry.json`) need no change.
-- reversible: false
-- rollback: forward-fix (dropping the lineage columns would orphan the reissued
--   codes from the rows they replaced; the widened CHECK is additive)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 5s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS reissued_from_code_id TEXT;
ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS replaced_by_code_id TEXT;

CREATE INDEX IF NOT EXISTS idx_promotion_code_reissued_from
  ON promotion_code (tenant_id, reissued_from_code_id);

ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;
ALTER TABLE promotion_audit_event ADD CONSTRAINT chk_pae_action
  CHECK (action IN ('codes.reveal','codes.export','codes.disable','codes.batch_disable',
                    'user_coupons.void','user_coupons.extend','lifecycle.expire',
                    'discount_applications.settle','discount_applications.release',
                    'budget.release_expired','codes.reissue'));

COMMIT;
