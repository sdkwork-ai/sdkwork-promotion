use chrono::Utc;
use sdkwork_commerce_promotion_service::{
    code_disable_decision, code_reissue_decision, finalize_code_export, release_decision,
    settle_decision, user_coupon_void_decision, CodeReissueOutcome, PromotionAdminListQuery,
    PromotionAdminPage, PromotionAdminScope, PromotionAdminUserCouponItem, PromotionCampaignInput,
    PromotionCampaignItem, PromotionCodeBatchDisableItem, PromotionCodeBatchInput,
    PromotionCodeBatchItem, PromotionCodeDisableItem, PromotionCodeExportItem,
    PromotionCodeRevealItem, PromotionCouponBenefit, PromotionCouponLedgerItem,
    PromotionCouponStockInput, PromotionCouponStockItem, PromotionDiscountApplicationReleaseItem,
    PromotionDiscountApplicationSettlementItem, PromotionDistributionInput,
    PromotionDistributionTaskItem, PromotionOfferInput, PromotionOfferItem,
    PromotionRedemptionAttemptItem, PromotionRedemptionAttemptListQuery,
    PromotionUserCouponExtendItem, PromotionUserCouponVoidItem, CODE_AUDIT_AGGREGATE_TYPE,
    CODE_BATCH_AUDIT_AGGREGATE_TYPE, CODE_BATCH_DISABLE_AUDIT_ACTION, CODE_DISABLE_AUDIT_ACTION,
    CODE_EXPORT_AUDIT_ACTION, CODE_EXPORT_FETCH_LIMIT, CODE_REISSUE_AUDIT_ACTION,
    CODE_REVEAL_AUDIT_ACTION, DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE,
    DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION, DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION,
    USER_COUPON_AUDIT_AGGREGATE_TYPE, USER_COUPON_EXTEND_AUDIT_ACTION,
    USER_COUPON_VOID_AUDIT_ACTION,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_utils_rust::uuid;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::code_identity::code_identity;
use crate::coupon_benefit::{parse_admin_coupon_benefit, serialize_admin_coupon_benefit};
use crate::promotion_admin::AdminPool;
use crate::timestamps;

const CAMPAIGN_COLUMNS: &str = "id, campaign_no, campaign_code, display_name, description, channel_scope, audience_scope, CAST(starts_at AS TEXT) AS starts_at, CAST(ends_at AS TEXT) AS ends_at, status, version, CAST(updated_at AS TEXT) AS updated_at";
const OFFER_COLUMNS: &str = "o.id, o.campaign_id, o.offer_no, o.offer_code, o.offer_type, o.audience_scope, o.combinability, o.goods_scope, o.display_name, o.description, o.priority, CAST(o.starts_at AS TEXT) AS starts_at, CAST(o.ends_at AS TEXT) AS ends_at, o.status, v.discount_type, CAST(v.discount_value AS TEXT) AS discount_value, CAST(v.minimum_amount AS TEXT) AS minimum_amount, CAST(v.maximum_discount_amount AS TEXT) AS maximum_discount_amount, v.currency_code, CAST(v.rule_json AS TEXT) AS rule_json, o.version, CAST(o.updated_at AS TEXT) AS updated_at";

pub(crate) async fn list_campaigns(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionCampaignItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM promotion_campaign WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(display_name) LIKE $3 OR LOWER(COALESCE(campaign_code, '')) LIKE $3 OR LOWER(campaign_no) LIKE $3)")
                .bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(&search)
                .fetch_one(pool).await.map_err(|error| storage("list campaigns", error))?;
            let sql = format!("SELECT {CAMPAIGN_COLUMNS} FROM promotion_campaign WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(display_name) LIKE $3 OR LOWER(COALESCE(campaign_code, '')) LIKE $3 OR LOWER(campaign_no) LIKE $3) ORDER BY created_at DESC LIMIT $4 OFFSET $5");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list campaigns", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
};
    Ok(PromotionAdminPage {
        items: rows
            .into_iter()
            .map(map_campaign)
            .collect::<Result<_, _>>()?,
        total_items,
    })
}

pub(crate) async fn retrieve_campaign(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    campaign_id: String,
) -> Result<Option<PromotionCampaignItem>, CommerceServiceError> {
    let row = match pool {
        AdminPool::Postgres(pool) => {
            let sql = format!("SELECT {CAMPAIGN_COLUMNS} FROM promotion_campaign WHERE id = $1 AND tenant_id = $2 AND organization_id = $3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(campaign_id)
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|error| storage("retrieve campaign", error))?
                .map(ManagementRow::Postgres)
        }
};
    row.map(map_campaign).transpose()
}

pub(crate) async fn create_campaign(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    input: &PromotionCampaignInput,
) -> Result<PromotionCampaignItem, CommerceServiceError> {
    let id = generated_id();
    let uuid = uuid();
    let campaign_no = reference("CAM");
    let actor_id = scope.actor_id()?;
    match pool {
        AdminPool::Postgres(pool) => {
            sqlx::query("INSERT INTO promotion_campaign (id, uuid, tenant_id, organization_id, campaign_no, campaign_code, display_name, description, channel_scope, audience_scope, starts_at, ends_at, status, version, created_by, created_at, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,0,$14,TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'))")
                .bind(id.as_str()).bind(uuid).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(campaign_no).bind(trimmed(&input.campaign_code)).bind(input.display_name.trim()).bind(trimmed(&input.description)).bind(input.channel_scope.trim()).bind(input.audience_scope.trim()).bind(input.starts_at.trim()).bind(trimmed(&input.ends_at)).bind(input.status.trim()).bind(actor_id.to_string())
                .execute(pool).await.map_err(|error| storage("create campaign", error))?;
        }
}
    retrieve_campaign(pool, scope, id)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("created campaign was not found"))
}

pub(crate) async fn update_campaign(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    campaign_id: String,
    input: &PromotionCampaignInput,
) -> Result<Option<PromotionCampaignItem>, CommerceServiceError> {
    let version = input.version.expect("validated campaign version");
    let affected = match pool {
        AdminPool::Postgres(pool) => sqlx::query("UPDATE promotion_campaign SET campaign_code=$1, display_name=$2, description=$3, channel_scope=$4, audience_scope=$5, starts_at=$6, ends_at=$7, status=$8, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE id=$9 AND tenant_id=$10 AND organization_id=$11 AND version=$12 AND status NOT IN ('ended','cancelled','archived')")
            .bind(trimmed(&input.campaign_code)).bind(input.display_name.trim()).bind(trimmed(&input.description)).bind(input.channel_scope.trim()).bind(input.audience_scope.trim()).bind(input.starts_at.trim()).bind(trimmed(&input.ends_at)).bind(input.status.trim()).bind(campaign_id.as_str()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(version)
            .execute(pool).await.map_err(|error| storage("update campaign", error))?.rows_affected(),
    };
    if affected == 0 {
        // Distinguish the three reasons the CAS can miss: a stale `version`, a
        // campaign in a terminal status, or a missing campaign. Collapsing them
        // into "not found" made a concurrent edit look like a deleted record.
        let state = match pool {
            AdminPool::Postgres(pool) => sqlx::query(
                "SELECT (SELECT COUNT(1) FROM promotion_campaign WHERE id=$1 AND tenant_id=$2 AND organization_id=$3) AS existing, COALESCE((SELECT status FROM promotion_campaign WHERE id=$1 AND tenant_id=$2 AND organization_id=$3), '') AS status",
            )
            .bind(campaign_id.as_str())
            .bind(scope.tenant_id.to_string())
            .bind(scope.organization_id.to_string())
            .fetch_one(pool)
            .await
            .map_err(|error| storage("check campaign state", error))?,
        };
        let existing: i64 = state.try_get("existing").unwrap_or(0);
        let status: String = state.try_get("status").unwrap_or_default();
        if existing > 0 && matches!(status.as_str(), "ended" | "cancelled" | "archived") {
            return Err(CommerceServiceError::conflict(
                "campaign is closed and can no longer be edited",
            ));
        }
        if existing > 0 {
            return Err(CommerceServiceError::conflict(
                "campaign was modified by another session; reload the campaign and retry",
            ));
        }
        return Ok(None);
    }
    retrieve_campaign(pool, scope, campaign_id).await
}

pub(crate) async fn delete_campaign(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    campaign_id: String,
) -> Result<bool, CommerceServiceError> {
    let affected = match pool {
        AdminPool::Postgres(pool) => sqlx::query("DELETE FROM promotion_campaign c WHERE c.id=$1 AND c.tenant_id=$2 AND c.organization_id=$3 AND c.status='draft' AND NOT EXISTS (SELECT 1 FROM promotion_offer o WHERE o.tenant_id=c.tenant_id AND o.campaign_id=c.id AND o.deleted_at IS NULL)").bind(campaign_id.as_str()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).execute(pool).await.map_err(|error| storage("delete campaign", error))?.rows_affected(),
    };
    Ok(affected == 1)
}

pub(crate) async fn retrieve_offer(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    offer_id: String,
) -> Result<Option<PromotionOfferItem>, CommerceServiceError> {
    let row = match pool {
        AdminPool::Postgres(pool) => retrieve_offer_postgres(pool, scope, offer_id)
            .await?
            .map(ManagementRow::Postgres),
    };
    row.map(map_offer).transpose()
}

pub(crate) async fn create_offer(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    input: &PromotionOfferInput,
) -> Result<PromotionOfferItem, CommerceServiceError> {
    let offer_id = generated_id();
    let version_id = generated_id();
    let offer_uuid = uuid();
    let version_uuid = uuid();
    let offer_no = reference("OFR");
    match pool {
        AdminPool::Postgres(pool) => {
            create_offer_postgres(
                pool,
                scope,
                input,
                offer_id.clone(),
                version_id,
                &offer_uuid,
                &version_uuid,
                &offer_no,
            )
            .await?
        }
}
    retrieve_offer(pool, scope, offer_id)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("created offer was not found"))
}

pub(crate) async fn update_offer(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    offer_id: String,
    input: &PromotionOfferInput,
) -> Result<Option<PromotionOfferItem>, CommerceServiceError> {
    let version_id = generated_id();
    let version_uuid = uuid();
    let affected = match pool {
        AdminPool::Postgres(pool) => {
            update_offer_postgres(
                pool,
                scope,
                offer_id.clone(),
                input,
                version_id,
                &version_uuid,
            )
            .await?
        }
};
    if !affected {
        return Ok(None);
    }
    retrieve_offer(pool, scope, offer_id).await
}

pub(crate) async fn delete_offer(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    offer_id: String,
) -> Result<bool, CommerceServiceError> {
    let affected = match pool {
        AdminPool::Postgres(pool) => sqlx::query("UPDATE promotion_offer o SET o.deleted_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'), o.version=o.version+1, o.updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE o.id=$1 AND o.tenant_id=$2 AND o.organization_id=$3 AND o.deleted_at IS NULL AND o.status='disabled' AND o.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM promotion_coupon_stock s WHERE s.tenant_id=o.tenant_id AND s.offer_id=o.id)").bind(offer_id.to_string()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).execute(pool).await.map_err(|error| storage("delete offer", error))?.rows_affected(),
    };
    Ok(affected == 1)
}

/// Creates one coupon stock and, when a monetary cap was requested, its budget
/// account in the same transaction.
///
/// Money-unit rule for this change: `promotion_coupon_stock.budget_amount_minor`
/// and every `promotion_budget_account` / `promotion_budget_ledger_entry` amount
/// is an integer minor unit (`*_minor BIGINT`). The request already carries minor
/// units, so this boundary stores the value as given — no float and no division
/// by 100. Legacy decimal-major strings live only on
/// `promotion_offer_version.discount_value` / `minimum_amount` and are converted
/// with `money_cents` at the point where an application amount is derived.
///
/// `budget_no` is derived deterministically from the stock (`BUD:<tenant>:<stock
/// id>`), so replaying the same creation cannot mint a second account for it.
pub(crate) async fn create_coupon_stock(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    input: &PromotionCouponStockInput,
) -> Result<PromotionCouponStockItem, CommerceServiceError> {
    let id = generated_id();
    let uuid = uuid();
    let stock_no = reference("STK");
    match pool {
        AdminPool::Postgres(pool) => {
            let mut tx = pool
                .begin()
                .await
                .map_err(|error| storage("begin create coupon stock", error))?;
            let row = sqlx::query(
                "SELECT o.current_offer_version_id, COALESCE(v.currency_code, '') AS currency_code FROM promotion_offer o LEFT JOIN promotion_offer_version v ON v.id = o.current_offer_version_id AND v.tenant_id = o.tenant_id WHERE o.id=$1 AND o.tenant_id=$2 AND o.organization_id=$3",
            )
            .bind(input.offer_id.to_string())
            .bind(scope.tenant_id.to_string())
            .bind(scope.organization_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| storage("resolve offer version", error))?
            .ok_or_else(|| CommerceServiceError::not_found("promotion offer not found"))?;
            let version_id = row
                .try_get::<Option<String>, _>("current_offer_version_id")
                .map_err(|error| decode("current_offer_version_id", error))?
                .ok_or_else(|| CommerceServiceError::not_found("promotion offer not found"))?;
            let currency_code = row
                .try_get::<String, _>("currency_code")
                .map_err(|error| decode("currency_code", error))?;
            sqlx::query("INSERT INTO promotion_coupon_stock (id,uuid,tenant_id,organization_id,offer_id,offer_version_id,stock_no,stock_type,code_issue_mode,total_quantity,available_quantity,claimed_quantity,redeemed_quantity,locked_quantity,per_user_limit,claim_starts_at,claim_ends_at,status,budget_amount_minor,version,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10,0,0,0,$11,$12,$13,$14,$15,0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)")
                .bind(id.as_str())
                .bind(uuid)
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(input.offer_id.to_string())
                .bind(version_id.to_string())
                .bind(&stock_no)
                .bind(input.stock_type.trim())
                .bind(input.code_issue_mode.trim())
                .bind(input.total_quantity)
                .bind(input.per_user_limit)
                .bind(trimmed(&input.claim_starts_at))
                .bind(trimmed(&input.claim_ends_at))
                .bind(&input.status)
                .bind(input.budget_amount_minor)
                .execute(&mut *tx)
                .await
                .map_err(|error| storage("create coupon stock", error))?;
            if let Some(total_amount_minor) = input.budget_amount_minor {
                insert_coupon_budget_account(
                    &mut tx,
                    scope,
                    input,
                    &id,
                    &version_id,
                    &currency_code,
                    total_amount_minor,
                )
                .await?;
            }
            tx.commit()
                .await
                .map_err(|error| storage("commit create coupon stock", error))?;
        }
    }
    retrieve_stock(pool, scope, id)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("created coupon stock was not found"))
}

/// Inserts the hard-cap budget account that backs one capped coupon stock.
///
/// `available_amount_minor = total_amount_minor = cap`: a fresh account has
/// promised nothing yet, and the table's `chk_pba_amounts` check guarantees
/// `available + reserved + used <= total` from the first row.
///
/// The currency comes from the immutable offer version the stock is pinned to,
/// and is validated against `currency_code VARCHAR(3)` here so an over-long
/// offer currency is a `400` instead of a Postgres truncation error at runtime.
async fn insert_coupon_budget_account(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: &PromotionAdminScope,
    input: &PromotionCouponStockInput,
    stock_id: &str,
    offer_version_id: &str,
    currency_code: &str,
    total_amount_minor: i64,
) -> Result<(), CommerceServiceError> {
    let currency_code = currency_code.trim();
    if currency_code.is_empty() {
        return Err(CommerceServiceError::validation(
            "the promotion offer version currency is required to cap a coupon budget",
        ));
    }
    if currency_code.chars().count() > 3 {
        return Err(CommerceServiceError::validation(
            "the promotion offer version currency must be at most 3 characters",
        ));
    }
    let tenant_id = scope.tenant_id.to_string();
    sqlx::query(
        "INSERT INTO promotion_budget_account (id,uuid,tenant_id,organization_id,budget_no,offer_id,offer_version_id,stock_id,currency_code,total_amount_minor,available_amount_minor,reserved_amount_minor,used_amount_minor,status,version,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10,0,0,'active',0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",
    )
    .bind(crate::postgres_promotion::stable_storage_id(&[
        "budget-account",
        tenant_id.as_str(),
        stock_id,
    ]))
    .bind(uuid())
    .bind(tenant_id.as_str())
    .bind(scope.organization_id.to_string())
    .bind(crate::postgres_promotion::stable_storage_id(&[
        "BUD", tenant_id.as_str(), stock_id,
    ]))
    .bind(input.offer_id.to_string())
    .bind(offer_version_id)
    .bind(stock_id)
    .bind(currency_code)
    .bind(total_amount_minor)
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("create coupon budget account", error))?;
    Ok(())
}

async fn retrieve_offer_postgres(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    offer_id: String,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    let sql = format!("SELECT {OFFER_COLUMNS} FROM promotion_offer o LEFT JOIN promotion_offer_version v ON v.id=o.current_offer_version_id AND v.tenant_id=o.tenant_id WHERE o.id=$1 AND o.tenant_id=$2 AND o.organization_id=$3");
    sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(offer_id.to_string())
        .bind(scope.tenant_id.to_string())
        .bind(scope.organization_id.to_string())
        .fetch_optional(pool)
        .await
        .map_err(|error| storage("retrieve offer", error))
}

async fn create_offer_postgres(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    input: &PromotionOfferInput,
    offer_id: String,
    version_id: String,
    offer_uuid: &str,
    version_uuid: &str,
    offer_no: &str,
) -> Result<(), CommerceServiceError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| storage("begin create offer", error))?;
    validate_campaign_reference_postgres(&mut tx, scope, input.campaign_id.clone()).await?;
    sqlx::query("INSERT INTO promotion_offer (id,uuid,tenant_id,organization_id,campaign_id,offer_no,offer_code,offer_type,audience_scope,combinability,priority,goods_scope,current_offer_version_id,display_name,description,starts_at,ends_at,status,version,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,0,TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'))")
        .bind(offer_id.to_string()).bind(offer_uuid).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(input.campaign_id.clone().map(|value| value.to_string())).bind(offer_no).bind(trimmed(&input.offer_code)).bind(input.offer_type.trim()).bind(input.audience_scope.trim()).bind(input.combinability.trim()).bind(input.priority).bind(input.goods_scope.trim()).bind(version_id.to_string()).bind(input.display_name.trim()).bind(trimmed(&input.description)).bind(input.starts_at.trim()).bind(trimmed(&input.ends_at)).bind(&input.status)
        .execute(&mut *tx).await.map_err(|error| storage("insert offer", error))?;
    insert_offer_version_postgres(
        &mut tx,
        scope.tenant_id,
        offer_id,
        version_id,
        version_uuid,
        1,
        input,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit create offer", error))
}

