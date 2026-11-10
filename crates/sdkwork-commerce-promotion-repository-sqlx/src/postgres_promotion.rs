use std::time::{SystemTime, UNIX_EPOCH};

use sdkwork_commerce_promotion_service::{
    ApplyPromotionDiscountCommand, ClaimPromotionUserCouponCommand, ConsumeMemberCardCommand,
    GrantMemberCardCommand, MemberCardConsumptionOutcome, MemberCardListQuery, PointsBalance,
    PointsBalanceQuery, PointsHistoryItem, PointsHistoryQuery, PromotionCodeRedemptionCommand,
    PromotionCodeRedemptionOutcome, PromotionCodeRedemptionPreview, PromotionCouponBenefit,
    PromotionMemberCard,
    PromotionOrderCouponBenefit, PromotionSubscriptionPeriod,
    PromotionUserCouponItem, PromotionUserCouponListQuery, RetrieveMemberCardQuery,
    ReversePromotionDiscountCommand, DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE,
    DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
};
use sdkwork_contract_service::{
    CommerceAccountAssetType, CommerceLedgerDirection, CommerceServiceError,
};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::code_identity::{code_identity, CodeIdentity};
use crate::coupon_benefit::{parse_admin_coupon_benefit, parse_order_coupon_benefit};
use crate::discount_amount::compute_discount_amount;
use crate::redemption_attempt::{
    insert_redemption_attempt_in_tx, record_redemption_failure_attempt, submitted_code_hash,
    submitted_code_suffix, RedemptionAttemptRecord, ResolvedRedemptionTarget, CHANNEL_CODE,
    CHANNEL_PREVIEW, OUTCOME_SUCCEEDED,
};
use crate::timestamps::{self, window_not_ended, window_started};

const POINTS_CURRENCY_CODE: &str = "POINT";
const PROMOTION_CODE_REDEMPTION_SCOPE: &str = "promotions.codes.redemptions.create";
const PROMOTION_USER_COUPON_CLAIM_SCOPE: &str = "promotions.userCoupons.claims.create";
const PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE: &str = "promotions.discountApplications.create";
const PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE: &str =
    "promotions.discountApplications.reversals.create";
const USER_SUBJECT_TYPE: &str = "user";
const PROMOTION_USER_COUPON_SOURCE_TYPE: &str = "promotion_user_coupon";
/// Platform sentinel for a subject that carries no organization.
///
/// Every `organization_id` column is `TEXT NOT NULL DEFAULT '0'`
/// (`database/migrations/postgres/0001_organization_id_not_null.up.sql`), so a
/// write must never bind SQL `NULL` into it. The read predicates keep the same
/// sentinel rule (`organization_id = '0' AND $2 IS NULL`), which makes `'0'`
/// the one value that represents "platform scope" on both sides.
const PLATFORM_ORGANIZATION_ID: &str = "0";

/// Binds an optional organization to a NOT NULL `organization_id` column.
pub(crate) fn organization_binding(value: Option<&str>) -> &str {
    value.unwrap_or(PLATFORM_ORGANIZATION_ID)
}

#[derive(Debug, Clone)]
pub struct PostgresCommercePromotionStore {
    pool: PgPool,
}

#[derive(Debug, Clone)]
struct ClaimPromotion {
    stock_id: String,
    offer_id: String,
    offer_version_id: String,
    stock_type: String,
    discount_value: String,
    rule_json: Option<String>,
    total_quantity: Option<i64>,
    available_quantity: i64,
    stock_claimed_quantity: i64,
    per_user_limit: i64,
    expires_at: Option<String>,
}

#[derive(Debug, Clone)]
struct RedeemPromotion {
    code_id: String,
    stock_id: String,
    offer_id: String,
    offer_version_id: String,
    stock_type: String,
    discount_value: String,
    currency_code: String,
    rule_json: Option<String>,
    total_quantity: Option<i64>,
    available_quantity: i64,
    stock_claimed_quantity: i64,
    /// Stock-scoped per-subject redemption cap, mirroring the claim path.
    per_user_limit: i64,
    code_max_claims: i64,
    code_claimed_quantity: i64,
    expires_at: Option<String>,
}

#[derive(Debug, Clone)]
struct PointsAccount {
    id: String,
    available_points: i64,
}

impl PostgresCommercePromotionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Dry-run of a code redemption: applies every window / limit guard and
    /// reports the benefit that would be granted without mutating anything.
    ///
    /// This must stay side-effect free and share the exact guards used by
    /// [`Self::redeem_promotion_code`], so a preview never promises a benefit
    /// the redemption would reject.
    ///
    /// A rejected preview records a failure attempt through the same evidence
    /// ledger the redemption uses (see [`crate::redemption_attempt`]): support
    /// needs to see what a customer typed before it ever became a redemption.
    /// That costs no extra transaction — the preview already owns and releases
    /// its own read-only transaction, and the evidence row is a single
    /// auto-commit insert after it is gone.
    pub async fn preview_promotion_code(
        &self,
        command: PromotionCodeRedemptionCommand,
    ) -> Result<PromotionCodeRedemptionPreview, CommerceServiceError> {
        let mut resolved: Option<ResolvedRedemptionTarget> = None;
        match self
            .preview_promotion_code_in_tx(&command, &mut resolved)
            .await
        {
            Ok(preview) => Ok(preview),
            Err(error) => {
                // The preview transaction is gone by now (it is rolled back on
                // success and dropped on error), so this row is not part of it.
                record_redemption_failure_attempt(
                    &self.pool,
                    &command,
                    resolved.as_ref(),
                    CHANNEL_PREVIEW,
                    USER_SUBJECT_TYPE,
                    &current_timestamp_string(),
                    &error,
                )
                .await;
                Err(error)
            }
        }
    }

    async fn preview_promotion_code_in_tx(
        &self,
        command: &PromotionCodeRedemptionCommand,
        resolved: &mut Option<ResolvedRedemptionTarget>,
    ) -> Result<PromotionCodeRedemptionPreview, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin promotion code redemption preview", error)
        })?;
        let now = current_timestamp_string();
        let promotion = load_promotion_for_redeem(&mut tx, command, &now).await?;
        *resolved = Some(ResolvedRedemptionTarget {
            code_id: promotion.code_id.clone(),
            stock_id: promotion.stock_id.clone(),
            offer_id: promotion.offer_id.clone(),
        });
        ensure_promotion_can_be_redeemed(&mut tx, command, &promotion).await?;
        let benefit = parse_admin_coupon_benefit(promotion.rule_json.as_deref())?;
        let preview = map_redemption_preview(benefit, promotion.expires_at.as_deref())?;
        // Read-only preview: release the locks taken by `load_promotion_for_redeem`.
        tx.rollback().await.map_err(|error| {
            store_error(
                "failed to roll back promotion code redemption preview",
                error,
            )
        })?;
        Ok(preview)
    }

    pub async fn preview_promotion_code_for_order(
        &self,
        command: PromotionCodeRedemptionCommand,
    ) -> Result<PromotionOrderCouponBenefit, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin promotion order coupon preview", error)
        })?;
        let now = current_timestamp_string();
        let promotion = load_promotion_for_redeem(&mut tx, &command, &now).await?;
        ensure_promotion_can_be_redeemed(&mut tx, &command, &promotion).await?;
        parse_order_coupon_benefit(
            promotion.rule_json.as_deref(),
            &promotion.discount_value,
            &promotion.currency_code,
            false,
        )
    }

    pub async fn redeem_promotion_code_for_order(
        &self,
        command: PromotionCodeRedemptionCommand,
    ) -> Result<PromotionOrderCouponBenefit, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin promotion order coupon redemption", error)
        })?;
        let coupon_id = coupon_id(&command);
        let replay = sqlx::query(
            r#"
            SELECT pc.promotion_code,
                   CAST(v.discount_value AS TEXT) AS discount_value,
                   COALESCE(NULLIF(BTRIM(v.currency_code), ''), 'CNY') AS currency_code,
                   v.rule_json AS rule_json
            FROM promotion_user_coupon c
            JOIN promotion_code pc ON pc.tenant_id = c.tenant_id AND pc.id = c.code_id
            JOIN promotion_offer_version v
              ON v.tenant_id = c.tenant_id AND v.id = c.offer_version_id
            WHERE c.tenant_id = $1
              AND ((c.organization_id = $2) OR (c.organization_id = '0' AND $2 IS NULL))
              AND c.id = $3
              AND c.subject_type = $4
              AND c.subject_id = $5
            LIMIT 1
            "#,
        )
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(&coupon_id)
        .bind(USER_SUBJECT_TYPE)
        .bind(&command.owner_user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| store_error("failed to replay promotion order coupon", error))?;
        if let Some(row) = replay {
            if string_cell(&row, "promotion_code") != command.code {
                return Err(CommerceServiceError::conflict(
                    "coupon redemption request was replayed with a different code",
                ));
            }
            let benefit = parse_order_coupon_benefit(
                optional_string_cell(&row, "rule_json").as_deref(),
                &string_cell(&row, "discount_value"),
                &string_cell(&row, "currency_code"),
                true,
            )?;
            tx.commit().await.map_err(|error| {
                store_error("failed to commit promotion order coupon replay", error)
            })?;
            return Ok(benefit);
        }

        let now = current_timestamp_string();
        let promotion = load_promotion_for_redeem(&mut tx, &command, &now).await?;
        ensure_promotion_can_be_redeemed(&mut tx, &command, &promotion).await?;
        let benefit = parse_order_coupon_benefit(
            promotion.rule_json.as_deref(),
            &promotion.discount_value,
            &promotion.currency_code,
            false,
        )?;
        let coupon_ledger_entry_id = coupon_ledger_entry_id(&command);
        insert_user_coupon(&mut tx, &command, &promotion, &coupon_id, &now).await?;
        insert_coupon_ledger_entry(
            &mut tx,
            &command,
            &promotion,
            &coupon_id,
            &coupon_ledger_entry_id,
            &now,
        )
        .await?;
        update_promotion_counters(&mut tx, &promotion, &now).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit promotion order coupon redemption", error)
        })?;
        Ok(benefit)
    }

    pub async fn list_promotion_user_coupons(
        &self,
        query: PromotionUserCouponListQuery,
    ) -> Result<Vec<PromotionUserCouponItem>, CommerceServiceError> {
        let rows = sqlx::query(
            r#"
            SELECT c.id,
                   COALESCE(NULLIF(c.coupon_code, ''), '-') AS code,
                   CAST(COALESCE(a.discount_amount, v.discount_value, '0') AS TEXT) AS amount,
                   CAST(COALESCE(a.applied_at, c.redeemed_at, c.claimed_at, c.created_at) AS TEXT) AS date,
                   c.status AS status
            FROM promotion_user_coupon c
            JOIN promotion_offer_version v
              ON v.tenant_id = c.tenant_id
             AND v.id = c.offer_version_id
            LEFT JOIN promotion_discount_application a
              ON a.tenant_id = c.tenant_id
             AND a.user_coupon_id = c.id
             AND a.subject_type = c.subject_type
             AND a.subject_id = c.subject_id
            WHERE c.tenant_id = CAST($1 AS TEXT)
              AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id = '0' AND $2 IS NULL))
              AND c.subject_type = $3
              AND c.subject_id = CAST($4 AS TEXT)
              AND ($5 IS NULL OR c.status = $5)
            ORDER BY COALESCE(a.applied_at, c.redeemed_at, c.claimed_at, c.created_at) DESC NULLS LAST, c.id DESC
            "#,
        )
        .bind(&query.tenant_id)
        .bind(query.organization_id.as_deref())
        .bind(USER_SUBJECT_TYPE)
        .bind(&query.owner_user_id)
        .bind(query.status.as_deref())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| store_error("failed to list current user coupons", error))?;

        rows.iter()
            .map(|row| {
                let status = coupon_status_label(&required_status_cell(row, "status", "redeem")?)?
                    .to_owned();
                let amount = stored_money_minor_units(&string_cell(row, "amount"))?;
                PromotionUserCouponItem::new(
                    &string_cell(row, "id"),
                    &string_cell(row, "code"),
                    &amount,
                    &string_cell(row, "date"),
                    &status,
                )
            })
            .collect()
    }

    pub async fn retrieve_points_balance(
        &self,
        query: PointsBalanceQuery,
    ) -> Result<PointsBalance, CommerceServiceError> {
        let row = sqlx::query(
            r#"
            SELECT CAST(COALESCE(SUM(CASE WHEN status = 'active' THEN CAST(available_amount AS BIGINT) ELSE 0 END), 0) AS BIGINT) AS available_points,
                   CAST(COALESCE(SUM(CASE WHEN status = 'active' THEN CAST(frozen_amount AS BIGINT) ELSE 0 END), 0) AS BIGINT) AS frozen_points
            FROM commerce_account
            WHERE tenant_id = CAST($1 AS TEXT)
              AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $2 IS NULL))
              AND owner_user_id = CAST($3 AS TEXT)
              AND asset_type = $4
              AND currency_code = $5
            "#,
        )
        .bind(&query.tenant_id)
        .bind(query.organization_id.as_deref())
        .bind(&query.owner_user_id)
        .bind(CommerceAccountAssetType::Points.as_str())
        .bind(POINTS_CURRENCY_CODE)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| store_error("failed to retrieve points balance", error))?;

        PointsBalance::new(
            integer_cell(&row, "available_points"),
            integer_cell(&row, "frozen_points"),
        )
    }

    pub async fn list_points_history(
        &self,
        query: PointsHistoryQuery,
    ) -> Result<Vec<PointsHistoryItem>, CommerceServiceError> {
        let rows = sqlx::query(
            r#"
            SELECT id,
                   CAST(amount AS BIGINT) AS amount,
                   direction,
                   CAST(balance_after AS BIGINT) AS balance_after,
                   business_type,
                   CAST(created_at AS TEXT) AS created_at
            FROM commerce_account_ledger_entry
            WHERE tenant_id = CAST($1 AS TEXT)
              AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $2 IS NULL))
              AND owner_user_id = CAST($3 AS TEXT)
              AND asset_type = $4
            ORDER BY created_at DESC NULLS LAST, id DESC
            "#,
        )
        .bind(&query.tenant_id)
        .bind(query.organization_id.as_deref())
        .bind(&query.owner_user_id)
        .bind(CommerceAccountAssetType::Points.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| store_error("failed to list points history", error))?;

        rows.iter()
            .map(|row| {
                let amount = integer_cell(row, "amount").max(0);
                PointsHistoryItem::new(
                    &string_cell(row, "id"),
                    amount,
                    points_direction(&string_cell(row, "direction")),
                    integer_cell(row, "balance_after").max(0),
                    points_business_type(&string_cell(row, "business_type")),
                    &string_cell(row, "created_at"),
                )
            })
            .collect()
    }

    /// Redeems one promotion code for the calling subject.
    ///
    /// The redemption runs in [`Self::redeem_promotion_code_in_tx`]; this wrapper
    /// exists only to persist a **failed** attempt after that transaction has
    /// rolled back. See [`crate::redemption_attempt`] for why the success row and
    /// the failure row cannot share a transaction.
    pub async fn redeem_promotion_code(
        &self,
        command: PromotionCodeRedemptionCommand,
    ) -> Result<PromotionCodeRedemptionOutcome, CommerceServiceError> {
        // Captured while the transaction is alive so a failure can name the code
        // that was rejected even though its rows are gone after the rollback.
        let mut resolved: Option<ResolvedRedemptionTarget> = None;
        let result = self
            .redeem_promotion_code_in_tx(&command, &mut resolved)
            .await;
        match result {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                // `redeem_promotion_code_in_tx` owned the transaction and released
                // it (rollback) when it returned `Err`, so this runs on a separate
                // auto-commit connection *after* the rollback and the row
                // survives. A failed evidence write must not replace the business
                // rejection; the recorder logs it and returns the original error
                // unchanged.
                record_redemption_failure_attempt(
                    &self.pool,
                    &command,
                    resolved.as_ref(),
                    CHANNEL_CODE,
                    USER_SUBJECT_TYPE,
                    &current_timestamp_string(),
                    &error,
                )
                .await;
                Err(error)
            }
        }
    }

    async fn redeem_promotion_code_in_tx(
        &self,
        command: &PromotionCodeRedemptionCommand,
        resolved: &mut Option<ResolvedRedemptionTarget>,
    ) -> Result<PromotionCodeRedemptionOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error(
                "failed to begin promotion code redemption transaction",
                error,
            )
        })?;
        let now = current_timestamp_string();
        let request_hash = redeem_request_hash(command);
        if let Some(row) = load_redeem_idempotency_row(&mut tx, command).await? {
            if string_cell(&row, "request_hash") != request_hash {
                return Err(CommerceServiceError::conflict(
                    "idempotency key was used with a different promotion code redemption request",
                ));
            }
            if string_cell(&row, "status") == "completed" {
                let outcome = replay_redeem_outcome(&row)?;
                tx.commit().await.map_err(|error| {
                    store_error("failed to commit promotion code redemption replay", error)
                })?;
                return Ok(outcome);
            }
            // 锁仍有效：另一个请求正在执行，拒绝并发抢占
            let locked_until = string_cell(&row, "locked_until");
            if !locked_until.is_empty() && locked_until > now {
                return Err(CommerceServiceError::conflict(
                    "another promotion code redemption request is in progress for this idempotency key",
                ));
            }
            // 锁已过期：前序请求崩溃残留，抢占并继续执行
            refresh_redeem_idempotency_lock(&mut tx, command, &now).await?;
        } else {
            insert_redeem_idempotency_lock(&mut tx, command, &request_hash, &now).await?;
        }

        let promotion = load_promotion_for_redeem(&mut tx, command, &now).await?;
        *resolved = Some(ResolvedRedemptionTarget {
            code_id: promotion.code_id.clone(),
            stock_id: promotion.stock_id.clone(),
            offer_id: promotion.offer_id.clone(),
        });
        ensure_promotion_can_be_redeemed(&mut tx, command, &promotion).await?;
        let coupon_id = coupon_id(command);
        let credit =
            credit_coupon_benefit_in_tx(&mut tx, command, &promotion, &coupon_id, &now).await?;
        let coupon_ledger_entry_id = coupon_ledger_entry_id(command);

        insert_user_coupon(&mut tx, command, &promotion, &coupon_id, &now).await?;
        insert_coupon_ledger_entry(
            &mut tx,
            command,
            &promotion,
            &coupon_id,
            &coupon_ledger_entry_id,
            &now,
        )
        .await?;
        update_promotion_counters(&mut tx, &promotion, &now).await?;

        // Success evidence is written *inside* the redemption transaction: the
        // attempt and the redemption it describes commit or roll back together.
        insert_redemption_attempt_in_tx(
            &mut tx,
            &RedemptionAttemptRecord {
                tenant_id: &command.tenant_id,
                organization_id: organization_binding(command.organization_id.as_deref()),
                code_id: Some(&promotion.code_id),
                stock_id: Some(&promotion.stock_id),
                offer_id: Some(&promotion.offer_id),
                user_coupon_id: Some(&coupon_id),
                order_id: None,
                submitted_code_hash: &submitted_code_hash(&command.code),
                submitted_code_suffix: &submitted_code_suffix(&command.code),
                subject_type: USER_SUBJECT_TYPE,
                subject_id: &command.owner_user_id,
                channel: CHANNEL_CODE,
                outcome: OUTCOME_SUCCEEDED,
                failure_code: None,
                failure_detail: None,
                benefit_kind: credit.benefit_kind.as_deref(),
                granted_amount: credit.credited_amount.as_deref(),
                currency_code: Some(&promotion.currency_code),
                request_no: &command.request_no,
                idempotency_key: &command.idempotency_key,
                // `PromotionCodeRedemptionCommand` carries no trace id yet.
                trace_id: "",
                occurred_at: &now,
            },
        )
        .await?;

        let outcome = PromotionCodeRedemptionOutcome::new(
            "Promotion code redeemed",
            &credit.amount_minor,
            credit.credited_points,
            credit.balance,
        )?
        .with_benefit_credit(
            credit.benefit_kind,
            credit.credited_amount,
            credit.asset_type,
            credit.member_card_id,
            credit.member_card_no,
        )
        .with_coupon(coupon_id.clone(), issued_coupon_code(command));
        complete_redeem_idempotency(&mut tx, command, &outcome, &now).await?;

        tx.commit().await.map_err(|error| {
            store_error(
                "failed to commit promotion code redemption transaction",
                error,
            )
        })?;

        Ok(outcome)
    }
    pub async fn claim_promotion_user_coupon(
        &self,
        command: ClaimPromotionUserCouponCommand,
    ) -> Result<PromotionUserCouponItem, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error(
                "failed to begin promotion user coupon claim transaction",
                error,
            )
        })?;
        let now = current_timestamp_string();
        let request_hash = claim_request_hash(&command);
        if let Some(row) = load_claim_idempotency_row(&mut tx, &command).await? {
            if string_cell(&row, "request_hash") != request_hash {
                return Err(CommerceServiceError::conflict(
                    "idempotency key was used with a different promotion coupon claim request",
                ));
            }
            if string_cell(&row, "status") == "completed" {
                let coupon = replay_claim_coupon(&row)?;
                tx.commit().await.map_err(|error| {
                    store_error("failed to commit promotion coupon claim replay", error)
                })?;
                return Ok(coupon);
            }
            // 锁仍有效：另一个请求正在执行，拒绝并发抢占
            let locked_until = string_cell(&row, "locked_until");
            if !locked_until.is_empty() && locked_until > now {
                return Err(CommerceServiceError::conflict(
                    "another promotion coupon claim request is in progress for this idempotency key",
                ));
            }
            // 锁已过期：前序请求崩溃残留，抢占并继续执行
            refresh_claim_idempotency_lock(&mut tx, &command, &now).await?;
        } else {
            insert_claim_idempotency_lock(&mut tx, &command, &request_hash, &now).await?;
        }

        let promotion = load_promotion_for_claim(&mut tx, &command, &now).await?;
        ensure_promotion_offer_can_be_claimed(&mut tx, &command, &promotion).await?;
        let pool_code = claim_pool_code_if_available(&mut tx, &command, &promotion, &now).await?;
        let coupon_id = claim_coupon_id(&command);
        insert_claimed_user_coupon(
            &mut tx,
            &command,
            &promotion,
            pool_code.as_ref(),
            &coupon_id,
            &now,
        )
        .await?;
        insert_claim_coupon_ledger_entry(
            &mut tx,
            &command,
            &promotion,
            &coupon_id,
            &claim_coupon_ledger_entry_id(&command),
            &now,
        )
        .await?;
        update_claim_promotion_counters(&mut tx, &promotion, &now).await?;

        let amount = stored_money_minor_units(&promotion.discount_value)?;
        let coupon_code = pool_code
            .as_ref()
            .map(|code| code.coupon_code.clone())
            .unwrap_or_else(|| issued_claim_coupon_code(&command));
        let coupon =
            PromotionUserCouponItem::new(&coupon_id, &coupon_code, &amount, &now, "pending")?;
        complete_claim_idempotency(&mut tx, &command, &coupon, &now).await?;
        tx.commit().await.map_err(|error| {
            store_error(
                "failed to commit promotion user coupon claim transaction",
                error,
            )
        })?;
        Ok(coupon)
    }

    pub async fn grant_member_card(
        &self,
        command: GrantMemberCardCommand,
    ) -> Result<PromotionMemberCard, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin member card grant transaction", error)
        })?;
        let card = grant_member_card_in_tx(&mut tx, &command).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit member card grant transaction", error)
        })?;
        Ok(card)
    }

    pub async fn consume_member_card(
        &self,
        command: ConsumeMemberCardCommand,
    ) -> Result<MemberCardConsumptionOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin member card consumption transaction", error)
        })?;
        let now = current_timestamp_string();

        // 幂等：同一请求键的消耗直接重放
        let consumption_id = member_card_consumption_id(&command);
        if let Some(replayed) = load_member_card_consumption_replay(&mut tx, &command, &consumption_id).await? {
            tx.commit().await.map_err(|error| {
                store_error("failed to commit member card consumption replay", error)
            })?;
            return Ok(replayed);
        }

        let card = sqlx::query(
            r#"
            SELECT id, card_no, offer_id, offer_version_id, user_coupon_id, owner_user_id,
                   period, duration_days, daily_quota, total_quota, total_used, status,
                   starts_at, expires_at, created_at, updated_at
            FROM promotion_member_card
            WHERE tenant_id = $1
              AND ((organization_id = $2) OR (organization_id = '0' AND $2 IS NULL))
              AND owner_user_id = $3
              AND id = $4
              AND status = 'active'
            FOR UPDATE
            "#,
        )
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(&command.owner_user_id)
        .bind(&command.card_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| store_error("failed to load member card for consumption", error))?
        .ok_or_else(|| CommerceServiceError::conflict("member card is not available"))?;
        let card = map_member_card_row(card)?;

        if let Some(expires_at) = &card.expires_at {
            // Instant comparison: card windows may be stored either as the
            // canonical server form or as RFC 3339 (see `crate::timestamps`).
            let expires_at = timestamps::epoch_seconds(expires_at).ok_or_else(|| {
                CommerceServiceError::storage(format!(
                    "member card {} has an unreadable expires_at",
                    card.id
                ))
            })?;
            let now_epoch = timestamps::epoch_seconds(now.as_str())
                .ok_or_else(|| CommerceServiceError::storage("runtime clock is unreadable"))?;
            if expires_at < now_epoch {
                return Err(CommerceServiceError::conflict("member card has expired"));
            }
        }
        let used_today: i64 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(amount), 0)
            FROM promotion_member_card_consumption
            WHERE tenant_id = $1 AND card_id = $2
              AND CAST(occurred_at AS TIMESTAMP) >= date_trunc('day', CURRENT_TIMESTAMP AT TIME ZONE 'UTC')
            "#,
        )
        .bind(&command.tenant_id)
        .bind(&command.card_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| store_error("failed to sum member card daily consumption", error))?;
        if command.amount > card.daily_quota.saturating_sub(used_today) {
            return Err(CommerceServiceError::conflict(
                "member card daily quota is exhausted",
            ));
        }
        if command.amount > card.total_quota.saturating_sub(card.total_used) {
            return Err(CommerceServiceError::conflict(
                "member card total quota is exhausted",
            ));
        }

        let next_total_used = card.total_used + command.amount;
        let balance_after = card.total_quota - next_total_used;
        let consumption_uuid = sdkwork_utils_rust::uuid();
        let card_update = sqlx::query(
            r#"
            UPDATE promotion_member_card
            SET total_used = total_used + $3, version = version + 1,
                updated_at = $4
            WHERE id = $1 AND tenant_id = $2 AND status = 'active'
              AND total_used + $3 <= total_quota
            "#,
        )
        .bind(&command.card_id)
        .bind(&command.tenant_id)
        .bind(command.amount)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| store_error("failed to update member card usage", error))?;
        // The quota guard lives in the UPDATE predicate, so a zero-row result
        // means the guard rejected the debit; ignoring it would persist a usage
        // ledger row and report a consumption that never happened.
        if card_update.rows_affected() != 1 {
            return Err(CommerceServiceError::conflict(
                "member card total quota was not updated atomically",
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO promotion_member_card_consumption
                (id, uuid, tenant_id, organization_id, card_id, amount, balance_after,
                 business_type, source_type, source_id, request_no, idempotency_key, trace_id,
                 occurred_at, created_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, 'usage', $8, $9, $10, $11, '', $12, $12)
            "#,
        )
        .bind(&consumption_id)
        .bind(&consumption_uuid)
        .bind(&command.tenant_id)
        .bind(organization_binding(command.organization_id.as_deref()))
        .bind(&command.card_id)
        .bind(command.amount)
        .bind(balance_after)
        .bind(command.source_type.as_deref())
        .bind(command.source_id.as_deref())
        .bind(&command.request_no)
        .bind(&command.idempotency_key)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|error| store_error("failed to record member card consumption", error))?;

        tx.commit().await.map_err(|error| {
            store_error("failed to commit member card consumption transaction", error)
        })?;
        Ok(MemberCardConsumptionOutcome {
            accepted: true,
            replayed: false,
            card_id: card.id,
            consumed_amount: command.amount,
            used_today: used_today + command.amount,
            daily_quota: card.daily_quota,
            total_used: next_total_used,
            total_quota: card.total_quota,
            balance: balance_after,
        })
    }

    pub async fn list_member_cards(
        &self,
        query: MemberCardListQuery,
    ) -> Result<Vec<PromotionMemberCard>, CommerceServiceError> {
        let rows = sqlx::query(
            r#"
            SELECT id, card_no, offer_id, offer_version_id, user_coupon_id, owner_user_id,
                   period, duration_days, daily_quota, total_quota, total_used, status,
                   starts_at, expires_at, created_at, updated_at
            FROM promotion_member_card
            WHERE tenant_id = $1
              AND ((organization_id = $2) OR (organization_id = '0' AND $2 IS NULL))
              AND owner_user_id = $3
            ORDER BY created_at DESC
            "#,
        )
        .bind(&query.tenant_id)
        .bind(query.organization_id.as_deref())
        .bind(&query.owner_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| store_error("failed to list member cards", error))?;
        rows.into_iter().map(map_member_card_row).collect()
    }

    pub async fn retrieve_member_card(
        &self,
        query: RetrieveMemberCardQuery,
    ) -> Result<Option<PromotionMemberCard>, CommerceServiceError> {
        let row = sqlx::query(
            r#"
            SELECT id, card_no, offer_id, offer_version_id, user_coupon_id, owner_user_id,
                   period, duration_days, daily_quota, total_quota, total_used, status,
                   starts_at, expires_at, created_at, updated_at
            FROM promotion_member_card
            WHERE tenant_id = $1
              AND ((organization_id = $2) OR (organization_id = '0' AND $2 IS NULL))
              AND owner_user_id = $3
              AND id = $4
            "#,
        )
        .bind(&query.tenant_id)
        .bind(query.organization_id.as_deref())
        .bind(&query.owner_user_id)
        .bind(&query.card_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| store_error("failed to retrieve member card", error))?;
        row.map(map_member_card_row).transpose()
    }

    /// 生命周期扫描：激活已到排期生效时间的会员卡（scheduled → active）。
    pub async fn activate_due_member_cards(&self) -> Result<i64, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin member card activation", error)
        })?;
        let activated = activate_due_member_cards_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit member card activation", error)
        })?;
        Ok(activated)
    }

    /// 生命周期扫描（advisory lock 保护，防多实例并发）：排期激活 + 到期过期。
    ///
    /// The lock is transaction-scoped (`pg_try_advisory_xact_lock`) and both
    /// sub-scans run on that same transaction, so PostgreSQL releases the lock
    /// on commit/rollback. An early `?` return therefore cannot leave the lock
    /// held on a connection that returns to the pool — the previous
    /// session-scoped `pg_try_advisory_lock` leaked it on the first scan error,
    /// after which every later sweep returned `skipped: true` forever.
    pub async fn run_member_card_lifecycle_sweep(&self) -> Result<MemberCardLifecycleSweepOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin member card lifecycle sweep", error)
        })?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| store_error("failed to acquire member card lifecycle sweep lock", error))?;
        if !locked {
            // 另一实例正在执行本轮扫描
            tx.rollback().await.map_err(|error| {
                store_error("failed to roll back skipped member card lifecycle sweep", error)
            })?;
            return Ok(MemberCardLifecycleSweepOutcome {
                activated: 0,
                expired: 0,
                skipped: true,
            });
        }
        let activated = activate_due_member_cards_in_tx(&mut tx).await?;
        let expired = expire_due_member_cards_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit member card lifecycle sweep", error)
        })?;
        Ok(MemberCardLifecycleSweepOutcome {
            activated,
            expired,
            skipped: false,
        })
    }

    /// 生命周期扫描：过期到期的会员卡（active → expired）。
    pub async fn expire_due_member_cards(&self) -> Result<i64, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin member card expiry", error)
        })?;
        let expired = expire_due_member_cards_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit member card expiry", error)
        })?;
        Ok(expired)
    }

    /// 券生命周期扫描：过期到期的钱包券（claimed/active/pending → expired）。
    ///
    /// Single `UPDATE`: the instant predicate is built by
    /// [`timestamps::instant`], so mixed text shapes cannot be compared as text.
    pub async fn expire_due_user_coupons(&self) -> Result<i64, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin user coupon expiry", error)
        })?;
        let expired = expire_due_user_coupons_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit user coupon expiry", error)
        })?;
        Ok(expired)
    }

    /// 券生命周期扫描：过期到期的未领取券码（active → expired）。
    ///
    /// A claimed code is never expired by this sweep: the wallet coupon it
    /// produced owns the remaining lifecycle. Single `UPDATE`, instant predicate.
    pub async fn expire_due_codes(&self) -> Result<i64, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin promotion code expiry", error)
        })?;
        let expired = expire_due_codes_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit promotion code expiry", error)
        })?;
        Ok(expired)
    }

    /// 券生命周期扫描（advisory lock 保护，防多实例并发）：过期的钱包券 + 券码。
    ///
    /// The lock is transaction-scoped (`pg_try_advisory_xact_lock`) and both
    /// updates run on that same transaction, so PostgreSQL releases it on
    /// commit/rollback. The key is distinct from
    /// [`MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY`]: the two sweeps must be able to
    /// run concurrently (they touch disjoint tables), and a shared key would make
    /// one instance's member-card sweep silently skip the coupon sweep.
    pub async fn run_promotion_lifecycle_sweep(
        &self,
    ) -> Result<PromotionLifecycleSweepOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin promotion lifecycle sweep", error)
        })?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                store_error("failed to acquire promotion lifecycle sweep lock", error)
            })?;
        if !locked {
            // 另一实例正在执行本轮扫描
            tx.rollback().await.map_err(|error| {
                store_error(
                    "failed to roll back skipped promotion lifecycle sweep",
                    error,
                )
            })?;
            return Ok(PromotionLifecycleSweepOutcome {
                expired_user_coupons: 0,
                expired_codes: 0,
                skipped: true,
            });
        }
        let expired_user_coupons = expire_due_user_coupons_in_tx(&mut tx).await?;
        let expired_codes = expire_due_codes_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit promotion lifecycle sweep", error)
        })?;
        Ok(PromotionLifecycleSweepOutcome {
            expired_user_coupons,
            expired_codes,
            skipped: false,
        })
    }

    /// 过期预算预留扫描（settlement lifecycle）：把过期未结算的 `applied` 申请释放。
    ///
    /// Every `applied` discount application whose `reservation_expires_at` instant
    /// is in the past is released in one **set-based** pass: a single
    /// `UPDATE ... RETURNING` marks the rows and collects them, then one aggregate
    /// budget restore (`reserved → available`), one bulk ledger insert and one
    /// aggregated audit row. There is no per-row statement loop, so the pass costs
    /// a constant number of round trips regardless of how many reservations
    /// expired.
    ///
    /// The lock is transaction-scoped (`pg_try_advisory_xact_lock`) and uses its
    /// own key, [`PROMOTION_BUDGET_RESERVATION_SWEEP_LOCK_KEY`] (`…430`), which is
    /// distinct from the member-card key (`…410`) and the coupon key (`…420`):
    /// all three sweeps must be able to run in the same tick, and a shared key
    /// would make one instance's sweep silently skip another's. When another
    /// instance holds the lock this pass reports `skipped: true` and changes
    /// nothing.
    pub async fn release_stale_discount_applications(
        &self,
    ) -> Result<BudgetReservationSweepOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error("failed to begin stale discount reservation sweep", error)
        })?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(PROMOTION_BUDGET_RESERVATION_SWEEP_LOCK_KEY)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                store_error("failed to acquire stale discount reservation sweep lock", error)
            })?;
        if !locked {
            tx.rollback().await.map_err(|error| {
                store_error(
                    "failed to roll back skipped stale discount reservation sweep",
                    error,
                )
            })?;
            return Ok(BudgetReservationSweepOutcome {
                released: 0,
                skipped: true,
            });
        }
        let released = release_stale_discount_applications_in_tx(&mut tx).await?;
        tx.commit().await.map_err(|error| {
            store_error("failed to commit stale discount reservation sweep", error)
        })?;
        Ok(BudgetReservationSweepOutcome {
            released,
            skipped: false,
        })
    }

    pub async fn apply_promotion_discount(
        &self,
        command: ApplyPromotionDiscountCommand,
    ) -> Result<PromotionUserCouponItem, CommerceServiceError> {
        if let Some(existing) = self.find_applied_promotion_coupon(&command).await? {
            return Ok(existing);
        }

        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error(
                "failed to begin promotion discount application transaction",
                error,
            )
        })?;
        let now = current_timestamp_string();
        let request_hash = apply_discount_request_hash(&command);
        if let Some(row) = load_apply_idempotency_row(&mut tx, &command).await? {
            if string_cell(&row, "request_hash") != request_hash {
                return Err(CommerceServiceError::conflict(
                    "idempotency key was used with a different promotion discount application request",
                ));
            }
            if string_cell(&row, "status") == "completed" {
                let coupon = replay_promotion_coupon_item(&row)?;
                tx.commit().await.map_err(|error| {
                    store_error(
                        "failed to commit promotion discount application replay",
                        error,
                    )
                })?;
                return Ok(coupon);
            }
            // 锁仍有效：另一个请求正在执行，拒绝并发抢占
            let locked_until = string_cell(&row, "locked_until");
            if !locked_until.is_empty() && locked_until > now {
                return Err(CommerceServiceError::conflict(
                    "another promotion discount application request is in progress for this idempotency key",
                ));
            }
            // 锁已过期：前序请求崩溃残留，抢占并继续执行
            refresh_apply_idempotency_lock(&mut tx, &command, &now).await?;
        } else {
            insert_apply_idempotency_lock(&mut tx, &command, &request_hash, &now).await?;
        }

        let coupon = load_user_coupon_for_discount_apply(&mut tx, &command).await?;
        ensure_user_coupon_can_be_applied(&coupon)?;
        let order = load_order_for_discount_apply(&mut tx, &command).await?;
        ensure_order_has_no_conflicting_application(&mut tx, &command, &coupon).await?;
        // The amount actually taken off the order comes from the pinned offer
        // version's terms applied to the order total — never from the coupon's
        // face value, which for a percentage coupon is a percentage, not money.
        // The order total is already canonical minor units in the shared order
        // schema (`commerce_order_amount_breakdown.original_amount` is
        // `NUMERIC(20,0)`), so it is consumed as-is.
        let discount_amount_minor = compute_discount_amount(
            &coupon.discount_type,
            &coupon.discount_value,
            &coupon.minimum_amount,
            coupon.maximum_discount_amount.as_deref(),
            order.total_amount_minor,
        )?;
        // `promotion_discount_application.discount_amount` stores integer minor
        // units, matching the order money columns and the admin console's
        // `formatMarketingAmountMinor` rendering.
        let discount_amount = discount_amount_minor.to_string();
        let application_id = discount_application_id(&command);
        // Budget guard: reserve the computed application amount first, in the
        // same transaction. A `409` here aborts the whole apply, so the coupon is
        // not marked applied and no application row is written.
        let reservation = reserve_coupon_budget(
            &mut tx,
            &coupon,
            &command,
            &application_id,
            discount_amount_minor,
            &now,
        )
        .await?;
        insert_discount_application(
            &mut tx,
            &command,
            &coupon,
            &order,
            &application_id,
            &discount_amount,
            &now,
            reservation.as_ref(),
        )
        .await?;
        mark_user_coupon_applied(&mut tx, &command, &now).await?;
        // Report the amount this application actually took off, in minor units
        // (the API unit), rather than the offer's face value.
        let item = build_applied_promotion_coupon_item(&coupon, discount_amount_minor, &now)?;
        complete_apply_idempotency(&mut tx, &command, &item, &now).await?;
        tx.commit().await.map_err(|error| {
            store_error(
                "failed to commit promotion discount application transaction",
                error,
            )
        })?;
        Ok(item)
    }

    pub async fn reverse_promotion_discount(
        &self,
        command: ReversePromotionDiscountCommand,
    ) -> Result<PromotionUserCouponItem, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error(
                "failed to begin promotion discount reversal transaction",
                error,
            )
        })?;
        let now = current_timestamp_string();
        let request_hash = reverse_discount_request_hash(&command);
        if let Some(row) = load_reverse_idempotency_row(&mut tx, &command).await? {
            if string_cell(&row, "request_hash") != request_hash {
                return Err(CommerceServiceError::conflict(
                    "idempotency key was used with a different promotion discount reversal request",
                ));
            }
            if string_cell(&row, "status") == "completed" {
                let coupon = replay_promotion_coupon_item(&row)?;
                tx.commit().await.map_err(|error| {
                    store_error("failed to commit promotion discount reversal replay", error)
                })?;
                return Ok(coupon);
            }
            // 锁仍有效：另一个请求正在执行，拒绝并发抢占
            let locked_until = string_cell(&row, "locked_until");
            if !locked_until.is_empty() && locked_until > now {
                return Err(CommerceServiceError::conflict(
                    "another promotion discount reversal request is in progress for this idempotency key",
                ));
            }
            // 锁已过期：前序请求崩溃残留，抢占并继续执行
            refresh_reverse_idempotency_lock(&mut tx, &command, &now).await?;
        } else {
            insert_reverse_idempotency_lock(&mut tx, &command, &request_hash, &now).await?;
        }

        let coupon = load_user_coupon_for_discount_reverse(&mut tx, &command).await?;
        reverse_discount_application(&mut tx, &command, &now).await?;
        restore_user_coupon_after_reverse(&mut tx, &command, &now).await?;
        let item = build_reversed_promotion_coupon_item(&coupon, &now)?;
        complete_reverse_idempotency(&mut tx, &command, &item, &now).await?;
        tx.commit().await.map_err(|error| {
            store_error(
                "failed to commit promotion discount reversal transaction",
                error,
            )
        })?;
        Ok(item)
    }

    async fn find_applied_promotion_coupon(
        &self,
        command: &ApplyPromotionDiscountCommand,
    ) -> Result<Option<PromotionUserCouponItem>, CommerceServiceError> {
        let row = sqlx::query(
            r#"
            SELECT c.id,
                   COALESCE(NULLIF(c.coupon_code, ''), '-') AS code,
                   CAST(a.discount_amount AS TEXT) AS amount,
                   CAST(a.applied_at AS TEXT) AS date,
                   c.status AS status
            FROM promotion_discount_application a
            JOIN promotion_user_coupon c
              ON c.tenant_id = a.tenant_id
             AND c.id = a.user_coupon_id
            WHERE a.tenant_id = CAST($1 AS TEXT)
              AND ((a.organization_id = CAST($2 AS TEXT)) OR (a.organization_id = '0' AND $3 IS NULL))
              AND a.order_id = CAST($4 AS TEXT)
              AND a.user_coupon_id = CAST($5 AS TEXT)
              AND LOWER(COALESCE(a.status, '')) IN ('applied', 'settled')
            LIMIT 1
           "#,
        )
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(command.organization_id.as_deref())
        .bind(&command.order_id)
        .bind(&command.user_coupon_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| store_error("failed to load existing discount application", error))?;

        row.map(|row| {
            let status =
                coupon_status_label(&required_status_cell(&row, "status", "apply")?)?.to_owned();
            let amount = stored_money_minor_units(&string_cell(&row, "amount"))?;
            PromotionUserCouponItem::new(
                &string_cell(&row, "id"),
                &string_cell(&row, "code"),
                &amount,
                &string_cell(&row, "date"),
                &status,
            )
        })
        .transpose()
    }
}

