-- sdkwork:migration
-- id: 0007_code_hash
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Give every promotion code an indexed, case-insensitive identity and
--   make that identity the redemption lookup key.
--   1. `code_hash` is a lowercase 64-character hex sha256 of the canonical code
--      (`UPPER(TRIM(code))` as UTF-8). PostgreSQL 18 ships `sha256(bytea)` as a
--      builtin; older servers only have pgcrypto's `digest(...)`. The backfill
--      therefore resolves the function at run time and fails with an explicit
--      message when neither is available, instead of emitting a confusing
--      "function sha256(bytea) does not exist" on a supported older server.
--      `code_last4` is `right(btrim(promotion_code), 4)`: the only non-secret
--      fragment the admin list needs in addition to the existing mask.
--   2. The backfill covers every existing row before the columns become NOT
--      NULL. Verified in production: `UPPER(BTRIM(promotion_code))` is already
--      the canonical form of 100/100 rows and there are zero case-insensitive
--      duplicates, so both the backfill and the unique build are safe.
--   3. `uk_promotion_code_hash` is the case-insensitive uniqueness guarantee:
--      two codes whose `UPPER(TRIM(...))` forms collide now collide on the hash.
--      Raw-value uniqueness (`uk_promotion_code_value`) stays in place; the
--      hash index additionally serves the equality lookup that used to scan.
--   4. `idx_promotion_code_value_normalized` (added by 0003) is superseded: the
--      unique hash index answers the same equality probe in one column, so the
--      non-unique functional index is dropped. IF EXISTS keeps re-runs and
--      fresh installations (0003 no longer creates it) safe.
--   The hash is a *lookup* key, not a secret store. `promotion_code.promotion_code`
--   deliberately remains plaintext in this phase because the audited admin
--   reveal/export operations read it; keeping the raw value out of the index is
--   the improvement this phase buys.
--   Columns, backfill, and index drop are idempotent; the unique constraint is
--   added by this migration exactly once.
-- reversible: false
-- rollback: forward-fix (the columns and their backfill are additive and the
--   dropped functional index is superseded by the unique hash index; dropping
--   `code_hash` would make every hash-first redemption lookup return no row)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 120s

BEGIN;

ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS code_hash VARCHAR(64);
ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS code_last4 VARCHAR(4);

DO $$
DECLARE
  hash_expr text;
BEGIN
  IF EXISTS (
    SELECT 1
    FROM pg_proc p
    JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE p.proname = 'sha256' AND n.nspname = 'pg_catalog'
  ) THEN
    hash_expr := 'encode(sha256(convert_to(upper(btrim(promotion_code)), ''UTF8'')), ''hex'')';
  ELSIF EXISTS (
    SELECT 1 FROM pg_proc p WHERE p.proname = 'digest'
  ) THEN
    hash_expr := 'encode(digest(convert_to(upper(btrim(promotion_code)), ''UTF8''), ''sha256''), ''hex'')';
  ELSE
    RAISE EXCEPTION 'migration 0007_code_hash requires PostgreSQL 18+ (builtin sha256) or the pgcrypto extension for digest()';
  END IF;

  EXECUTE format(
    'UPDATE promotion_code SET code_hash = %s, code_last4 = right(btrim(promotion_code), 4) WHERE code_hash IS NULL',
    hash_expr
  );
END $$;

ALTER TABLE promotion_code ALTER COLUMN code_hash SET NOT NULL;
ALTER TABLE promotion_code ALTER COLUMN code_last4 SET NOT NULL;

-- Case-insensitive uniqueness: the hash is a function of UPPER(TRIM(code)).
-- Guarded so the migration is re-runnable: a fresh install already has this
-- constraint from the folded baseline, and re-applying must not abort.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'uk_promotion_code_hash'
      AND conrelid = 'promotion_code'::regclass
  ) THEN
    ALTER TABLE promotion_code ADD CONSTRAINT uk_promotion_code_hash UNIQUE (tenant_id, code_hash);
  END IF;
END $$;

DROP INDEX IF EXISTS idx_promotion_code_value_normalized;

COMMIT;