async fn update_offer_postgres(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    offer_id: String,
    input: &PromotionOfferInput,
    version_id: String,
    version_uuid: &str,
) -> Result<bool, CommerceServiceError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| storage("begin update offer", error))?;
    validate_campaign_reference_postgres(&mut tx, scope, input.campaign_id.clone()).await?;
    let version_no = sqlx::query_scalar::<_, i32>("SELECT COALESCE(MAX(version_no),0)+1 FROM promotion_offer_version WHERE tenant_id=$1 AND offer_id=$2").bind(scope.tenant_id.to_string()).bind(offer_id.to_string()).fetch_one(&mut *tx).await.map_err(|error| storage("resolve offer version", error))?;
    let affected = sqlx::query("UPDATE promotion_offer SET campaign_id=$1,offer_code=$2,offer_type=$3,audience_scope=$4,combinability=$5,priority=$6,goods_scope=$7,current_offer_version_id=$8,display_name=$9,description=$10,starts_at=$11,ends_at=$12,status=$13,version=version+1,updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE id=$14 AND tenant_id=$15 AND organization_id=$16 AND version=$17 AND deleted_at IS NULL")
        .bind(input.campaign_id.clone().map(|value| value.to_string())).bind(trimmed(&input.offer_code)).bind(input.offer_type.trim()).bind(input.audience_scope.trim()).bind(input.combinability.trim()).bind(input.priority).bind(input.goods_scope.trim()).bind(version_id.to_string()).bind(input.display_name.trim()).bind(trimmed(&input.description)).bind(input.starts_at.trim()).bind(trimmed(&input.ends_at)).bind(&input.status).bind(offer_id.to_string()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(input.version.expect("validated offer version"))
        .execute(&mut *tx).await.map_err(|error| storage("update offer", error))?.rows_affected();
    if affected == 0 {
        // A stale `version` and a missing offer both affect zero rows. They are
        // different outcomes for the operator (retry vs give up), so they must
        // not collapse into the same response.
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM promotion_offer WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 AND deleted_at IS NULL)",
        )
        .bind(offer_id.to_string())
        .bind(scope.tenant_id.to_string())
        .bind(scope.organization_id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| storage("check offer existence", error))?;
        tx.rollback()
            .await
            .map_err(|error| storage("rollback update offer", error))?;
        if exists {
            return Err(CommerceServiceError::conflict(
                "offer was modified by another session; reload the offer and retry",
            ));
        }
        return Ok(false);
    }
    insert_offer_version_postgres(
        &mut tx,
        scope.tenant_id,
        offer_id,
        version_id,
        version_uuid,
        version_no,
        input,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit update offer", error))?;
    Ok(true)
}

async fn validate_campaign_reference_postgres(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: &PromotionAdminScope,
    campaign_id: Option<String>,
) -> Result<(), CommerceServiceError> {
    if let Some(id) = campaign_id {
        let exists = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM promotion_campaign WHERE id=$1 AND tenant_id=$2 AND organization_id=$3)").bind(id.as_str()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).fetch_one(&mut **tx).await.map_err(|error| storage("validate campaign", error))?;
        if !exists {
            return Err(CommerceServiceError::not_found(
                "promotion campaign not found",
            ));
        }
    }
    Ok(())
}

async fn insert_offer_version_postgres(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: i64,
    offer_id: String,
    version_id: String,
    uuid: &str,
    version_no: i32,
    input: &PromotionOfferInput,
) -> Result<(), CommerceServiceError> {
    let rule_json = serialize_admin_coupon_benefit(input.coupon_benefit.as_ref())?;
    let (benefit_kind, benefit_grant_amount, benefit_bonus_amount, benefit_grant_points,
         benefit_period, benefit_duration_days, benefit_daily_quota, benefit_total_quota) =
        benefit_column_values(input.coupon_benefit.as_ref());
    sqlx::query("INSERT INTO promotion_offer_version (id,uuid,tenant_id,offer_id,version_no,discount_type,discount_value,minimum_amount,maximum_discount_amount,maximum_quantity_per_order,currency_code,rule_json,stack_rule_json,benefit_kind,benefit_grant_amount,benefit_bonus_amount,benefit_grant_points,benefit_period,benefit_duration_days,benefit_daily_quota,benefit_total_quota,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,1,$10,$11,NULL,$12,$13,$14,$15,$16,$17,$18,$19,TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'),TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS'))")
        .bind(version_id.to_string()).bind(uuid).bind(tenant_id).bind(offer_id.to_string()).bind(version_no)
        // The validation normalizes the closed set to upper case; store the
        // canonical form so apply/compute never has to reinterpret casing.
        .bind(input.discount_type.trim().to_ascii_uppercase()).bind(input.discount_value.trim()).bind(input.minimum_amount.trim()).bind(trimmed(&input.maximum_discount_amount)).bind(input.currency_code.trim()).bind(rule_json)
        .bind(benefit_kind).bind(benefit_grant_amount).bind(benefit_bonus_amount).bind(benefit_grant_points).bind(benefit_period).bind(benefit_duration_days).bind(benefit_daily_quota).bind(benefit_total_quota)
        .execute(&mut **tx).await.map_err(|error| storage("insert offer version", error))?;
    Ok(())
}

/// 从权益领域类型提取结构化列值（benefit_kind, grant_amount, bonus_amount, grant_points,
/// period, duration_days, daily_quota, total_quota），与 rule_json 双写保持一致。
#[allow(clippy::type_complexity)]
fn benefit_column_values(
    benefit: Option<&PromotionCouponBenefit>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
) {
    match benefit {
        None => (None, None, None, None, None, None, None, None),
        Some(PromotionCouponBenefit::TokenBankCredit {
            grant_amount,
            bonus_amount,
        }) => (
            Some("token_bank_credit".to_owned()),
            Some(grant_amount.to_string()),
            (*bonus_amount > 0).then(|| bonus_amount.to_string()),
            None,
            None,
            None,
            None,
            None,
        ),
        Some(PromotionCouponBenefit::PointsCredit { grant_points }) => (
            Some("points_credit".to_owned()),
            None,
            None,
            Some(grant_points.to_string()),
            None,
            None,
            None,
            None,
        ),
        Some(PromotionCouponBenefit::CashCredit { grant_amount }) => (
            Some("cash_credit".to_owned()),
            Some(grant_amount.to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        Some(PromotionCouponBenefit::Subscription {
            period,
            duration_days,
            daily_quota,
            total_quota,
            ..
        }) => (
            Some("subscription".to_owned()),
            None,
            None,
            None,
            Some(period.as_str().to_owned()),
            Some(*duration_days),
            Some(*daily_quota),
            Some(*total_quota),
        ),
    }
}

async fn retrieve_stock(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    stock_id: String,
) -> Result<Option<PromotionCouponStockItem>, CommerceServiceError> {
    let columns = "s.id,s.offer_id,s.stock_no,s.stock_type,s.code_issue_mode,s.total_quantity,s.available_quantity,s.claimed_quantity,s.redeemed_quantity,s.locked_quantity,s.per_user_limit,CAST(s.claim_starts_at AS TEXT) AS claim_starts_at,CAST(s.claim_ends_at AS TEXT) AS claim_ends_at,s.status,b.total_amount_minor AS budget_amount_minor,b.available_amount_minor AS budget_available_minor,b.reserved_amount_minor AS budget_reserved_minor,b.used_amount_minor AS budget_used_minor";
    let row = match pool {
        AdminPool::Postgres(pool) => {
            let sql=format!("SELECT {columns} FROM promotion_coupon_stock s LEFT JOIN promotion_budget_account b ON b.tenant_id = s.tenant_id AND b.stock_id = s.id WHERE s.id=$1 AND s.tenant_id=$2 AND s.organization_id=$3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(stock_id.to_string())
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|error| storage("retrieve stock", error))?
                .map(ManagementRow::Postgres)
        }
};
    row.map(map_stock).transpose()
}

pub(crate) async fn list_code_batches(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionCodeBatchItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let columns = "id,stock_id,offer_id,batch_no,code_type,requested_quantity,generated_quantity,code_length,code_prefix,CAST(starts_at AS TEXT) AS starts_at,CAST(expires_at AS TEXT) AS expires_at,status,CAST(created_at AS TEXT) AS created_at";
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total=sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM promotion_code_batch WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(batch_no) LIKE $3) AND ($4 IS NULL OR stock_id=CAST($4 AS TEXT))").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(&search).bind(query.stock_id).fetch_one(pool).await.map_err(|error|storage("list code batches",error))?;
            let sql=format!("SELECT {columns} FROM promotion_code_batch WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(batch_no) LIKE $3) AND ($4 IS NULL OR stock_id=CAST($4 AS TEXT)) ORDER BY created_at DESC LIMIT $5 OFFSET $6");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.stock_id)
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list code batches", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
};
    Ok(PromotionAdminPage {
        items: rows
            .into_iter()
            .map(map_code_batch)
            .collect::<Result<_, _>>()?,
        total_items,
    })
}

pub(crate) async fn create_code_batch(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    input: &PromotionCodeBatchInput,
) -> Result<PromotionCodeBatchItem, CommerceServiceError> {
    if let Some(item) = retrieve_code_batch_by_key(pool, scope, &input.idempotency_key).await? {
        return Ok(item);
    }
    let batch_id = generated_id();
    let batch_uuid = uuid();
    let batch_no = reference("CBT");
    let actor_id = scope.actor_id()?;
    match pool {
        AdminPool::Postgres(pool) => {
            create_code_batch_postgres(
                pool,
                scope,
                input,
                batch_id.clone(),
                &batch_uuid,
                &batch_no,
                actor_id,
            )
            .await?
        }
}
    retrieve_code_batch(pool, scope, batch_id)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("created code batch was not found"))
}

/// The one `promotion_code` insert path in this repository.
///
/// `code_hash`/`code_last4` are indexed identity columns and every new row must
/// carry them: `load_promotion_for_redeem` finds a code by `code_hash`, so a row
/// inserted without them would be invisible to the hash-first lookup. They are
/// appended to the column list and bound last (`$15`/`$16`) so the positional
/// binds above stay stable.
const INSERT_PROMOTION_CODE_SQL: &str = r#"
INSERT INTO promotion_code
    (id, uuid, tenant_id, organization_id, code_batch_id, stock_id, offer_id,
     offer_version_id, code_no, promotion_code, code_type, max_claims,
     claimed_quantity, starts_at, expires_at, status, version, created_at,
     updated_at, code_hash, code_last4)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0, $13, $14,
     'active', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, $15, $16)
"#;

/// The reissue insert: the same identity columns as [`INSERT_PROMOTION_CODE_SQL`]
/// plus the lineage column.
///
/// The replacement deliberately inherits `stock_id`, `offer_id`,
/// `offer_version_id`, `code_batch_id`, `code_type`, the validity window, and
/// `max_claims` from the original row instead of accepting them from the
/// operator, so a reissued code redeems under exactly the same terms. It starts
/// `active`, unclaimed, and at version 0 because it is a brand-new row.
const INSERT_REISSUED_PROMOTION_CODE_SQL: &str = r#"
INSERT INTO promotion_code
    (id, uuid, tenant_id, organization_id, code_batch_id, stock_id, offer_id,
     offer_version_id, code_no, promotion_code, code_type, max_claims,
     claimed_quantity, starts_at, expires_at, status, version, created_at,
     updated_at, code_hash, code_last4, reissued_from_code_id)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 0, $13, $14,
     'active', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, $15, $16, $17)
"#;

async fn create_code_batch_postgres(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    input: &PromotionCodeBatchInput,
    batch_id: String,
    batch_uuid: &str,
    batch_no: &str,
    actor_id: i64,
) -> Result<(), CommerceServiceError> {
    // A batch must only produce codes the redemption path will actually honour,
    // so the offer/version state and the issue capacity are validated here with
    // the same rules `load_promotion_for_redeem` applies.
    let starts_at = crate::timestamps::canonical_window(input.starts_at.as_deref(), "startsAt")?;
    let expires_at = crate::timestamps::canonical_window(input.expires_at.as_deref(), "expiresAt")?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| storage("begin code generation", error))?;
    let stock = sqlx::query(
        r#"
        SELECT s.offer_id, s.offer_version_id, s.stock_type, s.total_quantity,
               COALESCE(s.available_quantity, 0) AS available_quantity,
               o.status AS offer_status, o.deleted_at AS offer_deleted_at,
               v.lifecycle_status AS version_lifecycle_status,
               o.starts_at AS offer_starts_at, o.ends_at AS offer_ends_at,
               (SELECT COUNT(1) FROM promotion_code existing
                 WHERE existing.tenant_id = s.tenant_id AND existing.stock_id = s.id) AS existing_code_count
        FROM promotion_coupon_stock s
        JOIN promotion_offer o
          ON o.tenant_id = s.tenant_id AND o.id = s.offer_id
        JOIN promotion_offer_version v
          ON v.tenant_id = s.tenant_id AND v.id = s.offer_version_id
        WHERE s.id = $1 AND s.tenant_id = $2 AND s.organization_id = $3 AND s.status = 'active'
        "#,
    )
    .bind(input.stock_id.to_string())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("resolve code stock", error))?
    .ok_or_else(|| CommerceServiceError::not_found("active coupon stock not found"))?;
    let offer_id: String = stock
        .try_get("offer_id")
        .map_err(|error| decode("offer_id", error))?;
    let version_id: String = stock
        .try_get("offer_version_id")
        .map_err(|error| decode("offer_version_id", error))?;
    ensure_stock_can_issue_codes(&stock, input.quantity, &starts_at, &expires_at)?;
    // A custom-code batch has no generated prefix and no single code length; the
    // batch row records the longest defined code so the operator can see what
    // the pool actually contains.
    let (batch_code_length, batch_code_prefix) = if input.custom_codes.is_empty() {
        (input.code_length, input.code_prefix.trim().to_owned())
    } else {
        let longest = input
            .custom_codes
            .iter()
            .map(|code| code.trim().chars().count())
            .max()
            .unwrap_or(0);
        (i32::try_from(longest).unwrap_or(input.code_length), String::new())
    };
    sqlx::query("INSERT INTO promotion_code_batch (id,uuid,tenant_id,organization_id,stock_id,offer_id,offer_version_id,batch_no,code_type,requested_quantity,generated_quantity,code_length,code_prefix,starts_at,expires_at,status,idempotency_key,created_by,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,0,$11,$12,$13,$14,'generating',$15,$16,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)").bind(batch_id.to_string()).bind(batch_uuid).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(input.stock_id.to_string()).bind(offer_id.to_string()).bind(version_id.to_string()).bind(batch_no).bind(input.code_type.trim()).bind(input.quantity).bind(batch_code_length).bind(batch_code_prefix).bind(starts_at.as_deref()).bind(expires_at.as_deref()).bind(input.idempotency_key.trim()).bind(actor_id.to_string()).execute(&mut *tx).await.map_err(|error|storage("create code batch",error))?;
    let custom_codes: Vec<&String> = input.custom_codes.iter().collect();
    for index in 0..input.quantity as usize {
        let id = generated_id();
        let uuid = uuid();
        let code_no = reference("COD");
        // Custom codes are stored exactly as the admin defined them (trimmed and
        // upper-cased); the redemption lookup normalizes the submitted code the
        // same way, so both sides agree on one canonical code string.
        let value = match custom_codes.get(index) {
            Some(code) => normalized_custom_code(code)?,
            None => generated_code(&input.code_prefix, input.code_length)?,
        };
        // Both code creation paths (custom and generated) converge on this one
        // insert, so the indexed identity is derived once, here, from the value
        // that is actually stored.
        let identity = code_identity(&value);
        sqlx::query(INSERT_PROMOTION_CODE_SQL)
            .bind(id.as_str())
            .bind(uuid)
            .bind(scope.tenant_id.to_string())
            .bind(scope.organization_id.to_string())
            .bind(batch_id.to_string())
            .bind(input.stock_id.to_string())
            .bind(offer_id.to_string())
            .bind(version_id.to_string())
            .bind(code_no)
            .bind(value)
            .bind(input.code_type.trim())
            .bind(input.max_claims)
            .bind(starts_at.as_deref())
            .bind(expires_at.as_deref())
            .bind(identity.hash.as_str())
            .bind(identity.last4.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|error| code_insert_error(error))?;
    }
    sqlx::query("UPDATE promotion_code_batch SET generated_quantity=requested_quantity,status='ready',updated_at=CURRENT_TIMESTAMP WHERE id=$1 AND tenant_id=$2").bind(batch_id.to_string()).bind(scope.tenant_id.to_string()).execute(&mut *tx).await.map_err(|error|storage("complete code batch",error))?;
    tx.commit()
        .await
        .map_err(|error| storage("commit code generation", error))
}

