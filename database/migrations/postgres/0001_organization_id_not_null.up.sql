-- sdkwork:migration
-- id: 0001_organization_id_not_null
-- engine: postgres
-- module: sdkwork-promotion
-- purpose: Enforce organization_id NOT NULL DEFAULT on all tables in the
--   consolidated baseline. NULL rows (pre-standard data anomalies) are
--   backfilled with the platform sentinel before NOT NULL is set, and
--   NOT NULL columns without an explicit default receive the sentinel
--   default, keeping existing deployments consistent with fresh baseline
--   installs.
-- reversible: false
-- rollback: forward-fix (sentinel backfill is the canonical fix; NULL
--   organization rows are data anomalies)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE promotion_campaign ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_campaign SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_campaign ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_campaign ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_offer ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_offer SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_offer ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_offer ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_offer_version ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_offer_version SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_offer_version ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_offer_version ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_coupon_stock ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_coupon_stock SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_coupon_stock ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_coupon_stock ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_code_batch ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_code_batch SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_code_batch ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_code_batch ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_code SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_code ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_code ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_user_coupon ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_user_coupon SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_user_coupon ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_user_coupon ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_distribution_task ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_distribution_task SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_distribution_task ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_distribution_task ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_distribution_record ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_distribution_record SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_distribution_record ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_distribution_record ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_discount_application ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_discount_application SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_discount_application ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_discount_application ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_member_card ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_member_card SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_member_card ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_member_card ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE promotion_member_card_consumption ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0';
UPDATE promotion_member_card_consumption SET organization_id = '0' WHERE organization_id IS NULL;
ALTER TABLE promotion_member_card_consumption ALTER COLUMN organization_id SET DEFAULT '0';
ALTER TABLE promotion_member_card_consumption ALTER COLUMN organization_id SET NOT NULL;

COMMIT;