#[derive(Debug, Clone)]
struct DiscountApplyCoupon {
    id: String,
    code: String,
    offer_id: String,
    offer_version_id: String,
    /// Immutable rule version's discount type; `promotion_discount_application.discount_type`
    /// is NOT NULL without a default, so apply must copy it from the pinned version.
    discount_type: String,
    discount_value: String,
    /// Minimum order amount (decimal major) the pinned version requires before
    /// the coupon may discount anything.
    minimum_amount: String,
    /// Optional cap (decimal major) for a `PERCENTAGE` discount.
    maximum_discount_amount: Option<String>,
    /// Amount actually recorded on the application row, in integer minor units.
    /// Only populated by the reversal loader (the apply path computes it), so a
    /// reversal reports the amount that was really discounted instead of the
    /// coupon's face value.
    applied_amount_minor: Option<i64>,
    /// `promotion_offer.combinability`; drives whether a second coupon may be
    /// applied to the same order.
    combinability: String,
    status: String,
    /// The hard-cap budget account of the stock this coupon belongs to.
    ///
    /// `None` (the historical shape, most coupons today) means "no cap": apply
    /// never reserves and behaves exactly as before. When present, apply MUST
    /// reserve the application amount before the application row is written.
    budget_account_id: Option<String>,
}

/// Which budget account was reserved for one application, and until when.
#[derive(Debug, Clone)]
struct BudgetReservation {
    account_id: String,
    expires_at: String,
}

/// How long an application's budget reservation stays valid before the
/// stale-reservation sweep releases it.
///
/// 24 hours (`86_400` seconds): long enough for a real checkout to settle or
/// roll back, short enough that an abandoned order cannot hold a coupon budget
/// hostage. The value is a plain integer second count; the deadline stored on the
/// row is `reservation_expires_at = applied_at + RESERVATION_TTL_SECONDS`.
const RESERVATION_TTL_SECONDS: i64 = 86_400;

#[derive(Debug, Clone)]
struct DiscountApplyOrder {
    order_no: String,
    currency_code: String,
    /// Order-level total from `commerce_order_amount_breakdown`
    /// (`allocation_type = 'order_total'`) in **canonical integer minor units**:
    /// the shared order schema migrates that column to `NUMERIC(20,0)`.
    ///
    /// A monetary discount cannot be computed without it, so the loader turns a
    /// missing breakdown row into a `409` instead of silently discounting `0`.
    total_amount_minor: i64,
}

fn apply_discount_request_hash(command: &ApplyPromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "apply-discount",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        &command.order_id,
        &command.user_coupon_id,
        &command.request_no,
    ])
}

fn reverse_discount_request_hash(command: &ReversePromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "reverse-discount",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        &command.user_coupon_id,
        command.reason.as_deref().unwrap_or("none"),
        &command.request_no,
    ])
}

fn discount_application_id(command: &ApplyPromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "discount-application",
        &command.tenant_id,
        &command.order_id,
        &command.user_coupon_id,
    ])
}

fn discount_application_no(command: &ApplyPromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "discount-application-no",
        &command.tenant_id,
        &command.request_no,
    ])
}

fn apply_idempotency_id(command: &ApplyPromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "apply-idempotency",
        &command.tenant_id,
        &command.idempotency_key,
    ])
}

fn reverse_idempotency_id(command: &ReversePromotionDiscountCommand) -> String {
    stable_storage_id(&[
        "reverse-idempotency",
        &command.tenant_id,
        &command.idempotency_key,
    ])
}

async fn load_apply_idempotency_row(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    sqlx::query(
        r#"
        SELECT request_hash, response_json, status, locked_until
        FROM commerce_idempotency_key
        WHERE tenant_id = $1 AND scope = $2 AND idempotency_key = $3
        LIMIT 1
        FOR UPDATE
       "#,
    )
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE)
    .bind(&command.idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load apply idempotency record", error))
}

async fn refresh_apply_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET status = 'locked', locked_until = $1, expires_at = $2, updated_at = $3
        WHERE tenant_id = $4 AND scope = $5 AND idempotency_key = $6
       "#,
    )
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to refresh apply idempotency lock", error))?;
    Ok(())
}

async fn insert_apply_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    request_hash: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        INSERT INTO commerce_idempotency_key
            (id, tenant_id, organization_id, scope, idempotency_key, request_hash,
             status, locked_until, expires_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'locked', $7, $8, $9, $10)
       "#,
    )
    .bind(apply_idempotency_id(command))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE)
    .bind(&command.idempotency_key)
    .bind(request_hash)
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert apply idempotency lock", error))?;
    Ok(())
}

async fn complete_apply_idempotency(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    coupon: &PromotionUserCouponItem,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let response_json = serde_json::json!({
        "id": coupon.id,
        "code": coupon.code,
        "amount": coupon.amount.as_str(),
        "date": coupon.date,
        "status": coupon.status,
    })
    .to_string();
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET response_json = $1, status = 'completed', locked_until = NULL, updated_at = $2
        WHERE tenant_id = $3 AND scope = $4 AND idempotency_key = $5
       "#,
    )
    .bind(response_json)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to complete apply idempotency record", error))?;
    Ok(())
}

async fn load_reverse_idempotency_row(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    sqlx::query(
        r#"
        SELECT request_hash, response_json, status, locked_until
        FROM commerce_idempotency_key
        WHERE tenant_id = $1 AND scope = $2 AND idempotency_key = $3
        LIMIT 1
        FOR UPDATE
       "#,
    )
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE)
    .bind(&command.idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load reverse idempotency record", error))
}

async fn refresh_reverse_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET status = 'locked', locked_until = $1, expires_at = $2, updated_at = $3
        WHERE tenant_id = $4 AND scope = $5 AND idempotency_key = $6
       "#,
    )
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to refresh reverse idempotency lock", error))?;
    Ok(())
}

async fn insert_reverse_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    request_hash: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        INSERT INTO commerce_idempotency_key
            (id, tenant_id, organization_id, scope, idempotency_key, request_hash,
             status, locked_until, expires_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'locked', $7, $8, $9, $10)
       "#,
    )
    .bind(reverse_idempotency_id(command))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE)
    .bind(&command.idempotency_key)
    .bind(request_hash)
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert reverse idempotency lock", error))?;
    Ok(())
}

async fn complete_reverse_idempotency(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    coupon: &PromotionUserCouponItem,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let response_json = serde_json::json!({
        "id": coupon.id,
        "code": coupon.code,
        "amount": coupon.amount.as_str(),
        "date": coupon.date,
        "status": coupon.status,
    })
    .to_string();
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET response_json = $1, status = 'completed', locked_until = NULL, updated_at = $2
        WHERE tenant_id = $3 AND scope = $4 AND idempotency_key = $5
       "#,
    )
    .bind(response_json)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to complete reverse idempotency record", error))?;
    Ok(())
}