/// Guards that keep generated codes redeemable: the stock's offer must be
/// issuable, the batch window must sit inside the offer window, and the pool
/// must not exceed the stock's remaining issue capacity.
fn ensure_stock_can_issue_codes(
    stock: &sqlx::postgres::PgRow,
    quantity: i64,
    starts_at: &Option<String>,
    expires_at: &Option<String>,
) -> Result<(), CommerceServiceError> {
    let offer_status: String = stock
        .try_get("offer_status")
        .map_err(|error| decode("offer_status", error))?;
    let offer_deleted: Option<String> = stock
        .try_get("offer_deleted_at")
        .map_err(|error| decode("offer_deleted_at", error))?;
    let version_status: String = stock
        .try_get("version_lifecycle_status")
        .map_err(|error| decode("version_lifecycle_status", error))?;
    if !offer_status.trim().eq_ignore_ascii_case("active") || offer_deleted.is_some() {
        return Err(CommerceServiceError::conflict(
            "coupon offer is not active; generated codes would be unredeemable",
        ));
    }
    if !version_status.trim().eq_ignore_ascii_case("published") {
        return Err(CommerceServiceError::conflict(
            "coupon offer version is not published; generated codes would be unredeemable",
        ));
    }

    let offer_starts_at: Option<String> = stock
        .try_get("offer_starts_at")
        .map_err(|error| decode("offer_starts_at", error))?;
    let offer_ends_at: Option<String> = stock
        .try_get("offer_ends_at")
        .map_err(|error| decode("offer_ends_at", error))?;
    let offer_start = optional_epoch(offer_starts_at.as_deref());
    let offer_end = optional_epoch(offer_ends_at.as_deref());
    let code_start = optional_epoch(starts_at.as_deref());
    let code_end = optional_epoch(expires_at.as_deref());
    if let (Some(start), Some(end)) = (code_start, code_end) {
        if end <= start {
            return Err(CommerceServiceError::validation(
                "codes valid-until must be later than codes valid-from",
            ));
        }
    }
    if let (Some(start), Some(offer_start)) = (code_start, offer_start) {
        if start < offer_start {
            return Err(CommerceServiceError::validation(
                "codes valid-from must not be earlier than the coupon offer start",
            ));
        }
    }
    if let (Some(end), Some(offer_end)) = (code_end, offer_end) {
        if end > offer_end {
            return Err(CommerceServiceError::validation(
                "codes valid-until must not be later than the coupon offer end",
            ));
        }
    }

    let stock_type: String = stock
        .try_get("stock_type")
        .map_err(|error| decode("stock_type", error))?;
    if !stock_type.trim().eq_ignore_ascii_case("unlimited") {
        let available: i64 = stock
            .try_get("available_quantity")
            .map_err(|error| decode("available_quantity", error))?;
        let existing: i64 = stock
            .try_get("existing_code_count")
            .map_err(|error| decode("existing_code_count", error))?;
        if existing + quantity > available {
            return Err(CommerceServiceError::conflict(format!(
                "coupon stock can issue {available} more codes but this batch requested {quantity} (already generated {existing})",
            )));
        }
    }
    Ok(())
}

fn optional_epoch(value: Option<&str>) -> Option<i64> {
    value.and_then(crate::timestamps::epoch_seconds)
}

/// Maps a duplicate-code unique violation to a conflict the admin can act on.
fn code_insert_error(error: sqlx::Error) -> CommerceServiceError {
    let duplicate = error.as_database_error().is_some_and(|database_error| {
        database_error.code().as_deref() == Some("23505")
    });
    if duplicate {
        CommerceServiceError::conflict(
            "promotion code already exists in this tenant; custom codes must be unique",
        )
    } else {
        storage("generate promotion code", error)
    }
}

async fn retrieve_code_batch_by_key(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    key: &str,
) -> Result<Option<PromotionCodeBatchItem>, CommerceServiceError> {
    // `promotion_code_batch.id` is TEXT; the id must be decoded as a string and
    // passed through unchanged, otherwise an idempotent retry fails to decode
    // instead of returning the original batch.
    let id=match pool{
        AdminPool::Postgres(pool)=>sqlx::query_scalar::<_,String>("SELECT id FROM promotion_code_batch WHERE tenant_id=$1 AND organization_id=$2 AND idempotency_key=$3").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(key.trim()).fetch_optional(pool).await.map_err(|error|storage("retrieve code batch idempotency",error))?,
    };
    match id {
        Some(id) => retrieve_code_batch(pool, scope, id).await,
        None => Ok(None),
    }
}

async fn retrieve_code_batch(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    id: String,
) -> Result<Option<PromotionCodeBatchItem>, CommerceServiceError> {
    let columns="id,stock_id,offer_id,batch_no,code_type,requested_quantity,generated_quantity,code_length,code_prefix,CAST(starts_at AS TEXT) AS starts_at,CAST(expires_at AS TEXT) AS expires_at,status,CAST(created_at AS TEXT) AS created_at";
    let row = match pool {
        AdminPool::Postgres(pool) => {
            let sql=format!("SELECT {columns} FROM promotion_code_batch WHERE id=$1 AND tenant_id=$2 AND organization_id=$3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|error| storage("retrieve code batch", error))?
                .map(ManagementRow::Postgres)
        }
};
    row.map(map_code_batch).transpose()
}

pub(crate) async fn list_distribution_tasks(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionDistributionTaskItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let columns="id,stock_id,offer_id,task_no,distribution_type,requested_quantity,succeeded_quantity,failed_quantity,status,CAST(created_at AS TEXT) AS created_at,CAST(completed_at AS TEXT) AS completed_at";
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total=sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM promotion_distribution_task WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(task_no) LIKE $3)").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(&search).fetch_one(pool).await.map_err(|error|storage("list distribution tasks",error))?;
            let sql=format!("SELECT {columns} FROM promotion_distribution_task WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(task_no) LIKE $3) ORDER BY created_at DESC LIMIT $4 OFFSET $5");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list distribution tasks", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
};
    Ok(PromotionAdminPage {
        items: rows
            .into_iter()
            .map(map_distribution_task)
            .collect::<Result<_, _>>()?,
        total_items,
    })
}

pub(crate) async fn create_distribution_task(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    input: &PromotionDistributionInput,
) -> Result<PromotionDistributionTaskItem, CommerceServiceError> {
    if let Some(item) = retrieve_distribution_by_key(pool, scope, &input.idempotency_key).await? {
        return Ok(item);
    }
    let task_id = generated_id();
    let task_uuid = uuid();
    let task_no = reference("DST");
    let actor_id = scope.actor_id()?;
    match pool {
        AdminPool::Postgres(pool) => {
            create_distribution_postgres(
                pool,
                scope,
                input,
                task_id.clone(),
                &task_uuid,
                &task_no,
                actor_id,
            )
            .await?
        }
}
    retrieve_distribution_task(pool, scope, task_id)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("created distribution task was not found"))
}

async fn create_distribution_postgres(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    input: &PromotionDistributionInput,
    task_id: String,
    task_uuid: &str,
    task_no: &str,
    actor_id: i64,
) -> Result<(), CommerceServiceError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| storage("begin coupon distribution", error))?;
    let stock=sqlx::query("SELECT offer_id,offer_version_id,stock_type,available_quantity,CAST(claim_ends_at AS TEXT) AS expires_at FROM promotion_coupon_stock WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 AND status='active' FOR UPDATE").bind(input.stock_id.to_string()).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).fetch_optional(&mut *tx).await.map_err(|error|storage("resolve distribution stock",error))?.ok_or_else(||CommerceServiceError::not_found("active coupon stock not found"))?;
    let offer_id: String = stock
        .try_get("offer_id")
        .map_err(|error| decode("offer_id", error))?;
    let version_id: String = stock
        .try_get("offer_version_id")
        .map_err(|error| decode("offer_version_id", error))?;
    let stock_type: String = stock
        .try_get("stock_type")
        .map_err(|error| decode("stock_type", error))?;
    let available: i64 = stock
        .try_get("available_quantity")
        .map_err(|error| decode("available_quantity", error))?;
    let expires: Option<String> = stock
        .try_get("expires_at")
        .map_err(|error| decode("expires_at", error))?;
    let requested = input.owner_user_ids.len() as i64;
    if stock_type != "unlimited" && available < requested {
        return Err(CommerceServiceError::validation(
            "coupon stock is insufficient",
        ));
    }
    sqlx::query("INSERT INTO promotion_distribution_task (id,uuid,tenant_id,organization_id,stock_id,offer_id,offer_version_id,task_no,distribution_type,requested_quantity,succeeded_quantity,failed_quantity,status,idempotency_key,created_by,started_at,completed_at,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'direct',$9,0,0,'running',$10,$11,CURRENT_TIMESTAMP,NULL,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)").bind(task_id.to_string()).bind(task_uuid).bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(input.stock_id.to_string()).bind(offer_id.to_string()).bind(version_id.to_string()).bind(task_no).bind(requested).bind(input.idempotency_key.trim()).bind(actor_id.to_string()).execute(&mut *tx).await.map_err(|error|storage("create distribution task",error))?;
    for (index, user_id) in input.owner_user_ids.iter().enumerate() {
        insert_distributed_coupon_postgres(
            &mut tx,
            scope,
            input,
            task_id.clone(),
            offer_id.to_string(),
            version_id.to_string(),
            user_id,
            expires.as_deref(),
            if stock_type == "unlimited" {
                available
            } else {
                available - index as i64 - 1
            },
        )
        .await?;
    }
    let stock_update = if stock_type == "unlimited" {
        "UPDATE promotion_coupon_stock SET claimed_quantity=claimed_quantity+$1,version=version+1,updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4"
    } else {
        "UPDATE promotion_coupon_stock SET available_quantity=available_quantity-$1,claimed_quantity=claimed_quantity+$1,version=version+1,updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND available_quantity >= $1"
    };
    let affected = sqlx::query(sqlx::AssertSqlSafe(stock_update))
        .bind(requested)
        .bind(input.stock_id.to_string())
        .bind(scope.tenant_id.to_string())
        .bind(scope.organization_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|error| storage("consume coupon stock", error))?
        .rows_affected();
    if affected != 1 {
        return Err(CommerceServiceError::validation(
            "coupon stock changed during distribution",
        ));
    }
    sqlx::query("UPDATE promotion_distribution_task SET succeeded_quantity=requested_quantity,status='succeeded',completed_at=CURRENT_TIMESTAMP,updated_at=CURRENT_TIMESTAMP WHERE id=$1 AND tenant_id=$2").bind(task_id.to_string()).bind(scope.tenant_id.to_string()).execute(&mut *tx).await.map_err(|error|storage("complete distribution task",error))?;
    tx.commit()
        .await
        .map_err(|error| storage("commit coupon distribution", error))
}

async fn insert_distributed_coupon_postgres(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: &PromotionAdminScope,
    input: &PromotionDistributionInput,
    task_id: String,
    offer_id: String,
    version_id: String,
    user_id: &str,
    expires: Option<&str>,
    balance_after: i64,
) -> Result<(), CommerceServiceError> {
    let coupon_id = generated_id();
    let coupon_uuid = uuid();
    let coupon_no = reference("CPN");
    let coupon_code = generated_code("AC", 16)?;
    let request_no = reference("REQ");
    let item_key = format!("{}:{user_id}", input.idempotency_key.trim());
    // The admin list and the discount-apply path both read the canonical
    // lowercase enum vocabulary (`coupon_status_label`, the `subject_type = $6`
    // filters bound to `USER_SUBJECT_TYPE`), so a distributed coupon is written
    // exactly like a claimed one: `status='claimed'`, `subject_type='user'`.
    sqlx::query(
        r#"
        INSERT INTO promotion_user_coupon
            (id,uuid,tenant_id,organization_id,coupon_no,stock_id,code_id,offer_id,
             offer_version_id,subject_type,subject_id,owner_user_id,coupon_code,status,
             claimed_at,valid_from,expires_at,redeemed_at,disabled_at,source_type,
             source_id,request_no,idempotency_key,trace_id,version,created_at,updated_at)
        VALUES
            ($1,$2,$3,$4,$5,$6,NULL,$7,$8,'user',$9,$9,$10,'claimed',
             CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,$11,NULL,NULL,'admin_distribution',
             $12,$13,$14,'',0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(coupon_id.as_str())
    .bind(coupon_uuid)
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(coupon_no)
    .bind(input.stock_id.to_string())
    .bind(offer_id.to_string())
    .bind(version_id.to_string())
    .bind(user_id.to_string())
    .bind(coupon_code)
    .bind(expires)
    .bind(task_id.to_string())
    .bind(&request_no)
    .bind(&item_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("issue user coupon", error))?;
    sqlx::query(
        r#"
        INSERT INTO promotion_distribution_record
            (id,uuid,tenant_id,organization_id,task_id,owner_user_id,user_coupon_id,
             status,failure_code,failure_detail,created_at)
        VALUES
            ($1,$2,$3,$4,$5,$6,$7,'succeeded',NULL,NULL,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(generated_id())
    .bind(uuid())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(task_id.to_string())
    .bind(user_id.to_string())
    .bind(coupon_id.as_str())
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("record coupon distribution", error))?;
    // Ledger enums are lowercase (`debit`/`distribution`) because the ledger
    // readers normalize with `LOWER(...)` (`points_direction`,
    // `points_business_type`, admin list filters) and label the retired
    // uppercase forms as unknown. `source_type`/`source_id` mirror the
    // claim/redeem ledger exactly: the source is the issued wallet coupon.
    sqlx::query(
        r#"
        INSERT INTO promotion_coupon_ledger_entry
            (id,uuid,tenant_id,stock_id,user_coupon_id,offer_id,subject_type,subject_id,
             direction,quantity_delta,balance_after,business_type,business_no,request_no,
             idempotency_key,source_type,source_id,trace_id,created_at)
        VALUES
            ($1,$2,$3,$4,$5,$6,'user',$7,'debit',-1,$8,'distribution',$9,$10,$11,
             'promotion_user_coupon',$12,'',CURRENT_TIMESTAMP)
        "#,
    )
    .bind(generated_id())
    .bind(uuid())
    .bind(scope.tenant_id.to_string())
    .bind(input.stock_id.to_string())
    .bind(coupon_id.as_str())
    .bind(offer_id.to_string())
    .bind(user_id.to_string())
    .bind(balance_after)
    .bind(task_id.to_string())
    .bind(&request_no)
    .bind(format!("ledger:{item_key}"))
    .bind(coupon_id.as_str())
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("record coupon ledger", error))?;
    Ok(())
}

async fn retrieve_distribution_by_key(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    key: &str,
) -> Result<Option<PromotionDistributionTaskItem>, CommerceServiceError> {
    // `promotion_distribution_task.id` is TEXT, decoded as `String` so an
    // idempotent distribution retry returns the original task instead of
    // failing to decode the key.
    let id=match pool{AdminPool::Postgres(pool)=>sqlx::query_scalar::<_,String>("SELECT id FROM promotion_distribution_task WHERE tenant_id=$1 AND organization_id=$2 AND idempotency_key=$3").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(key.trim()).fetch_optional(pool).await.map_err(|error|storage("retrieve distribution idempotency",error))?};
    match id {
        Some(id) => retrieve_distribution_task(pool, scope, id).await,
        None => Ok(None),
    }
}

async fn retrieve_distribution_task(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    id: String,
) -> Result<Option<PromotionDistributionTaskItem>, CommerceServiceError> {
    let columns="id,stock_id,offer_id,task_no,distribution_type,requested_quantity,succeeded_quantity,failed_quantity,status,CAST(created_at AS TEXT) AS created_at,CAST(completed_at AS TEXT) AS completed_at";
    let row = match pool {
        AdminPool::Postgres(pool) => {
            let sql=format!("SELECT {columns} FROM promotion_distribution_task WHERE id=$1 AND tenant_id=$2 AND organization_id=$3");
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|error| storage("retrieve distribution task", error))?
                .map(ManagementRow::Postgres)
        }
};
    row.map(map_distribution_task).transpose()
}

pub(crate) async fn list_user_coupons(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionAdminUserCouponItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let columns="id,coupon_no,stock_id,offer_id,owner_user_id,coupon_code,status,CAST(claimed_at AS TEXT) AS claimed_at,CAST(valid_from AS TEXT) AS valid_from,CAST(expires_at AS TEXT) AS expires_at,CAST(redeemed_at AS TEXT) AS redeemed_at,source_type,source_id";
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total=sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM promotion_user_coupon WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(coupon_no) LIKE $3 OR LOWER(coupon_code) LIKE $3 OR CAST(owner_user_id AS TEXT) LIKE $3) AND ($4 IS NULL OR status=$4)").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(&search).bind(query.status.as_deref()).fetch_one(pool).await.map_err(|error|storage("list user coupons",error))?;
            let sql=format!("SELECT {columns} FROM promotion_user_coupon WHERE tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(coupon_no) LIKE $3 OR LOWER(coupon_code) LIKE $3 OR CAST(owner_user_id AS TEXT) LIKE $3) AND ($4 IS NULL OR status=$4) ORDER BY claimed_at DESC LIMIT $5 OFFSET $6");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.status.as_deref())
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list user coupons", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
};
    Ok(PromotionAdminPage {
        items: rows
            .into_iter()
            .map(map_user_coupon)
            .collect::<Result<_, _>>()?,
        total_items,
    })
}

pub(crate) async fn list_coupon_ledger(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionCouponLedgerItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let columns="l.id,l.stock_id,l.user_coupon_id,l.offer_id,l.subject_id,l.direction,l.quantity_delta,l.balance_after,l.business_type,l.business_no,CAST(l.created_at AS TEXT) AS created_at";
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total=sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM promotion_coupon_ledger_entry l JOIN promotion_coupon_stock s ON s.tenant_id=l.tenant_id AND s.id=l.stock_id WHERE l.tenant_id=$1 AND s.organization_id=$2 AND ($3='%%' OR LOWER(l.business_no) LIKE $3 OR LOWER(l.business_type) LIKE $3 OR CAST(l.subject_id AS TEXT) LIKE $3)").bind(scope.tenant_id.to_string()).bind(scope.organization_id.to_string()).bind(&search).fetch_one(pool).await.map_err(|error|storage("list coupon ledger",error))?;
            let sql=format!("SELECT {columns} FROM promotion_coupon_ledger_entry l JOIN promotion_coupon_stock s ON s.tenant_id=l.tenant_id AND s.id=l.stock_id WHERE l.tenant_id=$1 AND s.organization_id=$2 AND ($3='%%' OR LOWER(l.business_no) LIKE $3 OR LOWER(l.business_type) LIKE $3 OR CAST(l.subject_id AS TEXT) LIKE $3) ORDER BY l.created_at DESC LIMIT $4 OFFSET $5");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list coupon ledger", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
};
    Ok(PromotionAdminPage {
        items: rows.into_iter().map(map_ledger).collect::<Result<_, _>>()?,
        total_items,
    })
}

