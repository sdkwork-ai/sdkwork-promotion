-- sdkwork:migration
-- id: 0002_coupon_redemption_alignment
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Align coupon generation with coupon redemption.
--   1. Custom (admin-defined) coupon codes are 4-64 characters, so the batch
--      code_length range widens from 12-32 to 4-64. Generated codes still
--      enforce 12-32 in the service layer; the database only has to admit the
--      admin-defined length.
--   2. promotion_coupon_stock.stock_type was lower-cased by migration 0001, but
--      chk_promotion_coupon_stock_available_le_total still tests the retired
--      uppercase literal 'UNLIMITED'. For an unlimited stock with
--      total_quantity = 0 the first claim/redeem increments claimed_quantity to
--      1 while available_quantity stays 0, so the stale check aborts the
--      redemption with a constraint violation (HTTP 500) instead of issuing the
--      coupon. The constraint is recreated against the canonical lowercase
--      value.
-- reversible: false
-- rollback: forward-fix (both predicates encode the canonical shape; the
--   retired uppercase literal cannot come back)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE promotion_code_batch
    DROP CONSTRAINT IF EXISTS chk_promotion_code_batch_length;
ALTER TABLE promotion_code_batch
    ADD CONSTRAINT chk_promotion_code_batch_length
    CHECK (code_length BETWEEN 4 AND 64);

ALTER TABLE promotion_coupon_stock
    DROP CONSTRAINT IF EXISTS chk_promotion_coupon_stock_available_le_total;
ALTER TABLE promotion_coupon_stock
    ADD CONSTRAINT chk_promotion_coupon_stock_available_le_total
    CHECK (
        stock_type = 'unlimited'
        OR (available_quantity + claimed_quantity <= total_quantity)
    );

COMMIT;