fn replay_promotion_coupon_item(
    row: &sqlx::postgres::PgRow,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    let response_json = optional_string_cell(row, "response_json").ok_or_else(|| {
        CommerceServiceError::invalid_state("promotion idempotency record has no response")
    })?;
    let value: serde_json::Value = serde_json::from_str(&response_json).map_err(|error| {
        CommerceServiceError::storage(format!("invalid promotion idempotency response: {error}"))
    })?;
    let amount = contract_money_minor_units(
        value
            .get("amount")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("promotion response amount is missing"))?,
    )?;
    PromotionUserCouponItem::new(
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("promotion response id is missing"))?,
        value
            .get("code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("promotion response code is missing"))?,
        &amount,
        value
            .get("date")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("promotion response date is missing"))?,
        value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("promotion response status is missing"))?,
    )
}

async fn load_user_coupon_for_discount_apply(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
) -> Result<DiscountApplyCoupon, CommerceServiceError> {
    let sql = format!(
        r#"
        SELECT c.id,
               COALESCE(NULLIF(c.coupon_code, ''), '-') AS code,
               c.offer_id,
               c.offer_version_id,
               CAST(v.discount_type AS TEXT) AS discount_type,
               CAST(v.discount_value AS TEXT) AS discount_value,
               CAST(COALESCE(NULLIF(TRIM(v.minimum_amount), ''), '0') AS TEXT) AS minimum_amount,
               CAST(v.maximum_discount_amount AS TEXT) AS maximum_discount_amount,
               COALESCE(o.combinability, 'EXCLUSIVE') AS combinability,
               c.status,
               b.id AS budget_account_id
        FROM promotion_user_coupon c
        JOIN promotion_offer_version v
          ON v.tenant_id = c.tenant_id
         AND v.id = c.offer_version_id
        JOIN promotion_offer o
          ON o.tenant_id = c.tenant_id
         AND o.id = c.offer_id
        LEFT JOIN promotion_budget_account b
          ON b.tenant_id = c.tenant_id
         AND b.stock_id = c.stock_id
        WHERE c.tenant_id = CAST($1 AS TEXT)
          AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id = '0' AND $3 IS NULL))
          AND c.id = CAST($4 AS TEXT)
          AND c.owner_user_id = CAST($5 AS TEXT)
          AND c.subject_type = $6
          AND c.subject_id = CAST($7 AS TEXT)
          AND {valid_from}
          AND {valid_until}
        LIMIT 1
        FOR UPDATE
       "#,
        valid_from = window_started("c.valid_from"),
        valid_until = window_not_ended("c.expires_at"),
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&command.user_coupon_id)
    .bind(&command.owner_user_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load user coupon for discount apply", error))?
    .ok_or_else(|| CommerceServiceError::not_found("user coupon was not found"))?;

    Ok(DiscountApplyCoupon {
        id: string_cell(&row, "id"),
        code: string_cell(&row, "code"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        discount_type: string_cell(&row, "discount_type"),
        discount_value: string_cell(&row, "discount_value"),
        minimum_amount: string_cell(&row, "minimum_amount"),
        maximum_discount_amount: optional_string_cell(&row, "maximum_discount_amount"),
        applied_amount_minor: None,
        combinability: string_cell(&row, "combinability"),
        status: string_cell(&row, "status"),
        budget_account_id: optional_string_cell(&row, "budget_account_id"),
    })
}

async fn load_user_coupon_for_discount_reverse(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
) -> Result<DiscountApplyCoupon, CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT c.id,
               COALESCE(NULLIF(c.coupon_code, ''), '-') AS code,
               c.offer_id,
               c.offer_version_id,
               CAST(v.discount_type AS TEXT) AS discount_type,
               CAST(COALESCE(a.discount_amount, v.discount_value, '0') AS TEXT) AS discount_value,
               CAST(COALESCE(a.discount_amount, '0') AS TEXT) AS applied_amount,
               COALESCE(o.combinability, 'EXCLUSIVE') AS combinability,
               c.status,
               a.budget_account_id
        FROM promotion_user_coupon c
        JOIN promotion_offer_version v
          ON v.tenant_id = c.tenant_id
         AND v.id = c.offer_version_id
        JOIN promotion_offer o
          ON o.tenant_id = c.tenant_id
         AND o.id = c.offer_id
        LEFT JOIN promotion_discount_application a
          ON a.tenant_id = c.tenant_id
         AND a.user_coupon_id = c.id
         AND LOWER(COALESCE(a.status, '')) IN ('applied', 'settled')
        WHERE c.tenant_id = CAST($1 AS TEXT)
          AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id = '0' AND $3 IS NULL))
          AND c.id = CAST($4 AS TEXT)
          AND c.owner_user_id = CAST($5 AS TEXT)
          AND c.subject_type = $6
          AND c.subject_id = CAST($7 AS TEXT)
        LIMIT 1
       "#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&command.user_coupon_id)
    .bind(&command.owner_user_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load user coupon for discount reverse", error))?
    .ok_or_else(|| CommerceServiceError::not_found("user coupon was not found"))?;

    Ok(DiscountApplyCoupon {
        id: string_cell(&row, "id"),
        code: string_cell(&row, "code"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        discount_type: string_cell(&row, "discount_type"),
        // The reversal reads the already-applied amount, so the pricing terms
        // are not used; they are carried only to keep one coupon shape.
        discount_value: string_cell(&row, "discount_value"),
        minimum_amount: "0".to_owned(),
        maximum_discount_amount: None,
        // Minor units of the recorded application amount, used as the reversal
        // response/ledger figure instead of the coupon's face value.
        applied_amount_minor: string_cell(&row, "applied_amount").trim().parse().ok(),
        combinability: string_cell(&row, "combinability"),
        status: string_cell(&row, "status"),
        // Informational only: the reversal releases the budget from the locked
        // application row (which is authoritative), not from this unlocked join.
        // The column is selected so the struct keeps one shape.
        budget_account_id: optional_string_cell(&row, "budget_account_id"),
    })
}

/// Rejects a second coupon on an order when either side is non-combinable.
///
/// `promotion_offer.combinability` was previously decorative: the apply path
/// accepted any number of distinct coupons for the same order, so every
/// "exclusive" coupon stacked in practice and the order was discounted twice.
/// The rule is evaluated from the immutable offer rows rather than the
/// application rows so a later offer edit cannot relax historical stacks.
async fn ensure_order_has_no_conflicting_application(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    coupon: &DiscountApplyCoupon,
) -> Result<(), CommerceServiceError> {
    let existing = sqlx::query(
        r#"
        SELECT a.offer_id,
               COALESCE(NULLIF(BTRIM(a.discount_type), ''), 'FIXED') AS discount_type,
               COALESCE(o.combinability, 'EXCLUSIVE') AS combinability
        FROM promotion_discount_application a
        LEFT JOIN promotion_offer o
          ON o.tenant_id = a.tenant_id
         AND o.id = a.offer_id
        WHERE a.tenant_id = CAST($1 AS TEXT)
          AND a.order_id = CAST($2 AS TEXT)
          AND LOWER(COALESCE(a.status, '')) IN ('applied', 'settled')
        LIMIT 1
        "#,
    )
    .bind(&command.tenant_id)
    .bind(&command.order_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load applied promotions for order", error))?;
    let Some(existing) = existing else {
        return Ok(());
    };
    let existing_offer_id = string_cell(&existing, "offer_id");
    if existing_offer_id == coupon.offer_id {
        // Same offer re-applied through a different wallet coupon: the coupon
        // level guards already decide this case.
        return Ok(());
    }
    let existing_combinability = string_cell(&existing, "combinability")
        .trim()
        .to_ascii_uppercase();
    let requested_combinability = coupon.combinability.trim().to_ascii_uppercase();
    if existing_combinability != "COMBINABLE" || requested_combinability != "COMBINABLE" {
        return Err(CommerceServiceError::conflict(
            "order already has a coupon that cannot be combined with another one",
        ));
    }
    Ok(())
}

fn ensure_user_coupon_can_be_applied(
    coupon: &DiscountApplyCoupon,
) -> Result<(), CommerceServiceError> {
    match coupon.status.trim().to_ascii_lowercase().as_str() {
        "claimed" | "issued" | "active" | "pending" | "claimable" => Ok(()),
        "used" | "redeemed" => Err(CommerceServiceError::conflict(
            "user coupon has already been applied",
        )),
        "expired" | "disabled" | "voided" | "cancelled" => {
            Err(CommerceServiceError::conflict("user coupon is not usable"))
        }
        status => Err(CommerceServiceError::conflict(format!(
            "user coupon status {status} is not applicable"
        ))),
    }
}

async fn load_order_for_discount_apply(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
) -> Result<DiscountApplyOrder, CommerceServiceError> {
    // The order-level breakdown row is the amount a discount is computed against.
    // It is read with a LEFT JOIN so a missing row is distinguishable from "the
    // order itself is not applicable"; the currency is cross-checked against the
    // order below so a mismatched breakdown can never price a discount in the
    // wrong currency.
    let row = sqlx::query(
        r#"
        SELECT o.order_no,
               COALESCE(o.currency_code, '') AS currency_code,
               o.status,
               CAST(b.original_amount AS TEXT) AS total_amount,
               COALESCE(b.currency_code, '') AS breakdown_currency_code
        FROM commerce_order o
        LEFT JOIN LATERAL (
            SELECT original_amount, currency_code
            FROM commerce_order_amount_breakdown
            WHERE tenant_id = o.tenant_id
              AND order_id = o.id
              AND allocation_type = 'order_total'
            ORDER BY id
            LIMIT 1
        ) b ON TRUE
        WHERE o.tenant_id = CAST($1 AS TEXT)
          AND ((o.organization_id = CAST($2 AS TEXT)) OR (o.organization_id = '0' AND $3 IS NULL))
          AND o.id = CAST($4 AS TEXT)
          AND o.owner_user_id = CAST($5 AS TEXT)
          AND LOWER(COALESCE(o.status, '')) IN (
              'draft', 'pending', 'pending_payment', 'unpaid', 'wait_pay'
          )
        LIMIT 1
       "#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&command.order_id)
    .bind(&command.owner_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load order for discount apply", error))?
    .ok_or_else(|| CommerceServiceError::conflict("order is not discount applicable"))?;

    let total_amount = string_cell(&row, "total_amount");
    if total_amount.trim().is_empty() {
        return Err(CommerceServiceError::conflict(
            "order amount is not available for discount application",
        ));
    }
    // The shared order schema stores this column as `NUMERIC(20,0)` in minor
    // units (verified against the deployed database), so it is parsed directly.
    let total_amount_minor = total_amount.trim().parse::<i64>().map_err(|_| {
        CommerceServiceError::storage(format!(
            "order amount breakdown is not integer minor units: {total_amount}"
        ))
    })?;
    if total_amount_minor < 0 {
        return Err(CommerceServiceError::conflict(
            "order amount is not available for discount application",
        ));
    }
    let currency_code = string_cell(&row, "currency_code");
    let breakdown_currency_code = string_cell(&row, "breakdown_currency_code");
    ensure_discount_currencies_match(&currency_code, &breakdown_currency_code)?;

    Ok(DiscountApplyOrder {
        order_no: string_cell(&row, "order_no"),
        currency_code,
        total_amount_minor,
    })
}

/// Rejects an order whose amount-breakdown row prices it in another currency.
///
/// Either side may legitimately be empty (legacy rows); only two present and
/// different codes are a conflict, so a discount can never be computed from an
/// amount denominated in a currency the order does not use.
fn ensure_discount_currencies_match(
    order_currency_code: &str,
    breakdown_currency_code: &str,
) -> Result<(), CommerceServiceError> {
    if !order_currency_code.trim().is_empty()
        && !breakdown_currency_code.trim().is_empty()
        && order_currency_code != breakdown_currency_code
    {
        return Err(CommerceServiceError::conflict(
            "order currency does not match the discount currency",
        ));
    }
    Ok(())
}

/// Reserves the application amount against the coupon's hard-cap budget.
///
/// `amount_minor` is the amount the caller computed for this application
/// ([`compute_discount_amount`] in minor units), not the offer's face value:
/// a percentage coupon must reserve the money it actually discounts.
///
/// Money-unit rule for this change: the new `promotion_budget_account` /
/// `promotion_budget_ledger_entry` amounts are `*_minor BIGINT` integer minor
/// units. The offer version still stores `discount_value` as a decimal-major
/// string, so [`compute_discount_amount`] is the one conversion point and this
/// function only moves an already-minor amount. No float is ever constructed
/// and nothing is divided by 100 here.
///
/// The reservation is a single atomic `UPDATE` whose `available_amount_minor >=
/// $amount` predicate is the guard; `rows_affected != 1` (a missing / inactive
/// account or an exhausted pool) is the documented `409` "coupon budget has been
/// exhausted". Reserving and recording the ledger row happen in the caller's
/// transaction, so a failure anywhere rolls the reservation back.
///
/// Returns `None` when the stock has no budget account — the historical shape
/// that most coupons still use — which leaves apply unchanged.
async fn reserve_coupon_budget(
    tx: &mut Transaction<'_, Postgres>,
    coupon: &DiscountApplyCoupon,
    command: &ApplyPromotionDiscountCommand,
    application_id: &str,
    amount_minor: i64,
    now: &str,
) -> Result<Option<BudgetReservation>, CommerceServiceError> {
    let Some(account_id) = coupon.budget_account_id.as_deref() else {
        return Ok(None);
    };
    let expires_at = timestamp_after_seconds(now, RESERVATION_TTL_SECONDS);
    let row = sqlx::query(
        r#"
        UPDATE promotion_budget_account
        SET available_amount_minor = available_amount_minor - $1,
            reserved_amount_minor = reserved_amount_minor + $1,
            version = version + 1,
            updated_at = $2
        WHERE id = $3
          AND tenant_id = $4
          AND status = 'active'
          AND available_amount_minor >= $1
        RETURNING available_amount_minor, reserved_amount_minor, used_amount_minor, currency_code
       "#,
    )
    .bind(amount_minor)
    .bind(now)
    .bind(account_id)
    .bind(&command.tenant_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to reserve coupon budget", error))?
    .ok_or_else(|| CommerceServiceError::conflict("coupon budget has been exhausted"))?;
    insert_budget_reserve_ledger_entry(
        tx,
        command,
        account_id,
        application_id,
        amount_minor,
        &row,
        now,
    )
    .await?;
    Ok(Some(BudgetReservation {
        account_id: account_id.to_owned(),
        expires_at,
    }))
}

/// Appends the `reserve` ledger row for one application.
///
/// `amount_minor` is positive and `direction` carries the sign; the three
/// `_after_minor` values come straight from the guarded `UPDATE ... RETURNING`, so
/// the evidence row is the account state the reservation actually produced.
async fn insert_budget_reserve_ledger_entry(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    account_id: &str,
    application_id: &str,
    amount_minor: i64,
    row: &sqlx::postgres::PgRow,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO promotion_budget_ledger_entry
            (id, uuid, tenant_id, organization_id, budget_account_id, application_id,
             direction, amount_minor, available_after_minor, reserved_after_minor,
             used_after_minor, currency_code, request_no, idempotency_key, occurred_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'reserve', $7, $8, $9, $10, $11, $12, $13, $14)
       "#,
    )
    .bind(stable_storage_id(&[
        "budget-ledger",
        &command.tenant_id,
        "reserve",
        application_id,
    ]))
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(account_id)
    .bind(application_id)
    .bind(amount_minor)
    .bind(integer_cell(row, "available_amount_minor"))
    .bind(integer_cell(row, "reserved_amount_minor"))
    .bind(integer_cell(row, "used_amount_minor"))
    .bind(string_cell(row, "currency_code"))
    .bind(&command.request_no)
    // Deterministic per application: one reservation, one ledger row.
    .bind(stable_storage_id(&[
        "budget-ledger",
        &command.tenant_id,
        "reserve",
        application_id,
    ]))
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record coupon budget reservation", error))?;
    Ok(())
}

async fn insert_discount_application(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    coupon: &DiscountApplyCoupon,
    order: &DiscountApplyOrder,
    application_id: &str,
    discount_amount: &str,
    now: &str,
    reservation: Option<&BudgetReservation>,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO promotion_discount_application
            (id, uuid, tenant_id, organization_id, application_no, offer_id, offer_version_id,
             user_coupon_id, order_id, order_no, subject_type, subject_id, discount_type,
             discount_amount, currency_code, status, request_no, idempotency_key, applied_at,
             rolled_back_at, created_at, updated_at, budget_account_id, reservation_expires_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'applied', $16, $17, $18, NULL, $19, $20, $21, $22)
       "#,
    )
    .bind(application_id)
    // `promotion_discount_application.uuid` is NOT NULL without a database
    // default, so the apply insert must supply it.
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(discount_application_no(command))
    .bind(&coupon.offer_id)
    .bind(&coupon.offer_version_id)
    .bind(&coupon.id)
    .bind(&command.order_id)
    .bind(&order.order_no)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    // `discount_type` is NOT NULL with no default: copy it from the immutable
    // offer version the wallet coupon is pinned to, so a later version change
    // cannot rewrite history.
    .bind(&coupon.discount_type)
    // The computed amount, not the coupon's face value: a percentage coupon
    // records the money it took off, and a capped one records the capped amount.
    .bind(discount_amount)
    .bind(&order.currency_code)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(now)
    .bind(now)
    .bind(now)
    // Budget link + deadline: both NULL when the stock has no cap, which keeps
    // the pre-S4 rows and behaviour identical.
    .bind(reservation.map(|reservation| reservation.account_id.as_str()))
    .bind(reservation.map(|reservation| reservation.expires_at.as_str()))
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert promotion discount application", error))?;
    Ok(())
}

async fn mark_user_coupon_applied(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let updated = sqlx::query(
        r#"
        UPDATE promotion_user_coupon
        SET status = 'redeemed', redeemed_at = $1, updated_at = $2
        WHERE tenant_id = CAST($3 AS TEXT)
          AND id = CAST($4 AS TEXT)
          AND owner_user_id = CAST($5 AS TEXT)
          AND LOWER(COALESCE(status, '')) IN ('claimed', 'issued', 'active', 'pending', 'claimable')
       "#,
    )
    .bind(now)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(&command.user_coupon_id)
    .bind(&command.owner_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to mark user coupon applied", error))?
    .rows_affected();

    if updated != 1 {
        return Err(CommerceServiceError::conflict(
            "user coupon has already been applied or is not applicable",
        ));
    }
    Ok(())
}

/// Decides whether one locked discount application may be reversed.
///
/// `applied` (the reservation is still live) and `settled` (the historical flow
/// allowed reversing a settled application) may be reversed. Every other state
/// is terminal and is refused **before** any money moves, so a second reverse of
/// an already-rolled-back row can never release the reservation twice.
fn reverse_status_decision(status: &str) -> Result<(), &'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "applied" | "settled" => Ok(()),
        "rolled_back" => Err("promotion discount application was already rolled back"),
        "released" => Err("promotion discount application was already released"),
        _ => Err("active promotion discount application was not found"),
    }
}

/// Resolves the amount a reversal returns to the budget's available pool.
///
/// The authoritative amount is the application's `reserve` ledger entry — the
/// same lookup the stale-reservation sweep performs (newest `reserve` row for
/// the application). When no such row exists, the application's stored
/// decimal-major `discount_amount` is converted with [`money_cents`], exactly as
/// the settlement lifecycle converts it. In both cases the value is integer
/// minor units; no float is constructed and nothing is divided by 100.
fn reversal_release_amount_minor(
    ledger_amount_minor: Option<i64>,
    discount_amount: &str,
) -> Result<i64, CommerceServiceError> {
    match ledger_amount_minor {
        Some(amount_minor) => Ok(amount_minor),
        None => money_cents(discount_amount),
    }
}

/// Reverses the live discount application of one wallet coupon.
///
/// The order of the steps is the safety property, and all of them run in the
/// caller's reversal transaction:
///
/// 1. the application row is locked (`FOR UPDATE`) and its status is checked by
///    [`reverse_status_decision`] — a terminal row stops here, before money
///    moves, so repeated reversals cannot double-release;
/// 2. the status transition writes `rolled_back` / `rolled_back_at` and the
///    request `reason` into `rollback_reason`, guarded by the same status set,
///    and must affect exactly one row;
/// 3. only then, when the locked row carries a `budget_account_id`, the reserved
///    amount is returned to the available pool by
///    [`release_reversed_budget_reservation`], which also appends the `release`
///    ledger row and the audit row. A failed move aborts the whole transaction,
///    so the status transition and coupon restore roll back with it.
async fn reverse_discount_application(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT id,
               status,
               CAST(discount_amount AS TEXT) AS discount_amount,
               budget_account_id,
               request_no
        FROM promotion_discount_application
        WHERE tenant_id = CAST($1 AS TEXT)
          AND user_coupon_id = CAST($2 AS TEXT)
          AND subject_type = $3
          AND subject_id = CAST($4 AS TEXT)
        ORDER BY applied_at DESC
        LIMIT 1
        FOR UPDATE
       "#,
    )
    .bind(&command.tenant_id)
    .bind(&command.user_coupon_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| {
        store_error("failed to lock promotion discount application for reversal", error)
    })?
    .ok_or_else(|| {
        CommerceServiceError::conflict("active promotion discount application was not found")
    })?;

    // Status transition guard: before any budget move.
    let status = string_cell(&row, "status");
    reverse_status_decision(&status)
        .map_err(|message| CommerceServiceError::conflict(message))?;

    let application_id = string_cell(&row, "id");
    let updated = sqlx::query(
        r#"
        UPDATE promotion_discount_application
        SET status = 'rolled_back',
            rolled_back_at = $1,
            rollback_reason = $2,
            updated_at = $1
        WHERE id = $3
          AND tenant_id = CAST($4 AS TEXT)
          AND LOWER(COALESCE(status, '')) IN ('applied', 'settled')
       "#,
    )
    .bind(now)
    .bind(command.reason.as_deref())
    .bind(&application_id)
    .bind(&command.tenant_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to reverse promotion discount application", error))?
    .rows_affected();

    if updated != 1 {
        return Err(CommerceServiceError::conflict(
            "active promotion discount application was not found",
        ));
    }

    let Some(account_id) = optional_string_cell(&row, "budget_account_id") else {
        return Ok(());
    };
    let ledger_amount = load_reverse_reserve_amount(tx, &command.tenant_id, &application_id).await?;
    let amount_minor =
        reversal_release_amount_minor(ledger_amount, &string_cell(&row, "discount_amount"))?;
    release_reversed_budget_reservation(
        tx,
        command,
        &account_id,
        &application_id,
        amount_minor,
        &string_cell(&row, "request_no"),
        now,
    )
    .await?;
    Ok(())
}

/// Reads the `reserve` ledger amount recorded for one application.
///
/// Mirrors the stale-reservation sweep lookup: newest `reserve` entry for
/// `(tenant, application)`. `None` means the reservation was never recorded and
/// the caller falls back to the application's own amount.
async fn load_reverse_reserve_amount(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: &str,
    application_id: &str,
) -> Result<Option<i64>, CommerceServiceError> {
    sqlx::query_scalar(
        r#"
        SELECT amount_minor
        FROM promotion_budget_ledger_entry
        WHERE tenant_id = $1
          AND application_id = $2
          AND direction = 'reserve'
        ORDER BY occurred_at DESC, id DESC
        LIMIT 1
       "#,
    )
    .bind(tenant_id)
    .bind(application_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load reversal budget reservation amount", error))
}

/// Returns a reversed application's reservation to the available pool.
///
/// One guarded `UPDATE` carries the whole safety property: `status = 'active'`
/// and `reserved_amount_minor >= $amount` are evaluated by the database under
/// the account row lock, so two reversals (or a reversal racing the stale sweep)
/// cannot both consume the same reserved amount. A zero-row result fails the
/// reversal with a conflict instead of letting the money disappear.
///
/// The account state returned by that `UPDATE` is written straight into the
/// `release` ledger row, so the evidence row is self-checking; one
/// `discount_applications.release` audit row records the reversal provenance.
async fn release_reversed_budget_reservation(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    account_id: &str,
    application_id: &str,
    amount_minor: i64,
    request_no: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let row = sqlx::query(
        r#"
        UPDATE promotion_budget_account
        SET reserved_amount_minor = reserved_amount_minor - $1,
            available_amount_minor = available_amount_minor + $1,
            version = version + 1,
            updated_at = $2
        WHERE id = $3
          AND tenant_id = $4
          AND status = 'active'
          AND reserved_amount_minor >= $1
        RETURNING available_amount_minor, reserved_amount_minor, used_amount_minor, currency_code
       "#,
    )
    .bind(amount_minor)
    .bind(now)
    .bind(account_id)
    .bind(&command.tenant_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to release reversed coupon budget", error))?
    .ok_or_else(|| {
        CommerceServiceError::conflict("budget reservation could not be released")
    })?;
    insert_reversal_release_ledger_entry(
        tx,
        command,
        account_id,
        application_id,
        amount_minor,
        &row,
        request_no,
        now,
    )
    .await?;
    insert_reversal_release_audit_event(tx, command, application_id, now).await?;
    Ok(())
}

/// Appends the `release` ledger row for one reversed application.
///
/// `amount_minor` is positive and `direction` carries the sign; the
/// `_after_minor` values are the account state the guarded `UPDATE` returned.
/// The deterministic idempotency key is the same per-application `release` key
/// the settlement lifecycle uses, so one application can never have two release
/// evidence rows.
async fn insert_reversal_release_ledger_entry(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    account_id: &str,
    application_id: &str,
    amount_minor: i64,
    row: &sqlx::postgres::PgRow,
    request_no: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let ledger_id = stable_storage_id(&[
        "budget-ledger",
        &command.tenant_id,
        "release",
        application_id,
    ]);
    sqlx::query(
        r#"
        INSERT INTO promotion_budget_ledger_entry
            (id, uuid, tenant_id, organization_id, budget_account_id, application_id,
             direction, amount_minor, available_after_minor, reserved_after_minor,
             used_after_minor, currency_code, request_no, idempotency_key, occurred_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, 'release', $7, $8, $9, $10, $11, $12, $13, $14)
       "#,
    )
    .bind(&ledger_id)
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(account_id)
    .bind(application_id)
    .bind(amount_minor)
    .bind(integer_cell(row, "available_amount_minor"))
    .bind(integer_cell(row, "reserved_amount_minor"))
    .bind(integer_cell(row, "used_amount_minor"))
    .bind(string_cell(row, "currency_code"))
    .bind(request_no)
    .bind(&ledger_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record reversed budget release", error))?;
    Ok(())
}

/// Writes the `discount_applications.release` audit row for a reversal.
///
/// The existing allowed action is reused (the CHECK vocabulary is not widened);
/// `detail_json` records that the release came from a reversal and why, and the
/// acting subject is the wallet-coupon owner.
async fn insert_reversal_release_audit_event(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    application_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let detail_json = serde_json::json!({
        "source": "reversal",
        "reason": command.reason.as_deref(),
    })
    .to_string();
    sqlx::query(
        r#"
        INSERT INTO promotion_audit_event
            (id, uuid, tenant_id, organization_id, actor_id, action, aggregate_type,
             aggregate_id, subject_count, detail_json, trace_id, occurred_at, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, 1, $9, '', $10, $10)
       "#,
    )
    .bind(stable_storage_id(&[
        "audit-event",
        DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
        application_id,
    ]))
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(&command.owner_user_id)
    .bind(DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION)
    .bind(DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE)
    .bind(application_id)
    .bind(&detail_json)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record reversal budget release audit event", error))?;
    Ok(())
}

async fn restore_user_coupon_after_reverse(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let updated = sqlx::query(
        r#"
        UPDATE promotion_user_coupon
        SET status = 'claimed', redeemed_at = NULL, updated_at = $1
        WHERE tenant_id = CAST($2 AS TEXT)
          AND id = CAST($3 AS TEXT)
          AND owner_user_id = CAST($4 AS TEXT)
          AND LOWER(COALESCE(status, '')) = 'redeemed'
       "#,
    )
    .bind(now)
    .bind(&command.tenant_id)
    .bind(&command.user_coupon_id)
    .bind(&command.owner_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to restore user coupon after reverse", error))?
    .rows_affected();

    if updated != 1 {
        return Err(CommerceServiceError::conflict(
            "user coupon was not in used state and could not be restored",
        ));
    }
    Ok(())
}

/// Builds the apply response for one wallet coupon.
///
/// `discount_amount_minor` is the computed amount the application records
/// (already minor units, the unit `PromotionUserCouponItem.amount` exposes).
fn build_applied_promotion_coupon_item(
    coupon: &DiscountApplyCoupon,
    discount_amount_minor: i64,
    now: &str,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    let amount = discount_amount_minor.to_string();
    PromotionUserCouponItem::new(&coupon.id, &coupon.code, &amount, now, "success")
}

fn build_reversed_promotion_coupon_item(
    coupon: &DiscountApplyCoupon,
    now: &str,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    // The reversal reports the amount that was actually applied. Falling back to
    // the pinned terms only matters for a legacy row with no application amount.
    let amount = match coupon.applied_amount_minor {
        Some(minor) => minor.to_string(),
        None => stored_money_minor_units(&coupon.discount_value)?,
    };
    PromotionUserCouponItem::new(&coupon.id, &coupon.code, &amount, now, "pending")
}

async fn load_redeem_idempotency_row(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    sqlx::query(
        r#"
        SELECT request_hash, response_json, status, locked_until
        FROM commerce_idempotency_key
        WHERE tenant_id = $1 AND scope = $2 AND idempotency_key = $3
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(&command.tenant_id)
    .bind(PROMOTION_CODE_REDEMPTION_SCOPE)
    .bind(&command.idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load redeem idempotency record", error))
}

async fn refresh_redeem_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET status = 'locked',
            locked_until = $1,
            expires_at = $2,
            updated_at = $3
        WHERE tenant_id = $4 AND scope = $5 AND idempotency_key = $6
        "#,
    )
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_CODE_REDEMPTION_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to refresh redeem idempotency lock", error))?;
    Ok(())
}

async fn insert_redeem_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    request_hash: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"
        INSERT INTO commerce_idempotency_key
            (id, tenant_id, organization_id, scope, idempotency_key, request_hash,
             response_json, status, locked_until, expires_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, NULL, 'locked', $7, $8, $9, $10)
        "#,
    )
    .bind(redeem_idempotency_id(command))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(PROMOTION_CODE_REDEMPTION_SCOPE)
    .bind(&command.idempotency_key)
    .bind(request_hash)
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert redeem idempotency lock", error))?;
    Ok(())
}

async fn complete_redeem_idempotency(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    outcome: &PromotionCodeRedemptionOutcome,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let response_json = serde_json::json!({
        "message": outcome.message,
        "amount": outcome.amount.as_str(),
        "creditedPoints": outcome.credited_points,
        "balance": outcome.balance,
        "benefitKind": outcome.benefit_kind,
        "creditedAmount": outcome.credited_amount,
        "assetType": outcome.asset_type,
        "memberCardId": outcome.member_card_id,
        "memberCardNo": outcome.member_card_no,
        "userCouponId": outcome.user_coupon_id,
        "couponCode": outcome.coupon_code,
    })
    .to_string();
    sqlx::query(
        r#"
        UPDATE commerce_idempotency_key
        SET response_json = $1,
            status = 'completed',
            locked_until = NULL,
            updated_at = $2
        WHERE tenant_id = $3 AND scope = $4 AND idempotency_key = $5
        "#,
    )
    .bind(response_json)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_CODE_REDEMPTION_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to complete redeem idempotency record", error))?;
    Ok(())
}

fn replay_redeem_outcome(
    row: &sqlx::postgres::PgRow,
) -> Result<PromotionCodeRedemptionOutcome, CommerceServiceError> {
    let response_json = optional_string_cell(row, "response_json").ok_or_else(|| {
        CommerceServiceError::invalid_state("redeem idempotency record has no response")
    })?;
    let value: serde_json::Value = serde_json::from_str(&response_json).map_err(|error| {
        CommerceServiceError::storage(format!("invalid redeem idempotency response: {error}"))
    })?;
    let message = value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CommerceServiceError::storage("redeem response message is missing"))?;
    let amount = value
        .get("amount")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CommerceServiceError::storage("redeem response amount is missing"))?;
    let credited_points = value
        .get("creditedPoints")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            CommerceServiceError::storage("redeem response creditedPoints is missing")
        })?;
    let balance = value
        .get("balance")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| CommerceServiceError::storage("redeem response balance is missing"))?;

    let amount = contract_money_minor_units(amount)?;
    Ok(PromotionCodeRedemptionOutcome::new(message, &amount, credited_points, balance)?
        .with_benefit_credit(
            value.get("benefitKind").and_then(serde_json::Value::as_str).map(str::to_owned),
            value.get("creditedAmount").and_then(serde_json::Value::as_str).map(str::to_owned),
            value.get("assetType").and_then(serde_json::Value::as_str).map(str::to_owned),
            value.get("memberCardId").and_then(serde_json::Value::as_str).map(str::to_owned),
            value.get("memberCardNo").and_then(serde_json::Value::as_str).map(str::to_owned),
        )
        .with_coupon(
            value.get("userCouponId").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(),
            value.get("couponCode").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(),
        ))
}