/// Lists code-redemption attempts (successes and failures), newest first.
///
/// Scoping matches every other admin list: `tenant_id` plus the scope's
/// `organization_id`, where `'0'` is the platform sentinel
/// (`PromotionAdminScope.organization_id == 0` binds `'0'`, and every write path
/// stores `'0'` for a row that belongs to no organization).
///
/// The predicates are ordered so the ledger indexes can serve them:
/// `(tenant_id, outcome, ...)` for the outcome filter, `(tenant_id, code_id, ...)`
/// for the code filter, and `occurred_at` for the sort the indexes carry as
/// their trailing column.
///
/// `occurred_at` is ordered as text on purpose: the attempt recorder is its only
/// writer and always stores the canonical server form (`YYYY-MM-DD HH:MM:SS`),
/// so text order is chronological. Wrapping it in an instant expression (as the
/// redemption windows do for mixed-shape columns) would make the ordering
/// non-indexable.
pub(crate) async fn list_redemption_attempts(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionRedemptionAttemptListQuery,
) -> Result<PromotionAdminPage<PromotionRedemptionAttemptItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let columns = "id,code_id,submitted_code_suffix,subject_type,subject_id,outcome,failure_code,failure_detail,benefit_kind,CAST(granted_amount AS TEXT) AS granted_amount,currency_code,request_no,order_id,trace_id,CAST(occurred_at AS TEXT) AS occurred_at";
    let filter = "tenant_id=$1 AND organization_id=$2 AND ($3='%%' OR LOWER(submitted_code_suffix) LIKE $3 OR CAST(subject_id AS TEXT) LIKE $3 OR LOWER(request_no) LIKE $3) AND ($4 IS NULL OR outcome=$4) AND ($5 IS NULL OR code_id=CAST($5 AS TEXT))";
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM promotion_code_redemption_attempt WHERE {filter}"
            )))
            .bind(scope.tenant_id.to_string())
            .bind(scope.organization_id.to_string())
            .bind(&search)
            .bind(query.outcome.as_deref())
            .bind(query.code_id.as_deref())
            .fetch_one(pool)
            .await
            .map_err(|error| storage("list redemption attempts", error))?;
            let sql = format!(
                "SELECT {columns} FROM promotion_code_redemption_attempt WHERE {filter} ORDER BY occurred_at DESC, id DESC LIMIT $6 OFFSET $7"
            );
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.outcome.as_deref())
                .bind(query.code_id.as_deref())
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage("list redemption attempts", error))?;
            (
                total,
                rows.into_iter()
                    .map(ManagementRow::Postgres)
                    .collect::<Vec<_>>(),
            )
        }
    };
    Ok(PromotionAdminPage {
        items: rows
            .into_iter()
            .map(map_redemption_attempt)
            .collect::<Result<_, _>>()?,
        total_items,
    })
}

/// Disables one leaked code together with its audit evidence.
///
/// Guard: `status = 'active'` AND `COALESCE(claimed_quantity, 0) = 0`. The row is
/// read `FOR UPDATE`, so the guard decision and the mutation cannot be split by a
/// concurrent claim. A claimed code and a non-active code are distinct `409`
/// conflicts (see [`code_disable_decision`]).
///
/// Ordering inside the single transaction: read + guard, then the
/// `codes.disable` evidence row (aggregate `promotion_code`), then the mutation,
/// then `subject_count = 1`. Any audit failure aborts before the mutation, so a
/// disabled code is never committed without evidence. A rejected or absent
/// target rolls back, so no evidence row survives for a mutation that did not
/// happen.
pub(crate) async fn disable_code(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    code_id: String,
    reason: String,
    trace_id: &str,
) -> Result<Option<PromotionCodeDisableItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin code disable").await?;
    let row = sqlx::query(
        "SELECT status, claimed_quantity FROM promotion_code WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 FOR UPDATE",
    )
    .bind(code_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read promotion code for disable", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    let claimed_quantity = row
        .try_get::<i32, _>("claimed_quantity")
        .map_err(|error| decode("claimed_quantity", error))?;
    if let Err(message) = code_disable_decision(&status, i64::from(claimed_quantity)) {
        return Err(CommerceServiceError::conflict(message));
    }
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        CODE_DISABLE_AUDIT_ACTION,
        CODE_AUDIT_AGGREGATE_TYPE,
        code_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let affected = sqlx::query(
        "UPDATE promotion_code SET status='disabled', voided_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'), void_reason=$1, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND status='active' AND COALESCE(claimed_quantity,0)=0",
    )
    .bind(reason.as_str())
    .bind(code_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("disable promotion code", error))?
    .rows_affected();
    if affected != 1 {
        // The row lock was held, so only a row that vanished between the read
        // and the write can reach this branch.
        return Err(CommerceServiceError::conflict("code is not active"));
    }
    update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit code disable", error))?;
    Ok(Some(PromotionCodeDisableItem {
        code_id,
        status: "disabled".to_owned(),
    }))
}

/// Disables every eligible code of one batch together with its audit evidence.
///
/// The batch existence check is scoped (`None` -> 404), then one
/// `UPDATE ... WHERE code_batch_id=$ AND status='active' AND
/// COALESCE(claimed_quantity,0)=0` stops every eligible row. Codes that are
/// already claimed or no longer active are reported as `skipped_count` instead of
/// failing the batch; codes already `voided` are outside the guard's population
/// and are neither disabled nor skipped.
///
/// `codes.batch_disable` (aggregate `promotion_code_batch`) is written before the
/// mutation, and `subject_count` is the number of codes actually disabled.
pub(crate) async fn disable_code_batch(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    batch_id: String,
    reason: String,
    trace_id: &str,
) -> Result<Option<PromotionCodeBatchDisableItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin code batch disable").await?;
    let batch_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM promotion_code_batch WHERE id=$1 AND tenant_id=$2 AND organization_id=$3)",
    )
    .bind(batch_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| storage("resolve promotion code batch for disable", error))?;
    if !batch_exists {
        return Ok(None);
    }
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        CODE_BATCH_DISABLE_AUDIT_ACTION,
        CODE_BATCH_AUDIT_AGGREGATE_TYPE,
        batch_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let disabled_count = sqlx::query(
        "UPDATE promotion_code SET status='disabled', voided_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'), void_reason=$1, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') WHERE code_batch_id=$2 AND tenant_id=$3 AND organization_id=$4 AND status='active' AND COALESCE(claimed_quantity,0)=0",
    )
    .bind(reason.as_str())
    .bind(batch_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("disable promotion code batch", error))?
    .rows_affected() as i64;
    let non_voided = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM promotion_code WHERE code_batch_id=$1 AND tenant_id=$2 AND organization_id=$3 AND status <> 'voided'",
    )
    .bind(batch_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| storage("count promotion code batch codes", error))?;
    // Everything the guard did not stop is skipped: claimed codes and codes that
    // are no longer active. The count is clamped so a concurrent insert can never
    // make the reported total negative.
    let skipped_count = (non_voided - disabled_count).max(0);
    update_audit_subject_count(&mut tx, &audit_id, scope, disabled_count).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit code batch disable", error))?;
    Ok(Some(PromotionCodeBatchDisableItem {
        disabled_count,
        skipped_count,
    }))
}

/// Voids one wallet coupon together with its audit evidence.
///
/// Allowed only for `claimed|active|pending` (case-insensitive). A
/// redeemed/used coupon is a distinct `409` ("coupon has already been
/// redeemed"); an already voided/expired/cancelled/disabled coupon is "coupon is
/// not active" (see [`user_coupon_void_decision`]).
///
/// `user_coupons.void` (aggregate `promotion_user_coupon`) is written before the
/// mutation, and the row keeps `voided_at` + `void_reason` so the stop is
/// auditable from the row itself.
pub(crate) async fn void_user_coupon(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    user_coupon_id: String,
    reason: String,
    trace_id: &str,
) -> Result<Option<PromotionUserCouponVoidItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin user coupon void").await?;
    let row = sqlx::query(
        "SELECT status FROM promotion_user_coupon WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 FOR UPDATE",
    )
    .bind(user_coupon_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read promotion user coupon for void", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    if let Err(message) = user_coupon_void_decision(&status) {
        return Err(CommerceServiceError::conflict(message));
    }
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        USER_COUPON_VOID_AUDIT_ACTION,
        USER_COUPON_AUDIT_AGGREGATE_TYPE,
        user_coupon_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let affected = sqlx::query(
        "UPDATE promotion_user_coupon SET status='voided', voided_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'), void_reason=$1, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND LOWER(status) IN ('claimed','active','pending')",
    )
    .bind(reason.as_str())
    .bind(user_coupon_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("void promotion user coupon", error))?
    .rows_affected();
    if affected != 1 {
        return Err(CommerceServiceError::conflict("coupon is not active"));
    }
    update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit user coupon void", error))?;
    Ok(Some(PromotionUserCouponVoidItem {
        user_coupon_id,
        status: "voided".to_owned(),
    }))
}

/// Extends one wallet coupon's expiry together with its audit evidence.
///
/// The new expiry is parsed and canonicalized by [`timestamps`] (both accepted
/// promotion shapes normalize to `YYYY-MM-DDTHH:MM:SS.sssZ`), and the "later than
/// now" decision is taken by the database, never by the Rust clock:
/// `($new)::timestamptz > CURRENT_TIMESTAMP`. A coupon without a previous
/// `expires_at` is allowed — extending re-arms it with a bound.
///
/// `user_coupons.extend` carries the old and new expiry in `detail_json`, and is
/// written before the mutation in the same transaction.
pub(crate) async fn extend_user_coupon(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    user_coupon_id: String,
    expires_at: String,
    trace_id: &str,
) -> Result<Option<PromotionUserCouponExtendItem>, CommerceServiceError> {
    // Authoritative parse + canonical shape; unparseable input is a 400 and never
    // opens a mutation.
    let expires_at = timestamps::required_instant(&expires_at, "expiresAt")?;
    let mut tx = begin_admin_transaction(pool, "begin user coupon extension").await?;
    let row = sqlx::query(
        "SELECT status, CAST(expires_at AS TEXT) AS previous_expires_at, ($4::timestamptz > CURRENT_TIMESTAMP) AS expires_in_future FROM promotion_user_coupon WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 FOR UPDATE",
    )
    .bind(user_coupon_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(expires_at.as_str())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read promotion user coupon for extension", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    if let Err(message) = user_coupon_void_decision(&status) {
        return Err(CommerceServiceError::conflict(message));
    }
    let expires_in_future = row
        .try_get::<bool, _>("expires_in_future")
        .map_err(|error| decode("expires_in_future", error))?;
    timestamps::extension_future_decision(expires_in_future)?;
    let previous_expires_at = row
        .try_get::<Option<String>, _>("previous_expires_at")
        .map_err(|error| decode("previous_expires_at", error))?;
    let detail_json = serde_json::json!({
        "previousExpiresAt": previous_expires_at,
        "expiresAt": expires_at,
    })
    .to_string();
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        USER_COUPON_EXTEND_AUDIT_ACTION,
        USER_COUPON_AUDIT_AGGREGATE_TYPE,
        user_coupon_id.as_str(),
        Some(detail_json.as_str()),
        trace_id,
    )
    .await?;
    let affected = sqlx::query(
        "UPDATE promotion_user_coupon SET expires_at=$1, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND LOWER(status) IN ('claimed','active','pending') AND $1::timestamptz > CURRENT_TIMESTAMP",
    )
    .bind(expires_at.as_str())
    .bind(user_coupon_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("extend promotion user coupon", error))?
    .rows_affected();
    if affected != 1 {
        // The row lock and the status guard both held, so a zero-row update can
        // only mean the database clock crossed the target between the read and
        // the write: re-state the same validation error the read would have.
        timestamps::extension_future_decision(false)?;
        return Err(CommerceServiceError::conflict("coupon is not active"));
    }
    update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit user coupon extension", error))?;
    Ok(Some(PromotionUserCouponExtendItem {
        user_coupon_id,
        expires_at,
    }))
}

// ---------------------------------------------------------------------------
// Code reissue: replace one unused code with a freshly issued one.
//
// A reissue is a two-row operator mutation with one audit trail: the original
// row is disabled and the replacement is inserted in the same transaction, and
// the `codes.reissue` evidence row (written first, `subject_count = 2`) names
// both ids and the operator's reason.
// ---------------------------------------------------------------------------

/// Replaces one unused code with a newly issued one, in a single transaction.
///
/// Semantics: the operator states the old code was lost or leaked before anyone
/// claimed it. The original must therefore be `active` with
/// `COALESCE(claimed_quantity, 0) = 0`; a claimed code and a non-active code are
/// distinct `409` conflicts (see [`code_reissue_decision`]). The original row is
/// then disabled with `voided_at`, `void_reason`, and `replaced_by_code_id`, and
/// the replacement is inserted with `reissued_from_code_id`, inheriting the
/// original's `stock_id`, `offer_id`, `offer_version_id`, `code_batch_id`,
/// `code_type`, validity window, and `max_claims`.
///
/// Ordering inside the single transaction: read + guard, then the `codes.reissue`
/// evidence row, then the disable, then the insert. Any failure — including an
/// explicit code colliding on `uk_promotion_code_hash` — rolls the whole
/// replacement back, so neither half can commit alone and no evidence row
/// survives a mutation that did not happen.
///
/// The value is generated from the batch's `code_prefix`/`code_length` when the
/// batch defines them (falling back to a 16-character generated code), or taken
/// from `explicit_code`, which is normalized exactly like a custom batch code.
///
/// The returned outcome carries the new row's identity and a display mask only.
/// The redeemable value is never returned here: the audited reveal/export
/// operations remain the only read paths.
pub(crate) async fn reissue_code(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    code_id: String,
    reason: String,
    explicit_code: Option<&str>,
    trace_id: &str,
) -> Result<Option<CodeReissueOutcome>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin code reissue").await?;
    // `FOR UPDATE OF c`: only the code row is locked (the batch side of the
    // outer join cannot be locked), so the guard decision and both mutations
    // cannot be split by a concurrent claim.
    let row = sqlx::query(
        "SELECT c.status, c.claimed_quantity, c.stock_id, c.offer_id, c.offer_version_id, c.code_batch_id, c.code_type, c.max_claims, CAST(c.starts_at AS TEXT) AS starts_at, CAST(c.expires_at AS TEXT) AS expires_at, b.code_prefix AS batch_code_prefix, b.code_length AS batch_code_length FROM promotion_code c LEFT JOIN promotion_code_batch b ON b.tenant_id = c.tenant_id AND b.id = c.code_batch_id WHERE c.id=$1 AND c.tenant_id=$2 AND c.organization_id=$3 FOR UPDATE OF c",
    )
    .bind(code_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read promotion code for reissue", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    let claimed_quantity = row
        .try_get::<i32, _>("claimed_quantity")
        .map_err(|error| decode("claimed_quantity", error))?;
    if let Err(message) = code_reissue_decision(&status, i64::from(claimed_quantity)) {
        return Err(CommerceServiceError::conflict(message));
    }
    let stock_id = row
        .try_get::<String, _>("stock_id")
        .map_err(|error| decode("stock_id", error))?;
    let offer_id = row
        .try_get::<String, _>("offer_id")
        .map_err(|error| decode("offer_id", error))?;
    let offer_version_id = row
        .try_get::<String, _>("offer_version_id")
        .map_err(|error| decode("offer_version_id", error))?;
    let code_batch_id = row
        .try_get::<Option<String>, _>("code_batch_id")
        .map_err(|error| decode("code_batch_id", error))?;
    let code_type = row
        .try_get::<String, _>("code_type")
        .map_err(|error| decode("code_type", error))?;
    let max_claims = row
        .try_get::<i32, _>("max_claims")
        .map_err(|error| decode("max_claims", error))?;
    let starts_at = row
        .try_get::<Option<String>, _>("starts_at")
        .map_err(|error| decode("starts_at", error))?;
    let expires_at = row
        .try_get::<Option<String>, _>("expires_at")
        .map_err(|error| decode("expires_at", error))?;
    let batch_code_prefix = row
        .try_get::<Option<String>, _>("batch_code_prefix")
        .map_err(|error| decode("batch_code_prefix", error))?;
    let batch_code_length = row
        .try_get::<Option<i32>, _>("batch_code_length")
        .map_err(|error| decode("batch_code_length", error))?;

    // Operator-supplied codes are stored in the same canonical form custom batch
    // codes use; otherwise the batch's own prefix/length generate the value, and
    // a batch without a usable pair falls back to a plain 16-character code (a
    // custom-code batch records an empty prefix, for example).
    let value = match explicit_code {
        Some(code) => normalized_custom_code(code)?,
        None => {
            let prefix = batch_code_prefix
                .as_deref()
                .map(str::trim)
                .filter(|prefix| !prefix.is_empty());
            let length = batch_code_length.filter(|length| (4..=64).contains(length));
            match (prefix, length) {
                (Some(prefix), Some(length))
                    if usize::try_from(length)
                        .is_ok_and(|length| length > prefix.chars().count()) =>
                {
                    generated_code(prefix, length)?
                }
                _ => generated_code("", 16)?,
            }
        }
    };
    let identity = code_identity(&value);
    let new_code_id = generated_id();
    let new_code_no = reference("COD");
    let code_masked = mask_code(&value);
    let detail_json = serde_json::json!({
        "replacedCodeId": code_id.as_str(),
        "newCodeId": new_code_id.as_str(),
        "reason": reason.as_str(),
    })
    .to_string();
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        CODE_REISSUE_AUDIT_ACTION,
        CODE_AUDIT_AGGREGATE_TYPE,
        code_id.as_str(),
        Some(detail_json.as_str()),
        trace_id,
    )
    .await?;
    let affected = sqlx::query(
        "UPDATE promotion_code SET status='disabled', voided_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'), void_reason=$1, replaced_by_code_id=$5, version=version+1, updated_at=TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS') WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND status='active' AND COALESCE(claimed_quantity,0)=0",
    )
    .bind(reason.as_str())
    .bind(code_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(new_code_id.as_str())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("disable reissued promotion code", error))?
    .rows_affected();
    if affected != 1 {
        // The row lock was held, so only a row that vanished between the read
        // and the write can reach this branch.
        return Err(CommerceServiceError::conflict("code is not active"));
    }
    sqlx::query(INSERT_REISSUED_PROMOTION_CODE_SQL)
        .bind(new_code_id.as_str())
        .bind(uuid())
        .bind(scope.tenant_id.to_string())
        .bind(scope.organization_id.to_string())
        .bind(code_batch_id.as_deref())
        .bind(stock_id.as_str())
        .bind(offer_id.as_str())
        .bind(offer_version_id.as_str())
        .bind(new_code_no.as_str())
        .bind(value.as_str())
        .bind(code_type.as_str())
        .bind(max_claims)
        .bind(starts_at.as_deref())
        .bind(expires_at.as_deref())
        .bind(identity.hash.as_str())
        .bind(identity.last4.as_str())
        .bind(code_id.as_str())
        .execute(&mut *tx)
        .await
        // A duplicate explicit code reuses the batch path's unique-violation
        // mapping, so both insertion paths report the same `409`.
        .map_err(code_insert_error)?;
    // One evidence row covers both sides of the lineage.
    update_audit_subject_count(&mut tx, &audit_id, scope, 2).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit code reissue", error))?;
    Ok(Some(CodeReissueOutcome {
        new_code_id,
        new_code_no,
        code_masked,
        code_last4: identity.last4,
    }))
}

