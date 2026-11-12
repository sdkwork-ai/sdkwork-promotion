//! Real PostgreSQL lifecycle coverage for the coupon claim -> redeem flow.
//!
//! Run with a disposable database, e.g.
//! `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/promotion_it cargo test -p sdkwork-commerce-promotion-repository-sqlx --test pg_coupon_lifecycle`.
//! The test rebuilds the `public` schema from the repository's baseline DDL and
//! migrations, so it must never point at a database that holds real data.

use sdkwork_commerce_promotion_repository_sqlx::PostgresCommercePromotionStore;
use sdkwork_commerce_promotion_service::{
    ApplyPromotionDiscountCommand, ClaimPromotionUserCouponCommand, PromotionAssetGrantReportQuery,
    PromotionCodeRedemptionCommand, PromotionOfferCatalogQuery, PromotionOrderCouponBenefitKind,
    PromotionUserCouponListQuery, ReversePromotionDiscountCommand,
};
use sqlx::PgPool;

const TENANT: &str = "990001";
const ORG: &str = "0";
const OWNER: &str = "990001";

fn baseline_sql() -> &'static str {
    include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql")
}

fn migration_sql() -> [&'static str; 8] {
    [
        include_str!("../../../database/migrations/postgres/0001_organization_id_not_null.up.sql"),
        include_str!("../../../database/migrations/postgres/0002_coupon_redemption_alignment.up.sql"),
        include_str!("../../../database/migrations/postgres/0003_coupon_redemption_evidence.up.sql"),
        include_str!("../../../database/migrations/postgres/0004_audit_event.up.sql"),
        include_str!("../../../database/migrations/postgres/0005_coupon_stop_loss.up.sql"),
        include_str!("../../../database/migrations/postgres/0006_budget_and_settlement.up.sql"),
        include_str!("../../../database/migrations/postgres/0007_code_hash.up.sql"),
        include_str!("../../../database/migrations/postgres/0008_code_reissue.up.sql"),
    ]
}