/// Predicate for the indexed redemption lookup.
///
/// `promotion_code.code_hash` is `UPPER(TRIM(code))` hashed once at write time
/// and indexed by `uk_promotion_code_hash (tenant_id, code_hash)`, so this is a
/// single indexed equality probe instead of a sequential scan over the largest
/// promotion table. The submitted code is hashed in Rust with
/// [`crate::code_identity::code_identity`] before the query.
const REDEEM_BY_HASH_PREDICATE: &str = "pc.code_hash = CAST($3 AS TEXT)";

/// Pre-phase-2 predicate, kept only for the documented fallback.
///
/// It cannot use the hash index, so it must never be the first lookup. It runs
/// only after the hash probe returned no row — a row created before the
/// backfill, restored from an older backup, or hashed by a different algorithm
/// revision — and a hit is repaired in the same transaction so the next lookup
/// of that code is indexed again.
const REDEEM_BY_NORMALIZED_CODE_PREDICATE: &str =
    "UPPER(BTRIM(pc.promotion_code)) = UPPER(BTRIM(CAST($3 AS TEXT)))";

/// Builds the redemption row query for one code-equality predicate.
///
/// Every other guard (tenant/organization scope, active statuses, validity
/// windows, and the `FOR UPDATE` locks) is shared by the hash-first lookup and
/// its fallback, so the fallback can never apply weaker conditions than the
/// primary probe.
fn redeem_load_sql(code_predicate: &str) -> String {
    format!(
        r#"
        SELECT pc.id AS code_id,
               s.id AS stock_id,
               pc.offer_id AS offer_id,
               s.offer_version_id AS offer_version_id,
               s.stock_type AS stock_type,
               CAST(v.discount_value AS TEXT) AS discount_value,
               COALESCE(NULLIF(BTRIM(v.currency_code), ''), 'CNY') AS currency_code,
               v.rule_json AS rule_json,
               s.total_quantity AS total_quantity,
               COALESCE(s.available_quantity, 0) AS available_quantity,
               COALESCE(s.claimed_quantity, 0) AS stock_claimed_quantity,
               COALESCE(s.per_user_limit, 1) AS per_user_limit,
               COALESCE(pc.max_claims, 1) AS code_max_claims,
               COALESCE(pc.claimed_quantity, 0) AS code_claimed_quantity,
               {expires_at} AS expires_at
        FROM promotion_code pc
        JOIN promotion_coupon_stock s
          ON s.tenant_id = pc.tenant_id
         AND s.id = pc.stock_id
        JOIN promotion_offer o
          ON o.tenant_id = pc.tenant_id
         AND o.id = pc.offer_id
        JOIN promotion_offer_version v
          ON v.tenant_id = pc.tenant_id
         AND v.id = s.offer_version_id
        WHERE pc.tenant_id = CAST($1 AS TEXT)
          AND ((pc.organization_id = CAST($2 AS TEXT)) OR (pc.organization_id = '0' AND $2 IS NULL))
          AND {code_predicate}
          AND pc.status = 'active'
          AND s.status = 'active'
          AND o.status = 'active'
          AND o.deleted_at IS NULL
          AND v.lifecycle_status = 'published'
          AND {pc_started}
          AND {pc_not_ended}
          AND {stock_started}
          AND {stock_not_ended}
          AND {offer_started}
          AND {offer_not_ended}
        ORDER BY pc.organization_id DESC NULLS LAST, pc.id ASC
        LIMIT 1
        FOR UPDATE OF pc, s
        "#,
        code_predicate = code_predicate,
        expires_at = format!(
            "TO_CHAR(COALESCE({pc}, {stock}, {offer}) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            pc = timestamps::instant("pc.expires_at"),
            stock = timestamps::instant("s.claim_ends_at"),
            offer = timestamps::instant("o.ends_at"),
        ),
        pc_started = window_started("pc.starts_at"),
        pc_not_ended = window_not_ended("pc.expires_at"),
        stock_started = window_started("s.claim_starts_at"),
        stock_not_ended = window_not_ended("s.claim_ends_at"),
        offer_started = window_started("o.starts_at"),
        offer_not_ended = window_not_ended("o.ends_at"),
    )
}

async fn load_promotion_for_redeem(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    now: &str,
) -> Result<RedeemPromotion, CommerceServiceError> {
    let _ = now;
    // Time windows are compared as instants, never as text: admin rows carry
    // RFC 3339 (`2026-11-10T09:12:00.000Z`) while server rows carry
    // `2026-11-10 09:12:00`, and text ordering across those shapes is wrong.
    // See `crate::timestamps`.
    //
    // The submitted code is normalized and hashed exactly once, here, from the
    // value the customer submitted. `code_identity` is the same helper the write
    // path uses to fill `promotion_code.code_hash`, so the probe cannot drift
    // from storage.
    let identity = code_identity(&command.code);
    let sql = redeem_load_sql(REDEEM_BY_HASH_PREDICATE);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(&identity.hash)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| store_error("failed to load promotion code", error))?;
    let row = match row {
        Some(row) => row,
        // Documented fallback (rolling-deploy safety): an old instance may still
        // write a row through the pre-hash insert, a restored row may predate the
        // backfill, or the hash algorithm may have been revised. Run the
        // pre-phase-2 normalized-text match once; a hit is repaired inside this
        // transaction (and logged at `info`), so the next lookup is indexed.
        None => load_promotion_for_redeem_by_normalized_code(tx, command, &identity)
            .await?
            .ok_or_else(|| {
                CommerceServiceError::conflict("promotion code is invalid or unavailable")
            })?,
    };

    Ok(RedeemPromotion {
        code_id: string_cell(&row, "code_id"),
        stock_id: string_cell(&row, "stock_id"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        stock_type: string_cell(&row, "stock_type"),
        discount_value: string_cell(&row, "discount_value"),
        currency_code: string_cell(&row, "currency_code"),
        rule_json: optional_string_cell(&row, "rule_json"),
        total_quantity: optional_integer_cell(&row, "total_quantity"),
        available_quantity: integer_cell(&row, "available_quantity"),
        stock_claimed_quantity: integer_cell(&row, "stock_claimed_quantity"),
        per_user_limit: integer_cell(&row, "per_user_limit"),
        code_max_claims: integer_cell(&row, "code_max_claims"),
        code_claimed_quantity: integer_cell(&row, "code_claimed_quantity"),
        expires_at: optional_string_cell(&row, "expires_at"),
    })
}

/// Hash-miss fallback for [`load_promotion_for_redeem`].
///
/// Returns `None` when the pre-phase-2 normalized-text match finds nothing (the
/// caller turns that into the same public rejection). On a hit it repairs
/// `code_hash`/`code_last4` in the caller's transaction before returning the row,
/// so a hash miss is paid at most once per code. The fallback query takes the
/// same `FOR UPDATE OF pc, s` locks as the primary probe, which serializes the
/// repair against every other redemption of the same code.
async fn load_promotion_for_redeem_by_normalized_code(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    identity: &CodeIdentity,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    let sql = redeem_load_sql(REDEEM_BY_NORMALIZED_CODE_PREDICATE);
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(&command.code)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|error| store_error("failed to load promotion code", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    // Repair inside the redemption transaction: the row is already locked by the
    // fallback query, the update rolls back with a failed redemption, and the
    // next lookup of this code is a single indexed hash probe.
    let code_id = string_cell(&row, "code_id");
    let repaired = sqlx::query(
        r#"
        UPDATE promotion_code
        SET code_hash = CAST($1 AS TEXT), code_last4 = CAST($2 AS TEXT)
        WHERE id = CAST($3 AS TEXT)
          AND tenant_id = CAST($4 AS TEXT)
          AND code_hash IS DISTINCT FROM CAST($1 AS TEXT)
        "#,
    )
    .bind(&identity.hash)
    .bind(&identity.last4)
    .bind(&code_id)
    .bind(&command.tenant_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to repair promotion code hash identity", error))?;
    if repaired.rows_affected() == 1 {
        tracing::info!(
            operation = "repair_promotion_code_hash_identity",
            code_id = %code_id,
            "repaired a missing promotion code hash identity on the documented fallback lookup"
        );
    }
    Ok(Some(row))
}

async fn ensure_promotion_can_be_redeemed(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    promotion: &RedeemPromotion,
) -> Result<(), CommerceServiceError> {
    let requires_stock_quantity = promotion_requires_stock_quantity(promotion);
    if let Some(total_quantity) = promotion.total_quantity {
        if promotion.stock_claimed_quantity >= total_quantity || promotion.available_quantity <= 0 {
            return Err(CommerceServiceError::conflict(
                "promotion code has reached its issue limit",
            ));
        }
    } else if requires_stock_quantity && promotion.available_quantity <= 0 {
        return Err(CommerceServiceError::conflict(
            "promotion code has reached its issue limit",
        ));
    }
    if promotion.code_max_claims > 0 && promotion.code_claimed_quantity >= promotion.code_max_claims
    {
        return Err(CommerceServiceError::conflict(
            "promotion code has reached its issue limit",
        ));
    }
    let received_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(1)
        FROM promotion_user_coupon
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $2 IS NULL))
          AND subject_type = $3
          AND subject_id = CAST($4 AS TEXT)
          AND code_id = $5
          AND LOWER(COALESCE(status, '')) NOT IN ('expired', 'disabled', 'voided', 'cancelled')
        "#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&promotion.code_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| store_error("failed to check promotion code subject limit", error))?;
    // The stock's `per_user_limit` is the redemption cap for one subject, the
    // same field the claim path enforces. A hard `> 0` check here made a code
    // with `max_claims > 1` and `per_user_limit > 1` unredeemable a second time.
    if received_count >= promotion.per_user_limit.max(1) {
        return Err(CommerceServiceError::conflict(
            "promotion code subject receive limit has been reached",
        ));
    }
    Ok(())
}

async fn ensure_points_account(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    now: &str,
) -> Result<PointsAccount, CommerceServiceError> {
    if let Some(account) = load_points_account(tx, command).await? {
        return Ok(account);
    }

    let account_id = account_id(command);
    sqlx::query(
        r#"
        INSERT INTO commerce_account
            (id, tenant_id, organization_id, owner_user_id, asset_type, currency_code,
             available_amount, frozen_amount, version, status, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, '0', '0', 0, 'active', $7, $8)
        ON CONFLICT (tenant_id, organization_id, owner_user_id, asset_type, currency_code)
        DO NOTHING
        "#,
    )
    .bind(&account_id)
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(&command.owner_user_id)
    .bind(CommerceAccountAssetType::Points.as_str())
    .bind(POINTS_CURRENCY_CODE)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to create points account", error))?;

    load_points_account(tx, command).await?.ok_or_else(|| {
        CommerceServiceError::storage("points account was not available after creation")
    })
}

async fn load_points_account(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
) -> Result<Option<PointsAccount>, CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT id, CAST(COALESCE(available_amount, '0') AS BIGINT) AS available_points
        FROM commerce_account
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $2 IS NULL))
          AND owner_user_id = CAST($3 AS TEXT)
          AND asset_type = $4
          AND currency_code = $5
          AND status = 'active'
        ORDER BY id ASC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(&command.owner_user_id)
    .bind(CommerceAccountAssetType::Points.as_str())
    .bind(POINTS_CURRENCY_CODE)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load points account", error))?;

    Ok(row.map(|row| PointsAccount {
        id: string_cell(&row, "id"),
        available_points: integer_cell(&row, "available_points"),
    }))
}

/// Wallet coupon validity start in the canonical RFC 3339 shape so the wallet
/// window is comparable with the offer/code windows it is checked against.
fn valid_from_iso(now: &str) -> String {
    timestamps::server_form_to_iso(now).unwrap_or_else(timestamps::now_iso)
}

/// scheduled → active for member cards whose `starts_at` instant has arrived.
async fn activate_due_member_cards_in_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<i64, CommerceServiceError> {
    let sql = format!(
        r#"
        UPDATE promotion_member_card
        SET status = 'active', version = version + 1,
            updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
        WHERE status = 'scheduled'
          AND {started}
        "#,
        started = window_started("starts_at"),
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&mut **tx)
        .await
        .map_err(|error| store_error("failed to activate due member cards", error))?;
    Ok(result.rows_affected() as i64)
}

/// active → expired for member cards whose `expires_at` instant has passed.
async fn expire_due_member_cards_in_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<i64, CommerceServiceError> {
    let sql = format!(
        r#"
        UPDATE promotion_member_card
        SET status = 'expired', version = version + 1,
            updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
        WHERE status = 'active'
          AND expires_at IS NOT NULL
          AND {ended}
        "#,
        ended = format!("{} < NOW()", timestamps::instant("expires_at")),
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&mut **tx)
        .await
        .map_err(|error| store_error("failed to expire due member cards", error))?;
    Ok(result.rows_affected() as i64)
}

/// claimed/active/pending → expired for wallet coupons whose `expires_at`
/// instant has passed.
///
/// One statement over the whole table: the sweep is system-level (no tenant
/// scope), so a per-row loop would turn a large backlog into N round trips. The
/// predicate is an instant comparison, never text comparison, and it matches
/// `idx_promotion_user_coupon_status_expires (tenant_id, status, expires_at)` on
/// its status column.
async fn expire_due_user_coupons_in_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<i64, CommerceServiceError> {
    let sql = format!(
        r#"
        UPDATE promotion_user_coupon
        SET status = 'expired', version = version + 1,
            updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
        WHERE status IN ('claimed', 'active', 'pending')
          AND expires_at IS NOT NULL
          AND {ended}
        "#,
        ended = format!("{} < NOW()", timestamps::instant("expires_at")),
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&mut **tx)
        .await
        .map_err(|error| store_error("failed to expire due user coupons", error))?;
    Ok(result.rows_affected() as i64)
}

/// active → expired for unused promotion codes whose `expires_at` instant has
/// passed.
///
/// `COALESCE(claimed_quantity, 0) = 0` keeps a claimed code out of this sweep: its
/// wallet coupon owns the remaining lifecycle, and expiring the code would make
/// the already-issued benefit look unredeemable. One statement, instant predicate.
async fn expire_due_codes_in_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<i64, CommerceServiceError> {
    let sql = format!(
        r#"
        UPDATE promotion_code
        SET status = 'expired', version = version + 1,
            updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
        WHERE status = 'active'
          AND COALESCE(claimed_quantity, 0) = 0
          AND expires_at IS NOT NULL
          AND {ended}
        "#,
        ended = format!("{} < NOW()", timestamps::instant("expires_at")),
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
        .execute(&mut **tx)
        .await
        .map_err(|error| store_error("failed to expire due promotion codes", error))?;
    Ok(result.rows_affected() as i64)
}

async fn insert_user_coupon(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    promotion: &RedeemPromotion,
    coupon_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO promotion_user_coupon
            (id, uuid, tenant_id, organization_id, coupon_no, stock_id, code_id, offer_id,
             offer_version_id, subject_type, subject_id, owner_user_id, coupon_code,
             status, claimed_at, valid_from, expires_at, redeemed_at, disabled_at,
             request_no, idempotency_key, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
             'redeemed', $14, $15, $16, $17, NULL, $18, $19, $20, $21)
        "#,
    )
    .bind(coupon_id)
    .bind(format!("{coupon_id}-uuid"))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(coupon_no(command))
    .bind(&promotion.stock_id)
    .bind(&promotion.code_id)
    .bind(&promotion.offer_id)
    .bind(&promotion.offer_version_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&command.owner_user_id)
    .bind(issued_coupon_code(command))
    .bind(now)
    .bind(valid_from_iso(now))
    .bind(promotion.expires_at.as_deref())
    .bind(now)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to issue user coupon", error))?;
    Ok(())
}

async fn insert_coupon_ledger_entry(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    promotion: &RedeemPromotion,
    coupon_id: &str,
    coupon_ledger_entry_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let balance_after = (promotion.available_quantity - 1).max(0);
    sqlx::query(
        r#"
        INSERT INTO promotion_coupon_ledger_entry
            (id, uuid, tenant_id, stock_id, user_coupon_id, offer_id,
             subject_type, subject_id, direction, quantity_delta, balance_after, business_type,
             business_no, request_no, idempotency_key, source_type, source_id,
             trace_id, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, 'debit', -1, $9, 'redeem',
             $10, $11, $12, $13, $14, '', $15)
        "#,
    )
    .bind(coupon_ledger_entry_id)
    .bind(format!("{coupon_ledger_entry_id}-uuid"))
    .bind(&command.tenant_id)
    .bind(&promotion.stock_id)
    .bind(coupon_id)
    .bind(&promotion.offer_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(balance_after)
    .bind(coupon_ledger_no(command))
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(coupon_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record promotion coupon ledger entry", error))?;
    Ok(())
}

async fn update_promotion_counters(
    tx: &mut Transaction<'_, Postgres>,
    promotion: &RedeemPromotion,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let requires_stock_quantity = if promotion_requires_stock_quantity(promotion) {
        1_i64
    } else {
        0_i64
    };
    let stock_update = sqlx::query(
        r#"
        UPDATE promotion_coupon_stock
        SET available_quantity = CASE
                WHEN $1 = 1 THEN available_quantity - 1
                ELSE available_quantity
            END,
            claimed_quantity = COALESCE(claimed_quantity, 0) + 1,
            redeemed_quantity = COALESCE(redeemed_quantity, 0) + 1,
            updated_at = $2
        WHERE id = $3
          AND status = 'active'
          AND ($4 = 0 OR available_quantity > 0)
          AND COALESCE(claimed_quantity, 0) = $5
          AND ($6 IS NULL OR COALESCE(claimed_quantity, 0) < $7)
        "#,
    )
    .bind(requires_stock_quantity)
    .bind(now)
    .bind(&promotion.stock_id)
    .bind(requires_stock_quantity)
    .bind(promotion.stock_claimed_quantity)
    .bind(promotion.total_quantity)
    .bind(promotion.total_quantity)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to update promotion coupon stock counters", error))?;
    if stock_update.rows_affected() != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion coupon stock was not updated atomically",
        ));
    }

    let code_update = sqlx::query(
        r#"
        UPDATE promotion_code
        SET claimed_quantity = COALESCE(claimed_quantity, 0) + 1,
            updated_at = $1
        WHERE id = $2
          AND status = 'active'
          AND COALESCE(claimed_quantity, 0) = $3
          AND ($4 <= 0 OR COALESCE(claimed_quantity, 0) < $5)
        "#,
    )
    .bind(now)
    .bind(&promotion.code_id)
    .bind(promotion.code_claimed_quantity)
    .bind(promotion.code_max_claims)
    .bind(promotion.code_max_claims)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to update promotion code counters", error))?;
    if code_update.rows_affected() != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion code counter was not updated atomically",
        ));
    }
    Ok(())
}