// ---------------------------------------------------------------------------
// Settlement lifecycle: settle / release one discount application.
//
// Money-unit rule: every `promotion_budget_account` /
// `promotion_budget_ledger_entry` amount is an integer minor unit (`*_minor
// BIGINT`). The application's own `discount_amount` is still the legacy
// decimal-major string, so it is converted once, at this boundary, with
// `money_cents`. No float and no implicit division by 100 is involved.
// ---------------------------------------------------------------------------

/// Which bucket a reservation moves into.
#[derive(Clone, Copy)]
enum BudgetTransfer {
    /// `reserved -> used` (the discount became real spend).
    Settle,
    /// `reserved -> available` (the reservation was abandoned).
    Release,
}

impl BudgetTransfer {
    fn direction(self) -> &'static str {
        match self {
            Self::Settle => "settle",
            Self::Release => "release",
        }
    }
}

/// Account balances immediately after one budget transfer.
///
/// Carried into the ledger row so `available_after + reserved_after + used_after
/// <= total` can be checked from the evidence alone.
struct BudgetBalances {
    available_minor: i64,
    reserved_minor: i64,
    used_minor: i64,
    currency_code: String,
}

/// Settles one `applied` discount application together with its audit evidence.
///
/// The reserved amount moves `reserved -> used` in a single guarded `UPDATE`
/// (`reserved_amount_minor >= $amount`), so the account can never go negative and
/// the `chk_pba_amounts` invariant (`available + reserved + used <= total`) is
/// preserved. An application without a budget account (the historical shape) is
/// still settled: there is simply no money to move and no ledger row.
///
/// `discount_applications.settle` (aggregate `promotion_discount_application`) is
/// written before the mutation in the same transaction; if that write fails the
/// operation fails and nothing changes.
pub(crate) async fn settle_discount_application(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    application_id: String,
    trace_id: &str,
) -> Result<Option<PromotionDiscountApplicationSettlementItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin discount application settlement").await?;
    let now = now_string();
    let row = sqlx::query(
        "SELECT status, CAST(discount_amount AS TEXT) AS discount_amount, budget_account_id, request_no FROM promotion_discount_application WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 FOR UPDATE",
    )
    .bind(application_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read discount application for settlement", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    if let Err(message) = settle_decision(&status) {
        return Err(CommerceServiceError::conflict(message));
    }
    let amount_minor = application_amount_minor(&row)?;
    let budget_account_id = row
        .try_get::<Option<String>, _>("budget_account_id")
        .map_err(|error| decode("budget_account_id", error))?;
    let request_no = row
        .try_get::<String, _>("request_no")
        .map_err(|error| decode("request_no", error))?;
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION,
        DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE,
        application_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let moved_minor = match budget_account_id.as_deref() {
        Some(account_id) => {
            let balances = transfer_reserved_budget(
                &mut tx,
                BudgetTransfer::Settle,
                account_id,
                scope.tenant_id,
                amount_minor,
                &now,
            )
            .await?;
            insert_budget_ledger_entry(
                &mut tx,
                scope,
                account_id,
                Some(application_id.as_str()),
                BudgetTransfer::Settle,
                amount_minor,
                &balances,
                request_no.as_str(),
                &now,
            )
            .await?;
            amount_minor
        }
        // No budget account: the historical flow, no money to move.
        None => 0,
    };
    let affected = sqlx::query(
        "UPDATE promotion_discount_application SET status='settled', settled_at=$1, version=version+1, updated_at=$1 WHERE id=$2 AND tenant_id=$3 AND organization_id=$4 AND status='applied'",
    )
    .bind(now.as_str())
    .bind(application_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("settle promotion discount application", error))?
    .rows_affected();
    if affected != 1 {
        // The row lock and the status guard both held, so only a state change
        // committed by another session can reach this branch.
        return Err(CommerceServiceError::conflict(
            "promotion discount application is not in the applied state",
        ));
    }
    update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit discount application settlement", error))?;
    Ok(Some(PromotionDiscountApplicationSettlementItem {
        application_id,
        status: "settled".to_owned(),
        settled_amount_minor: moved_minor,
    }))
}

/// Releases one `applied` discount application together with its audit evidence.
///
/// Mirror of [`settle_discount_application`]: the reserved amount moves
/// `reserved -> available` in one guarded `UPDATE`, the row keeps `released_at`
/// plus `release_reason` so the stop is auditable from the row itself, and
/// `discount_applications.release` is written before the mutation.
pub(crate) async fn release_discount_application(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    application_id: String,
    reason: String,
    trace_id: &str,
) -> Result<Option<PromotionDiscountApplicationReleaseItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin discount application release").await?;
    let now = now_string();
    let row = sqlx::query(
        "SELECT status, CAST(discount_amount AS TEXT) AS discount_amount, budget_account_id, request_no FROM promotion_discount_application WHERE id=$1 AND tenant_id=$2 AND organization_id=$3 FOR UPDATE",
    )
    .bind(application_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("read discount application for release", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let status = row
        .try_get::<String, _>("status")
        .map_err(|error| decode("status", error))?;
    if let Err(message) = release_decision(&status) {
        return Err(CommerceServiceError::conflict(message));
    }
    let amount_minor = application_amount_minor(&row)?;
    let budget_account_id = row
        .try_get::<Option<String>, _>("budget_account_id")
        .map_err(|error| decode("budget_account_id", error))?;
    let request_no = row
        .try_get::<String, _>("request_no")
        .map_err(|error| decode("request_no", error))?;
    let detail_json = serde_json::json!({ "reason": reason }).to_string();
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
        DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE,
        application_id.as_str(),
        Some(detail_json.as_str()),
        trace_id,
    )
    .await?;
    let moved_minor = match budget_account_id.as_deref() {
        Some(account_id) => {
            let balances = transfer_reserved_budget(
                &mut tx,
                BudgetTransfer::Release,
                account_id,
                scope.tenant_id,
                amount_minor,
                &now,
            )
            .await?;
            insert_budget_ledger_entry(
                &mut tx,
                scope,
                account_id,
                Some(application_id.as_str()),
                BudgetTransfer::Release,
                amount_minor,
                &balances,
                request_no.as_str(),
                &now,
            )
            .await?;
            amount_minor
        }
        None => 0,
    };
    let affected = sqlx::query(
        "UPDATE promotion_discount_application SET status='released', released_at=$1, release_reason=$2, version=version+1, updated_at=$1 WHERE id=$3 AND tenant_id=$4 AND organization_id=$5 AND status='applied'",
    )
    .bind(now.as_str())
    .bind(reason.as_str())
    .bind(application_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|error| storage("release promotion discount application", error))?
    .rows_affected();
    if affected != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion discount application is not in the applied state",
        ));
    }
    update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    tx.commit()
        .await
        .map_err(|error| storage("commit discount application release", error))?;
    Ok(Some(PromotionDiscountApplicationReleaseItem {
        application_id,
        status: "released".to_owned(),
        released_amount_minor: moved_minor,
    }))
}

/// Converts the application's stored decimal-major `discount_amount` into
/// integer minor units.
///
/// This is the one place the new settlement code crosses from the legacy money
/// representation to `*_minor`: `money_cents` owns the decimal-to-minor parse
/// (it rejects signs, exponents and more than two fractional digits), and the
/// result is used verbatim for the budget transfer and the ledger row.
fn application_amount_minor(
    row: &sqlx::postgres::PgRow,
) -> Result<i64, CommerceServiceError> {
    let amount = row
        .try_get::<String, _>("discount_amount")
        .map_err(|error| decode("discount_amount", error))?;
    // `promotion_discount_application.discount_amount` is persisted in integer
    // minor units by the apply path; a legacy decimal-major row is still
    // normalised by `contract_money_minor_units`.
    crate::postgres_promotion::contract_money_minor_units(&amount)?
        .parse::<i64>()
        .map_err(|_| CommerceServiceError::storage("discount amount is not integer minor units"))
}

/// Moves a reservation between buckets in one guarded `UPDATE`.
///
/// The `reserved_amount_minor >= $1` predicate is the whole safety property: it
/// is evaluated by the database under the row lock, so two concurrent operations
/// on the same account cannot both consume the same reserved amount. A
/// zero-row result is a domain conflict, never a silent no-op.
async fn transfer_reserved_budget(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transfer: BudgetTransfer,
    budget_account_id: &str,
    tenant_id: i64,
    amount_minor: i64,
    now: &str,
) -> Result<BudgetBalances, CommerceServiceError> {
    let sql = match transfer {
        BudgetTransfer::Settle => {
            "UPDATE promotion_budget_account SET reserved_amount_minor = reserved_amount_minor - $1, used_amount_minor = used_amount_minor + $1, version = version + 1, updated_at = $4 WHERE id = $2 AND tenant_id = $3 AND status = 'active' AND reserved_amount_minor >= $1 RETURNING available_amount_minor, reserved_amount_minor, used_amount_minor, currency_code"
        }
        BudgetTransfer::Release => {
            "UPDATE promotion_budget_account SET available_amount_minor = available_amount_minor + $1, reserved_amount_minor = reserved_amount_minor - $1, version = version + 1, updated_at = $4 WHERE id = $2 AND tenant_id = $3 AND status = 'active' AND reserved_amount_minor >= $1 RETURNING available_amount_minor, reserved_amount_minor, used_amount_minor, currency_code"
        }
    };
    let row = sqlx::query(sql)
        .bind(amount_minor)
        .bind(budget_account_id)
        .bind(tenant_id.to_string())
        .bind(now)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| storage("move promotion budget reservation", error))?
        .ok_or_else(|| {
            CommerceServiceError::conflict(
                "promotion discount budget reservation is not available",
            )
        })?;
    Ok(BudgetBalances {
        available_minor: row
            .try_get::<i64, _>("available_amount_minor")
            .map_err(|error| decode("available_amount_minor", error))?,
        reserved_minor: row
            .try_get::<i64, _>("reserved_amount_minor")
            .map_err(|error| decode("reserved_amount_minor", error))?,
        used_minor: row
            .try_get::<i64, _>("used_amount_minor")
            .map_err(|error| decode("used_amount_minor", error))?,
        currency_code: row
            .try_get::<String, _>("currency_code")
            .map_err(|error| decode("currency_code", error))?,
    })
}

/// Appends one immutable budget ledger row for a settle/release transfer.
///
/// `amount_minor` is always positive and `direction` carries the sign; the three
/// `_after_minor` columns are the account state the guarded `UPDATE` returned, so
/// a ledger row can be reconciled without replaying the table.
#[allow(clippy::too_many_arguments)]
async fn insert_budget_ledger_entry(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: &PromotionAdminScope,
    budget_account_id: &str,
    application_id: Option<&str>,
    transfer: BudgetTransfer,
    amount_minor: i64,
    balances: &BudgetBalances,
    request_no: &str,
    occurred_at: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        "INSERT INTO promotion_budget_ledger_entry (id,uuid,tenant_id,organization_id,budget_account_id,application_id,direction,amount_minor,available_after_minor,reserved_after_minor,used_after_minor,currency_code,request_no,idempotency_key,occurred_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
    )
    .bind(generated_id())
    .bind(uuid())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(budget_account_id)
    .bind(application_id)
    .bind(transfer.direction())
    .bind(amount_minor)
    .bind(balances.available_minor)
    .bind(balances.reserved_minor)
    .bind(balances.used_minor)
    .bind(balances.currency_code.as_str())
    .bind(request_no)
    .bind(crate::postgres_promotion::stable_storage_id(&[
        "budget-ledger",
        &scope.tenant_id.to_string(),
        transfer.direction(),
        application_id.unwrap_or(budget_account_id),
    ]))
    .bind(occurred_at)
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("append promotion budget ledger entry", error))?;
    Ok(())
}

/// Canonical server-form timestamp for a lifecycle mutation.
fn now_string() -> String {
    Utc::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Persists one audit-evidence row for an audited mutation.
///
/// The row is inserted inside the caller's transaction and *before* the code
/// read on purpose: a redeemable code must never leave the service without
/// evidence, so a failed audit INSERT aborts the whole operation (the error
/// propagates and the transaction is dropped, so the read never commits).
/// `subject_count` starts at 0 because the number of codes an export returns is
/// not known until the read completes; the same transaction then updates it to
/// the number of codes actually returned.
///
/// `actor_id` is the IAM-authenticated operator from [`PromotionAdminScope`].
/// No body or query field can select it: the actor identity is never read from
/// the request payload.
async fn insert_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: &PromotionAdminScope,
    action: &str,
    aggregate_type: &str,
    aggregate_id: &str,
    detail_json: Option<&str>,
    trace_id: &str,
) -> Result<String, CommerceServiceError> {
    let id = generated_id();
    let actor_id = scope.actor_id()?;
    sqlx::query(
        "INSERT INTO promotion_audit_event (id,uuid,tenant_id,organization_id,actor_id,action,aggregate_type,aggregate_id,subject_count,detail_json,trace_id,occurred_at,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,0,$9,$10,TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'),TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS'))",
    )
    .bind(id.as_str())
    .bind(uuid())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .bind(actor_id.to_string())
    .bind(action)
    .bind(aggregate_type)
    .bind(aggregate_id)
    .bind(detail_json)
    .bind(trace_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| storage("write promotion audit event", error))?;
    Ok(id)
}

/// Records how many codes the audited access actually returned.
///
/// Still inside the same transaction, so the committed evidence row and the
/// served codes can never disagree.
async fn update_audit_subject_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    audit_id: &str,
    scope: &PromotionAdminScope,
    subject_count: i64,
) -> Result<(), CommerceServiceError> {
    sqlx::query("UPDATE promotion_audit_event SET subject_count=$1 WHERE id=$2 AND tenant_id=$3")
        .bind(subject_count)
        .bind(audit_id)
        .bind(scope.tenant_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|error| storage("update promotion audit event", error))?;
    Ok(())
}

async fn begin_admin_transaction<'a>(
    pool: &'a AdminPool,
    context: &str,
) -> Result<sqlx::Transaction<'a, sqlx::Postgres>, CommerceServiceError> {
    match pool {
        AdminPool::Postgres(pool) => pool.begin().await.map_err(|error| storage(context, error)),
    }
}

/// Reveals one redeemable code together with its audit evidence.
///
/// The audit row is written first, then the code is read scoped by
/// tenant/organization, and both are committed in one transaction. An audit
/// failure returns an error before any code is read; a commit failure returns an
/// error after the read, so neither path can serve a code without evidence.
///
/// The audit row is committed even when the code is absent (`None` -> 404): an
/// operator attempting to reveal a code outside their scope is itself auditable,
/// and the row truthfully records `subject_count = 0`.
pub(crate) async fn reveal_code(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    code_id: String,
    trace_id: &str,
) -> Result<Option<PromotionCodeRevealItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin code reveal").await?;
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        CODE_REVEAL_AUDIT_ACTION,
        CODE_AUDIT_AGGREGATE_TYPE,
        code_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let row = sqlx::query(
        "SELECT code_no, promotion_code FROM promotion_code WHERE id=$1 AND tenant_id=$2 AND organization_id=$3",
    )
    .bind(code_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| storage("reveal promotion code", error))?;
    let item = match row {
        Some(row) => Some(PromotionCodeRevealItem {
            code_no: row
                .try_get("code_no")
                .map_err(|error| decode("code_no", error))?,
            code: row
                .try_get("promotion_code")
                .map_err(|error| decode("promotion_code", error))?,
        }),
        None => None,
    };
    if item.is_some() {
        update_audit_subject_count(&mut tx, &audit_id, scope, 1).await?;
    }
    tx.commit()
        .await
        .map_err(|error| storage("commit code reveal", error))?;
    Ok(item)
}

/// Exports a batch's redeemable codes together with their audit evidence.
///
/// Same evidence contract as [`reveal_code`]: the audit row is written first and
/// the batch existence check plus the code read happen in the same transaction.
/// The batch must be in the operator's tenant/organization scope, otherwise the
/// result is `None` (404).
///
/// Codes are ordered by `code_no` so the same batch always exports in the same
/// order, and the query fetches one row past the cap so `truncated` is provable
/// without a COUNT that could race the page (see [`finalize_code_export`]).
pub(crate) async fn export_code_batch(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    batch_id: String,
    trace_id: &str,
) -> Result<Option<PromotionCodeExportItem>, CommerceServiceError> {
    let mut tx = begin_admin_transaction(pool, "begin code export").await?;
    let audit_id = insert_audit_event(
        &mut tx,
        scope,
        CODE_EXPORT_AUDIT_ACTION,
        CODE_BATCH_AUDIT_AGGREGATE_TYPE,
        batch_id.as_str(),
        None,
        trace_id,
    )
    .await?;
    let batch_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM promotion_code_batch WHERE id=$1 AND tenant_id=$2 AND organization_id=$3)",
    )
    .bind(batch_id.as_str())
    .bind(scope.tenant_id.to_string())
    .bind(scope.organization_id.to_string())
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| storage("resolve promotion code batch", error))?;
    let export = if batch_exists {
        let rows = sqlx::query(
            "SELECT code_no, promotion_code FROM promotion_code WHERE code_batch_id=$1 AND tenant_id=$2 AND organization_id=$3 ORDER BY code_no LIMIT $4",
        )
        .bind(batch_id.as_str())
        .bind(scope.tenant_id.to_string())
        .bind(scope.organization_id.to_string())
        .bind(CODE_EXPORT_FETCH_LIMIT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| storage("export promotion codes", error))?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(PromotionCodeRevealItem {
                code_no: row
                    .try_get("code_no")
                    .map_err(|error| decode("code_no", error))?,
                code: row
                    .try_get("promotion_code")
                    .map_err(|error| decode("promotion_code", error))?,
            });
        }
        Some(finalize_code_export(items))
    } else {
        None
    };
    if let Some(export) = &export {
        update_audit_subject_count(&mut tx, &audit_id, scope, i64::from(export.count)).await?;
    }
    tx.commit()
        .await
        .map_err(|error| storage("commit code export", error))?;
    Ok(export)
}

