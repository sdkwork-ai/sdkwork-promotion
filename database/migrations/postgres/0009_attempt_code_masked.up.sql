-- sdkwork:migration
-- id: 0009_attempt_code_masked
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Persist the display mask of the code a customer submitted, so the
--   admin redemption-attempt ledger can show more than the last four characters
--   without ever storing the redeemable value.
--   1. `submitted_code_masked` is a display-only derivative of the submitted
--      code: codes of nine characters or more keep their first and last four
--      characters (`3C76A1B2C3D44E21` -> `3C76****4E21`), shorter codes expose
--      only the fixed `****` mask. The column is VARCHAR(80) and NOT NULL with
--      an empty default so the append-only write path always binds it and a
--      value can never be NULL.
--   2. Existing rows are backfilled with `****` plus the suffix they already
--      store. That adds no new information (the suffix column is already part of
--      the ledger contract) and keeps every historical row renderable.
--   The column is additive with a default, so re-running this migration is safe.
--   No table is added, so the contract registries (`database/contract/schema.yaml`,
--   `database/contract/table-registry.json`) need no change.
-- reversible: false
-- rollback: forward-fix (dropping the column would discard the display mask of
--   every recorded attempt; the mask is a display derivative and the suffix
--   column remains the stable support fragment)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 5s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE promotion_code_redemption_attempt
  ADD COLUMN IF NOT EXISTS submitted_code_masked VARCHAR(80) NOT NULL DEFAULT '';

UPDATE promotion_code_redemption_attempt
  SET submitted_code_masked = '****' || submitted_code_suffix
  WHERE submitted_code_masked = '';

COMMIT;