async fn update_account_points(
    tx: &mut Transaction<'_, Postgres>,
    account_id: &str,
    current_available_points: i64,
    credited_points: i64,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let max_allowed_before_credit = i64::MAX
        .checked_sub(credited_points)
        .ok_or_else(|| CommerceServiceError::storage("promotion credit points overflow"))?;
    let account_update = sqlx::query(
        r#"
        UPDATE commerce_account
        SET available_amount = (available_amount::bigint + $1)::text,
            version = version + 1,
            updated_at = $2
        WHERE id = $3
          AND COALESCE(available_amount, '') ~ '^[0-9]+$'
          AND available_amount::bigint = $4
          AND available_amount::bigint <= $5
        "#,
    )
    .bind(credited_points)
    .bind(now)
    .bind(account_id)
    .bind(current_available_points)
    .bind(max_allowed_before_credit)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to update account points", error))?;
    if account_update.rows_affected() != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion points account was not updated atomically",
        ));
    }
    Ok(())
}

async fn insert_account_ledger(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    account_id: &str,
    balance_after: i64,
    credited_points: i64,
    source_coupon_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO commerce_account_ledger_entry
            (id, tenant_id, organization_id, account_id, owner_user_id, asset_type, direction,
             amount, balance_after, business_type, transaction_no, request_no, idempotency_key,
             source_type, source_id, remark, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'redeem', $10, $11, $12, $13, $14, $15, $16)
        "#,
    )
    .bind(ledger_entry_id(command))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(account_id)
    .bind(&command.owner_user_id)
    .bind(CommerceAccountAssetType::Points.as_str())
    .bind(CommerceLedgerDirection::Credit.as_str())
    .bind(credited_points.to_string())
    .bind(balance_after.to_string())
    .bind(&command.request_no)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(source_coupon_id)
    .bind(format!("redeem_promotion_code={}", command.code))
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert account ledger entry", error))?;
    Ok(())
}

async fn insert_redeem_billing_history(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    source_coupon_id: &str,
    amount: &str,
    currency_code: &str,
    credited_points: i64,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO commerce_billing_history
            (id, tenant_id, organization_id, owner_user_id, history_no, history_type,
             direction, asset_type, amount, currency_code, points_delta, status,
             title, reference_no, source_type, source_id, related_order_id,
             related_order_no, payment_method, occurred_at, metadata_json, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, 'redeem',
             'credit', 'points', $6, $7, $8, 'success',
             'Promotion code redemption', $9, $10, $11, NULL,
             NULL, NULL, $12, NULL, $12, $12)
        ON CONFLICT (tenant_id, source_type, source_id) DO NOTHING
        "#,
    )
    .bind(format!("billing-history-{source_coupon_id}"))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(&command.owner_user_id)
    .bind(format!("BH-{source_coupon_id}"))
    .bind(amount)
    .bind(currency_code)
    .bind(credited_points)
    .bind(&command.code)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(source_coupon_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert redeem billing history", error))?;
    Ok(())
}

async fn load_claim_idempotency_row(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
) -> Result<Option<sqlx::postgres::PgRow>, CommerceServiceError> {
    sqlx::query(
        r#"

        SELECT request_hash, response_json, status, locked_until
        FROM commerce_idempotency_key
        WHERE tenant_id = $1 AND scope = $2 AND idempotency_key = $3
        LIMIT 1
        FOR UPDATE

"#,
    )
    .bind(&command.tenant_id)
    .bind(PROMOTION_USER_COUPON_CLAIM_SCOPE)
    .bind(&command.idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load claim idempotency record", error))
}

async fn refresh_claim_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"

        UPDATE commerce_idempotency_key
        SET status = 'locked', locked_until = $1, expires_at = $2, updated_at = $3
        WHERE tenant_id = $4 AND scope = $5 AND idempotency_key = $6

"#,
    )
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_USER_COUPON_CLAIM_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to refresh claim idempotency lock", error))?;
    Ok(())
}

async fn insert_claim_idempotency_lock(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    request_hash: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let lock_expiry = lock_expiry_timestamp(now);
    let record_expiry = record_expiry_timestamp(now);
    sqlx::query(
        r#"

        INSERT INTO commerce_idempotency_key
            (id, tenant_id, organization_id, scope, idempotency_key, request_hash,
             response_json, status, locked_until, expires_at, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, NULL, 'locked', $7, $8, $9, $10)

"#,
    )
    .bind(claim_idempotency_id(command))
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(PROMOTION_USER_COUPON_CLAIM_SCOPE)
    .bind(&command.idempotency_key)
    .bind(request_hash)
    .bind(&lock_expiry)
    .bind(&record_expiry)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert claim idempotency lock", error))?;
    Ok(())
}

async fn complete_claim_idempotency(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    coupon: &PromotionUserCouponItem,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let response_json = serde_json::json!({
        "id": coupon.id,
        "code": coupon.code,
        "amount": coupon.amount.as_str(),
        "date": coupon.date,
        "status": coupon.status,
    })
    .to_string();
    sqlx::query(
        r#"

        UPDATE commerce_idempotency_key
        SET response_json = $1, status = 'completed', locked_until = NULL, updated_at = $2
        WHERE tenant_id = $3 AND scope = $4 AND idempotency_key = $5

"#,
    )
    .bind(response_json)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(PROMOTION_USER_COUPON_CLAIM_SCOPE)
    .bind(&command.idempotency_key)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to complete claim idempotency record", error))?;
    Ok(())
}

fn replay_claim_coupon(
    row: &sqlx::postgres::PgRow,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    let response_json = optional_string_cell(row, "response_json").ok_or_else(|| {
        CommerceServiceError::invalid_state("claim idempotency record has no response")
    })?;
    let value: serde_json::Value = serde_json::from_str(&response_json).map_err(|error| {
        CommerceServiceError::storage(format!("invalid claim idempotency response: {error}"))
    })?;
    let amount = contract_money_minor_units(
        value
            .get("amount")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("claim response amount is missing"))?,
    )?;
    PromotionUserCouponItem::new(
        value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("claim response id is missing"))?,
        value
            .get("code")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("claim response code is missing"))?,
        &amount,
        value
            .get("date")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("claim response date is missing"))?,
        value
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| CommerceServiceError::storage("claim response status is missing"))?,
    )
}

async fn load_promotion_for_claim(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    now: &str,
) -> Result<ClaimPromotion, CommerceServiceError> {
    let _ = now;
    // Window bounds are compared as instants; see `crate::timestamps`.
    let sql = format!(
        r#"

        SELECT s.id AS stock_id,
               s.offer_id AS offer_id,
               s.offer_version_id AS offer_version_id,
               s.stock_type AS stock_type,
               CAST(v.discount_value AS TEXT) AS discount_value,
               v.rule_json AS rule_json,
               s.total_quantity AS total_quantity,
               COALESCE(s.available_quantity, 0) AS available_quantity,
               COALESCE(s.claimed_quantity, 0) AS stock_claimed_quantity,
               COALESCE(s.per_user_limit, 1) AS per_user_limit,
               TO_CHAR(COALESCE({stock_expiry}, {offer_expiry}) AT TIME ZONE 'UTC',
                       'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at
        FROM promotion_offer o
        JOIN promotion_coupon_stock s
          ON s.tenant_id = o.tenant_id
         AND s.offer_id = o.id
        JOIN promotion_offer_version v
          ON v.tenant_id = s.tenant_id
         AND v.id = s.offer_version_id
        WHERE o.tenant_id = CAST($1 AS TEXT)
          AND ((o.organization_id = CAST($2 AS TEXT)) OR (o.organization_id = '0' AND $3 IS NULL))
          AND o.id = CAST($4 AS TEXT)
          AND o.status = 'active'
          AND o.deleted_at IS NULL
          AND s.status = 'active'
          AND v.lifecycle_status = 'published'
          AND {stock_started}
          AND {stock_not_ended}
          AND {offer_started}
          AND {offer_not_ended}
        ORDER BY s.created_at ASC, s.id ASC
        LIMIT 1

"#,
        stock_expiry = timestamps::instant("s.claim_ends_at"),
        offer_expiry = timestamps::instant("o.ends_at"),
        stock_started = window_started("s.claim_starts_at"),
        stock_not_ended = window_not_ended("s.claim_ends_at"),
        offer_started = window_started("o.starts_at"),
        offer_not_ended = window_not_ended("o.ends_at"),
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&command.offer_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load promotion offer for claim", error))?
    .ok_or_else(|| CommerceServiceError::conflict("promotion offer is invalid or unavailable"))?;

    Ok(ClaimPromotion {
        stock_id: string_cell(&row, "stock_id"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        stock_type: string_cell(&row, "stock_type"),
        discount_value: string_cell(&row, "discount_value"),
        rule_json: optional_string_cell(&row, "rule_json"),
        total_quantity: optional_integer_cell(&row, "total_quantity"),
        available_quantity: integer_cell(&row, "available_quantity"),
        stock_claimed_quantity: integer_cell(&row, "stock_claimed_quantity"),
        per_user_limit: integer_cell(&row, "per_user_limit"),
        expires_at: optional_string_cell(&row, "expires_at"),
    })
}

async fn ensure_promotion_offer_can_be_claimed(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    promotion: &ClaimPromotion,
) -> Result<(), CommerceServiceError> {
    if promotion.stock_type.trim().to_ascii_lowercase() != "unlimited"
        && promotion.available_quantity <= 0
    {
        return Err(CommerceServiceError::conflict(
            "promotion offer has reached its issue limit",
        ));
    }
    let received_count: i64 = sqlx::query_scalar(
        r#"

        SELECT COUNT(1)
        FROM promotion_user_coupon
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $3 IS NULL))
          AND subject_type = $4
          AND subject_id = CAST($5 AS TEXT)
          AND offer_id = $6
          AND LOWER(COALESCE(status, '')) NOT IN ('expired', 'disabled', 'voided', 'cancelled')

"#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&promotion.offer_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| store_error("failed to check promotion offer subject limit", error))?;
    if received_count >= promotion.per_user_limit {
        return Err(CommerceServiceError::conflict(
            "promotion offer subject receive limit has been reached",
        ));
    }
    Ok(())
}

struct ClaimedPoolCode {
    code_id: String,
    coupon_code: String,
    expires_at: Option<String>,
}

/// 领取时从该 stock 的预生成券码池中发放一张券码。
/// 池不存在（实时生成模式）时返回 None；池存在但已无可用券码时拒绝领取。
async fn claim_pool_code_if_available(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    promotion: &ClaimPromotion,
    now: &str,
) -> Result<Option<ClaimedPoolCode>, CommerceServiceError> {
    let pool_sql = format!(
        r#"

        SELECT id AS code_id,
               promotion_code AS coupon_code,
               TO_CHAR({expires_at} AT TIME ZONE 'UTC',
                       'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS expires_at
        FROM promotion_code
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $3 IS NULL))
          AND stock_id = $4
          AND status = 'active'
          AND claimed_quantity = 0
          AND {starts_at}
          AND {expires_at_bound}
        ORDER BY created_at ASC, id ASC
        LIMIT 1
        FOR UPDATE

"#,
        expires_at = timestamps::instant("expires_at"),
        starts_at = window_started("starts_at"),
        expires_at_bound = window_not_ended("expires_at"),
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(pool_sql.as_str()))
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(command.organization_id.as_deref())
        .bind(&promotion.stock_id)
        .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to claim promotion pool code", error))?;
    let Some(row) = row else {
        // 未命中：区分「无池（实时生成模式）」与「池存在但已耗尽」
        let pool_exists = sqlx::query_scalar::<_, bool>(
            r#"

            SELECT EXISTS(
                SELECT 1
                FROM promotion_code
                WHERE tenant_id = CAST($1 AS TEXT)
                  AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $3 IS NULL))
                  AND stock_id = $4
                  AND status = 'active'
            )

"#,
        )
        .bind(&command.tenant_id)
        .bind(command.organization_id.as_deref())
        .bind(command.organization_id.as_deref())
        .bind(&promotion.stock_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| store_error("failed to check promotion code pool existence", error))?;
        if !pool_exists {
            return Ok(None);
        }
        return Err(CommerceServiceError::conflict(
            "promotion code pool is exhausted",
        ));
    };
    let code_id = string_cell(&row, "code_id");
    let coupon_code = string_cell(&row, "coupon_code");
    let consumed = sqlx::query(
        r#"

        UPDATE promotion_code
        SET claimed_quantity = COALESCE(claimed_quantity, 0) + 1,
            updated_at = $1
        WHERE id = $2
          AND status = 'active'
          AND claimed_quantity < max_claims

"#,
    )
    .bind(now)
    .bind(&code_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to consume promotion pool code", error))?;
    if consumed.rows_affected() != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion code pool was not consumed atomically",
        ));
    }
    Ok(Some(ClaimedPoolCode {
        code_id,
        coupon_code,
        expires_at: optional_string_cell(&row, "expires_at"),
    }))
}

async fn insert_claimed_user_coupon(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    promotion: &ClaimPromotion,
    pool_code: Option<&ClaimedPoolCode>,
    coupon_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let code_id = pool_code.map(|code| code.code_id.as_str());
    let coupon_code = pool_code
        .map(|code| code.coupon_code.clone())
        .unwrap_or_else(|| issued_claim_coupon_code(command));
    let expires_at = pool_code
        .and_then(|code| code.expires_at.as_deref())
        .or_else(|| promotion.expires_at.as_deref());
    sqlx::query(
        r#"

        INSERT INTO promotion_user_coupon
            (id, uuid, tenant_id, organization_id, coupon_no, stock_id, code_id, offer_id,
             offer_version_id, subject_type, subject_id, owner_user_id, coupon_code,
             status, claimed_at, valid_from, expires_at, redeemed_at, disabled_at,
             request_no, idempotency_key, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'claimed', $14, $15, $16, NULL, NULL, $17, $18, $19, $20)

"#,
    )
    .bind(coupon_id)
    // `promotion_user_coupon.uuid` is NOT NULL without a database default, so
    // the claim insert must supply it; omitting it aborted every claim.
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(claim_coupon_no(command))
    .bind(&promotion.stock_id)
    .bind(code_id)
    .bind(&promotion.offer_id)
    .bind(&promotion.offer_version_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&command.owner_user_id)
    .bind(&coupon_code)
    .bind(now)
    .bind(valid_from_iso(now))
    .bind(expires_at)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to issue claimed user coupon", error))?;
    Ok(())
}

async fn insert_claim_coupon_ledger_entry(
    tx: &mut Transaction<'_, Postgres>,
    command: &ClaimPromotionUserCouponCommand,
    promotion: &ClaimPromotion,
    coupon_id: &str,
    coupon_ledger_entry_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    // Stock availability after this claim's decrement, matching the redeem
    // path: `available_quantity - 1` clamped at 0. A claim against an
    // `unlimited` stock leaves `available_quantity` unchanged (see
    // `update_claim_promotion_counters`); the ledger value stays aligned with
    // the redeem path's `- 1` instead of reporting an unchanged balance.
    let balance_after = (promotion.available_quantity - 1).max(0);
    sqlx::query(
        r#"

        INSERT INTO promotion_coupon_ledger_entry
            (id, uuid, tenant_id, stock_id, user_coupon_id, offer_id,
             subject_type, subject_id, direction, quantity_delta, balance_after,
             business_type, business_no, request_no, idempotency_key,
             source_type, source_id, trace_id, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, 'debit', -1, $9,
             'claim', $10, $11, $12, $13, $14, '', $15)

"#,
    )
    .bind(coupon_ledger_entry_id)
    // `promotion_coupon_ledger_entry.uuid` is NOT NULL without a database
    // default, so every ledger writer must supply it.
    .bind(sdkwork_utils_rust::uuid())
    .bind(&command.tenant_id)
    .bind(&promotion.stock_id)
    .bind(coupon_id)
    .bind(&promotion.offer_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(balance_after)
    .bind(claim_coupon_ledger_no(command))
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(coupon_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record promotion claim ledger entry", error))?;
    Ok(())
}

async fn update_claim_promotion_counters(
    tx: &mut Transaction<'_, Postgres>,
    promotion: &ClaimPromotion,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let requires_stock_quantity = promotion.stock_type.trim().to_ascii_lowercase() != "unlimited";
    let requires_stock_quantity_flag = if requires_stock_quantity {
        1_i64
    } else {
        0_i64
    };
    let stock_update = sqlx::query(
        r#"

        UPDATE promotion_coupon_stock
        SET available_quantity = CASE
                WHEN $1 = 1 THEN available_quantity - 1
                ELSE available_quantity
            END,
            claimed_quantity = COALESCE(claimed_quantity, 0) + 1,
            updated_at = $2
        WHERE id = $3
          AND status = 'active'
          AND ($4 = 0 OR available_quantity > 0)
          AND COALESCE(claimed_quantity, 0) = $5
          AND ($6 IS NULL OR COALESCE(claimed_quantity, 0) < $7)

"#,
    )
    .bind(requires_stock_quantity_flag)
    .bind(now)
    .bind(&promotion.stock_id)
    .bind(requires_stock_quantity_flag)
    .bind(promotion.stock_claimed_quantity)
    .bind(promotion.total_quantity)
    .bind(promotion.total_quantity)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to update promotion claim stock counters", error))?;
    if stock_update.rows_affected() != 1 {
        return Err(CommerceServiceError::conflict(
            "promotion coupon stock was not updated atomically",
        ));
    }
    Ok(())
}

fn claim_request_hash(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&[
        "claim",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        &command.offer_id,
        &command.request_no,
    ])
}

fn claim_idempotency_id(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&[
        "idem",
        &command.tenant_id,
        PROMOTION_USER_COUPON_CLAIM_SCOPE,
        &command.idempotency_key,
    ])
}

fn claim_coupon_id(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&["claim-coupon", &command.tenant_id, &command.request_no])
}

fn claim_coupon_no(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&["claim-coupon-no", &command.tenant_id, &command.request_no])
}

fn claim_coupon_ledger_entry_id(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&[
        "promotion-coupon-claim-ledger-entry",
        &command.tenant_id,
        &command.request_no,
    ])
}

fn claim_coupon_ledger_no(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&[
        "promotion-coupon-claim-ledger",
        &command.tenant_id,
        &command.request_no,
    ])
}

fn issued_claim_coupon_code(command: &ClaimPromotionUserCouponCommand) -> String {
    stable_storage_id(&["CL", &command.request_no])
}

fn coupon_credit_points(discount_value: &str) -> Result<i64, CommerceServiceError> {
    let cents = money_cents(discount_value)?;
    if cents <= 0 {
        Ok(0)
    } else {
        Ok((cents / 10).max(1))
    }
}

pub(crate) fn money_cents(value: &str) -> Result<i64, CommerceServiceError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.starts_with('-') || normalized.starts_with('+') {
        return Err(CommerceServiceError::storage(format!(
            "invalid commerce money amount: {value}"
        )));
    }
    let mut parts = normalized.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.chars().all(|character| character.is_ascii_digit())
    {
        return Err(CommerceServiceError::storage(format!(
            "invalid commerce money amount: {value}"
        )));
    }
    let integer_value = integer.parse::<i64>().map_err(|_| {
        CommerceServiceError::storage(format!("invalid commerce money amount: {value}"))
    })?;
    let integer_cents = integer_value.checked_mul(100).ok_or_else(|| {
        CommerceServiceError::storage(format!("commerce money amount is too large: {value}"))
    })?;
    let fraction_cents = match fraction {
        Some(fraction) => {
            if fraction.is_empty()
                || fraction.len() > 2
                || !fraction.chars().all(|character| character.is_ascii_digit())
            {
                return Err(CommerceServiceError::storage(format!(
                    "invalid commerce money amount: {value}"
                )));
            }
            let padded = if fraction.len() == 1 {
                format!("{fraction}0")
            } else {
                fraction.to_string()
            };
            padded.parse::<i64>().map_err(|_| {
                CommerceServiceError::storage(format!("invalid commerce money amount: {value}"))
            })?
        }
        None => 0,
    };
    integer_cents.checked_add(fraction_cents).ok_or_else(|| {
        CommerceServiceError::storage(format!("commerce money amount is too large: {value}"))
    })
}

fn stored_money_minor_units(value: &str) -> Result<String, CommerceServiceError> {
    money_cents(value).map(|amount| amount.to_string())
}

fn contract_money_minor_units(value: &str) -> Result<String, CommerceServiceError> {
    let normalized = value.trim();
    if normalized.contains('.') {
        return stored_money_minor_units(normalized);
    }
    if normalized.is_empty()
        || normalized
            .chars()
            .any(|character| !character.is_ascii_digit())
    {
        return Err(CommerceServiceError::storage(format!(
            "invalid commerce money minor-unit amount: {value}"
        )));
    }
    Ok(normalized.to_string())
}

fn checked_points_add(left: i64, right: i64) -> Result<i64, CommerceServiceError> {
    left.checked_add(right)
        .ok_or_else(|| CommerceServiceError::storage("promotion points balance overflow"))
}

fn promotion_requires_stock_quantity(promotion: &RedeemPromotion) -> bool {
    promotion.stock_type.trim().to_ascii_lowercase() != "unlimited"
}

fn coupon_status_label(value: &str) -> Result<&'static str, CommerceServiceError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "redeemed" | "used" => Ok("success"),
        "claimable" | "claimed" | "issued" | "active" | "draft" => Ok("pending"),
        "expired" | "disabled" | "voided" | "cancelled" => Ok("failed"),
        status => Err(CommerceServiceError::storage(format!(
            "unsupported billing coupon status: {status}"
        ))),
    }
}

fn points_direction(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "credit" => "in",
        "debit" => "out",
        _ => "unknown",
    }
}

fn points_business_type(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "redeem" => "redeem",
        "recharge" => "recharge",
        "transfer" => "transfer",
        "exchange" => "exchange",
        _ => "adjustment",
    }
}

fn points_to_money_string(points: i64) -> String {
    let cents = i128::from(points) * 10;
    format!("{}.{:02}", cents / 100, cents % 100)
}

fn points_to_minor_units_string(points: i64) -> String {
    (i128::from(points) * 10).to_string()
}

pub(crate) fn stable_storage_id(parts: &[&str]) -> String {
    // 用 ':' 作为 join 分隔符：sanitization 只保留字母数字与 '-_.'，
    // ':' 会被替换为 '-'，因此清理后的各部分永远不会包含 ':'，join 无歧义、零碰撞。
    parts
        .iter()
        .map(|part| {
            part.chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                        character
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(":")
}

fn account_id(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&[
        "account",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        "points",
        POINTS_CURRENCY_CODE,
    ])
}

fn coupon_id(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&["coupon", &command.tenant_id, &command.request_no])
}

fn coupon_no(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&["coupon-no", &command.tenant_id, &command.request_no])
}

fn coupon_ledger_entry_id(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&[
        "promotion-coupon-ledger-entry",
        &command.tenant_id,
        &command.request_no,
    ])
}

fn coupon_ledger_no(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&[
        "promotion-coupon-ledger",
        &command.tenant_id,
        &command.request_no,
    ])
}

fn ledger_entry_id(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&["ledger", &command.tenant_id, &command.request_no])
}

fn issued_coupon_code(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&["CP", &command.request_no])
}

fn redeem_idempotency_id(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&[
        "idem",
        &command.tenant_id,
        PROMOTION_CODE_REDEMPTION_SCOPE,
        &command.idempotency_key,
    ])
}

fn redeem_request_hash(command: &PromotionCodeRedemptionCommand) -> String {
    stable_storage_id(&[
        "redeem",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        &command.code,
        &command.request_no,
    ])
}

fn optional_string_cell(row: &sqlx::postgres::PgRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column).ok().flatten()
}

fn string_cell(row: &sqlx::postgres::PgRow, column: &str) -> String {
    optional_string_cell(row, column).unwrap_or_default()
}