enum ManagementRow {
    Postgres(sqlx::postgres::PgRow),
}
impl ManagementRow {
    fn i32(&self, name: &str) -> Result<i32, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode(name, error))
    }
    fn i64(&self, name: &str) -> Result<i64, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode(name, error))
    }
    fn string(&self, name: &str) -> Result<String, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode(name, error))
    }
    fn optional_string(&self, name: &str) -> Result<Option<String>, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode(name, error))
    }
    fn optional_i64(&self, name: &str) -> Result<Option<i64>, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode(name, error))
    }
}

fn map_campaign(row: ManagementRow) -> Result<PromotionCampaignItem, CommerceServiceError> {
    Ok(PromotionCampaignItem {
        id: row.string("id")?,
        campaign_no: row.string("campaign_no")?,
        campaign_code: row.optional_string("campaign_code")?,
        display_name: row.string("display_name")?,
        description: row.optional_string("description")?,
        channel_scope: row.string("channel_scope")?,
        audience_scope: row.string("audience_scope")?,
        starts_at: row.string("starts_at")?,
        ends_at: row.optional_string("ends_at")?,
        status: row.string("status")?,
        version: row.i64("version")?,
        updated_at: row.string("updated_at")?,
    })
}

fn map_offer(row: ManagementRow) -> Result<PromotionOfferItem, CommerceServiceError> {
    let coupon_benefit = parse_admin_coupon_benefit(row.optional_string("rule_json")?.as_deref())?;
    Ok(PromotionOfferItem {
        id: row.string("id")?,
        campaign_id: row.optional_string("campaign_id")?,
        offer_no: row.string("offer_no")?,
        offer_code: row.optional_string("offer_code")?,
        offer_type: row.string("offer_type")?,
        audience_scope: row.string("audience_scope")?,
        combinability: row.string("combinability")?,
        goods_scope: row.string("goods_scope")?,
        display_name: row.string("display_name")?,
        description: row.optional_string("description")?,
        priority: row.i32("priority")?,
        starts_at: row.string("starts_at")?,
        ends_at: row.optional_string("ends_at")?,
        status: row.string("status")?,
        discount_type: row.optional_string("discount_type")?,
        discount_value: row.optional_string("discount_value")?,
        minimum_amount: row.optional_string("minimum_amount")?,
        maximum_discount_amount: row.optional_string("maximum_discount_amount")?,
        currency_code: row.optional_string("currency_code")?,
        coupon_benefit,
        version: row.i64("version")?,
        updated_at: row.string("updated_at")?,
    })
}

fn map_stock(row: ManagementRow) -> Result<PromotionCouponStockItem, CommerceServiceError> {
    Ok(PromotionCouponStockItem {
        id: row.string("id")?,
        offer_id: row.string("offer_id")?,
        stock_no: row.string("stock_no")?,
        stock_type: row.string("stock_type")?,
        code_issue_mode: row.string("code_issue_mode")?,
        total_quantity: row.i64("total_quantity")?,
        available_quantity: row.i64("available_quantity")?,
        claimed_quantity: row.i64("claimed_quantity")?,
        redeemed_quantity: row.i64("redeemed_quantity")?,
        locked_quantity: row.i64("locked_quantity")?,
        per_user_limit: row.i32("per_user_limit")?,
        claim_starts_at: row.optional_string("claim_starts_at")?,
        claim_ends_at: row.optional_string("claim_ends_at")?,
        status: row.string("status")?,
        budget_amount_minor: row.optional_i64("budget_amount_minor")?,
        budget_available_minor: row.optional_i64("budget_available_minor")?,
        budget_reserved_minor: row.optional_i64("budget_reserved_minor")?,
        budget_used_minor: row.optional_i64("budget_used_minor")?,
    })
}

fn map_code_batch(row: ManagementRow) -> Result<PromotionCodeBatchItem, CommerceServiceError> {
    Ok(PromotionCodeBatchItem {
        id: row.string("id")?,
        stock_id: row.string("stock_id")?,
        offer_id: row.string("offer_id")?,
        batch_no: row.string("batch_no")?,
        code_type: row.string("code_type")?,
        requested_quantity: row.i64("requested_quantity")?,
        generated_quantity: row.i64("generated_quantity")?,
        code_length: row.i32("code_length")?,
        code_prefix: row.string("code_prefix")?,
        starts_at: row.optional_string("starts_at")?,
        expires_at: row.optional_string("expires_at")?,
        status: row.string("status")?,
        created_at: row.string("created_at")?,
    })
}

fn map_distribution_task(
    row: ManagementRow,
) -> Result<PromotionDistributionTaskItem, CommerceServiceError> {
    Ok(PromotionDistributionTaskItem {
        id: row.string("id")?,
        stock_id: row.string("stock_id")?,
        offer_id: row.string("offer_id")?,
        task_no: row.string("task_no")?,
        distribution_type: row.string("distribution_type")?,
        requested_quantity: row.i64("requested_quantity")?,
        succeeded_quantity: row.i64("succeeded_quantity")?,
        failed_quantity: row.i64("failed_quantity")?,
        status: row.string("status")?,
        created_at: row.string("created_at")?,
        completed_at: row.optional_string("completed_at")?,
    })
}

fn map_user_coupon(
    row: ManagementRow,
) -> Result<PromotionAdminUserCouponItem, CommerceServiceError> {
    Ok(PromotionAdminUserCouponItem {
        id: row.string("id")?,
        coupon_no: row.string("coupon_no")?,
        stock_id: row.string("stock_id")?,
        offer_id: row.string("offer_id")?,
        owner_user_id: row.string("owner_user_id")?,
        coupon_code: mask_code(&row.string("coupon_code")?),
        status: row.string("status")?,
        claimed_at: row.string("claimed_at")?,
        valid_from: row.string("valid_from")?,
        expires_at: row.optional_string("expires_at")?,
        redeemed_at: row.optional_string("redeemed_at")?,
        source_type: row.optional_string("source_type")?,
        source_id: row
            .optional_string("source_id")?
            .map(|value| value.to_string()),
    })
}

fn map_ledger(row: ManagementRow) -> Result<PromotionCouponLedgerItem, CommerceServiceError> {
    Ok(PromotionCouponLedgerItem {
        id: row.string("id")?,
        stock_id: row.string("stock_id")?,
        user_coupon_id: row
            .optional_string("user_coupon_id")?
            .map(|value| value.to_string()),
        offer_id: row.string("offer_id")?,
        subject_id: row
            .optional_string("subject_id")?
            .map(|value| value.to_string()),
        direction: row.string("direction")?,
        quantity_delta: row.i64("quantity_delta")?,
        balance_after: row.i64("balance_after")?,
        business_type: row.string("business_type")?,
        business_no: row.string("business_no")?,
        created_at: row.string("created_at")?,
    })
}

fn map_redemption_attempt(
    row: ManagementRow,
) -> Result<PromotionRedemptionAttemptItem, CommerceServiceError> {
    Ok(PromotionRedemptionAttemptItem {
        id: row.string("id")?,
        code_id: row.optional_string("code_id")?,
        submitted_code_suffix: row.string("submitted_code_suffix")?,
        subject_type: row.string("subject_type")?,
        subject_id: row.string("subject_id")?,
        outcome: row.string("outcome")?,
        failure_code: row.optional_string("failure_code")?,
        failure_detail: row.optional_string("failure_detail")?,
        benefit_kind: row.optional_string("benefit_kind")?,
        granted_amount: row.optional_string("granted_amount")?,
        currency_code: row.optional_string("currency_code")?,
        request_no: row.string("request_no")?,
        order_id: row.optional_string("order_id")?,
        trace_id: row.string("trace_id")?,
        occurred_at: row.string("occurred_at")?,
    })
}

fn generated_id() -> String {
    let value = (Uuid::new_v4().as_u128() & 0x7fff_ffff_ffff_ffff) as i64;
    if value == 0 { 1 } else { value }.to_string()
}

fn reference(prefix: &str) -> String {
    let suffix = Uuid::new_v4().simple().to_string();
    format!(
        "{prefix}-{}-{}",
        Utc::now().format("%Y%m%d%H%M%S"),
        &suffix[..8]
    )
    .to_ascii_uppercase()
}
/// Canonical storage form of an admin-defined coupon code.
///
/// The redemption lookup compares `UPPER(BTRIM(promotion_code))` with the
/// upper-cased submitted code, so a custom code is persisted in that same
/// canonical form; storing it verbatim would make a lower-case submission or a
/// lower-case stored value ambiguous.
fn normalized_custom_code(code: &str) -> Result<String, CommerceServiceError> {
    let normalized = code.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err(CommerceServiceError::validation(
            "custom coupon code must not be empty",
        ));
    }
    Ok(normalized)
}