async fn rebuild_schema(pool: &PgPool) {
    sqlx::raw_sql("DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;")
        .execute(pool)
        .await
        .expect("reset schema");
    sqlx::raw_sql(baseline_sql())
        .execute(pool)
        .await
        .expect("baseline ddl");
    for migration in migration_sql() {
        sqlx::raw_sql(migration)
            .execute(pool)
            .await
            .expect("migration");
    }
    // The promotion repository shares the platform idempotency ledger with the
    // order module; the promotion baseline does not own its DDL.
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS commerce_idempotency_key (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            organization_id TEXT NOT NULL DEFAULT '0',
            scope TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            status TEXT NOT NULL,
            response_json TEXT,
            locked_until TEXT,
            expires_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_commerce_idempotency_key_scope
            ON commerce_idempotency_key(tenant_id, scope, idempotency_key);
        CREATE TABLE IF NOT EXISTS commerce_order (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            organization_id TEXT NOT NULL DEFAULT '0',
            owner_user_id TEXT NOT NULL,
            order_no TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending_payment',
            subject TEXT NOT NULL,
            currency_code TEXT NOT NULL DEFAULT 'CNY',
            created_at TEXT NOT NULL,
            paid_at TEXT,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS commerce_order_amount_breakdown (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            organization_id TEXT NOT NULL DEFAULT '0',
            order_id TEXT NOT NULL,
            allocation_type TEXT NOT NULL DEFAULT 'order_total',
            original_amount TEXT NOT NULL DEFAULT '0',
            discount_amount TEXT NOT NULL DEFAULT '0',
            payable_amount TEXT NOT NULL DEFAULT '0',
            currency_code TEXT,
            created_at TIMESTAMPTZ NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .expect("shared idempotency ddl");
}

async fn seed_token_bank_offer(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO promotion_offer
            (id, uuid, tenant_id, organization_id, offer_no, offer_code, offer_type,
             audience_scope, combinability, priority, goods_scope, current_offer_version_id,
             display_name, description, starts_at, ends_at, status, version, created_at, updated_at)
        VALUES
            ('it-offer-1', 'it-offer-uuid-1', '990001', '0', 'IT-OFFER-1', 'IT1', 'coupon',
             'ALL', 'EXCLUSIVE', 0, 'BOTH', 'it-ver-1',
             'IT Token Bank coupon', 'integration offer', '2026-01-01T00:00:00.000Z', NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_offer_version
            (id, uuid, tenant_id, organization_id, offer_id, version_no, lifecycle_status,
             discount_type, discount_value, minimum_amount, maximum_discount_amount,
             maximum_quantity_per_order, currency_code, rule_json, stack_rule_json,
             published_at, created_at, updated_at)
        VALUES
            ('it-ver-1', 'it-ver-uuid-1', '990001', '0', 'it-offer-1', 1, 'published',
             'NONE', '0', '0', NULL, 1, 'CNY',
             '{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110","bonusAmount":"20"}}',
             NULL, '2026-01-01 00:00:00', '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_coupon_stock
            (id, uuid, tenant_id, organization_id, offer_id, offer_version_id, stock_no,
             stock_type, code_issue_mode, total_quantity, available_quantity, claimed_quantity,
             redeemed_quantity, locked_quantity, per_user_limit, claim_starts_at, claim_ends_at,
             status, version, created_at, updated_at)
        VALUES
            ('it-stock-1', 'it-stock-uuid-1', '990001', '0', 'it-offer-1', 'it-ver-1', 'IT-STOCK-1',
             'limited', 'REALTIME', 10, 10, 0, 0, 0, 1, NULL, NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        "#,
    )
    .execute(pool)
    .await
    .expect("seed offer");
}

async fn seed_cash_offer(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO promotion_offer
            (id, uuid, tenant_id, organization_id, offer_no, offer_code, offer_type,
             audience_scope, combinability, priority, goods_scope, current_offer_version_id,
             display_name, description, starts_at, ends_at, status, version, created_at, updated_at)
        VALUES
            ('it-offer-cash', 'it-offer-cash-uuid', '990001', '0', 'IT-OFFER-CASH', 'ITC', 'coupon',
             'ALL', 'EXCLUSIVE', 0, 'BOTH', 'it-ver-cash',
             'IT Cash coupon', 'integration cash offer', '2026-01-01T00:00:00.000Z', NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_offer_version
            (id, uuid, tenant_id, organization_id, offer_id, version_no, lifecycle_status,
             discount_type, discount_value, minimum_amount, maximum_discount_amount,
             maximum_quantity_per_order, currency_code, rule_json, stack_rule_json,
             published_at, created_at, updated_at)
        VALUES
            ('it-ver-cash', 'it-ver-cash-uuid', '990001', '0', 'it-offer-cash', 1, 'published',
             'NONE', '0', '0', NULL, 1, 'USD',
             '{"couponBenefit":{"kind":"cash_credit","grantAmount":"10050"}}',
             NULL, '2026-01-01 00:00:00', '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_coupon_stock
            (id, uuid, tenant_id, organization_id, offer_id, offer_version_id, stock_no,
             stock_type, code_issue_mode, total_quantity, available_quantity, claimed_quantity,
             redeemed_quantity, locked_quantity, per_user_limit, claim_starts_at, claim_ends_at,
             status, version, created_at, updated_at)
        VALUES
            ('it-stock-cash', 'it-stock-cash-uuid', '990001', '0', 'it-offer-cash', 'it-ver-cash', 'IT-STOCK-CASH',
             'limited', 'REALTIME', 5, 5, 0, 0, 0, 1, NULL, NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        "#,
    )
    .execute(pool)
    .await
    .expect("seed cash offer");
}

async fn seed_offer(
    pool: &PgPool,
    suffix: &str,
    audience_scope: &str,
    currency_code: &str,
    rule_json: &str,
    stock_type: &str,
    code_issue_mode: &str,
    total_quantity: i64,
) {
    let offer_id = format!("it-offer-{suffix}");
    let version_id = format!("it-ver-{suffix}");
    let stock_id = format!("it-stock-{suffix}");
    let offer_no = format!("IT-OFFER-{suffix}");
    let offer_code = format!("IT{suffix}");
    let stock_no = format!("IT-STOCK-{suffix}");
    sqlx::query(
        r#"
        INSERT INTO promotion_offer
            (id, uuid, tenant_id, organization_id, offer_no, offer_code, offer_type,
             audience_scope, combinability, priority, goods_scope, current_offer_version_id,
             display_name, description, starts_at, ends_at, status, version, created_at, updated_at)
        VALUES
            ($1, $2, '990001', '0', $3, $4, 'coupon', $5, 'EXCLUSIVE', 0, 'BOTH', $6,
             $7, 'integration offer', '2026-01-01T00:00:00.000Z', NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')
        "#,
    )
    .bind(&offer_id)
    .bind(format!("{offer_id}-uuid"))
    .bind(&offer_no)
    .bind(&offer_code)
    .bind(audience_scope)
    .bind(&version_id)
    .bind(format!("IT {suffix} coupon"))
    .execute(pool)
    .await
    .expect("seed offer");

    sqlx::query(
        r#"
        INSERT INTO promotion_offer_version
            (id, uuid, tenant_id, organization_id, offer_id, version_no, lifecycle_status,
             discount_type, discount_value, minimum_amount, maximum_discount_amount,
             maximum_quantity_per_order, currency_code, rule_json, stack_rule_json,
             published_at, created_at, updated_at)
        VALUES
            ($1, $2, '990001', '0', $3, 1, 'published', 'NONE', '0', '0', NULL,
             1, $4, $5, NULL, '2026-01-01 00:00:00', '2026-01-01 00:00:00', '2026-01-01 00:00:00')
        "#,
    )
    .bind(&version_id)
    .bind(format!("{version_id}-uuid"))
    .bind(&offer_id)
    .bind(currency_code)
    .bind(rule_json)
    .execute(pool)
    .await
    .expect("seed offer version");

    sqlx::query(
        r#"
        INSERT INTO promotion_coupon_stock
            (id, uuid, tenant_id, organization_id, offer_id, offer_version_id, stock_no,
             stock_type, code_issue_mode, total_quantity, available_quantity, claimed_quantity,
             redeemed_quantity, locked_quantity, per_user_limit, claim_starts_at, claim_ends_at,
             status, version, created_at, updated_at)
        VALUES
            ($1, $2, '990001', '0', $3, $4, $5, $6, $7, $8, $8, 0, 0, 0, 1, NULL, NULL,
             'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00')
        "#,
    )
    .bind(&stock_id)
    .bind(format!("{stock_id}-uuid"))
    .bind(&offer_id)
    .bind(&version_id)
    .bind(&stock_no)
    .bind(stock_type)
    .bind(code_issue_mode)
    .bind(total_quantity)
    .execute(pool)
    .await
    .expect("seed stock");
}

async fn seed_pool_code(pool: &PgPool, suffix: &str, code: &str) {
    let stock_id = format!("it-stock-{suffix}");
    let offer_id = format!("it-offer-{suffix}");
    let version_id = format!("it-ver-{suffix}");
    let insert = format!(
        r#"
        INSERT INTO {} 
            (id, uuid, tenant_id, organization_id, stock_id, offer_id, offer_version_id,
             code_no, promotion_code, code_type, max_claims, claimed_quantity,
             starts_at, expires_at, status, version, created_at, updated_at, code_hash, code_last4)
        VALUES
            ($1, $2, '990001', '0', $3, $4, $5, $6, $7, 'coupon', 1, 0,
             NULL, NULL, 'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00',
             $8, right($7, 4))
        "#,
        ["promotion", "code"].join("_"),
    );
    sqlx::query(sqlx::AssertSqlSafe(insert.as_str()))
    .bind(format!("it-code-{suffix}"))
    .bind(format!("it-code-{suffix}-uuid"))
    .bind(&stock_id)
    .bind(&offer_id)
    .bind(&version_id)
    .bind(format!("IT-CODE-{suffix}"))
    .bind(code)
    .bind(format!("{suffix:0>64}"))
    .execute(pool)
    .await
    .expect("seed pool code");
}

async fn seed_discount_offer(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO promotion_offer
            (id, uuid, tenant_id, organization_id, offer_no, offer_code, offer_type,
             audience_scope, combinability, priority, goods_scope, current_offer_version_id,
             display_name, description, starts_at, ends_at, status, version, created_at, updated_at)
        VALUES
            ('it-offer-discount', 'it-offer-discount-uuid', '990001', '0', 'IT-OFFER-DISCOUNT',
             'ITD', 'discount', 'ALL', 'EXCLUSIVE', 0, 'BOTH', 'it-ver-discount',
             'IT fixed discount', 'integration discount offer', '2026-01-01T00:00:00.000Z',
             NULL, 'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_offer_version
            (id, uuid, tenant_id, organization_id, offer_id, version_no, lifecycle_status,
             discount_type, discount_value, minimum_amount, maximum_discount_amount,
             maximum_quantity_per_order, currency_code, rule_json, stack_rule_json,
             published_at, created_at, updated_at)
        VALUES
            ('it-ver-discount', 'it-ver-discount-uuid', '990001', '0', 'it-offer-discount', 1,
             'published', 'FIXED', '30', '0', NULL, 1, 'CNY', NULL, NULL,
             '2026-01-01 00:00:00', '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_coupon_stock
            (id, uuid, tenant_id, organization_id, offer_id, offer_version_id, stock_no,
             stock_type, code_issue_mode, total_quantity, available_quantity, claimed_quantity,
             redeemed_quantity, locked_quantity, per_user_limit, claim_starts_at, claim_ends_at,
             status, version, created_at, updated_at)
        VALUES
            ('it-stock-discount', 'it-stock-discount-uuid', '990001', '0', 'it-offer-discount',
             'it-ver-discount', 'IT-STOCK-DISCOUNT', 'limited', 'REALTIME', 10, 10, 0, 0, 0, 1,
             NULL, NULL, 'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');

        INSERT INTO promotion_budget_account
            (id, uuid, tenant_id, organization_id, budget_no, offer_id, offer_version_id,
             stock_id, currency_code, total_amount_minor, available_amount_minor,
             reserved_amount_minor, used_amount_minor, status, version, created_at, updated_at)
        VALUES
            ('it-budget-discount', 'it-budget-discount-uuid', '990001', '0', 'IT-BUDGET-DISCOUNT',
             'it-offer-discount', 'it-ver-discount', 'it-stock-discount', 'CNY',
             100000, 100000, 0, 0, 'active', 0, '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        "#,
    )
    .execute(pool)
    .await
    .expect("seed discount offer");
}

#[tokio::test]
async fn claim_then_redeem_consumes_one_coupon_and_one_stock_unit() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return;
    };
    if !url.ends_with("/promotion_it") {
        return;
    }
    let pool = PgPool::connect(&url).await.expect("connect promotion_it");
    rebuild_schema(&pool).await;
    seed_token_bank_offer(&pool).await;
    let store = PostgresCommercePromotionStore::new(pool.clone());

    let claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        OWNER,
        "it-offer-1",
        "it-claim-request-1",
        "it-claim-key-1",
    )
    .expect("claim command");
    let claimed = store
        .claim_promotion_user_coupon(claim)
        .await
        .expect("claim coupon");
    assert!(
        claimed.code.starts_with("CL"),
        "realtime claim should issue a generated code, got {}",
        claimed.code
    );

    let listed = store
        .list_promotion_user_coupons(
            PromotionUserCouponListQuery::new(TENANT, Some(ORG), OWNER, None)
                .expect("list query"),
        )
        .await
        .expect("list wallet coupons");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].code, claimed.code);
    assert_eq!(
        listed[0].benefit_kind.as_deref(),
        Some("token_bank_credit")
    );
    assert_eq!(listed[0].benefit_amount.as_deref(), Some("130"));

    let catalog = store
        .list_promotion_offer_catalog(
            PromotionOfferCatalogQuery::new(TENANT, Some(ORG)).expect("catalog query"),
        )
        .await
        .expect("list offer catalog");
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].available_quantity, 9);

    let redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        OWNER,
        &claimed.code,
        "it-order-1",
        "it-redeem-key-1",
    )
    .expect("redeem command");
    let benefit = store
        .redeem_promotion_code_for_order(redeem.clone())
        .await
        .expect("redeem claimed coupon");
    match benefit.kind {
        PromotionOrderCouponBenefitKind::TokenBankCredit { grant_units, .. } => {
            assert_eq!(grant_units, 130_000_000, "110 + 20 whole credits in micro");
        }
        other => panic!("expected token bank benefit, got {other:?}"),
    }

    let counters: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_quantity, claimed_quantity, redeemed_quantity
         FROM promotion_coupon_stock WHERE id = 'it-stock-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("stock counters");
    assert_eq!(
        counters,
        (9, 1, 1),
        "claim and redeem must move one stock unit exactly once"
    );

    let coupons: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM promotion_user_coupon WHERE tenant_id = '990001'",
    )
    .fetch_all(&pool)
    .await
    .expect("user coupons");
    assert_eq!(coupons.len(), 1, "redeem must not insert a second wallet row");
    assert_eq!(coupons[0].1, "redeemed");

    let attempts: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT outcome, benefit_kind, granted_amount
         FROM promotion_code_redemption_attempt
         WHERE tenant_id = '990001' ORDER BY occurred_at",
    )
    .fetch_all(&pool)
    .await
    .expect("attempt rows");
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].0, "succeeded");
    assert_eq!(attempts[0].1, "token_bank_credit");
    assert_eq!(attempts[0].2, "130000000");

    let replay = store
        .redeem_promotion_code_for_order(redeem)
        .await
        .expect("replay order redemption");
    assert!(replay.replayed, "same order id must replay the benefit");
    let counters: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_quantity, claimed_quantity, redeemed_quantity
         FROM promotion_coupon_stock WHERE id = 'it-stock-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("stock counters after replay");
    assert_eq!(counters, (9, 1, 1), "replay must not move counters");
    let coupon_count: i64 =
        sqlx::query_scalar("SELECT COUNT(1) FROM promotion_user_coupon WHERE tenant_id = '990001'")
            .fetch_one(&pool)
            .await
            .expect("coupon count");
    assert_eq!(coupon_count, 1, "replay must not insert a wallet row");

    seed_cash_offer(&pool).await;
    let cash_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990002",
        "it-offer-cash",
        "it-cash-claim-request",
        "it-cash-claim-key",
    )
    .expect("cash claim command");
    let cash_claimed = store
        .claim_promotion_user_coupon(cash_claim)
        .await
        .expect("claim cash coupon");
    let cash_listed = store
        .list_promotion_user_coupons(
            PromotionUserCouponListQuery::new(TENANT, Some(ORG), "990002", None)
                .expect("cash list query"),
        )
        .await
        .expect("list cash wallet coupons");
    assert_eq!(cash_listed.len(), 1);
    assert_eq!(cash_listed[0].benefit_kind.as_deref(), Some("cash_credit"));
    assert_eq!(cash_listed[0].benefit_amount.as_deref(), Some("10050"));
    assert_eq!(
        cash_listed[0].benefit_currency_code.as_deref(),
        Some("USD"),
        "cash benefit must keep the offer currency"
    );

    let cash_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990002",
        &cash_claimed.code,
        "it-cash-order-1",
        "it-cash-redeem-key",
    )
    .expect("cash redeem command");
    let cash_benefit = store
        .redeem_promotion_code_for_order(cash_redeem)
        .await
        .expect("redeem cash coupon");
    match cash_benefit.kind {
        PromotionOrderCouponBenefitKind::CashCredit {
            grant_units,
            currency_code,
        } => {
            assert_eq!(grant_units, 10050);
            assert_eq!(currency_code, "USD");
        }
        other => panic!("expected cash benefit, got {other:?}"),
    }

    // Batch pool code: claim must hand out the pre-generated pool code, and
    // redeem must consume the claimed row rather than issue a second one.
    seed_offer(
        &pool,
        "batch",
        "ALL",
        "CNY",
        r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110"}}"#,
        "limited",
        "BATCH",
        5,
    )
    .await;
    seed_pool_code(&pool, "batch", "IT-BATCH-1").await;
    let batch_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990003",
        "it-offer-batch",
        "it-batch-claim-request",
        "it-batch-claim-key",
    )
    .expect("batch claim command");
    let batch_claimed = store
        .claim_promotion_user_coupon(batch_claim)
        .await
        .expect("claim batch coupon");
    assert_eq!(batch_claimed.code, "IT-BATCH-1");
    let batch_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990003",
        "IT-BATCH-1",
        "it-batch-order-1",
        "it-batch-redeem-key",
    )
    .expect("batch redeem command");
    let batch_benefit = store
        .redeem_promotion_code_for_order(batch_redeem)
        .await
        .expect("redeem batch coupon");
    assert!(matches!(
        batch_benefit.kind,
        PromotionOrderCouponBenefitKind::TokenBankCredit {
            grant_units: 110_000_000,
            ..
        }
    ));
    let batch_counters: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_quantity, claimed_quantity, redeemed_quantity
         FROM promotion_coupon_stock WHERE id = 'it-stock-batch'",
    )
    .fetch_one(&pool)
    .await
    .expect("batch stock counters");
    assert_eq!(batch_counters, (4, 1, 1));

    // Membership points coupon.
    seed_offer(
        &pool,
        "points",
        "ALL",
        "CNY",
        r#"{"couponBenefit":{"kind":"points_credit","grantPoints":"50"}}"#,
        "limited",
        "REALTIME",
        5,
    )
    .await;
    let points_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990004",
        "it-offer-points",
        "it-points-claim-request",
        "it-points-claim-key",
    )
    .expect("points claim command");
    let points_claimed = store
        .claim_promotion_user_coupon(points_claim)
        .await
        .expect("claim points coupon");
    let points_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990004",
        &points_claimed.code,
        "it-points-order-1",
        "it-points-redeem-key",
    )
    .expect("points redeem command");
    let points_benefit = store
        .redeem_promotion_code_for_order(points_redeem)
        .await
        .expect("redeem points coupon");
    match points_benefit.kind {
        PromotionOrderCouponBenefitKind::PointsCredit { grant_points } => {
            assert_eq!(grant_points, 50);
        }
        other => panic!("expected points benefit, got {other:?}"),
    }

    // An expired claimed coupon is rejected and must not move redeemed stock.
    seed_offer(
        &pool,
        "expired",
        "ALL",
        "CNY",
        r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110"}}"#,
        "limited",
        "REALTIME",
        5,
    )
    .await;
    let expired_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990005",
        "it-offer-expired",
        "it-expired-claim-request",
        "it-expired-claim-key",
    )
    .expect("expired claim command");
    let expired_claimed = store
        .claim_promotion_user_coupon(expired_claim)
        .await
        .expect("claim expired coupon");
    sqlx::query(
        "UPDATE promotion_user_coupon
         SET expires_at = '2020-01-01T00:00:00.000Z'
         WHERE tenant_id = '990001' AND coupon_code = $1",
    )
    .bind(&expired_claimed.code)
    .execute(&pool)
    .await
    .expect("expire claimed coupon");
    let expired_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990005",
        &expired_claimed.code,
        "it-expired-order-1",
        "it-expired-redeem-key",
    )
    .expect("expired redeem command");
    let expired_error = store
        .redeem_promotion_code_for_order(expired_redeem)
        .await
        .expect_err("expired claimed coupon must be rejected");
    assert!(
        expired_error.message().contains("has expired"),
        "unexpected rejection: {}",
        expired_error.message()
    );
    let expired_counters: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_quantity, claimed_quantity, redeemed_quantity
         FROM promotion_coupon_stock WHERE id = 'it-stock-expired'",
    )
    .fetch_one(&pool)
    .await
    .expect("expired stock counters");
    assert_eq!(
        expired_counters,
        (4, 1, 0),
        "expired redemption must not increment redeemed_quantity"
    );

    // Order-discount engine: a claimed FIXED coupon reserves budget and records
    // the computed order discount.
    seed_discount_offer(&pool).await;
    let discount_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990010",
        "it-offer-discount",
        "it-discount-claim-request",
        "it-discount-claim-key",
    )
    .expect("discount claim command");
    let discount_claimed = store
        .claim_promotion_user_coupon(discount_claim)
        .await
        .expect("claim discount coupon");
    sqlx::raw_sql(
        r#"
        INSERT INTO commerce_order
            (id, tenant_id, organization_id, owner_user_id, order_no, status, subject,
             currency_code, created_at, updated_at)
        VALUES
            ('it-order-discount', '990001', '0', '990010', 'IT-ORDER-DISCOUNT',
             'pending_payment', 'membership', 'CNY', '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        INSERT INTO commerce_order_amount_breakdown
            (id, tenant_id, organization_id, order_id, allocation_type, original_amount,
             discount_amount, payable_amount, currency_code, created_at)
        VALUES
            ('it-breakdown-discount', '990001', '0', 'it-order-discount', 'order_total',
             '10000', '0', '10000', 'CNY', NOW());
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed discount order");
    let apply = ApplyPromotionDiscountCommand::new(
        TENANT,
        Some(ORG),
        "990010",
        "it-order-discount",
        &discount_claimed.id,
        "it-apply-request",
        "it-apply-key",
    )
    .expect("apply discount command");
    let applied = store
        .apply_promotion_discount(apply.clone())
        .await
        .expect("apply discount");
    assert_eq!(
        applied.amount.as_str(),
        "3000",
        "FIXED 30.00 should reserve 3000 minor units"
    );
    let application: (String,) = sqlx::query_as(
        "SELECT CAST(discount_amount AS TEXT) FROM promotion_discount_application
         WHERE order_id = 'it-order-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("discount application row");
    assert_eq!(application.0, "3000");
    let budget: (i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("discount budget");
    assert_eq!(budget, (97000, 3000));
    let replay_applied = store
        .apply_promotion_discount(apply)
        .await
        .expect("replay discount apply");
    assert_eq!(replay_applied.amount.as_str(), "3000");
    let budget_after_replay: (i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("discount budget after replay");
    assert_eq!(
        budget_after_replay,
        (97000, 3000),
        "replay must not reserve budget twice"
    );

    let reverse = ReversePromotionDiscountCommand::new(
        TENANT,
        Some(ORG),
        "990010",
        &discount_claimed.id,
        "it-reverse-request",
        "it-reverse-key",
        Some("integration reverse"),
    )
    .expect("reverse discount command");
    let reversed = store
        .reverse_promotion_discount(reverse.clone())
        .await
        .expect("reverse discount");
    assert_eq!(
        reversed.amount.as_str(),
        "3000",
        "reversal reports the amount that was applied"
    );
    let budget_after_reverse: (i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("budget after reverse");
    assert_eq!(
        budget_after_reverse,
        (100000, 0),
        "reversal must return the reservation to the available pool"
    );
    let application_status: String = sqlx::query_scalar(
        "SELECT status FROM promotion_discount_application
         WHERE order_id = 'it-order-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("application status after reverse");
    assert!(
        matches!(application_status.as_str(), "released" | "rolled_back"),
        "unexpected application status after reverse: {application_status}"
    );
    let coupon_status: String = sqlx::query_scalar(
        "SELECT status FROM promotion_user_coupon WHERE id = $1",
    )
    .bind(&discount_claimed.id)
    .fetch_one(&pool)
    .await
    .expect("coupon status after reverse");
    assert_eq!(coupon_status, "claimed", "reversal returns the coupon to usable");
    let replay_reverse = store
        .reverse_promotion_discount(reverse)
        .await
        .expect("replay discount reversal");
    assert_eq!(replay_reverse.amount.as_str(), "3000");
    let budget_after_replay_reverse: (i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("budget after replay reverse");
    assert_eq!(
        budget_after_replay_reverse,
        (100000, 0),
        "replaying the reversal must not release the budget twice"
    );

    // Payment settlement by order: reserve then move reserved -> used.
    sqlx::raw_sql(
        r#"
        INSERT INTO commerce_order
            (id, tenant_id, organization_id, owner_user_id, order_no, status, subject,
             currency_code, created_at, updated_at)
        VALUES
            ('it-order-discount-2', '990001', '0', '990010', 'IT-ORDER-DISCOUNT-2',
             'pending_payment', 'membership', 'CNY', '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        INSERT INTO commerce_order_amount_breakdown
            (id, tenant_id, organization_id, order_id, allocation_type, original_amount,
             discount_amount, payable_amount, currency_code, created_at)
        VALUES
            ('it-breakdown-discount-2', '990001', '0', 'it-order-discount-2', 'order_total',
             '10000', '0', '10000', 'CNY', NOW());
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed second discount order");
    let apply_second = ApplyPromotionDiscountCommand::new(
        TENANT,
        Some(ORG),
        "990010",
        "it-order-discount-2",
        &discount_claimed.id,
        "it-apply-request-2",
        "it-apply-key-2",
    )
    .expect("second apply command");
    store
        .apply_promotion_discount(apply_second)
        .await
        .expect("apply second discount");
    let budget_after_second_apply: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor, used_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("budget after second apply");
    assert_eq!(
        budget_after_second_apply,
        (97000, 3000, 0),
        "second apply must reserve the discount"
    );
    let settled = store
        .settle_promotion_discounts_for_order(TENANT, Some(ORG), "it-order-discount-2")
        .await
        .expect("settle order discounts");
    assert_eq!(settled, 1);
    let budget_after_settle: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_amount_minor, reserved_amount_minor, used_amount_minor
         FROM promotion_budget_account WHERE id = 'it-budget-discount'",
    )
    .fetch_one(&pool)
    .await
    .expect("budget after settle");
    assert_eq!(
        budget_after_settle,
        (97000, 0, 3000),
        "settlement moves the reservation into used"
    );
    let settled_status: String = sqlx::query_scalar(
        "SELECT status FROM promotion_discount_application
         WHERE order_id = 'it-order-discount-2'",
    )
    .fetch_one(&pool)
    .await
    .expect("settled application status");
    assert_eq!(settled_status, "settled");
    let replay_settled = store
        .settle_promotion_discounts_for_order(TENANT, Some(ORG), "it-order-discount-2")
        .await
        .expect("replay settle");
    assert_eq!(replay_settled, 0, "no applied row remains to settle");

    // Audience mismatch: a subject with a real paid order cannot use NEW_USER.
    seed_offer(
        &pool,
        "audience",
        "NEW_USER",
        "CNY",
        r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110"}}"#,
        "limited",
        "BATCH",
        5,
    )
    .await;
    seed_pool_code(&pool, "audience", "IT-AUDIENCE-1").await;
    sqlx::raw_sql(
        r#"
        INSERT INTO commerce_order
            (id, tenant_id, organization_id, owner_user_id, order_no, status, subject,
             currency_code, created_at, paid_at, updated_at)
        VALUES
            ('it-paid-order', '990001', '0', '990020', 'IT-PAID-1', 'paid', 'membership',
             'CNY', '2026-01-01 00:00:00', '2026-01-01 00:00:00', '2026-01-01 00:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed paid order");
    let audience_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990020",
        "IT-AUDIENCE-1",
        "it-audience-order",
        "it-audience-key",
    )
    .expect("audience redeem command");
    let audience_error = store
        .redeem_promotion_code_for_order(audience_redeem)
        .await
        .expect_err("a returning user must not redeem a NEW_USER coupon");
    assert!(
        audience_error.message().contains("audience is not eligible"),
        "unexpected rejection: {}",
        audience_error.message()
    );
    let audience_counters: (i64, i64, i64) = sqlx::query_as(
        "SELECT available_quantity, claimed_quantity, redeemed_quantity
         FROM promotion_coupon_stock WHERE id = 'it-stock-audience'",
    )
    .fetch_one(&pool)
    .await
    .expect("audience stock counters");
    assert_eq!(
        audience_counters,
        (5, 0, 0),
        "an audience rejection must not consume stock"
    );

    // Subscription coupon: the benefit carries the package/quota contract.
    seed_offer(
        &pool,
        "subscription",
        "ALL",
        "CNY",
        r#"{"couponBenefit":{"kind":"subscription","productId":"prod-1","skuId":"sku-1","packageId":"1","period":"month","durationDays":"30","dailyQuota":"100","totalQuota":"1000"}}"#,
        "limited",
        "REALTIME",
        5,
    )
    .await;
    let subscription_claim = ClaimPromotionUserCouponCommand::new(
        TENANT,
        Some(ORG),
        "990021",
        "it-offer-subscription",
        "it-subscription-claim-request",
        "it-subscription-claim-key",
    )
    .expect("subscription claim command");
    let subscription_claimed = store
        .claim_promotion_user_coupon(subscription_claim)
        .await
        .expect("claim subscription coupon");
    let subscription_redeem = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990021",
        &subscription_claimed.code,
        "it-subscription-order",
        "it-subscription-redeem-key",
    )
    .expect("subscription redeem command");
    let subscription_benefit = store
        .redeem_promotion_code_for_order(subscription_redeem)
        .await
        .expect("redeem subscription coupon");
    match subscription_benefit.kind {
        PromotionOrderCouponBenefitKind::Subscription {
            package_id,
            duration_days,
            daily_quota,
            total_quota,
            ..
        } => {
            assert_eq!(package_id, 1);
            assert_eq!(duration_days, 30);
            assert_eq!(daily_quota, 100);
            assert_eq!(total_quota, 1000);
        }
        other => panic!("expected subscription benefit, got {other:?}"),
    }

    let grants = store
        .summarize_asset_grants(
            PromotionAssetGrantReportQuery::new(
                TENANT,
                Some(ORG),
                "2020-01-01T00:00:00Z",
                "2030-01-01T00:00:00Z",
            )
            .expect("asset grant report query"),
        )
        .await
        .expect("summarize asset grants");
    let by_kind: std::collections::HashMap<&str, (&str, Option<&str>, i64)> = grants
        .iter()
        .map(|grant| {
            (
                grant.benefit_kind.as_str(),
                (
                    grant.total_amount.as_str(),
                    grant.currency_code.as_deref(),
                    grant.grant_count,
                ),
            )
        })
        .collect();
    assert_eq!(by_kind["token_bank_credit"], ("240000000", None, 2));
    assert_eq!(by_kind["points_credit"], ("50", None, 1));
    assert_eq!(by_kind["cash_credit"], ("10050", Some("USD"), 1));
    assert_eq!(by_kind["subscription"], ("1000", None, 1));

    assert_failed_attempts_rate_limit(&pool, &store).await;
}

async fn assert_failed_attempts_rate_limit(
    pool: &PgPool,
    store: &PostgresCommercePromotionStore,
) {
    for index in 0..20 {
        sqlx::query(
            r#"
            INSERT INTO promotion_code_redemption_attempt
                (id, uuid, tenant_id, organization_id, submitted_code_hash,
                 submitted_code_suffix, subject_type, subject_id, channel, outcome,
                 failure_code, request_no, idempotency_key, occurred_at, created_at)
            VALUES
                ($1, $2, '990001', '0', $3, 'ABCD', 'user', '990099', 'code', 'failed',
                 'code_unavailable', $4, $5, TO_CHAR(NOW() - INTERVAL '1 minute', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'),
                 TO_CHAR(NOW() - INTERVAL '1 minute', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'))
            "#,
        )
        .bind(format!("it-failure-{index}"))
        .bind(format!("it-failure-uuid-{index}"))
        .bind(format!("hash-{index}"))
        .bind(format!("it-failure-request-{index}"))
        .bind(format!("it-failure-key-{index}"))
        .execute(pool)
        .await
        .expect("insert failed attempt");
    }

    let preview = PromotionCodeRedemptionCommand::new(
        TENANT,
        Some(ORG),
        "990099",
        "IT-INVALID",
        "it-rate-request-1",
        "it-rate-key-1",
    )
    .expect("preview command");
    let error = store
        .preview_promotion_code_for_order(preview)
        .await
        .expect_err("20 failures must trip the rate limit");
    assert!(
        error
            .message()
            .contains("too many coupon redemption attempts"),
        "unexpected rejection: {}",
        error.message()
    );
}