fn required_status_cell(
    row: &sqlx::postgres::PgRow,
    column: &str,
    source: &str,
) -> Result<String, CommerceServiceError> {
    let value = string_cell(row, column);
    if value.trim().is_empty() {
        Err(missing_billing_status_error(source))
    } else {
        Ok(value)
    }
}

fn missing_billing_status_error(source: &str) -> CommerceServiceError {
    let message = match source {
        "redeem" => "missing billing redeem status from database row".to_owned(),
        source => format!("missing billing {source} status from database row"),
    };
    CommerceServiceError::storage(message)
}

fn integer_cell(row: &sqlx::postgres::PgRow, column: &str) -> i64 {
    row.try_get::<i64, _>(column)
        .or_else(|_| row.try_get::<i32, _>(column).map(i64::from))
        .unwrap_or(0)
}

fn optional_integer_cell(row: &sqlx::postgres::PgRow, column: &str) -> Option<i64> {
    row.try_get::<Option<i64>, _>(column)
        .or_else(|_| {
            row.try_get::<Option<i32>, _>(column)
                .map(|value| value.map(i64::from))
        })
        .ok()
        .flatten()
}

fn store_error(context: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("{context}: {error}"))
}

fn current_timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    format_unix_timestamp(seconds)
}

/// 幂等锁的有效期（秒）。持有锁的请求必须在此时间内完成，否则视为崩溃残留，允许后续请求抢占。
const IDEMPOTENCY_LOCK_TTL_SECONDS: i64 = 60;

/// 幂等记录的整体过期时间（秒）。超过此时间的记录视为历史遗留，可被清理。
const IDEMPOTENCY_RECORD_TTL_SECONDS: i64 = 86_400;

/// 基于当前时间戳计算锁过期时间字符串。
fn lock_expiry_timestamp(now: &str) -> String {
    timestamp_after_seconds(now, IDEMPOTENCY_LOCK_TTL_SECONDS)
}

/// 基于当前时间戳计算记录过期时间字符串。
fn record_expiry_timestamp(now: &str) -> String {
    timestamp_after_seconds(now, IDEMPOTENCY_RECORD_TTL_SECONDS)
}

/// 将 "YYYY-MM-DD HH:MM:SS" 格式时间戳加指定秒数，返回同格式字符串。
fn timestamp_after_seconds(now: &str, seconds: i64) -> String {
    let total = parse_unix_timestamp(now).saturating_add(seconds);
    format_unix_timestamp(total)
}