fn generated_code(prefix: &str, length: i32) -> Result<String, CommerceServiceError> {    let prefix = prefix.trim().to_ascii_uppercase();
    if !prefix.chars().all(|value| value.is_ascii_alphanumeric()) || prefix.len() >= length as usize
    {
        return Err(CommerceServiceError::validation(
            "code_prefix must be alphanumeric and shorter than code_length",
        ));
    }
    let mut random = String::new();
    while random.len() < length as usize - prefix.len() {
        random.push_str(&Uuid::new_v4().simple().to_string().to_ascii_uppercase());
    }
    random.truncate(length as usize - prefix.len());
    Ok(format!("{prefix}{random}"))
}
fn mask_code(value: &str) -> String {
    if value.chars().count() <= 8 {
        return "****".to_owned();
    }
    let prefix = value.chars().take(4).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}****{suffix}")
}
fn trimmed(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
fn storage(context: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("{context}: {error}"))
}
fn decode(column: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("decode promotion admin column {column}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Body of one free function, from its `async fn` marker to the next item.
    fn function_body<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("missing function: {marker}"));
        let rest = &source[start..];
        let end = rest
            .find("\nasync fn ")
            .or_else(|| rest.find("\nfn "))
            .unwrap_or(rest.len());
        &rest[..end]
    }

    /// Body of one function, from `marker` to an explicit end marker.
    fn section<'a>(source: &'a str, marker: &str, end_marker: &str) -> &'a str {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("missing function: {marker}"));
        let rest = &source[start..];
        let end = rest.find(end_marker).unwrap_or(rest.len());
        &rest[..end]
    }

    /// Column names of one `CREATE TABLE IF NOT EXISTS <table> (...)` block.
    ///
    /// Constraint lines are not columns; inline trailing comments and the
    /// separating commas are stripped. Works on CRLF files (`lines()` trims the
    /// carriage return).
    fn create_table_columns(sql: &str, table: &str) -> Vec<String> {
        let marker = format!("CREATE TABLE IF NOT EXISTS {table} (");
        let start = sql
            .find(&marker)
            .unwrap_or_else(|| panic!("missing table {table}"));
        let rest = &sql[start + marker.len()..];
        let end = rest
            .find("\n);")
            .unwrap_or_else(|| panic!("unterminated table {table}"));
        let mut columns = rest[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("--"))
            .filter(|line| !line.starts_with("CONSTRAINT"))
            .map(|line| {
                line.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(',')
                    .to_owned()
            })
            .collect::<Vec<_>>();
        columns.sort();
        columns
    }

    #[test]
    fn audited_code_access_writes_evidence_in_the_same_transaction_as_the_read() {
        let source = include_str!("promotion_admin_management.rs");

        // The reason for the ordering lives in the code, not only in the tests.
        // Both needles are assembled with `concat!` so the assertion cannot match
        // its own string literal in this test source: a redeemable code must
        // never be served when the evidence write failed.
        const EVIDENCE_REASON: &str =
            concat!("a redeemable code must never ", "leave the service without");
        const FAIL_CLOSED_REASON: &str =
            concat!("a failed audit INSERT ", "aborts the whole operation");
        assert!(
            source.contains(EVIDENCE_REASON),
            "the audit ordering must stay documented next to the write"
        );
        assert!(
            source.contains(FAIL_CLOSED_REASON),
            "the fail-closed rationale must stay in a comment"
        );

        for (marker, end_marker, action) in [
            (
                "pub(crate) async fn reveal_code",
                "\n/// Exports a batch's redeemable codes",
                "CODE_REVEAL_AUDIT_ACTION",
            ),
            (
                "pub(crate) async fn export_code_batch",
                "\nenum ManagementRow",
                "CODE_EXPORT_AUDIT_ACTION",
            ),
        ] {
            let body = section(source, marker, end_marker);
            // One transaction wraps the evidence write and the read.
            assert!(
                body.contains("begin_admin_transaction(pool,"),
                "{marker} must open a transaction"
            );
            let audit_at = body
                .find("insert_audit_event(")
                .unwrap_or_else(|| panic!("{marker} must write the audit row"));
            let read_at = body
                .find("SELECT code_no, promotion_code FROM promotion_code")
                .unwrap_or_else(|| panic!("{marker} must read the redeemable code"));
            assert!(
                audit_at < read_at,
                "{marker} must write the audit row BEFORE reading the code"
            );
            // The write is fail-closed: `?` after the awaited insert aborts the
            // operation (and drops the transaction) instead of serving a code.
            assert!(
                body[audit_at..read_at].contains(".await?;"),
                "{marker} must abort when the audit write fails"
            );
            assert!(
                body.contains(action),
                "{marker} must record the documented audit action"
            );
            // The read runs on the transaction handle, not on the pool.
            assert!(
                body.contains("&mut *tx"),
                "{marker} must read inside the same transaction"
            );
            // Tenant and organization scoping is part of the read predicate, so a
            // code/batch outside the operator scope is indistinguishable from a
            // missing one (404) and never returned.
            assert!(
                body.contains("tenant_id=$2 AND organization_id=$3")
                    || body.contains("tenant_id = $1 AND organization_id = $2"),
                "{marker} must scope the read by tenant and organization"
            );
            let commit_at = body
                .find("tx.commit()")
                .unwrap_or_else(|| panic!("{marker} must commit"));
            assert!(
                commit_at > read_at,
                "{marker} must commit after the read, so both land together"
            );
            // The evidence row records the served subject count in the same tx.
            assert!(
                body.contains("update_audit_subject_count"),
                "{marker} must record how many codes were served"
            );
            // The actor can only come from the IAM-derived scope.
            assert!(
                !body.contains("actor_id: String"),
                "{marker} must not accept an actor id"
            );
            assert!(
                !body.contains("Json("),
                "{marker} must not read the request body"
            );
        }

        let insert = section(
            source,
            "async fn insert_audit_event",
            "\n/// Records how many codes",
        );
        assert!(insert.contains("INSERT INTO promotion_audit_event"));
        // Actor and tenancy come from the scope, never from a request payload.
        assert!(insert.contains("scope.actor_id()?"));
        assert!(insert.contains("scope.tenant_id.to_string()"));
        assert!(insert.contains("scope.organization_id.to_string()"));
        // `occurred_at` is the server-form timestamp, like every other evidence
        // table in this schema.
        assert!(insert.contains("TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC'"));
        assert!(!insert.contains("unwrap"));
    }

    #[test]
    fn audit_event_migration_and_baseline_declare_the_same_columns() {
        let migration =
            include_str!("../../../database/migrations/postgres/0004_audit_event.up.sql");
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");

        let from_migration = create_table_columns(migration, "promotion_audit_event");
        let from_baseline = create_table_columns(baseline, "promotion_audit_event");

        assert_eq!(
            from_migration, from_baseline,
            "the folded baseline and the 0004 migration must declare the same columns"
        );
        assert_eq!(
            from_migration,
            vec![
                "action",
                "actor_id",
                "aggregate_id",
                "aggregate_type",
                "created_at",
                "detail_json",
                "id",
                "occurred_at",
                "organization_id",
                "subject_count",
                "tenant_id",
                "trace_id",
                "uuid",
            ],
            "promotion_audit_event must keep the audited evidence column set"
        );

        // Both files must also carry the audit indexes and the action check.
        for sql in [migration, baseline] {
            assert!(sql.contains("idx_pae_tenant_time"));
            assert!(sql.contains("idx_pae_aggregate"));
            assert!(sql.contains("chk_pae_action"));
        }
        assert_eq!(
            check_actions(migration),
            vec!["codes.export".to_owned(), "codes.reveal".to_owned()],
            "0004 is history-immutable and keeps its original two-action check"
        );
        assert_eq!(
            check_actions(baseline),
            reissue_audit_actions(),
            "the folded baseline must admit the same vocabulary as the migrations"
        );
    }

    /// Ordered audit actions admitted by `chk_pae_action`.
    ///
    /// Reads the constraint body out of the SQL text, so the test fails if any
    /// action is dropped, added, or misspelled on either side.
    fn check_actions(sql: &str) -> Vec<String> {
        let start = sql
            .find("chk_pae_action")
            .unwrap_or_else(|| panic!("missing chk_pae_action"));
        let rest = &sql[start..];
        let open = rest
            .find("action IN (")
            .unwrap_or_else(|| panic!("missing chk_pae_action vocabulary"));
        let actions = &rest[open + "action IN (".len()..];
        let end = actions
            .find(')')
            .unwrap_or_else(|| panic!("unterminated chk_pae_action"));
        let mut actions = actions[..end]
            .split(',')
            .map(|value| value.trim().trim_matches('\'').to_owned())
            .collect::<Vec<_>>();
        actions.sort();
        actions
    }

    fn widened_audit_actions() -> Vec<String> {
        let mut actions = [
            "codes.reveal",
            "codes.export",
            "codes.disable",
            "codes.batch_disable",
            "user_coupons.void",
            "user_coupons.extend",
            "lifecycle.expire",
        ]
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
        actions.sort();
        actions
    }

    /// Ordered audit actions admitted by the *current* folded baseline.
    ///
    /// `widened_audit_actions()` is the 0005 vocabulary; this is the 0006 one
    /// (settlement lifecycle + the system reservation sweep). Keeping both lets
    /// the history-immutable migrations assert their own era while the baseline
    /// asserts the latest.
    fn settlement_audit_actions() -> Vec<String> {
        let mut actions = widened_audit_actions();
        actions.extend(
            [
                "discount_applications.settle",
                "discount_applications.release",
                "budget.release_expired",
            ]
            .iter()
            .map(|value| (*value).to_owned()),
        );
        actions.sort();
        actions
    }

    /// Ordered audit actions admitted by the *current* folded baseline (0008).
    ///
    /// The reissue action was added after the settlement vocabulary, so the
    /// baseline assertion uses this list while the history-immutable 0006-era
    /// checks keep asserting [`settlement_audit_actions`].
    fn reissue_audit_actions() -> Vec<String> {
        let mut actions = settlement_audit_actions();
        actions.push("codes.reissue".to_owned());
        actions.sort();
        actions
    }

    #[test]
    fn stop_loss_mutations_audit_before_the_write_in_the_same_transaction() {
        let source = include_str!("promotion_admin_management.rs");
        for (marker, end_marker, action, update_marker) in [
            (
                "pub(crate) async fn disable_code",
                "\n/// Disables every eligible code of one batch",
                "CODE_DISABLE_AUDIT_ACTION",
                "UPDATE promotion_code SET status='disabled'",
            ),
            (
                "pub(crate) async fn disable_code_batch",
                "\n/// Voids one wallet coupon",
                "CODE_BATCH_DISABLE_AUDIT_ACTION",
                "UPDATE promotion_code SET status='disabled'",
            ),
            (
                "pub(crate) async fn void_user_coupon",
                "\n/// Extends one wallet coupon's expiry",
                "USER_COUPON_VOID_AUDIT_ACTION",
                "UPDATE promotion_user_coupon SET status='voided'",
            ),
            (
                "pub(crate) async fn extend_user_coupon",
                "\n/// Persists one audit-evidence row",
                "USER_COUPON_EXTEND_AUDIT_ACTION",
                "UPDATE promotion_user_coupon SET expires_at=",
            ),
        ] {
            let body = section(source, marker, end_marker);
            // One transaction wraps the evidence write and the mutation.
            assert!(
                body.contains("begin_admin_transaction(pool,"),
                "{marker} must open a transaction"
            );
            let audit_at = body
                .find("insert_audit_event(")
                .unwrap_or_else(|| panic!("{marker} must write the audit row"));
            let update_at = body
                .find(update_marker)
                .unwrap_or_else(|| panic!("{marker} must mutate"));
            assert!(
                audit_at < update_at,
                "{marker} must write the audit row BEFORE the mutation"
            );
            // A failed audit insert aborts the operation (the `?` drops the tx).
            assert!(
                body[audit_at..update_at].contains(".await?;"),
                "{marker} must abort when the audit write fails"
            );
            assert!(
                body.contains(action),
                "{marker} must record the documented audit action"
            );
            // The mutation runs on the transaction handle, not on the pool.
            assert!(
                body.contains("&mut *tx"),
                "{marker} must mutate inside the same transaction"
            );
            // The evidence row records what the mutation actually changed.
            assert!(
                body.contains("update_audit_subject_count"),
                "{marker} must record the subject count"
            );
            let commit_at = body
                .find("tx.commit()")
                .unwrap_or_else(|| panic!("{marker} must commit"));
            assert!(
                commit_at > update_at,
                "{marker} must commit after the mutation"
            );
            // The actor/tenant/organization only come from the IAM scope.
            assert!(!body.contains("Json("), "{marker} must not read a body");
            assert!(
                !body.contains("actor_id: String"),
                "{marker} must not accept an actor id"
            );
            // The guard is scoped by tenant + organization.
            assert!(
                body.contains("tenant_id=$2 AND organization_id=$3"),
                "{marker} must scope the row by tenant and organization"
            );
        }

        // Disable guards: the pure decision plus the same predicate in SQL.
        let disable = section(
            source,
            "pub(crate) async fn disable_code",
            "\n/// Disables every eligible code of one batch",
        );
        assert!(disable.contains("code_disable_decision(&status"));
        assert!(disable.contains("status='active'"));
        assert!(disable.contains("COALESCE(claimed_quantity,0)=0"));

        // Batch guard: one UPDATE over every eligible code of the batch.
        let batch = section(
            source,
            "pub(crate) async fn disable_code_batch",
            "\n/// Voids one wallet coupon",
        );
        assert!(batch.contains("code_batch_id=$2"));
        assert!(batch.contains("status='active'"));
        assert!(batch.contains("COALESCE(claimed_quantity,0)=0"));

        // Wallet-coupon guards reuse the pure status decision.
        for (marker, end_marker) in [
            (
                "pub(crate) async fn void_user_coupon",
                "\n/// Extends one wallet coupon's expiry",
            ),
            (
                "pub(crate) async fn extend_user_coupon",
                "\n/// Persists one audit-evidence row",
            ),
        ] {
            let body = section(source, marker, end_marker);
            assert!(body.contains("user_coupon_void_decision(&status)"));
            assert!(body.contains("LOWER(status) IN ('claimed','active','pending')"));
        }

        // Extension: canonical shape, database-clock future check, old/new detail.
        let extension = section(
            source,
            "pub(crate) async fn extend_user_coupon",
            "\n/// Persists one audit-evidence row",
        );
        assert!(extension.contains("timestamps::required_instant"));
        // The database clock gate is evaluated both when the row is read and in
        // the mutation statement itself, never against the Rust clock.
        assert!(
            extension
                .matches("::timestamptz > CURRENT_TIMESTAMP")
                .count()
                >= 2
        );
        assert!(extension.contains("timestamps::extension_future_decision"));
        assert!(extension.contains("detail_json"));
        assert!(extension.contains("previousExpiresAt"));
        assert!(extension.contains("\"expiresAt\""));
    }

    #[test]
    fn stop_loss_migration_and_baseline_add_the_same_columns_and_indexes() {
        let migration = include_str!(
            "../../../database/migrations/postgres/0005_coupon_stop_loss.up.sql"
        );
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");

        // The migration is idempotent: the action check is dropped before it is
        // re-added, so re-running the vocabulary widening cannot fail.
        assert!(migration.contains(
            "ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;"
        ));
        assert_eq!(
            check_actions(migration),
            widened_audit_actions(),
            "0005 must declare the full seven-action vocabulary"
        );
        assert_eq!(
            check_actions(baseline),
            reissue_audit_actions(),
            "the folded baseline must declare the latest vocabulary"
        );

        // Same void evidence columns, in the same order, on both sides.
        let expected = ["voided_at", "void_reason", "voided_at", "void_reason"];
        let folded_0005_start = baseline
            .find("-- folded migration: migrations/postgres/0005_coupon_stop_loss.up.sql")
            .expect("baseline must fold 0005");
        // The 0005 block ends where the 0006 block begins, so a later fold cannot
        // leak its columns into this history-immutable assertion.
        let folded_0006_start = baseline
            .find("-- folded migration: migrations/postgres/0006_budget_and_settlement.up.sql")
            .expect("baseline must fold 0006");
        assert!(folded_0005_start < folded_0006_start);
        let folded_0005 = &baseline[folded_0005_start..folded_0006_start];
        assert_eq!(added_columns(migration), expected);
        assert_eq!(added_columns(folded_0005), expected);
        for sql in [migration, baseline] {
            assert!(sql.contains("promotion_user_coupon ADD COLUMN IF NOT EXISTS voided_at TEXT"));
            assert!(
                sql.contains("promotion_user_coupon ADD COLUMN IF NOT EXISTS void_reason VARCHAR(256)")
            );
            assert!(sql.contains("promotion_code ADD COLUMN IF NOT EXISTS voided_at TEXT"));
            assert!(sql.contains("promotion_code ADD COLUMN IF NOT EXISTS void_reason VARCHAR(256)"));
            // The sweep indexes the expiry scan needs. The existing indexes all
            // lead with `tenant_id`/`owner_user_id`, so none serves `status`+window.
            assert!(sql.contains(
                "CREATE INDEX IF NOT EXISTS idx_promotion_user_coupon_status_expires"
            ));
            assert!(sql.contains("ON promotion_user_coupon (tenant_id, status, expires_at)"));
            assert!(sql.contains("CREATE INDEX IF NOT EXISTS idx_promotion_code_status_expires"));
            assert!(sql.contains("ON promotion_code (tenant_id, status, expires_at)"));
        }
    }

    /// `voided_at`/`void_reason` columns added by `ADD COLUMN IF NOT EXISTS`.
    fn added_columns(sql: &str) -> Vec<String> {
        sql.match_indices("ADD COLUMN IF NOT EXISTS ")
            .map(|(index, marker)| {
                sql[index + marker.len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn distribution_writes_lowercase_enum_values_the_readers_accept() {
        let source = include_str!("promotion_admin_management.rs");
        let body = function_body(source, "async fn insert_distributed_coupon_postgres");

        assert!(body.contains("INSERT INTO promotion_user_coupon"));
        // The admin list returns `status` verbatim and the apply path only
        // accepts `claimed`-family values; `1` was never a VARCHAR value.
        assert!(body.contains("$10,'claimed',"), "{body}");
        assert!(body.contains("'user'"), "{body}");
        assert!(!body.contains("'USER'"), "{body}");
        // Ledger readers normalize/label lowercase (`debit`, `distribution`).
        assert!(body.contains("'debit'"), "{body}");
        assert!(body.contains("'distribution'"), "{body}");
        assert!(!body.contains("'OUT'"), "{body}");
        assert!(!body.contains("'ADMIN_DISTRIBUTION'"), "{body}");
        // Coupon source is the admin distribution task; the ledger source
        // mirrors the claim/redeem ledger.
        assert!(body.contains("'admin_distribution'"), "{body}");
        assert!(body.contains("'promotion_user_coupon'"), "{body}");
        // Every written row keeps its uuid.
        assert!(body.contains(".bind(coupon_uuid)"), "{body}");
        assert!(body.matches(".bind(uuid())").count() >= 2, "{body}");
    }

    #[test]
    fn redemption_attempt_list_scopes_by_tenant_and_organization_and_sorts_newest_first() {
        let source = include_str!("promotion_admin_management.rs");
        let start = source
            .find("pub(crate) async fn list_redemption_attempts")
            .expect("redemption attempt list");
        let rest = &source[start..];
        let body = &rest[..rest.find("\nenum ManagementRow").unwrap_or(rest.len())];

        // Scoping: tenant plus the scope organization, where '0' is the platform
        // sentinel every write path stores for a row that belongs to no
        // organization (`PromotionAdminScope.organization_id == 0` binds "0").
        assert!(body.contains("tenant_id=$1"));
        assert!(body.contains("organization_id=$2"));
        // Both the count and the page query carry the identical predicate, so a
        // page can never disagree with its own total.
        assert_eq!(body.matches("let filter = ").count(), 1);
        assert_eq!(body.matches("{filter}").count(), 2);
        // Filters are bound, never interpolated, and an unknown outcome can only
        // arrive from the service (which rejects it).
        assert!(body.contains("($4 IS NULL OR outcome=$4)"));
        assert!(body.contains("($5 IS NULL OR code_id=CAST($5 AS TEXT))"));
        // Newest first, with the id as a stable tiebreaker for identical
        // `occurred_at` values (second-resolution timestamps collide).
        assert!(body.contains("ORDER BY occurred_at DESC, id DESC"));
        assert!(body.contains("LIMIT $6 OFFSET $7"));
        // The ORDER BY/count shape must stay index-friendly: no function wraps
        // the scoping columns.
        assert!(!body.contains("LOWER(tenant_id)"));
        assert!(!body.contains("UPPER(organization_id)"));
        // The submitted code is never selected, only the stored suffix.
        assert!(body.contains("submitted_code_suffix"));
        assert!(!body.contains("submitted_code_hash"));
    }

    #[test]
    fn settlement_lifecycle_audits_before_the_mutation_in_the_same_transaction() {
        let source = include_str!("promotion_admin_management.rs");
        for (marker, end_marker, action, update_marker, decision) in [
            (
                "pub(crate) async fn settle_discount_application",
                "\n/// Releases one `applied` discount application",
                "DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION",
                "UPDATE promotion_discount_application SET status='settled'",
                "settle_decision(&status)",
            ),
            (
                "pub(crate) async fn release_discount_application",
                "\n/// Converts the application's stored decimal-major",
                "DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION",
                "UPDATE promotion_discount_application SET status='released'",
                "release_decision(&status)",
            ),
        ] {
            let body = section(source, marker, end_marker);
            // One transaction wraps the evidence write and the mutation.
            assert!(
                body.contains("begin_admin_transaction(pool,"),
                "{marker} must open a transaction"
            );
            let audit_at = body
                .find("insert_audit_event(")
                .unwrap_or_else(|| panic!("{marker} must write the audit row"));
            let update_at = body
                .find(update_marker)
                .unwrap_or_else(|| panic!("{marker} must mutate"));
            assert!(
                audit_at < update_at,
                "{marker} must write the audit row BEFORE the mutation"
            );
            // A failed audit insert aborts the operation (the `?` drops the tx).
            assert!(
                body[audit_at..update_at].contains(".await?;"),
                "{marker} must abort when the audit write fails"
            );
            assert!(
                body.contains(action),
                "{marker} must record the documented audit action"
            );
            assert!(
                body.contains("DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE"),
                "{marker} must use the discount-application aggregate"
            );
            // The mutation runs on the transaction handle, not on the pool.
            assert!(
                body.contains("&mut *tx"),
                "{marker} must mutate inside the same transaction"
            );
            assert!(
                body.contains("update_audit_subject_count"),
                "{marker} must record the subject count"
            );
            let commit_at = body
                .find("tx.commit()")
                .unwrap_or_else(|| panic!("{marker} must commit"));
            assert!(
                commit_at > update_at,
                "{marker} must commit after the mutation"
            );
            // The actor/tenant/organization only come from the IAM scope.
            assert!(!body.contains("Json("), "{marker} must not read a body");
            assert!(
                !body.contains("actor_id: String"),
                "{marker} must not accept an actor id"
            );
            // The row is locked, scoped, and its state is the pure decision's.
            assert!(body.contains("FOR UPDATE"), "{marker} must lock the row");
            assert!(
                body.contains("tenant_id=$2 AND organization_id=$3"),
                "{marker} must scope the row by tenant and organization"
            );
            assert!(body.contains(decision), "{marker} must reuse the pure decision");
        }

        // Settle moves reserved→used, release reserved→available; both through
        // the same guarded transfer helper and ledger writer.
        for marker in [
            "pub(crate) async fn settle_discount_application",
            "pub(crate) async fn release_discount_application",
        ] {
            let body = section(source, marker, "\n/// Converts the application's stored decimal-major");
            assert!(body.contains("transfer_reserved_budget("));
            assert!(body.contains("insert_budget_ledger_entry("));
        }
        let settle = section(
            source,
            "pub(crate) async fn settle_discount_application",
            "\n/// Releases one `applied` discount application",
        );
        assert!(settle.contains("BudgetTransfer::Settle"));
        let release = section(
            source,
            "pub(crate) async fn release_discount_application",
            "\n/// Converts the application's stored decimal-major",
        );
        assert!(release.contains("BudgetTransfer::Release"));
        // The release reason lands on the row itself.
        assert!(release.contains("release_reason=$2"));
        assert!(release.contains("released_at=$1"));
        assert!(release.contains("serde_json::json!({ \"reason\": reason })"));
        // Settled applications keep the settled timestamp.
        assert!(settle.contains("settled_at=$1"));

        // The transfer helper's guard and direction vocabulary.
        let direction_impl = section(
            source,
            "impl BudgetTransfer",
            "/// Account balances immediately after one budget transfer.",
        );
        assert!(direction_impl.contains("Self::Settle => \"settle\""));
        assert!(direction_impl.contains("Self::Release => \"release\""));
        let transfer = section(
            source,
            "async fn transfer_reserved_budget",
            "async fn insert_budget_ledger_entry",
        );
        assert!(transfer.contains("reserved_amount_minor >= $1"));
        let ledger = section(source, "async fn insert_budget_ledger_entry", "fn now_string");
        assert!(ledger.contains("INSERT INTO promotion_budget_ledger_entry"));
        assert!(ledger.contains("transfer.direction()"));
        assert!(ledger.contains("balances.available_minor"));
        assert!(ledger.contains("balances.reserved_minor"));
        assert!(ledger.contains("balances.used_minor"));
    }

    #[test]
    fn budget_migration_and_baseline_declare_the_same_tables_columns_and_checks() {
        let migration = include_str!(
            "../../../database/migrations/postgres/0006_budget_and_settlement.up.sql"
        );
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");

        // Both tables are declared in the migration AND folded into the baseline,
        // with the same column set.
        for table in ["promotion_budget_account", "promotion_budget_ledger_entry"] {
            let from_migration = budget_table_columns(migration, table);
            let from_baseline = budget_table_columns(baseline, table);
            assert!(
                !from_migration.is_empty(),
                "{table} must be declared by 0006"
            );
            assert_eq!(
                from_migration, from_baseline,
                "{table}: the folded baseline must declare the same columns as 0006"
            );
        }
        assert_eq!(
            budget_table_columns(migration, "promotion_budget_account"),
            vec![
                "available_amount_minor",
                "budget_no",
                "created_at",
                "currency_code",
                "id",
                "offer_id",
                "offer_version_id",
                "organization_id",
                "reserved_amount_minor",
                "status",
                "stock_id",
                "tenant_id",
                "total_amount_minor",
                "updated_at",
                "used_amount_minor",
                "uuid",
                "version",
            ],
            "promotion_budget_account must keep its hard-cap column set"
        );
        assert_eq!(
            budget_table_columns(migration, "promotion_budget_ledger_entry"),
            vec![
                "amount_minor",
                "application_id",
                "available_after_minor",
                "budget_account_id",
                "currency_code",
                "direction",
                "id",
                "idempotency_key",
                "occurred_at",
                "organization_id",
                "request_no",
                "reserved_after_minor",
                "tenant_id",
                "used_after_minor",
                "uuid",
            ],
            "promotion_budget_ledger_entry must keep its evidence column set"
        );

        // Same additive column set on both sides, in the same order.
        let expected_columns = [
            "budget_account_id",
            "reservation_expires_at",
            "budget_amount_minor",
        ];
        assert_eq!(added_columns(migration), expected_columns);
        let folded_start = baseline
            .find("-- folded migration: migrations/postgres/0006_budget_and_settlement.up.sql")
            .expect("baseline must fold 0006");
        let folded_rest = &baseline[folded_start..];
        let folded_end = folded_rest
            .find("-- folded migration: migrations/postgres/0007_code_hash.up.sql")
            .unwrap_or(folded_rest.len());
        assert_eq!(added_columns(&folded_rest[..folded_end]), expected_columns);

        for sql in [migration, baseline] {
            // The money-unit rule is documented next to the new columns.
            assert!(sql.contains("*_minor BIGINT"));
            // Additive columns.
            assert!(sql.contains(
                "ALTER TABLE promotion_discount_application ADD COLUMN IF NOT EXISTS budget_account_id TEXT;"
            ));
            assert!(sql.contains(
                "ALTER TABLE promotion_discount_application ADD COLUMN IF NOT EXISTS reservation_expires_at TEXT;"
            ));
            assert!(sql.contains(
                "ALTER TABLE promotion_coupon_stock ADD COLUMN IF NOT EXISTS budget_amount_minor BIGINT;"
            ));
            // Partial index for the stale-reservation sweep.
            assert!(sql.contains(
                "CREATE INDEX IF NOT EXISTS idx_promotion_discount_application_status_expires"
            ));
            assert!(sql.contains("WHERE status = 'applied';"));
            // The hard-cap invariant and the closed direction vocabulary.
            assert!(sql.contains("CONSTRAINT chk_pba_amounts CHECK ("));
            assert!(sql.contains(
                "available_amount_minor + reserved_amount_minor + used_amount_minor <= total_amount_minor"
            ));
            assert!(sql.contains("total_amount_minor > 0 AND available_amount_minor >= 0"));
            assert!(sql.contains(
                "CONSTRAINT chk_pble_direction CHECK (direction IN ('reserve','settle','release'))"
            ));
            // One account per stock, one ledger row per idempotency key.
            assert!(sql.contains("CONSTRAINT uk_pba_stock UNIQUE (tenant_id, stock_id)"));
            assert!(sql.contains("CONSTRAINT uk_pble_idem UNIQUE (tenant_id, idempotency_key)"));
            assert!(sql.contains(
                "CREATE INDEX IF NOT EXISTS idx_pble_account ON promotion_budget_ledger_entry (tenant_id, budget_account_id, occurred_at DESC);"
            ));
        }

        // The widened audit vocabulary: 0006 drops and re-adds the check; the
        // history-immutable migration keeps its own era's ten actions while the
        // folded baseline has since folded 0008's `codes.reissue` on top.
        assert!(migration.contains(
            "ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;"
        ));
        assert_eq!(
            check_actions(migration),
            settlement_audit_actions(),
            "0006 must declare the settlement lifecycle vocabulary"
        );
        assert_eq!(
            check_actions(baseline),
            reissue_audit_actions(),
            "the folded baseline must declare the same vocabulary plus 0008"
        );
        for action in [
            "discount_applications.settle",
            "discount_applications.release",
            "budget.release_expired",
        ] {
            assert!(
                settlement_audit_actions().contains(&action.to_owned()),
                "{action} must be part of the widened vocabulary"
            );
        }
    }

    /// Column names of one budget `CREATE TABLE IF NOT EXISTS <table> (...)`.
    ///
    /// Unlike [`create_table_columns`] this tolerates a multi-line `CHECK` body:
    /// continuation lines start with a SQL keyword (`AND`, `OR`) or a comparison,
    /// so only `<lower_snake> <UPPERCASE TYPE>` lines are columns.
    fn budget_table_columns(sql: &str, table: &str) -> Vec<String> {
        let marker = format!("CREATE TABLE IF NOT EXISTS {table} (");
        let start = sql
            .find(&marker)
            .unwrap_or_else(|| panic!("missing table {table}"));
        let rest = &sql[start + marker.len()..];
        let mut columns = Vec::new();
        for line in rest.lines() {
            let line = line.trim();
            if line.starts_with(')') {
                break;
            }
            if line.is_empty() || line.starts_with("--") || line.starts_with("CONSTRAINT") {
                continue;
            }
            let mut tokens = line.split_whitespace();
            let (Some(name), Some(kind)) = (tokens.next(), tokens.next()) else {
                continue;
            };
            let lower_snake = name
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '_' || character.is_ascii_digit());
            let typed = kind.chars().any(|character| character.is_ascii_uppercase());
            if lower_snake && typed {
                columns.push(name.trim_end_matches(',').to_owned());
            }
        }
        columns.sort();
        columns
    }

    #[test]
    fn replay_helpers_decode_text_ids_as_strings() {
        let source = include_str!("promotion_admin_management.rs");
        for (marker, expected_call) in [
            (
                "async fn retrieve_code_batch_by_key",
                "retrieve_code_batch(pool, scope, id).await",
            ),
            (
                "async fn retrieve_distribution_by_key",
                "retrieve_distribution_task(pool, scope, id).await",
            ),
        ] {
            let body = function_body(source, marker);
            assert!(
                body.contains("query_scalar::<_,String>"),
                "{marker} must decode the TEXT primary key as String"
            );
            assert!(
                !body.contains("query_scalar::<_,i64>"),
                "{marker} must not decode a TEXT id as i64"
            );
            assert!(
                body.contains(expected_call),
                "{marker} must pass the decoded String id through unchanged"
            );
        }
    }

    /// Every insert into the promotion-code table must supply the indexed
    /// identity columns.
    ///
    /// A row written without `code_hash`/`code_last4` would be invisible to the
    /// hash-first redemption lookup, so this scans the authored `.rs`/`.sql`
    /// sources (build output, dependencies, and VCS metadata are skipped) and
    /// asserts that each promotion-code insert names both columns. The scan also
    /// counts the insert paths: adding another one must be reviewed and reflected
    /// here deliberately.
    #[test]
    fn every_promotion_code_insert_supplies_the_hash_identity_columns() {
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // Assembled at runtime so the test cannot match its own source text.
        let insert_marker = ["INSERT INTO ", "promotion_code"].concat();
        let mut source_files = Vec::new();
        collect_source_files(&repository_root, &mut source_files);
        assert!(
            source_files.len() > 20,
            "the repository source scan found too few files: {}",
            source_files.len()
        );

        let mut inserts = 0usize;
        for path in &source_files {
            let source = std::fs::read_to_string(path).unwrap_or_default();
            for (offset, _) in source.match_indices(insert_marker.as_str()) {
                let tail = &source[offset + insert_marker.len()..];
                // `promotion_code_redemption_attempt` and `promotion_code_batch`
                // share the table-name prefix but are different tables.
                let next = tail.chars().next();
                if next
                    .is_some_and(|character| character == '_' || character.is_ascii_alphanumeric())
                {
                    continue;
                }
                inserts += 1;
                // The statement runs to the closing quote of the Rust string
                // literal (the insert SQL contains no double quotes). Raw string
                // literals end at `"#;`, an inline literal at `")`.
                let statement_end = tail.find('"').unwrap_or(tail.len());
                let statement = &tail[..statement_end];
                assert!(
                    statement.contains("code_hash"),
                    "{}: a promotion-code insert is missing code_hash:\n{statement}",
                    path.display()
                );
                assert!(
                    statement.contains("code_last4"),
                    "{}: a promotion-code insert is missing code_last4:\n{statement}",
                    path.display()
                );
            }
        }
        assert_eq!(
            inserts, 2,
            "two promotion_code insert paths exist (batch generation and reissue); a new one must supply both identity columns and update this expectation"
        );
    }

    /// Body of [`reissue_code`], from its marker to the settlement section.
    fn reissue_body(source: &str) -> &str {
        section(
            source,
            "pub(crate) async fn reissue_code",
            "\n/// Which bucket a reservation moves into.",
        )
    }

    #[test]
    fn code_reissue_audits_disables_and_reinserts_with_lineage_in_one_transaction() {
        let source = include_str!("promotion_admin_management.rs");
        let body = reissue_body(source);

        // One transaction, opened through the shared admin helper; every
        // statement runs on its handle and not on the pool.
        assert!(
            body.contains("begin_admin_transaction(pool, \"begin code reissue\")"),
            "reissue must open one admin transaction"
        );
        assert!(
            !body.contains(".execute(pool)"),
            "reissue must not mutate the pool"
        );
        assert!(
            body.matches("&mut *tx").count() >= 2,
            "disabled + inserted rows must both be written inside the transaction"
        );

        // Ordering: evidence first, then the disable, then the insert, then the
        // commit. No half-replacement can commit.
        let audit_at = body
            .find("insert_audit_event(")
            .expect("reissue must write the audit row");
        let disable_at = body
            .find("UPDATE promotion_code SET status='disabled'")
            .expect("reissue must disable the original");
        let insert_at = body
            .find("INSERT_REISSUED_PROMOTION_CODE_SQL")
            .expect("reissue must insert the replacement");
        let commit_at = body.find("tx.commit()").expect("reissue must commit");
        assert!(
            audit_at < disable_at,
            "audit must be written before the disable"
        );
        assert!(
            disable_at < insert_at,
            "the disable must precede the insert"
        );
        assert!(
            insert_at < commit_at,
            "the commit must follow both mutations"
        );

        // The single evidence row records the documented action, both ids, and
        // the operator's reason, and counts both affected rows.
        assert!(body.contains("CODE_REISSUE_AUDIT_ACTION"));
        assert!(body.contains("CODE_AUDIT_AGGREGATE_TYPE"));
        assert!(body.contains("\"replacedCodeId\""));
        assert!(body.contains("\"newCodeId\""));
        assert!(body.contains("\"reason\""));
        assert!(
            body.contains("update_audit_subject_count(&mut tx, &audit_id, scope, 2)"),
            "one audit row covers both sides of the lineage"
        );

        // The original is disabled in place and points at its replacement.
        assert!(body.contains("voided_at=TO_CHAR(CURRENT_TIMESTAMP"));
        assert!(body.contains("void_reason=$1"));
        assert!(body.contains("replaced_by_code_id=$5"));

        // Guard: the pure decision plus the same predicate in SQL, scoped and
        // row-locked so a concurrent claim cannot split the decision.
        assert!(body.contains("code_reissue_decision(&status"));
        assert!(body.contains("status='active'"));
        assert!(body.contains("COALESCE(claimed_quantity,0)=0"));
        assert!(body.contains("FOR UPDATE OF c"));
        assert!(body.contains("c.tenant_id=$2 AND c.organization_id=$3"));
        // A zero-row disable aborts instead of committing a half replacement.
        assert!(body.contains("if affected != 1"));
        assert!(body.contains("CommerceServiceError::conflict(\"code is not active\")"));

        // An explicit-code collision reuses the batch insert's duplicate mapping.
        assert!(body.contains(".map_err(code_insert_error)"));
        let duplicate_mapping = function_body(source, "fn code_insert_error");
        assert!(duplicate_mapping.contains("\"23505\""));
        assert!(duplicate_mapping.contains("CommerceServiceError::conflict"));
    }

    #[test]
    fn reissued_code_inherits_the_original_terms_from_the_insert() {
        let source = include_str!("promotion_admin_management.rs");
        let body = reissue_body(source);
        let insert_start = source
            .find("const INSERT_REISSUED_PROMOTION_CODE_SQL")
            .expect("the reissue insert must be a named constant");
        let insert_rest = &source[insert_start..];
        let insert = &insert_rest[..insert_rest.find("\"#;").expect("unterminated insert")];

        // The column list names every inherited term plus the lineage column.
        for column in [
            "code_batch_id",
            "stock_id",
            "offer_id",
            "offer_version_id",
            "code_type",
            "max_claims",
            "claimed_quantity",
            "starts_at",
            "expires_at",
            "status",
            "code_hash",
            "code_last4",
            "reissued_from_code_id",
        ] {
            assert!(
                insert.contains(column),
                "the reissue insert must carry `{column}`"
            );
        }
        // The replacement starts active, unclaimed, and at version 0.
        assert!(insert.contains("'active', 0"));

        // Every inherited value is bound from the original row, never from the
        // operator's request; the last bind is the lineage id.
        for bind in [
            ".bind(code_batch_id.as_deref())",
            ".bind(stock_id.as_str())",
            ".bind(offer_id.as_str())",
            ".bind(offer_version_id.as_str())",
            ".bind(code_type.as_str())",
            ".bind(max_claims)",
            ".bind(starts_at.as_deref())",
            ".bind(expires_at.as_deref())",
            ".bind(identity.hash.as_str())",
            ".bind(identity.last4.as_str())",
            ".bind(code_id.as_str())",
        ] {
            assert!(body.contains(bind), "reissue must bind {bind}");
        }

        // Generation prefers the batch's own prefix/length and falls back to a
        // plain 16-character code when the batch defines neither.
        assert!(body.contains("batch_code_prefix"));
        assert!(body.contains("batch_code_length"));
        assert!(body.contains("generated_code(prefix, length)?"));
        assert!(body.contains("generated_code(\"\", 16)?"));
    }

    #[test]
    fn reissued_code_outcome_is_masked_only() {
        // The mask keeps its "never the code" invariant for every shape a
        // reissue can produce: a masked value always contains `*`, which no
        // valid code may contain.
        for code in ["3C76A1B2C3D44E21", "WELCOME2026", "A1-_", "AB12CD34"] {
            let masked = mask_code(code);
            assert_ne!(masked, code, "mask of {code:?} must differ from the code");
            assert!(masked.contains("****"), "mask of {code:?} must hide characters");
        }
        assert_eq!(mask_code("3C76A1B2C3D44E21"), "3C76****4E21");
        assert_eq!(mask_code("WELCOME2026"), "WELC****2026");
        assert_eq!(mask_code("AB12CD34"), "****");

        // The repository masks the value and returns a type with no plaintext
        // field: the full code never leaves this operation.
        let source = include_str!("promotion_admin_management.rs");
        let body = reissue_body(source);
        assert!(body.contains("let code_masked = mask_code(&value);"));
        assert!(!body.contains("promotion_code: value"));
        assert!(!body.contains("new_code: value"));

        let service = include_str!("../../sdkwork-commerce-promotion-service/src/backend_admin.rs");
        for (marker, expected_fields) in [
            (
                "pub struct CodeReissueOutcome {",
                vec!["new_code_id", "new_code_no", "code_masked", "code_last4"],
            ),
            (
                "pub struct PromotionCodeReissueItem {",
                vec![
                    "code_id",
                    "code_no",
                    "code_masked",
                    "code_last4",
                    "replaced_code_id",
                ],
            ),
        ] {
            let start = service
                .find(marker)
                .unwrap_or_else(|| panic!("missing {marker}"));
            let rest = &service[start..];
            let declaration = &rest[..rest.find('}').expect("unterminated struct")];
            for field in expected_fields {
                assert!(
                    declaration.contains(&format!("pub {field}:")),
                    "{marker} must carry `{field}`"
                );
            }
            // No plaintext field, whatever it is called. Doc comments may name
            // the `promotion_code` columns, so only field lines are inspected.
            let fields_only = declaration
                .lines()
                .filter(|line| line.trim_start().starts_with("pub "))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!fields_only.contains("promotion_code"));
            assert!(!fields_only.contains("pub code:"));
            assert!(!fields_only.contains("pub value:"));
        }
    }

    #[test]
    fn code_reissue_migration_and_baseline_add_the_same_columns_index_and_check() {
        let migration =
            include_str!("../../../database/migrations/postgres/0008_code_reissue.up.sql");
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");

        for sql in [migration, baseline] {
            assert!(sql.contains(
                "ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS reissued_from_code_id TEXT"
            ));
            assert!(sql.contains(
                "ALTER TABLE promotion_code ADD COLUMN IF NOT EXISTS replaced_by_code_id TEXT"
            ));
            assert!(sql.contains("CREATE INDEX IF NOT EXISTS idx_promotion_code_reissued_from"));
            assert!(sql.contains("ON promotion_code (tenant_id, reissued_from_code_id)"));
            assert!(
                check_actions(sql).contains(&"codes.reissue".to_owned()),
                "the audit vocabulary must admit codes.reissue"
            );
        }
        assert_eq!(
            check_actions(migration),
            reissue_audit_actions(),
            "0008 must declare the latest audit vocabulary"
        );
        assert_eq!(
            check_actions(baseline),
            reissue_audit_actions(),
            "the folded baseline must fold 0008"
        );

        // Re-runnable and transactional; no table is added, so the contract
        // registries (schema.yaml / table-registry.json) need no change.
        assert!(migration
            .contains("ALTER TABLE promotion_audit_event DROP CONSTRAINT IF EXISTS chk_pae_action;"));
        assert!(migration.contains("BEGIN;"));
        assert!(migration.contains("COMMIT;"));
        assert!(!migration.contains("CREATE TABLE"));
    }

    /// Authored Rust/SQL sources under `directory`, recursively.
    fn collect_source_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if matches!(
                    name.as_ref(),
                    ".git" | "node_modules" | "target" | "dist" | ".sdkwork"
                ) {
                    continue;
                }
                collect_source_files(&path, files);
            } else if matches!(
                path.extension().and_then(std::ffi::OsStr::to_str),
                Some("rs" | "sql")
            ) {
                files.push(path);
            }
        }
    }
}