/// 将 "YYYY-MM-DD HH:MM:SS" 格式时间戳解析为 unix 秒。
fn parse_unix_timestamp(value: &str) -> i64 {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<i64> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_parts: Vec<i64> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return 0;
    }
    let days = days_from_civil(date_parts[0], date_parts[1], date_parts[2]);
    let secs = days * 86_400 + time_parts[0] * 3_600 + time_parts[1] * 60 + time_parts[2];
    secs
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn format_unix_timestamp(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_card_dispatch_uses_atomic_guards_and_idempotency_constraints() {
        let source = include_str!("postgres_promotion.rs");
        // 消耗扣减必须带总额度原子守卫
        assert!(source.contains("UPDATE promotion_member_card"));
        assert!(source.contains("total_used + $3 <= total_quota"));
        // 消耗流水必须带幂等键唯一约束
        assert!(source.contains("INSERT INTO promotion_member_card_consumption"));
        assert!(source.contains("idempotency_key"));
        // 开卡必须按券幂等（ON CONFLICT (tenant_id, user_coupon_id)）
        assert!(source.contains("ON CONFLICT (tenant_id, user_coupon_id) DO NOTHING"));
        // 兑换分发必须解析权益规则
        assert!(source.contains("credit_coupon_benefit_in_tx"));
        assert!(source.contains("grant_member_card_in_tx"));
        // 每日限额按 UTC 自然日聚合
        assert!(source.contains("date_trunc('day', CURRENT_TIMESTAMP AT TIME ZONE 'UTC')"));
        // 生命周期扫描：transaction-scoped advisory lock 防多实例并发
        assert!(source.contains("pg_try_advisory_xact_lock"));
        assert!(source.contains("activate_due_member_cards"));
        assert!(source.contains("expire_due_member_cards"));
        // 排期开卡：scheduled 状态由 starts_at 决定
        assert!(source.contains("scheduled"));
        assert!(source.contains("starts_at_value"));
        // 兑换预览：不落库的preview 链路
        assert!(source.contains("preview_promotion_code"));
        assert!(source.contains("map_redemption_preview"));
        // 兑换响应携带券标识
        assert!(source.contains("with_coupon"));
        assert!(source.contains("issued_coupon_code"));
        // 每人每码限领排除失效券（与claim 口径一致：大小写归一）
        assert!(source.contains("LOWER(COALESCE(status, '')) NOT IN ('expired', 'disabled', 'voided', 'cancelled')"));
        // 限领上限取库存 per_user_limit，而不是硬编码 1
        assert!(source.contains("received_count >= promotion.per_user_limit.max(1)"));
    }

    #[test]
    fn promotion_window_checks_compare_instants_not_text() {
        let source = include_str!("postgres_promotion.rs");
        // Regression: promotion rows carry two TEXT timestamp shapes — RFC 3339
        // from the admin console (`2026-11-10T09:12:00.000Z`) and the canonical
        // server form (`2026-11-10 09:12:44`). Text ordering across them is not
        // chronological ('T' > ' '), so a same-day `starts_at` made every code
        // unredeemable with `409 promotion code is invalid or unavailable`.
        // Every window bound must therefore compare parsed instants.
        //
        // The needles are assembled at runtime so this test cannot match its own
        // source text.
        let legacy_comparisons: [(&str, &str); 8] = [
            ("pc.starts_at", "<= $4"),
            ("pc.expires_at", ">= $4"),
            ("s.claim_starts_at", "<= $5"),
            ("s.claim_ends_at", ">= $6"),
            ("o.starts_at", "<= $7"),
            ("o.ends_at", ">= $8"),
            ("AND starts_at", "<= $5"),
            ("AND expires_at", ">= $6"),
        ];
        for (column, comparison) in legacy_comparisons {
            let needle = format!("{column} {comparison}");
            assert!(
                !source.contains(needle.as_str()),
                "text window comparison came back: {needle}"
            );
        }
        let legacy_clock_comparison = format!("{} TO_CHAR(CURRENT_TIMESTAMP", "starts_at <=");
        assert!(!source.contains(legacy_clock_comparison.as_str()));
        assert!(source.contains("::timestamptz"));
        assert!(source.contains("<= NOW()"));
        assert!(source.contains(">= NOW()"));
        // The submitted code is compared case-insensitively so a lower-case
        // paste redeems the same code the admin console printed in upper case.
        assert!(source.contains("UPPER(BTRIM(pc.promotion_code))"));
    }

    fn postgres_promotion_redeem_updates_use_atomic_guards() {
        let source = include_str!("postgres_promotion.rs");
        let stock_update = source
            .split("UPDATE promotion_coupon_stock")
            .nth(1)
            .expect("promotion stock update");
        let account_update = source
            .split("UPDATE commerce_account")
            .nth(1)
            .expect("commerce account update");

        assert!(stock_update.contains("available_quantity > 0"));
        assert!(stock_update.contains("stock_update.rows_affected() != 1"));
        assert!(source.contains("code_update.rows_affected() != 1"));
        assert!(account_update.contains("available_amount::bigint = $4"));
        assert!(source.contains("account_update.rows_affected() != 1"));
    }

    /// First `( ... )` column list of the first INSERT in `body`.
    fn insert_columns<'a>(body: &'a str, table: &str) -> &'a str {
        let table_marker = format!("INSERT INTO {table}");
        let insert = body
            .find(&table_marker)
            .unwrap_or_else(|| panic!("missing INSERT INTO {table}"));
        let rest = &body[insert..];
        let open = rest.find('(').expect("INSERT column list must open");
        let close = rest[open..]
            .find(')')
            .map(|offset| open + offset)
            .expect("INSERT column list must close");
        &rest[open + 1..close]
    }

    #[test]
    fn claim_ledger_insert_uses_only_real_table_columns() {
        let source = include_str!("postgres_promotion.rs");
        let start = source
            .find("async fn insert_claim_coupon_ledger_entry")
            .expect("claim ledger helper");
        let rest = &source[start..];
        let end = rest.find("\nasync fn ").unwrap_or(rest.len());
        let body = &rest[..end];
        let columns = insert_columns(body, "promotion_coupon_ledger_entry");

        // The real column set of `promotion_coupon_ledger_entry` (baseline DDL
        // 0001, table at `promotion_coupon_ledger_entry`): every NOT NULL
        // column the claim path needs must be named explicitly.
        for column in [
            "id",
            "uuid",
            "tenant_id",
            "stock_id",
            "user_coupon_id",
            "offer_id",
            "subject_type",
            "subject_id",
            "direction",
            "quantity_delta",
            "balance_after",
            "business_type",
            "business_no",
            "request_no",
            "idempotency_key",
            "source_type",
            "source_id",
            "trace_id",
            "created_at",
        ] {
            assert!(
                columns.split(',').any(|name| name.trim() == column),
                "claim ledger INSERT is missing real column `{column}`: {columns}"
            );
        }
        // Columns that only existed in the broken statement: the table does not
        // have them, so naming them aborted every claim with `undefined_column`.
        assert!(!columns.contains("ledger_no"), "unknown column ledger_no: {columns}");
        assert!(!columns.contains("updated_at"), "unknown column updated_at: {columns}");
        assert!(!columns.contains("organization_id"), "unknown column organization_id: {columns}");

        assert!(body.contains(".bind(sdkwork_utils_rust::uuid())"));
        assert!(body.contains(".bind(claim_coupon_ledger_no(command))"));
        assert!(body.contains("'claim'"));
        assert!(body.contains("'debit'"));
    }

    #[test]
    fn organization_binding_maps_absent_organization_to_platform_sentinel() {
        // `organization_id` columns are TEXT NOT NULL DEFAULT '0'; the read
        // predicates treat the same '0' as platform scope.
        assert_eq!(organization_binding(None), "0");
        assert_eq!(organization_binding(Some("org-1")), "org-1");
        assert_eq!(PLATFORM_ORGANIZATION_ID, "0");
    }

    #[test]
    fn inserts_never_bind_a_null_organization() {
        let source = include_str!("postgres_promotion.rs");
        // Assemble the needles at runtime so this test cannot match its own
        // source text.
        let command_bind = [".bind(command.", "organization_id.as_deref())"].concat();
        let subject_bind = [".bind(subject.", "organization_id)"].concat();
        for chunk in source.split("INSERT INTO ").skip(1) {
            // Each INSERT statement plus its bind chain ends at `.execute(`.
            let statement = chunk.split(".execute(").next().unwrap_or(chunk);
            assert!(
                !statement.contains(&command_bind),
                "INSERT binds a nullable organization directly; use organization_binding(...): {statement}"
            );
            assert!(
                !statement.contains(&subject_bind),
                "INSERT binds a nullable organization directly; use organization_binding(...): {statement}"
            );
        }
    }

    #[test]
    fn discount_application_insert_supplies_uuid() {
        // Additional defect found while fixing the org sentinel:
        // `promotion_discount_application.uuid` is NOT NULL without a default,
        // so the apply insert must supply it like the claim/redeem inserts do.
        let source = include_str!("postgres_promotion.rs");
        let start = source
            .find("async fn insert_discount_application")
            .expect("discount application insert");
        let rest = &source[start..];
        let body = &rest[..rest.find("\nasync fn ").unwrap_or(rest.len())];
        assert!(body.contains("INSERT INTO promotion_discount_application"));
        assert!(body.contains(".bind(sdkwork_utils_rust::uuid())"));
        // `discount_type` is NOT NULL without a default as well: it must be
        // copied from the pinned offer version, not defaulted or omitted.
        assert!(body.contains("discount_type,"));
        assert!(body.contains(".bind(&coupon.discount_type)"));
        // ... and the loader must actually select it.
        let load_start = source
            .find("async fn load_user_coupon_for_discount_apply")
            .expect("discount apply coupon loader");
        let load_rest = &source[load_start..];
        let load_body = &load_rest[..load_rest.find("\nasync fn ").unwrap_or(load_rest.len())];
        assert!(load_body.contains("CAST(v.discount_type AS TEXT) AS discount_type"));
    }

    #[test]
    fn an_order_rejects_a_second_non_combinable_coupon() {
        // Regression: `promotion_offer.combinability` was decorative, so N
        // distinct coupons could apply to one order and the order was
        // discounted N times.
        let source = include_str!("postgres_promotion.rs");
        let apply_start = source
            .find("pub async fn apply_promotion_discount")
            .expect("apply entrypoint");
        let apply_rest = &source[apply_start..];
        let apply_body = &apply_rest[..apply_rest.find("\n    pub async fn ").unwrap_or(apply_rest.len())];
        assert!(apply_body.contains("ensure_order_has_no_conflicting_application"));

        let guard_start = source
            .find("async fn ensure_order_has_no_conflicting_application")
            .expect("combinability guard");
        let guard_rest = &source[guard_start..];
        let guard = &guard_rest[..guard_rest.find("\nfn ").unwrap_or(guard_rest.len())];
        // Only active applications conflict; historical/rolled-back rows do not.
        assert!(guard.contains("IN ('applied', 'settled')"));
        // Both sides must be COMBINABLE for the stack to be allowed.
        assert!(guard.contains("\"COMBINABLE\""));
        assert!(guard.contains("cannot be combined"));
    }

    #[test]
    fn lifecycle_sweep_uses_transaction_scoped_advisory_lock() {
        // Regression: a session-scoped `pg_try_advisory_lock` taken on a pooled
        // connection leaked whenever a sub-scan returned `?`, after which every
        // later sweep reported `skipped: true` and cards never activated or
        // expired. The transaction-scoped lock is released on commit/rollback.
        let source = include_str!("postgres_promotion.rs");
        let start = source
            .find("pub async fn run_member_card_lifecycle_sweep")
            .expect("lifecycle sweep");
        let rest = &source[start..];
        let end = rest.find("\n    ///").unwrap_or(rest.len());
        let body = &rest[..end];

        assert!(body.contains("pg_try_advisory_xact_lock"));
        assert!(!body.contains("pg_try_advisory_lock"));
        assert!(!body.contains("pg_advisory_unlock"));
        assert!(!body.contains("self.pool.acquire()"));
        assert!(body.contains("let mut tx = self.pool.begin()"));
        assert!(body.contains("activate_due_member_cards_in_tx(&mut tx)"));
        assert!(body.contains("expire_due_member_cards_in_tx(&mut tx)"));
        assert!(body.contains("tx.commit()"));
    }

    #[test]
    fn promotion_lifecycle_sweep_is_one_instant_update_per_entity_with_its_own_lock() {
        let source = include_str!("postgres_promotion.rs");

        // The sweep takes the transaction-scoped lock under its own key. The
        // member-card key must stay a different literal: a shared key would let
        // one sweep in one instance silently skip the other.
        assert!(source.contains("MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_410;"));
        assert!(source.contains("PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_420;"));
        let start = source
            .find("pub async fn run_promotion_lifecycle_sweep")
            .expect("promotion lifecycle sweep");
        let rest = &source[start..];
        let body = &rest[..rest.find("\n    ///").unwrap_or(rest.len())];
        assert!(body.contains("pg_try_advisory_xact_lock"));
        // Transaction-scoped: no session lock can leak on an early `?` return.
        assert!(!body.contains("pg_try_advisory_lock"));
        assert!(!body.contains("pg_advisory_unlock"));
        assert!(body.contains("let mut tx = self.pool.begin()"));
        assert!(body.contains("PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY"));
        assert!(body.contains("expire_due_user_coupons_in_tx(&mut tx)"));
        assert!(body.contains("expire_due_codes_in_tx(&mut tx)"));
        assert!(body.contains("skipped: true"));
        assert!(body.contains("tx.commit()"));

        // Each entity is one `UPDATE` with an instant predicate — never a per-row
        // loop and never a text comparison of `expires_at`.
        for (marker, table, status_predicate) in [
            (
                "async fn expire_due_user_coupons_in_tx",
                "UPDATE promotion_user_coupon",
                "status IN ('claimed', 'active', 'pending')",
            ),
            (
                "async fn expire_due_codes_in_tx",
                "UPDATE promotion_code",
                "status = 'active'",
            ),
        ] {
            let start = source
                .find(marker)
                .unwrap_or_else(|| panic!("missing {marker}"));
            let rest = &source[start..];
            let body = &rest[..rest.find("\nasync fn ").unwrap_or(rest.len())];
            assert_eq!(body.matches("UPDATE ").count(), 1, "{marker}: one statement");
            assert!(body.contains(table), "{marker}");
            assert!(body.contains(status_predicate), "{marker}");
            assert!(
                body.contains("timestamps::instant(\"expires_at\")"),
                "{marker} must compare parsed instants"
            );
            assert!(body.contains("< NOW()"), "{marker}");
            assert!(!body.contains("for row in"), "{marker}: no per-row loop");
            assert!(!body.contains("fetch_all"), "{marker}: no per-row fetch");
            // The claim guard keeps already-issued codes out of the code sweep.
            if marker.contains("codes") {
                assert!(body.contains("COALESCE(claimed_quantity, 0) = 0"), "{marker}");
            }
        }

        // The instant expression itself is the only place the text shapes are
        // normalized; both accepted shapes parse to a `timestamptz`.
        let timestamps = include_str!("timestamps.rs");
        assert!(timestamps.contains("::timestamptz"));
        assert!(timestamps.contains("AT TIME ZONE 'UTC'"));
    }

    #[test]
    fn member_card_consumption_checks_the_guarded_update() {
        // `total_used + $3 <= total_quota` is an UPDATE predicate, so the only
        // signal that the guard rejected the debit is `rows_affected()`.
        let source = include_str!("postgres_promotion.rs");
        let start = source
            .find("pub async fn consume_member_card")
            .expect("member card consumption");
        let rest = &source[start..];
        let body = &rest[..rest.find("\n    pub async fn ").unwrap_or(rest.len())];
        assert!(body.contains("total_used + $3 <= total_quota"));
        assert!(body.contains("card_update.rows_affected() != 1"));
    }

    /// Slice of `source` from `start` to the next `end` (or EOF).
    fn section_until<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let at = source
            .find(start)
            .unwrap_or_else(|| panic!("missing source marker: {start}"));
        let rest = &source[at..];
        let stop = rest.find(end).unwrap_or(rest.len());
        &rest[..stop]
    }

    #[test]
    fn budget_reservation_guards_the_available_pool_atomically() {
        // The reserve guard is one atomic UPDATE: the pool check, the move and
        // the `version` bump either all happen or none of them do. A
        // `fetch_optional` that returns no row IS the documented 409.
        let source = include_str!("postgres_promotion.rs");
        let reserve = section_until(
            source,
            "async fn reserve_coupon_budget",
            "async fn insert_budget_reserve_ledger_entry",
        );
        assert!(reserve.contains("UPDATE promotion_budget_account"));
        assert!(reserve.contains("available_amount_minor = available_amount_minor - $1"));
        assert!(reserve.contains("reserved_amount_minor = reserved_amount_minor + $1"));
        assert!(reserve.contains("version = version + 1"));
        assert!(reserve.contains("AND status = 'active'"));
        // The exact guard the pure `budget_reserve_decision` helper mirrors.
        assert!(reserve.contains("AND available_amount_minor >= $1"));
        assert!(reserve.contains(
            "RETURNING available_amount_minor, reserved_amount_minor, used_amount_minor, currency_code"
        ));
        assert!(reserve.contains("coupon budget has been exhausted"));
        assert!(reserve.contains("fetch_optional"));
        // `.ok_or_else` turns the empty result into the 409; the transaction is
        // dropped, so the coupon is not marked applied either.
        assert!(reserve.contains(".ok_or_else(|| CommerceServiceError::conflict("));

        // No budget account (most coupons today): unchanged behaviour, no write.
        assert!(reserve
            .contains("let Some(account_id) = coupon.budget_account_id.as_deref() else"));
        assert!(reserve.contains("return Ok(None);"));

        // Money unit rule: the caller hands in an already-computed minor-unit
        // amount; the reserve never re-derives it from the coupon face value.
        assert!(reserve.contains("amount_minor: i64,"));
        assert!(!reserve.contains("coupon.discount_value"));
        assert!(reserve.contains(".bind(amount_minor)"));
        // ... and the reservation deadline is `now + 24h`.
        assert!(reserve.contains("timestamp_after_seconds(now, RESERVATION_TTL_SECONDS)"));
        assert_eq!(RESERVATION_TTL_SECONDS, 86_400);

        // The apply flow reserves before it writes the application row, so a
        // budget rejection can never leave an `applied` application behind.
        let apply = section_until(
            source,
            "pub async fn apply_promotion_discount",
            "\n    pub async fn reverse_promotion_discount",
        );
        let reserve_at = apply
            .find("reserve_coupon_budget(")
            .expect("apply must reserve the coupon budget");
        let insert_at = apply
            .find("insert_discount_application(")
            .expect("apply must insert the application row");
        assert!(
            reserve_at < insert_at,
            "the budget guard must run before the application row is written"
        );

        // The application row stores the account link and the deadline; both are
        // NULL when the stock has no cap.
        let insert = section_until(
            source,
            "async fn insert_discount_application",
            "\nasync fn mark_user_coupon_applied",
        );
        assert!(insert.contains("budget_account_id, reservation_expires_at"));
        assert!(insert.contains("reservation.map(|reservation| reservation.account_id.as_str())"));
        assert!(insert.contains("reservation.map(|reservation| reservation.expires_at.as_str())"));

        // The coupon loader joins the account through the wallet coupon's stock.
        let loader = section_until(
            source,
            "async fn load_user_coupon_for_discount_apply",
            "async fn load_user_coupon_for_discount_reverse",
        );
        assert!(loader.contains("LEFT JOIN promotion_budget_account b"));
        assert!(loader.contains("AND b.stock_id = c.stock_id"));
        assert!(loader.contains("b.id AS budget_account_id"));
    }

    #[test]
    fn apply_writes_the_computed_discount_not_the_coupon_face_value() {
        // Regression: apply copied `discount_value` into
        // `promotion_discount_application.discount_amount`, so a `PERCENTAGE`
        // coupon stored its percentage number as money.
        let source = include_str!("postgres_promotion.rs");
        let apply = section_until(
            source,
            "pub async fn apply_promotion_discount",
            "\n    pub async fn reverse_promotion_discount",
        );
        // The amount is computed from the pinned version's terms and the order
        // total before anything is reserved or written.
        assert!(apply.contains("compute_discount_amount("));
        assert!(apply.contains("&coupon.discount_type"));
        assert!(apply.contains("&coupon.discount_value"));
        assert!(apply.contains("&coupon.minimum_amount"));
        assert!(apply.contains("coupon.maximum_discount_amount.as_deref()"));
        // The order total arrives in canonical minor units from the breakdown
        // row, so it is passed through without a second major→minor parsing.
        assert!(apply.contains("order.total_amount_minor"));
        assert!(!apply.contains("money_cents(&order.total_amount"));
        // The application stores integer minor units, matching the order money
        // columns and the admin console's minor-unit rendering.
        assert!(apply.contains("discount_amount_minor.to_string()"));
        assert!(!apply.contains("minor_to_major_decimal"));
        let compute_at = apply
            .find("compute_discount_amount(")
            .expect("apply must compute the discount");
        let reserve_at = apply
            .find("reserve_coupon_budget(")
            .expect("apply must reserve the coupon budget");
        assert!(
            compute_at < reserve_at,
            "the computed amount must exist before the budget is reserved"
        );

        // The reserve consumes the computed minor amount.
        let reserve = section_until(
            source,
            "async fn reserve_coupon_budget",
            "async fn insert_budget_reserve_ledger_entry",
        );
        assert!(reserve.contains("amount_minor: i64,"));
        assert!(!reserve.contains("coupon.discount_value"));

        // The application row writes the computed decimal-major amount, never
        // the coupon face value.
        let insert = section_until(
            source,
            "async fn insert_discount_application",
            "\nasync fn mark_user_coupon_applied",
        );
        assert!(insert.contains("discount_amount: &str"));
        assert!(insert.contains(".bind(discount_amount)"));
        assert!(!insert.contains(".bind(&coupon.discount_value)"));
        assert!(!insert.contains("coupon.discount_value"));
    }

    #[test]
    fn the_order_total_comes_from_the_amount_breakdown_row() {
        // A discount cannot be computed without the order total, so the loader
        // joins the order-level breakdown row and rejects its absence.
        let source = include_str!("postgres_promotion.rs");
        let loader = section_until(
            source,
            "async fn load_order_for_discount_apply",
            "\n/// Reserves the application amount",
        );
        assert!(loader.contains("commerce_order_amount_breakdown"));
        assert!(loader.contains("allocation_type = 'order_total'"));
        assert!(loader.contains("ORDER BY id"));
        assert!(loader.contains("LIMIT 1"));
        assert!(loader.contains("CAST(b.original_amount AS TEXT) AS total_amount"));
        assert!(loader.contains("order amount is not available for discount application"));
        // The existing order status guard and tenant / organization / owner
        // scoping are unchanged.
        assert!(loader.contains("'draft', 'pending', 'pending_payment', 'unpaid', 'wait_pay'"));
        assert!(loader.contains("o.organization_id = CAST($2 AS TEXT)"));
        assert!(loader.contains("o.owner_user_id = CAST($5 AS TEXT)"));
        // A breakdown whose currency disagrees with the order is a conflict.
        assert!(loader.contains("b.currency_code"));
        assert!(loader.contains(
            "ensure_discount_currencies_match(&currency_code, &breakdown_currency_code)"
        ));
        // The loaded struct carries the total through to the computation.
        let order = section_until(
            source,
            "struct DiscountApplyOrder",
            "fn apply_discount_request_hash",
        );
        assert!(order.contains("total_amount_minor: i64"));
    }

    #[test]
    fn discount_currency_mismatch_is_rejected() {
        // Both present and equal, or either side absent: no conflict.
        assert!(ensure_discount_currencies_match("CNY", "CNY").is_ok());
        assert!(ensure_discount_currencies_match("CNY", "").is_ok());
        assert!(ensure_discount_currencies_match("", "USD").is_ok());
        assert!(ensure_discount_currencies_match("", "").is_ok());
        // Two different currencies conflict before any money is computed.
        let error = ensure_discount_currencies_match("CNY", "USD")
            .expect_err("a mismatched breakdown currency must be rejected");
        assert_eq!(
            error.message(),
            "order currency does not match the discount currency"
        );
        // ... and the loader is what calls it.
        let source = include_str!("postgres_promotion.rs");
        let loader = section_until(
            source,
            "async fn load_order_for_discount_apply",
            "\n/// Reserves the application amount",
        );
        assert!(loader.contains(
            "ensure_discount_currencies_match(&currency_code, &breakdown_currency_code)"
        ));
    }

    #[test]
    fn reversal_releases_budget_after_the_status_guard_inside_the_same_transaction() {
        let source = include_str!("postgres_promotion.rs");

        // The whole reversal runs on one transaction: the entrypoint begins it
        // once and hands the same handle to the application reversal (which moves
        // the status and then the money) — the extra `tx.commit()` in the body is
        // the replayed-idempotency-key early return, which never reaches the
        // reversal at all.
        let entry = section_until(
            source,
            "pub async fn reverse_promotion_discount",
            "\n    async fn find_applied_promotion_coupon",
        );
        assert_eq!(entry.matches("self.pool.begin()").count(), 1);
        assert!(entry.contains("reverse_discount_application(&mut tx, &command, &now)"));
        assert!(entry.contains("restore_user_coupon_after_reverse(&mut tx, &command, &now)"));

        // The reversal body locks the application row, guards the status, moves
        // the status, and only then touches the budget — in that order.
        let reversal = section_until(
            source,
            "async fn reverse_discount_application",
            "\nasync fn load_reverse_reserve_amount",
        );
        assert!(reversal.contains("tx: &mut Transaction<'_, Postgres>"));
        assert!(reversal.contains("FOR UPDATE"));
        let guard_at = reversal
            .find("reverse_status_decision(&status)")
            .expect("the reversal must guard the status");
        let transition_at = reversal
            .find("UPDATE promotion_discount_application")
            .expect("the reversal must move the application to rolled_back");
        let release_at = reversal
            .find("release_reversed_budget_reservation(")
            .expect("the reversal must release the reservation");
        assert!(
            guard_at < transition_at,
            "the status guard must run before the status transition"
        );
        assert!(
            transition_at < release_at,
            "the budget release must run after the status transition guard"
        );
        // The transition keeps the live-status predicate and requires exactly
        // one row, so an already-rolled_back application stops before money moves.
        assert!(reversal.contains("LOWER(COALESCE(status, '')) IN ('applied', 'settled')"));
        assert!(reversal.contains("if updated != 1"));
        assert!(reversal.contains("let Some(account_id) = optional_string_cell(&row, \"budget_account_id\") else"));
        assert!(reversal.contains("load_reverse_reserve_amount("));

        // Terminal states are refused before any money moves.
        assert!(reverse_status_decision("applied").is_ok());
        assert!(reverse_status_decision("settled").is_ok());
        for terminal in ["rolled_back", "released", "expired", ""] {
            assert!(
                reverse_status_decision(terminal).is_err(),
                "{terminal:?} must not be reversible"
            );
        }

        // The money move is one guarded UPDATE; a zero-row result is a conflict,
        // and the ledger + audit evidence share the same transaction handle.
        let release = section_until(
            source,
            "async fn release_reversed_budget_reservation",
            "/// Appends the `release` ledger row",
        );
        assert!(release.contains("tx: &mut Transaction<'_, Postgres>"));
        assert!(release.contains("UPDATE promotion_budget_account"));
        assert!(release.contains("reserved_amount_minor = reserved_amount_minor - $1"));
        assert!(release.contains("available_amount_minor = available_amount_minor + $1"));
        assert!(release.contains("AND reserved_amount_minor >= $1"));
        assert!(release.contains("AND status = 'active'"));
        assert!(release.contains("budget reservation could not be released"));
        assert!(release.contains("insert_reversal_release_ledger_entry("));
        assert!(release.contains("insert_reversal_release_audit_event("));

        // No second transaction is opened anywhere in the release path.
        assert!(!release.contains("self.pool.begin()"));
        assert!(!release.contains("commit()"));
    }

    #[test]
    fn reversal_amount_prefers_the_reserve_ledger_and_falls_back_to_discount_amount() {
        // (reserve ledger amount, stored discount_amount, expected minor units)
        let cases: [(Option<i64>, &str, Option<i64>); 8] = [
            (Some(1_250), "99.99", Some(1_250)),
            (Some(1), "0.00", Some(1)),
            (None, "12.34", Some(1_234)),
            (None, "12.3", Some(1_230)),
            (None, "12", Some(1_200)),
            (None, "0.05", Some(5)),
            (None, "1.234", None),
            (None, "", None),
        ];
        for (ledger_amount, discount_amount, expected) in cases {
            let resolved = reversal_release_amount_minor(ledger_amount, discount_amount);
            match expected {
                Some(expected) => assert_eq!(
                    resolved.expect("valid reversal amount"),
                    expected,
                    "ledger={ledger_amount:?} discount_amount={discount_amount:?}"
                ),
                None => assert!(
                    resolved.is_err(),
                    "{discount_amount:?} must not become a silently rounded amount"
                ),
            }
        }

        // The fallback uses the same `reserve` lookup shape as the stale sweep.
        let source = include_str!("postgres_promotion.rs");
        let loader = section_until(
            source,
            "async fn load_reverse_reserve_amount",
            "/// Returns a reversed application's reservation",
        );
        assert!(loader.contains("FROM promotion_budget_ledger_entry"));
        assert!(loader.contains("AND direction = 'reserve'"));
        assert!(loader.contains("ORDER BY occurred_at DESC, id DESC"));
        assert!(loader.contains("LIMIT 1"));

        // ... and the decimal-major fallback is `money_cents`, never a float.
        let amount_helper = section_until(
            source,
            "fn reversal_release_amount_minor",
            "/// Reverses the live discount application",
        );
        assert!(amount_helper.contains("Some(amount_minor) => Ok(amount_minor)"));
        assert!(amount_helper.contains("money_cents(discount_amount)"));
    }

    #[test]
    fn reversal_persists_the_request_reason_as_rollback_reason() {
        let source = include_str!("postgres_promotion.rs");
        let reversal = section_until(
            source,
            "async fn reverse_discount_application",
            "\nasync fn load_reverse_reserve_amount",
        );
        // Today `reason` only feeds the request hash; the transition must also
        // persist it on the row.
        assert!(reversal.contains("rollback_reason = $2"));
        assert!(reversal.contains(".bind(command.reason.as_deref())"));
        assert!(reversal.contains("status = 'rolled_back'"));
        assert!(reversal.contains("rolled_back_at = $1"));
        let hash = section_until(
            source,
            "fn reverse_discount_request_hash",
            "fn discount_application_id",
        );
        assert!(hash.contains("command.reason.as_deref().unwrap_or(\"none\")"));
    }

    #[test]
    fn stale_reservation_sweep_is_set_based_with_its_own_lock() {
        let source = include_str!("postgres_promotion.rs");

        // Its own advisory-lock key, distinct from the member-card (`—410`) and
        // coupon (`—420`) sweeps: the three must be able to run concurrently.
        assert!(source.contains(
            "const PROMOTION_BUDGET_RESERVATION_SWEEP_LOCK_KEY: i64 = 71_091_238_430;"
        ));
        assert!(source.contains("const MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_410;"));
        assert!(source.contains("const PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_420;"));

        let entry = section_until(
            source,
            "pub async fn release_stale_discount_applications",
            "\n    pub async fn apply_promotion_discount",
        );
        assert!(entry.contains("pg_try_advisory_xact_lock"));
        // Transaction-scoped: no session lock can leak on an early `?` return.
        assert!(!entry.contains("pg_try_advisory_lock"));
        assert!(!entry.contains("pg_advisory_unlock"));
        assert!(entry.contains("PROMOTION_BUDGET_RESERVATION_SWEEP_LOCK_KEY"));
        assert!(entry.contains("let mut tx = self.pool.begin()"));
        assert!(entry.contains("skipped: true"));
        assert!(entry.contains("tx.commit()"));

        let sweep_start = ["async fn release_stale_discount_applications", "_in_tx"].concat();
        let audit_start = ["async fn insert_budget_release_expired", "_audit_event"].concat();
        let sweep = section_until(source, &sweep_start, &audit_start);
        // Set-based: no per-row loop and no per-row fetch. One application
        // UPDATE, one aggregate account UPDATE and one bulk ledger INSERT.
        assert!(!sweep.contains("for row in"), "no per-row loop");
        assert!(!sweep.contains("fetch_all"), "no per-row fetch");
        assert_eq!(sweep.matches("UPDATE ").count(), 2, "two set-based updates");
        assert_eq!(
            sweep.matches("INSERT INTO promotion_budget_ledger_entry").count(),
            1,
            "one bulk ledger insert"
        );
        assert!(sweep.contains("UPDATE promotion_discount_application a"));
        assert!(sweep.contains("RETURNING a.id, a.tenant_id, a.organization_id"));
        assert!(sweep.contains("SUM(amount_minor) AS total_amount_minor"));
        assert!(sweep.contains("GROUP BY budget_account_id"));
        assert!(sweep.contains("SELECT COUNT(*)::BIGINT FROM reservations"));
        // The window is an instant comparison, never text ordering.
        assert!(sweep.contains("timestamps::instant(\"a.reservation_expires_at\")"));
        assert!(sweep.contains("< NOW()"));
        assert!(sweep.contains("a.reservation_expires_at IS NOT NULL"));
        assert!(sweep.contains("WHERE a.status = 'applied'"));
        assert!(sweep.contains("release_reason = 'reservation_expired'"));
        assert!(sweep.contains("status = 'released'"));
        // The same instant predicate is the partial index's population.
        let migration = include_str!(
            "../../../database/migrations/postgres/0006_budget_and_settlement.up.sql"
        );
        assert!(migration.contains(
            "CREATE INDEX IF NOT EXISTS idx_promotion_discount_application_status_expires"
        ));
        assert!(migration.contains("ON promotion_discount_application (tenant_id, status, reservation_expires_at)"));
        assert!(migration.contains("WHERE status = 'applied';"));

        // One aggregated audit row carries the pass size.
        let audit = section_until(source, &audit_start, "/// advisory lock 键：会员卡生命周期扫描");
        assert_eq!(audit.matches("INSERT INTO promotion_audit_event").count(), 1);
        assert!(audit.contains("'budget.release_expired'"));
        assert!(audit.contains("$3"));
        assert!(sweep.contains("insert_budget_release_expired_audit_event(tx, released, &now)"));
        assert!(sweep.contains("if released > 0"));

        // The lifecycle worker runs it as an independent arm on the one timer.
        let host = include_str!("../../sdkwork-promotion-service-host/src/lib.rs");
        assert!(host.contains("store.release_stale_discount_applications().await"));
        assert!(host.contains("stale discount reservation sweep failed"));
    }

    #[test]
    fn budget_ledger_invariant_is_preserved_by_every_direction() {
        // Arithmetic mirror of the SQL predicates: `reserve` moves
        // available→reserved, `settle` reserved→used, `release`
        // reserved→available. Every step keeps the sum constant, so the table's
        // `chk_pba_amounts` invariant (`available + reserved + used <= total`)
        // holds and no bucket can go negative.
        #[derive(Clone, Copy)]
        struct Buckets {
            available: i64,
            reserved: i64,
            used: i64,
        }
        impl Buckets {
            fn sum(self) -> i64 {
                self.available + self.reserved + self.used
            }
            fn reserve(self, amount: i64) -> Self {
                // `WHERE available_amount_minor >= $amount`
                assert!(self.available >= amount, "reserve guard must reject");
                Self {
                    available: self.available - amount,
                    reserved: self.reserved + amount,
                    used: self.used,
                }
            }
            fn settle(self, amount: i64) -> Self {
                // `WHERE reserved_amount_minor >= $amount`
                assert!(self.reserved >= amount, "settle guard must reject");
                Self {
                    available: self.available,
                    reserved: self.reserved - amount,
                    used: self.used + amount,
                }
            }
            fn release(self, amount: i64) -> Self {
                assert!(self.reserved >= amount, "release guard must reject");
                Self {
                    available: self.available + amount,
                    reserved: self.reserved - amount,
                    used: self.used,
                }
            }
            fn assert_invariant(self, total: i64) {
                assert!(self.available >= 0 && self.reserved >= 0 && self.used >= 0);
                assert!(
                    self.sum() <= total,
                    "available + reserved + used must never exceed total"
                );
            }
        }

        let total = 1_500;
        let start = Buckets {
            available: total,
            reserved: 0,
            used: 0,
        };
        start.assert_invariant(total);
        let settled = start.reserve(500).settle(500);
        assert_eq!(
            settled.sum(),
            total,
            "settle moves money between buckets, it does not create or destroy it"
        );
        assert_eq!((settled.available, settled.reserved, settled.used), (1_000, 0, 500));
        settled.assert_invariant(total);

        let released = start.reserve(500).release(500);
        assert_eq!(released.sum(), total);
        assert_eq!(
            (released.available, released.reserved, released.used),
            (total, 0, 0),
            "release returns the whole reservation to the available pool"
        );
        released.assert_invariant(total);

        // Over-reserving, over-settling and over-releasing are all rejected by
        // the same `>= $amount` predicate the SQL carries.
        assert!(std::panic::catch_unwind(|| start.reserve(total + 1)).is_err());
        let reserved = start.reserve(500);
        assert!(std::panic::catch_unwind(|| reserved.settle(501)).is_err());
        assert!(std::panic::catch_unwind(|| reserved.release(501)).is_err());

        // The CHECK constraint text and the transfer predicates the arithmetic
        // mirrors.
        let migration = include_str!(
            "../../../database/migrations/postgres/0006_budget_and_settlement.up.sql"
        );
        assert!(migration.contains(
            "available_amount_minor + reserved_amount_minor + used_amount_minor <= total_amount_minor"
        ));
        assert!(migration.contains("CONSTRAINT chk_pba_amounts CHECK ("));
        let management = include_str!("promotion_admin_management.rs");
        let transfer = section_until(
            management,
            "async fn transfer_reserved_budget",
            "async fn insert_budget_ledger_entry",
        );
        assert!(transfer.contains("reserved_amount_minor = reserved_amount_minor - $1"));
        assert!(transfer.contains("used_amount_minor = used_amount_minor + $1"));
        assert!(transfer.contains("available_amount_minor = available_amount_minor + $1"));
        assert!(transfer.contains("AND reserved_amount_minor >= $1"));
    }

    #[test]
    fn settlement_money_conversion_uses_money_cents_and_never_a_float() {
        // The legacy offer-version money columns stay decimal-major strings; the
        // boundary conversion is `money_cents`, which yields integer minor units.
        assert_eq!(money_cents("12.34").expect("two decimals"), 1_234);
        assert_eq!(money_cents("12.3").expect("one decimal"), 1_230);
        assert_eq!(money_cents("12").expect("integer major"), 1_200);
        assert_eq!(money_cents("0.05").expect("five cents"), 5);
        // Rejected rather than silently rounded / divided.
        for rejected in ["", "-1.00", "+1.00", "1.234", "1e3", "1,00"] {
            assert!(
                money_cents(rejected).is_err(),
                "{rejected:?} must not become a silently rounded amount"
            );
        }

        // Source scan: the new budget/settlement code converts at the boundary
        // and contains no float type and no division by 100.
        let source = include_str!("postgres_promotion.rs");
        let sweep_start = ["async fn release_stale_discount_applications", "_in_tx"].concat();
        let audit_start = ["async fn insert_budget_release_expired", "_audit_event"].concat();
        let new_code = [
            section_until(
                source,
                "async fn reserve_coupon_budget",
                "async fn insert_budget_reserve_ledger_entry",
            ),
            section_until(
                source,
                "async fn insert_budget_reserve_ledger_entry",
                "async fn insert_discount_application",
            ),
            section_until(source, &sweep_start, &audit_start),
            section_until(source, &audit_start, "/// advisory lock 键：会员卡生命周期扫描"),
        ]
        .join("\n");
        // The application amount is handed to the reserve already converted, so
        // this budget/settlement code never re-derives it from the face value.
        assert!(!new_code.contains("coupon.discount_value"));
        assert!(new_code.contains(".bind(amount_minor)"));
        // Needles assembled at runtime so this test cannot match its own source.
        let forbidden = [
            ["f", "64"].concat(),
            ["f", "32"].concat(),
            ["/ ", "100"].concat(),
            ["as ", "f", "64"].concat(),
            ["100", ".0"].concat(),
        ];
        for needle in &forbidden {
            assert!(
                !new_code.contains(needle),
                "new money code must not contain {needle:?}"
            );
        }

        let management = include_str!("promotion_admin_management.rs");
        let new_code = [
            section_until(
                management,
                "fn application_amount_minor",
                "async fn transfer_reserved_budget",
            ),
            section_until(
                management,
                "async fn transfer_reserved_budget",
                "async fn insert_budget_ledger_entry",
            ),
            section_until(
                management,
                "async fn insert_budget_ledger_entry",
                "fn now_string",
            ),
        ]
        .join("\n");
        assert!(new_code.contains("crate::postgres_promotion::money_cents(&amount)"));
        let forbidden = [
            ["f", "64"].concat(),
            ["f", "32"].concat(),
            ["/ ", "100"].concat(),
            ["as ", "f", "64"].concat(),
            ["100", ".0"].concat(),
        ];
        for needle in &forbidden {
            assert!(
                !new_code.contains(needle),
                "new money code must not contain {needle:?}"
            );
        }

        // The whole repository crate is float-free on the money path.
        let crate_source = include_str!("postgres_promotion.rs");
        assert!(!crate_source.contains(&["f", "64"].concat()));
        assert!(!crate_source.contains(&["f", "32"].concat()));
    }

    /// The redemption lookup must be hash-first, with the documented
    /// normalized-text fallback reachable only when the hash probe found nothing.
    #[test]
    fn redeem_lookup_probes_the_indexed_code_hash_first() {
        let source = include_str!("postgres_promotion.rs");
        let primary = section_until(
            source,
            "async fn load_promotion_for_redeem(",
            "/// Hash-miss fallback",
        );

        // 1. The primary probe uses the indexed hash predicate. The predicate is
        // a constant so the SQL text and the test agree on one exact string.
        assert!(primary.contains("REDEEM_BY_HASH_PREDICATE"));
        let hash_sql = ["pc.", "code_hash =", " CAST($3 AS TEXT)"].concat();
        assert!(
            source.contains(hash_sql.as_str()),
            "the indexed hash equality predicate is missing"
        );

        // 2. The submitted code is normalized and hashed exactly once, and that
        // one identity feeds the primary lookup (`&identity.hash`); the fallback
        // repair reuses the same value rather than hashing again.
        let derive = ["let identity = ", "code_identity(&command.code);"].concat();
        assert_eq!(
            primary.matches(derive.as_str()).count(),
            1,
            "the submitted code must be hashed exactly once:\n{primary}"
        );
        assert!(primary.contains(".bind(&identity.hash)"));

        // 3. The pre-phase-2 predicate is not used by the primary probe. It is
        // documented as the fallback constant and only the fallback helper uses
        // it.
        let normalized = [
            "UPPER(BTRIM(pc.promotion_code))",
            " = UPPER(BTRIM(CAST($3 AS TEXT)))",
        ]
        .concat();
        assert!(!primary.contains(normalized.as_str()));
        let fallback_constant = ["REDEEM_BY_NORMALIZED_CODE_", "PREDICATE"].concat();
        let constant_start = source
            .find(fallback_constant.as_str())
            .expect("the fallback predicate constant must exist");
        let constant_body = &source[constant_start..];
        let constant_end = constant_body
            .find(';')
            .expect("the fallback predicate constant must terminate");
        assert!(constant_body[..constant_end].contains(normalized.as_str()));

        // 4. The fallback is reached from the hash-miss arm only, after the
        // primary `fetch_optional`, and a successful hash hit returns the row
        // straight from the `Some(row)` arm.
        let primary_fetch = primary.find("fetch_optional").expect("primary lookup");
        let hit_arm = primary.find("Some(row) =>").expect("hash-hit arm");
        let miss_arm = primary.find("None =>").expect("hash-miss arm");
        let fallback_call = primary
            .find("load_promotion_for_redeem_by_normalized_code(tx, command, &identity)")
            .expect("documented fallback call");
        assert!(primary_fetch < hit_arm, "the hit arm must follow the probe");
        assert!(hit_arm < miss_arm, "hit arm must precede the miss arm");
        assert!(
            miss_arm < fallback_call,
            "the fallback lives in the miss arm"
        );
        assert!(
            !primary[hit_arm..miss_arm].contains("load_promotion_for_redeem_by_normalized_code"),
            "a successful hash hit must not reach the fallback"
        );
        assert!(
            primary.contains("Documented fallback"),
            "the fallback must stay documented at the call site"
        );

        // 5. Both probes share one query template, so the fallback cannot apply
        // weaker status/window/lock guards than the primary lookup.
        assert!(source.contains("fn redeem_load_sql(code_predicate: &str) -> String"));
        assert!(source.contains("AND {code_predicate}"));
    }

    /// The fallback repair is part of the redemption transaction and is
    /// unreachable on a successful hash hit.
    #[test]
    fn fallback_hash_repair_runs_inside_the_redemption_transaction() {
        let source = include_str!("postgres_promotion.rs");
        let primary = section_until(
            source,
            "async fn load_promotion_for_redeem(",
            "/// Hash-miss fallback",
        );
        let fallback = section_until(
            source,
            "async fn load_promotion_for_redeem_by_normalized_code(",
            "async fn ensure_promotion_can_be_redeemed(",
        );

        // The repair statement exists only in the fallback helper. The primary
        // function can reach that helper only through its `None` arm (asserted by
        // the test above), so a hash hit can never execute the UPDATE.
        let set_hash = ["SET ", "code_hash"].concat();
        let update_promotion_code = ["UPDATE ", "promotion_code"].concat();
        assert!(!primary.contains(set_hash.as_str()));
        assert!(fallback.contains(update_promotion_code.as_str()));
        assert!(fallback.contains(set_hash.as_str()));
        assert!(fallback.contains("code_last4 = CAST($2 AS TEXT)"));
        assert_eq!(
            source.matches(set_hash.as_str()).count(),
            1,
            "exactly one hash repair statement may exist"
        );

        // It runs on the caller's `Transaction`, not on the pool (the failure
        // attempt recorder is the only auto-commit writer), so it commits or
        // rolls back atomically with the redemption.
        assert!(fallback.contains("tx: &mut Transaction<'_, Postgres>"));
        assert!(fallback.contains(".execute(&mut **tx)"));
        assert!(!fallback.contains(".execute(pool)"));

        // The repair is observable at `info` and guarded so a re-run is a no-op.
        assert!(fallback.contains("tracing::info!"));
        assert!(fallback.contains("repaired a missing promotion code hash identity"));
        assert!(fallback.contains("code_hash IS DISTINCT FROM CAST($1 AS TEXT)"));
    }
}

fn member_card_id(command: &GrantMemberCardCommand) -> String {
    stable_storage_id(&["member-card", &command.tenant_id, &command.request_no])
}

fn member_card_consumption_id(command: &ConsumeMemberCardCommand) -> String {
    stable_storage_id(&[
        "member-card-usage",
        &command.tenant_id,
        command.organization_id.as_deref().unwrap_or("global"),
        &command.owner_user_id,
        &command.idempotency_key,
    ])
}

async fn load_member_card_consumption_replay(
    tx: &mut Transaction<'_, Postgres>,
    command: &ConsumeMemberCardCommand,
    consumption_id: &str,
) -> Result<Option<MemberCardConsumptionOutcome>, CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT c.card_id, c.amount, c.balance_after, card.daily_quota, card.total_quota,
               card.total_used
        FROM promotion_member_card_consumption c
        JOIN promotion_member_card card ON card.tenant_id = c.tenant_id AND card.id = c.card_id
        WHERE c.id = $1 AND c.tenant_id = $2
        "#,
    )
    .bind(consumption_id)
    .bind(&command.tenant_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load member card consumption replay", error))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let used_today: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount), 0)
        FROM promotion_member_card_consumption
        WHERE tenant_id = $1 AND card_id = $2
          AND CAST(occurred_at AS TIMESTAMP) >= date_trunc('day', CURRENT_TIMESTAMP AT TIME ZONE 'UTC')
        "#,
    )
    .bind(&command.tenant_id)
    .bind(string_cell(&row, "card_id"))
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| store_error("failed to sum member card daily consumption replay", error))?;
    Ok(Some(MemberCardConsumptionOutcome {
        accepted: true,
        replayed: true,
        card_id: string_cell(&row, "card_id"),
        consumed_amount: integer_cell(&row, "amount"),
        used_today,
        daily_quota: integer_cell(&row, "daily_quota"),
        total_used: integer_cell(&row, "total_used"),
        total_quota: integer_cell(&row, "total_quota"),
        balance: integer_cell(&row, "balance_after"),
    }))
}

fn map_member_card_row(row: sqlx::postgres::PgRow) -> Result<PromotionMemberCard, CommerceServiceError> {
    let period = PromotionSubscriptionPeriod::parse(&string_cell(&row, "period"))?;
    Ok(PromotionMemberCard {
        id: string_cell(&row, "id"),
        card_no: string_cell(&row, "card_no"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        user_coupon_id: string_cell(&row, "user_coupon_id"),
        owner_user_id: string_cell(&row, "owner_user_id"),
        period,
        duration_days: integer_cell(&row, "duration_days"),
        daily_quota: integer_cell(&row, "daily_quota"),
        total_quota: integer_cell(&row, "total_quota"),
        total_used: integer_cell(&row, "total_used"),
        status: string_cell(&row, "status"),
        starts_at: string_cell(&row, "starts_at"),
        expires_at: optional_string_cell(&row, "expires_at"),
        created_at: string_cell(&row, "created_at"),
        updated_at: string_cell(&row, "updated_at"),
    })
}

async fn grant_member_card_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: &GrantMemberCardCommand,
) -> Result<PromotionMemberCard, CommerceServiceError> {
    let now = current_timestamp_string();
    let card_id = member_card_id(command);
    let card_uuid = sdkwork_utils_rust::uuid();
    let card_no = stable_storage_id(&["member-card", &command.tenant_id, &command.request_no]);
    // Normalize a caller-supplied planned start so the stored value, the
    // scheduled/active decision and the expiry derivation all use one instant.
    let requested_start = match command.starts_at.as_deref() {
        Some(value) => timestamps::canonical_window(Some(value), "starts_at")?,
        None => None,
    };
    let now_epoch = timestamps::epoch_seconds(&now)
        .ok_or_else(|| CommerceServiceError::storage("runtime clock is unreadable"))?;
    // 排期生效：starts_at 在未来→scheduled，由生命周期 worker 激活；否则立即 active
    let (status, starts_at_value, expiry_base) = match requested_start {
        Some(starts_at)
            if timestamps::epoch_seconds(&starts_at).is_some_and(|value| value > now_epoch) =>
        {
            ("scheduled", starts_at.clone(), Some(starts_at))
        }
        _ => ("active", timestamps::format_iso_epoch(now_epoch), None),
    };
    let inserted = sqlx::query(
        r#"
        INSERT INTO promotion_member_card
            (id, uuid, tenant_id, organization_id, card_no, offer_id, offer_version_id,
             user_coupon_id, subject_type, subject_id, owner_user_id, period, duration_days,
             daily_quota, total_quota, total_used, status, starts_at, expires_at, version,
             created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 0, $16,
             $17, to_char(COALESCE(
                 CASE WHEN $18::TIMESTAMPTZ IS NULL THEN NULL ELSE $18::TIMESTAMPTZ END,
                 CURRENT_TIMESTAMP) + make_interval(days => $19),
                 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'), 0, $20, $20)
        ON CONFLICT (tenant_id, user_coupon_id) DO NOTHING
        "#,
    )
    .bind(&card_id)
    .bind(&card_uuid)
    .bind(&command.tenant_id)
    .bind(organization_binding(command.organization_id.as_deref()))
    .bind(&card_no)
    .bind(&command.offer_id)
    .bind(&command.offer_version_id)
    .bind(&command.user_coupon_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&command.owner_user_id)
    .bind(command.period.as_str())
    .bind(command.duration_days)
    .bind(command.daily_quota)
    .bind(command.total_quota)
    .bind(status)
    .bind(&starts_at_value)
    .bind(expiry_base.as_deref())
    .bind(command.duration_days)
    .bind(&now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to grant member card", error))?
    .rows_affected();

    let row = if inserted == 0 {
        // 幂等重放：同一张券已开卡，返回既有卡
        sqlx::query(
            r#"
            SELECT id, card_no, offer_id, offer_version_id, user_coupon_id, owner_user_id,
                   period, duration_days, daily_quota, total_quota, total_used, status,
                   starts_at, expires_at, created_at, updated_at
            FROM promotion_member_card
            WHERE tenant_id = $1 AND user_coupon_id = $2
            "#,
        )
        .bind(&command.tenant_id)
        .bind(&command.user_coupon_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| store_error("failed to replay member card grant", error))?
    } else {
        sqlx::query(
            r#"
            SELECT id, card_no, offer_id, offer_version_id, user_coupon_id, owner_user_id,
                   period, duration_days, daily_quota, total_quota, total_used, status,
                   starts_at, expires_at, created_at, updated_at
            FROM promotion_member_card
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(&card_id)
        .bind(&command.tenant_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| store_error("failed to load granted member card", error))?
    };
    map_member_card_row(row)
}



/// 券兑现主体的通用字段（redeem 与 claim 共用）。
struct CouponSubject<'a> {
    tenant_id: &'a str,
    organization_id: Option<&'a str>,
    owner_user_id: &'a str,
    request_no: &'a str,
    idempotency_key: &'a str,
    source_remark: &'a str,
}

/// 权益兑现结果：按券权益类型分发的入账/开卡信息。
struct BenefitCredit {
    amount_minor: String,
    credited_points: i64,
    balance: i64,
    benefit_kind: Option<String>,
    credited_amount: Option<String>,
    asset_type: Option<String>,
    member_card_id: Option<String>,
    member_card_no: Option<String>,
}

/// 兑换码兑现分发：解析券权益并按类型入账（Token Bank/积分/现金）或开通会员卡。
/// 无权益规则的存量券回退按 discount_value 记积分。
async fn credit_coupon_benefit_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    promotion: &RedeemPromotion,
    coupon_id: &str,
    now: &str,
) -> Result<BenefitCredit, CommerceServiceError> {
    let benefit = parse_admin_coupon_benefit(promotion.rule_json.as_deref())?;
    let subject = CouponSubject {
        tenant_id: &command.tenant_id,
        organization_id: command.organization_id.as_deref(),
        owner_user_id: &command.owner_user_id,
        request_no: &command.request_no,
        idempotency_key: &command.idempotency_key,
        source_remark: &format!("redeem_promotion_code={}", command.code),
    };
    match benefit {
        None => credit_legacy_points_in_tx(tx, command, promotion, coupon_id, now).await,
        Some(PromotionCouponBenefit::PointsCredit { grant_points }) => {
            let account = ensure_points_account(tx, command, now).await?;
            let credited_points = grant_points;
            let balance_after = checked_points_add(account.available_points, credited_points)?;
            update_account_points(
                tx,
                &account.id,
                account.available_points,
                credited_points,
                now,
            )
            .await?;
            insert_account_ledger(
                tx,
                command,
                &account.id,
                balance_after,
                credited_points,
                coupon_id,
                now,
            )
            .await?;
            insert_redeem_billing_history(
                tx,
                command,
                coupon_id,
                &points_to_money_string(credited_points),
                &promotion.currency_code,
                credited_points,
                now,
            )
            .await?;
            Ok(BenefitCredit {
                amount_minor: points_to_minor_units_string(credited_points),
                credited_points,
                balance: balance_after,
                benefit_kind: Some("points_credit".to_owned()),
                credited_amount: Some(credited_points.to_string()),
                asset_type: Some("points".to_owned()),
                member_card_id: None,
                member_card_no: None,
            })
        }
        Some(PromotionCouponBenefit::TokenBankCredit {
            grant_amount,
            bonus_amount,
        }) => {
            let total = grant_amount
                .checked_add(bonus_amount)
                .ok_or_else(|| {
                    CommerceServiceError::validation(
                        "promotion Token Bank coupon grant amount exceeds the supported range",
                    )
                })?;
            let balance_after = credit_asset_account_in_tx(
                tx,
                &subject,
                "token_bank",
                &promotion.currency_code,
                total,
                "token-bank",
                coupon_id,
                now,
            )
            .await?;
            Ok(BenefitCredit {
                amount_minor: total.to_string(),
                credited_points: 0,
                balance: balance_after,
                benefit_kind: Some("token_bank_credit".to_owned()),
                credited_amount: Some(total.to_string()),
                asset_type: Some("token_bank".to_owned()),
                member_card_id: None,
                member_card_no: None,
            })
        }
        Some(PromotionCouponBenefit::CashCredit { grant_amount }) => {
            let balance_after = credit_asset_account_in_tx(
                tx,
                &subject,
                "cash",
                &promotion.currency_code,
                grant_amount,
                "cash",
                coupon_id,
                now,
            )
            .await?;
            Ok(BenefitCredit {
                amount_minor: grant_amount.to_string(),
                credited_points: 0,
                balance: balance_after,
                benefit_kind: Some("cash_credit".to_owned()),
                credited_amount: Some(grant_amount.to_string()),
                asset_type: Some("cash".to_owned()),
                member_card_id: None,
                member_card_no: None,
            })
        }
        Some(PromotionCouponBenefit::Subscription {
            period,
            duration_days,
            daily_quota,
            total_quota,
            ..
        }) => {
            let card_command = GrantMemberCardCommand::new(
                &command.tenant_id,
                command.organization_id.as_deref(),
                &command.owner_user_id,
                &promotion.offer_id,
                &promotion.offer_version_id,
                coupon_id,
                period,
                duration_days,
                daily_quota,
                total_quota,
                &command.request_no,
                None,
                &command.idempotency_key,
            )?;
            let card = grant_member_card_in_tx(tx, &card_command).await?;
            Ok(BenefitCredit {
                amount_minor: "0".to_owned(),
                credited_points: 0,
                balance: 0,
                benefit_kind: Some("subscription".to_owned()),
                credited_amount: None,
                asset_type: None,
                member_card_id: Some(card.id),
                member_card_no: Some(card.card_no),
            })
        }
    }
}

/// 存量无规则券兜底：按 discount_value 记积分。
async fn credit_legacy_points_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    promotion: &RedeemPromotion,
    coupon_id: &str,
    now: &str,
) -> Result<BenefitCredit, CommerceServiceError> {
    let account = ensure_points_account(tx, command, now).await?;
    let credited_points = coupon_credit_points(&promotion.discount_value)?;
    let balance_after = checked_points_add(account.available_points, credited_points)?;
    update_account_points(
        tx,
        &account.id,
        account.available_points,
        credited_points,
        now,
    )
    .await?;
    insert_account_ledger(
        tx,
        command,
        &account.id,
        balance_after,
        credited_points,
        coupon_id,
        now,
    )
    .await?;
    insert_redeem_billing_history(
        tx,
        command,
        coupon_id,
        &points_to_money_string(credited_points),
        &promotion.currency_code,
        credited_points,
        now,
    )
    .await?;
    Ok(BenefitCredit {
        amount_minor: points_to_minor_units_string(credited_points),
        credited_points,
        balance: balance_after,
        benefit_kind: Some("points_credit".to_owned()),
        credited_amount: Some(credited_points.to_string()),
        asset_type: Some("points".to_owned()),
        member_card_id: None,
        member_card_no: None,
    })
}

/// 通用资产账户入账（Token Bank/现金等）：确保账户、更新余额、写账户流水。
async fn credit_asset_account_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    subject: &CouponSubject<'_>,
    asset_type: &str,
    currency_code: &str,
    amount: i64,
    account_scope: &str,
    source_coupon_id: &str,
    now: &str,
) -> Result<i64, CommerceServiceError> {
    let account = ensure_asset_account(tx, subject, asset_type, currency_code, account_scope, now).await?;
    let balance_after = checked_points_add(account.available_amount, amount)?;
    update_account_points(tx, &account.id, account.available_amount, amount, now).await?;
    insert_asset_account_ledger(
        tx,
        subject,
        &account.id,
        asset_type,
        balance_after,
        amount,
        source_coupon_id,
        now,
    )
    .await?;
    Ok(balance_after)
}

#[derive(Debug, Clone)]
struct AssetAccount {
    id: String,
    available_amount: i64,
}

async fn ensure_asset_account(
    tx: &mut Transaction<'_, Postgres>,
    subject: &CouponSubject<'_>,
    asset_type: &str,
    currency_code: &str,
    account_scope: &str,
    now: &str,
) -> Result<AssetAccount, CommerceServiceError> {
    if let Some(account) = load_asset_account(tx, subject, asset_type, currency_code).await? {
        return Ok(account);
    }
    let account_id = stable_storage_id(&[
        "account",
        subject.tenant_id,
        subject.organization_id.unwrap_or("global"),
        subject.owner_user_id,
        account_scope,
        currency_code,
    ]);
    sqlx::query(
        r#"
        INSERT INTO commerce_account
            (id, tenant_id, organization_id, owner_user_id, asset_type, currency_code,
             available_amount, frozen_amount, version, status, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, '0', '0', 0, 'active', $7, $8)
        ON CONFLICT (tenant_id, organization_id, owner_user_id, asset_type, currency_code)
        DO NOTHING
        "#,
    )
    .bind(&account_id)
    .bind(subject.tenant_id)
    .bind(organization_binding(subject.organization_id))
    .bind(subject.owner_user_id)
    .bind(asset_type)
    .bind(currency_code)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to create coupon asset account", error))?;

    load_asset_account(tx, subject, asset_type, currency_code)
        .await?
        .ok_or_else(|| CommerceServiceError::storage("coupon asset account was not available after creation"))
}

async fn load_asset_account(
    tx: &mut Transaction<'_, Postgres>,
    subject: &CouponSubject<'_>,
    asset_type: &str,
    currency_code: &str,
) -> Result<Option<AssetAccount>, CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT id, CAST(COALESCE(available_amount, '0') AS BIGINT) AS available_amount
        FROM commerce_account
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id = '0' AND $2 IS NULL))
          AND owner_user_id = CAST($3 AS TEXT)
          AND asset_type = $4
          AND currency_code = $5
          AND status = 'active'
        ORDER BY id ASC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(subject.tenant_id)
    .bind(subject.organization_id)
    .bind(subject.owner_user_id)
    .bind(asset_type)
    .bind(currency_code)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load coupon asset account", error))?;
    Ok(row.map(|row| AssetAccount {
        id: string_cell(&row, "id"),
        available_amount: integer_cell(&row, "available_amount"),
    }))
}

async fn insert_asset_account_ledger(
    tx: &mut Transaction<'_, Postgres>,
    subject: &CouponSubject<'_>,
    account_id: &str,
    asset_type: &str,
    balance_after: i64,
    amount: i64,
    source_coupon_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO commerce_account_ledger_entry
            (id, tenant_id, organization_id, account_id, owner_user_id, asset_type, direction,
             amount, balance_after, business_type, transaction_no, request_no, idempotency_key,
             source_type, source_id, remark, created_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'redeem', $10, $11, $12, $13, $14, $15, $16)
        "#,
    )
    .bind(stable_storage_id(&[
        "ledger",
        subject.tenant_id,
        asset_type,
        subject.request_no,
    ]))
    .bind(subject.tenant_id)
    .bind(organization_binding(subject.organization_id))
    .bind(account_id)
    .bind(subject.owner_user_id)
    .bind(asset_type)
    .bind(CommerceLedgerDirection::Credit.as_str())
    .bind(amount.to_string())
    .bind(balance_after.to_string())
    .bind(subject.request_no)
    .bind(subject.request_no)
    .bind(subject.idempotency_key)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(source_coupon_id)
    .bind(subject.source_remark.to_owned())
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to insert coupon asset account ledger entry", error))?;
    Ok(())
}

fn map_redemption_preview(
    benefit: Option<PromotionCouponBenefit>,
    expires_at: Option<&str>,
) -> Result<PromotionCodeRedemptionPreview, CommerceServiceError> {
    Ok(match benefit {
        None => PromotionCodeRedemptionPreview {
            benefit_kind: Some("points_credit".to_owned()),
            credited_amount: None,
            asset_type: Some("points".to_owned()),
            period: None,
            duration_days: None,
            daily_quota: None,
            total_quota: None,
            expires_at: expires_at.map(str::to_owned),
        },
        Some(PromotionCouponBenefit::PointsCredit { grant_points }) => PromotionCodeRedemptionPreview {
            benefit_kind: Some("points_credit".to_owned()),
            credited_amount: Some(grant_points.to_string()),
            asset_type: Some("points".to_owned()),
            period: None,
            duration_days: None,
            daily_quota: None,
            total_quota: None,
            expires_at: expires_at.map(str::to_owned),
        },
        Some(PromotionCouponBenefit::TokenBankCredit {
            grant_amount,
            bonus_amount,
        }) => PromotionCodeRedemptionPreview {
            benefit_kind: Some("token_bank_credit".to_owned()),
            credited_amount: Some(
                grant_amount
                    .checked_add(bonus_amount)
                    .ok_or_else(|| {
                        CommerceServiceError::validation(
                            "promotion Token Bank coupon grant amount exceeds the supported range",
                        )
                    })?
                    .to_string(),
            ),
            asset_type: Some("token_bank".to_owned()),
            period: None,
            duration_days: None,
            daily_quota: None,
            total_quota: None,
            expires_at: expires_at.map(str::to_owned),
        },
        Some(PromotionCouponBenefit::CashCredit { grant_amount }) => PromotionCodeRedemptionPreview {
            benefit_kind: Some("cash_credit".to_owned()),
            credited_amount: Some(grant_amount.to_string()),
            asset_type: Some("cash".to_owned()),
            period: None,
            duration_days: None,
            daily_quota: None,
            total_quota: None,
            expires_at: expires_at.map(str::to_owned),
        },
        Some(PromotionCouponBenefit::Subscription {
            period,
            duration_days,
            daily_quota,
            total_quota,
            ..
        }) => PromotionCodeRedemptionPreview {
            benefit_kind: Some("subscription".to_owned()),
            credited_amount: None,
            asset_type: None,
            period: Some(period.as_str().to_owned()),
            duration_days: Some(duration_days),
            daily_quota: Some(daily_quota),
            total_quota: Some(total_quota),
            expires_at: expires_at.map(str::to_owned),
        },
    })
}

/// advisory lock 键：过期预算预留扫描（防多实例并发）。
///
/// Deliberately its own literal (`71_091_238_430`), distinct from
/// [`MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY`] (`…410`) and
/// [`PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY`] (`…420`): the three sweeps touch
/// disjoint tables and must be able to run concurrently. A shared key would make
/// one instance's sweep silently skip another's.
const PROMOTION_BUDGET_RESERVATION_SWEEP_LOCK_KEY: i64 = 71_091_238_430;

/// 过期预算预留扫描结果。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BudgetReservationSweepOutcome {
    /// Applications moved `applied` → `released` by this pass.
    pub released: i64,
    /// true 表示本轮被 advisory lock 跳过（另一实例正在执行）。
    pub skipped: bool,
}

/// Marks every expired reservation released and returns how many rows moved.
///
/// Set-based by construction: the `WITH` chain is one statement that (1) updates
/// the applications and returns them, (2) derives each row's reserved amount from
/// its `reserve` ledger row, (3) restores the account pools once per account with
/// an aggregate `UPDATE`, and (4) appends the `release` ledger rows in one
/// `INSERT ... SELECT`. The final `SELECT COUNT(*)` is the pass size; the audit row
/// is one aggregated insert after it.
///
/// `released_at`/`release_reason` are written on the application row itself, so
/// "when and why was this reservation dropped" is answerable from the row without
/// reading the sweep's logs.
async fn release_stale_discount_applications_in_tx(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<i64, CommerceServiceError> {
    let now = current_timestamp_string();
    let sql = format!(
        r#"
        WITH expired AS (
            UPDATE promotion_discount_application a
            SET status = 'released',
                released_at = $1,
                release_reason = 'reservation_expired',
                version = a.version + 1,
                updated_at = $1
            WHERE a.status = 'applied'
              AND a.reservation_expires_at IS NOT NULL
              AND {instant} < NOW()
            RETURNING a.id, a.tenant_id, a.organization_id, a.budget_account_id, a.request_no
        ),
        reservations AS (
            SELECT e.id, e.tenant_id, e.organization_id, e.budget_account_id, e.request_no,
                   COALESCE((
                       SELECT l.amount_minor
                       FROM promotion_budget_ledger_entry l
                       WHERE l.tenant_id = e.tenant_id
                         AND l.application_id = e.id
                         AND l.direction = 'reserve'
                       ORDER BY l.occurred_at DESC, l.id DESC
                       LIMIT 1
                   ), 0) AS amount_minor
            FROM expired e
        ),
        restored AS (
            UPDATE promotion_budget_account b
            SET available_amount_minor = b.available_amount_minor + totals.total_amount_minor,
                reserved_amount_minor = b.reserved_amount_minor - totals.total_amount_minor,
                version = b.version + 1,
                updated_at = $1
            FROM (
                SELECT budget_account_id, SUM(amount_minor) AS total_amount_minor
                FROM reservations
                WHERE budget_account_id IS NOT NULL AND amount_minor > 0
                GROUP BY budget_account_id
            ) totals
            WHERE b.id = totals.budget_account_id
              AND b.reserved_amount_minor >= totals.total_amount_minor
            RETURNING b.id AS budget_account_id,
                      b.available_amount_minor AS available_after_minor,
                      b.reserved_amount_minor AS reserved_after_minor,
                      b.used_amount_minor AS used_after_minor,
                      b.currency_code
        ),
        ledger AS (
            INSERT INTO promotion_budget_ledger_entry
                (id, uuid, tenant_id, organization_id, budget_account_id, application_id,
                 direction, amount_minor, available_after_minor, reserved_after_minor,
                 used_after_minor, currency_code, request_no, idempotency_key, occurred_at)
            SELECT 'budget-release:' || r.id,
                   'budget-release-' || md5(r.tenant_id || ':' || r.id),
                   r.tenant_id, r.organization_id, r.budget_account_id, r.id,
                   'release', r.amount_minor,
                   s.available_after_minor, s.reserved_after_minor, s.used_after_minor,
                   s.currency_code, r.request_no,
                   'budget-release-' || md5(r.tenant_id || ':' || r.id),
                   $1
            FROM reservations r
            JOIN restored s ON s.budget_account_id = r.budget_account_id
            WHERE r.amount_minor > 0
            RETURNING 1
        )
        SELECT COUNT(*)::BIGINT FROM reservations
       "#,
        instant = timestamps::instant("a.reservation_expires_at"),
    );
    let released: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(&now)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| store_error("failed to release stale discount reservations", error))?;
    if released > 0 {
        insert_budget_release_expired_audit_event(tx, released, &now).await?;
    }
    Ok(released)
}

/// One aggregated `budget.release_expired` audit row for a sweep pass.
///
/// The sweep is system-level and cross-tenant, so the row carries the platform
/// sentinel (`'0'`) for tenant, organization and actor instead of an operator
/// identity. `subject_count` is the number of applications the pass released,
/// which is the only quantity the evidence needs to record.
async fn insert_budget_release_expired_audit_event(
    tx: &mut Transaction<'_, Postgres>,
    released: i64,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO promotion_audit_event
            (id, uuid, tenant_id, organization_id, actor_id, action, aggregate_type,
             aggregate_id, subject_count, detail_json, trace_id, occurred_at, created_at)
        VALUES
            ($1, $2, '0', '0', '0', 'budget.release_expired', 'promotion_discount_application',
             'reservation_sweep', $3, NULL, '', $4, $4)
       "#,
    )
    .bind(stable_storage_id(&["audit-event", "budget.release_expired", now]))
    .bind(sdkwork_utils_rust::uuid())
    .bind(released)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to record budget release audit event", error))?;
    Ok(())
}

/// advisory lock 键：会员卡生命周期扫描（防多实例并发）。
const MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_410;

/// advisory lock 键：券生命周期扫描（防多实例并发）。
///
/// Deliberately a different literal from [`MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY`]
/// (`71_091_238_410`): the member-card sweep and the coupon sweep are separate
/// units of work over disjoint tables, and sharing a key would make a member-card
/// sweep in one instance silently skip the coupon sweep in another.
const PROMOTION_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_420;

/// 券生命周期扫描结果。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PromotionLifecycleSweepOutcome {
    /// Wallet coupons moved `claimed|active|pending` → `expired`.
    pub expired_user_coupons: i64,
    /// Unclaimed codes moved `active` → `expired`.
    pub expired_codes: i64,
    /// true 表示本轮被 advisory lock 跳过（另一实例正在执行）。
    pub skipped: bool,
}


/// 会员卡生命周期扫描结果。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MemberCardLifecycleSweepOutcome {
    pub activated: i64,
    pub expired: i64,
    /// true 表示本轮被 advisory lock 跳过（另一实例正在执行）。
    pub skipped: bool,
}
