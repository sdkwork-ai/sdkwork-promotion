use std::time::{SystemTime, UNIX_EPOCH};

use sdkwork_commerce_promotion_service::{
    ApplyPromotionDiscountCommand, ClaimPromotionUserCouponCommand, ConsumeMemberCardCommand,
    GrantMemberCardCommand, MemberCardConsumptionOutcome, MemberCardListQuery, PointsBalance,
    PointsBalanceQuery, PointsHistoryItem, PointsHistoryQuery, PromotionCodeRedemptionCommand,
    PromotionCodeRedemptionOutcome, PromotionCodeRedemptionPreview, PromotionCouponBenefit,
    PromotionMemberCard,
    PromotionOrderCouponBenefit, PromotionSubscriptionPeriod,
    PromotionUserCouponItem, PromotionUserCouponListQuery, RetrieveMemberCardQuery,
    ReversePromotionDiscountCommand,
};
use sdkwork_contract_service::{
    CommerceAccountAssetType, CommerceLedgerDirection, CommerceServiceError,
};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::coupon_benefit::{parse_admin_coupon_benefit, parse_order_coupon_benefit};

const POINTS_CURRENCY_CODE: &str = "POINT";
const PROMOTION_CODE_REDEMPTION_SCOPE: &str = "promotions.codes.redemptions.create";
const PROMOTION_USER_COUPON_CLAIM_SCOPE: &str = "promotions.userCoupons.claims.create";
const PROMOTION_DISCOUNT_APPLICATION_CREATE_SCOPE: &str = "promotions.discountApplications.create";
const PROMOTION_DISCOUNT_APPLICATION_REVERSAL_SCOPE: &str =
    "promotions.discountApplications.reversals.create";
const USER_SUBJECT_TYPE: &str = "user";
const PROMOTION_USER_COUPON_SOURCE_TYPE: &str = "promotion_user_coupon";

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
                   COALESCE(v.currency_code, 'CNY') AS currency_code,
                   v.rule_json AS rule_json
            FROM promotion_user_coupon c
            JOIN promotion_code pc ON pc.tenant_id = c.tenant_id AND pc.id = c.code_id
            JOIN promotion_offer_version v
              ON v.tenant_id = c.tenant_id AND v.id = c.offer_version_id
            WHERE c.tenant_id = $1
              AND ((c.organization_id = $2) OR (c.organization_id IS NULL AND $2 IS NULL))
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
              AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id IS NULL AND $2 IS NULL) OR (c.organization_id = '0' AND $2 IS NULL))
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
              AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $2 IS NULL) OR (organization_id = '0' AND $2 IS NULL))
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
              AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $2 IS NULL) OR (organization_id = '0' AND $2 IS NULL))
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

    pub async fn redeem_promotion_code(
        &self,
        command: PromotionCodeRedemptionCommand,
    ) -> Result<PromotionCodeRedemptionOutcome, CommerceServiceError> {
        let mut tx = self.pool.begin().await.map_err(|error| {
            store_error(
                "failed to begin promotion code redemption transaction",
                error,
            )
        })?;
        let now = current_timestamp_string();
        let request_hash = redeem_request_hash(&command);
        if let Some(row) = load_redeem_idempotency_row(&mut tx, &command).await? {
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
            refresh_redeem_idempotency_lock(&mut tx, &command, &now).await?;
        } else {
            insert_redeem_idempotency_lock(&mut tx, &command, &request_hash, &now).await?;
        }

        let promotion = load_promotion_for_redeem(&mut tx, &command, &now).await?;
        ensure_promotion_can_be_redeemed(&mut tx, &command, &promotion).await?;
        let coupon_id = coupon_id(&command);
        let credit = credit_coupon_benefit_in_tx(&mut tx, &command, &promotion, &coupon_id, &now).await?;
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
        .with_coupon(coupon_id.clone(), issued_coupon_code(&command));
        complete_redeem_idempotency(&mut tx, &command, &outcome, &now).await?;

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
              AND ((organization_id = $2) OR (organization_id IS NULL AND $2 IS NULL))
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
            if expires_at.as_str() < now.as_str() {
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
        sqlx::query(
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
        .bind(command.organization_id.as_deref())
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
              AND ((organization_id = $2) OR (organization_id IS NULL AND $2 IS NULL))
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
              AND ((organization_id = $2) OR (organization_id IS NULL AND $2 IS NULL))
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
        let result = sqlx::query(
            r#"
            UPDATE promotion_member_card
            SET status = 'active', version = version + 1,
                updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
            WHERE status = 'scheduled'
              AND starts_at <= TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|error| store_error("failed to activate due member cards", error))?;
        Ok(result.rows_affected() as i64)
    }

    /// 生命周期扫描（advisory lock 保护，防多实例并发）：排期激活 + 到期过期。
    pub async fn run_member_card_lifecycle_sweep(&self) -> Result<MemberCardLifecycleSweepOutcome, CommerceServiceError> {
        let mut conn = self.pool.acquire().await.map_err(|error| {
            store_error("failed to acquire connection for member card lifecycle sweep", error)
        })?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY)
            .fetch_one(&mut *conn)
            .await
            .map_err(|error| store_error("failed to acquire member card lifecycle sweep lock", error))?;
        if !locked {
            // 另一实例正在执行本轮扫描
            return Ok(MemberCardLifecycleSweepOutcome {
                activated: 0,
                expired: 0,
                skipped: true,
            });
        }
        let activated = self.activate_due_member_cards().await?;
        let expired = self.expire_due_member_cards().await?;
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY)
            .execute(&mut *conn)
            .await;
        Ok(MemberCardLifecycleSweepOutcome {
            activated,
            expired,
            skipped: false,
        })
    }

    /// 生命周期扫描：过期到期的会员卡（active → expired）。
    pub async fn expire_due_member_cards(&self) -> Result<i64, CommerceServiceError> {
        let result = sqlx::query(
            r#"
            UPDATE promotion_member_card
            SET status = 'expired', version = version + 1,
                updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
            WHERE status = 'active'
              AND expires_at IS NOT NULL
              AND expires_at < TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|error| store_error("failed to expire due member cards", error))?;
        Ok(result.rows_affected() as i64)
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
        let application_id = discount_application_id(&command);
        insert_discount_application(&mut tx, &command, &coupon, &order, &application_id, &now)
            .await?;
        mark_user_coupon_applied(&mut tx, &command, &now).await?;
        let item = build_applied_promotion_coupon_item(&coupon, &now)?;
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
              AND ((a.organization_id = CAST($2 AS TEXT)) OR (a.organization_id IS NULL AND $3 IS NULL) OR (a.organization_id = '0' AND $3 IS NULL))
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
    discount_value: String,
    status: String,
}

#[derive(Debug, Clone)]
struct DiscountApplyOrder {
    order_no: String,
    currency_code: String,
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
    .bind(command.organization_id.as_deref())
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
    .bind(command.organization_id.as_deref())
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
    let row = sqlx::query(
        r#"
        SELECT c.id,
               COALESCE(NULLIF(c.coupon_code, ''), '-') AS code,
               c.offer_id,
               c.offer_version_id,
               CAST(v.discount_value AS TEXT) AS discount_value,
               c.status
        FROM promotion_user_coupon c
        JOIN promotion_offer_version v
          ON v.tenant_id = c.tenant_id
         AND v.id = c.offer_version_id
        WHERE c.tenant_id = CAST($1 AS TEXT)
          AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id IS NULL AND $3 IS NULL) OR (c.organization_id = '0' AND $3 IS NULL))
          AND c.id = CAST($4 AS TEXT)
          AND c.owner_user_id = CAST($5 AS TEXT)
          AND c.subject_type = $6
          AND c.subject_id = CAST($7 AS TEXT)
        LIMIT 1
        FOR UPDATE
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
    .map_err(|error| store_error("failed to load user coupon for discount apply", error))?
    .ok_or_else(|| CommerceServiceError::not_found("user coupon was not found"))?;

    Ok(DiscountApplyCoupon {
        id: string_cell(&row, "id"),
        code: string_cell(&row, "code"),
        offer_id: string_cell(&row, "offer_id"),
        offer_version_id: string_cell(&row, "offer_version_id"),
        discount_value: string_cell(&row, "discount_value"),
        status: string_cell(&row, "status"),
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
               CAST(COALESCE(a.discount_amount, v.discount_value, '0') AS TEXT) AS discount_value,
               c.status
        FROM promotion_user_coupon c
        JOIN promotion_offer_version v
          ON v.tenant_id = c.tenant_id
         AND v.id = c.offer_version_id
        LEFT JOIN promotion_discount_application a
          ON a.tenant_id = c.tenant_id
         AND a.user_coupon_id = c.id
         AND LOWER(COALESCE(a.status, '')) IN ('applied', 'settled')
        WHERE c.tenant_id = CAST($1 AS TEXT)
          AND ((c.organization_id = CAST($2 AS TEXT)) OR (c.organization_id IS NULL AND $3 IS NULL) OR (c.organization_id = '0' AND $3 IS NULL))
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
        discount_value: string_cell(&row, "discount_value"),
        status: string_cell(&row, "status"),
    })
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
    let row = sqlx::query(
        r#"
        SELECT order_no, currency_code, status
        FROM commerce_order
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $3 IS NULL) OR (organization_id = '0' AND $3 IS NULL))
          AND id = CAST($4 AS TEXT)
          AND owner_user_id = CAST($5 AS TEXT)
          AND LOWER(COALESCE(status, '')) IN (
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

    Ok(DiscountApplyOrder {
        order_no: string_cell(&row, "order_no"),
        currency_code: string_cell(&row, "currency_code"),
    })
}

async fn insert_discount_application(
    tx: &mut Transaction<'_, Postgres>,
    command: &ApplyPromotionDiscountCommand,
    coupon: &DiscountApplyCoupon,
    order: &DiscountApplyOrder,
    application_id: &str,
    now: &str,
) -> Result<(), CommerceServiceError> {
    sqlx::query(
        r#"
        INSERT INTO promotion_discount_application
            (id, tenant_id, organization_id, application_no, offer_id, offer_version_id,
             user_coupon_id, order_id, order_no, subject_type, subject_id, discount_amount,
             currency_code, status, request_no, idempotency_key, applied_at, rolled_back_at,
             created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'applied', $14, $15, $16, NULL, $17, $18)
       "#,
    )
    .bind(application_id)
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(discount_application_no(command))
    .bind(&coupon.offer_id)
    .bind(&coupon.offer_version_id)
    .bind(&coupon.id)
    .bind(&command.order_id)
    .bind(&order.order_no)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(&coupon.discount_value)
    .bind(&order.currency_code)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(now)
    .bind(now)
    .bind(now)
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

async fn reverse_discount_application(
    tx: &mut Transaction<'_, Postgres>,
    command: &ReversePromotionDiscountCommand,
    now: &str,
) -> Result<(), CommerceServiceError> {
    let updated = sqlx::query(
        r#"
        UPDATE promotion_discount_application
        SET status = 'rolled_back', rolled_back_at = $1, updated_at = $2
        WHERE tenant_id = CAST($3 AS TEXT)
          AND user_coupon_id = CAST($4 AS TEXT)
          AND subject_type = $5
          AND subject_id = CAST($6 AS TEXT)
          AND LOWER(COALESCE(status, '')) IN ('applied', 'settled')
       "#,
    )
    .bind(now)
    .bind(now)
    .bind(&command.tenant_id)
    .bind(&command.user_coupon_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .execute(&mut **tx)
    .await
    .map_err(|error| store_error("failed to reverse promotion discount application", error))?
    .rows_affected();

    if updated == 0 {
        return Err(CommerceServiceError::conflict(
            "active promotion discount application was not found",
        ));
    }
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

fn build_applied_promotion_coupon_item(
    coupon: &DiscountApplyCoupon,
    now: &str,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    let amount = stored_money_minor_units(&coupon.discount_value)?;
    PromotionUserCouponItem::new(&coupon.id, &coupon.code, &amount, now, "success")
}

fn build_reversed_promotion_coupon_item(
    coupon: &DiscountApplyCoupon,
    now: &str,
) -> Result<PromotionUserCouponItem, CommerceServiceError> {
    let amount = stored_money_minor_units(&coupon.discount_value)?;
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
    .bind(command.organization_id.as_deref())
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

async fn load_promotion_for_redeem(
    tx: &mut Transaction<'_, Postgres>,
    command: &PromotionCodeRedemptionCommand,
    now: &str,
) -> Result<RedeemPromotion, CommerceServiceError> {
    let row = sqlx::query(
        r#"
        SELECT pc.id AS code_id,
               s.id AS stock_id,
               pc.offer_id AS offer_id,
               s.offer_version_id AS offer_version_id,
               s.stock_type AS stock_type,
               CAST(v.discount_value AS TEXT) AS discount_value,
               COALESCE(v.currency_code, 'CNY') AS currency_code,
               v.rule_json AS rule_json,
               s.total_quantity AS total_quantity,
               COALESCE(s.available_quantity, 0) AS available_quantity,
               COALESCE(s.claimed_quantity, 0) AS stock_claimed_quantity,
               COALESCE(pc.max_claims, 1) AS code_max_claims,
               COALESCE(pc.claimed_quantity, 0) AS code_claimed_quantity,
               CAST(COALESCE(pc.expires_at, s.claim_ends_at, o.ends_at) AS TEXT) AS expires_at
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
          AND ((pc.organization_id = CAST($2 AS TEXT)) OR (pc.organization_id IS NULL AND $2 IS NULL) OR (pc.organization_id = '0' AND $2 IS NULL))
          AND pc.promotion_code = CAST($3 AS TEXT)
          AND pc.status = 'active'
          AND s.status = 'active'
          AND o.status = 'active'
          AND o.deleted_at IS NULL
          AND v.lifecycle_status = 'published'
          AND (pc.starts_at IS NULL OR pc.starts_at <= $4)
          AND (pc.expires_at IS NULL OR pc.expires_at >= $4)
          AND (s.claim_starts_at IS NULL OR s.claim_starts_at <= $4)
          AND (s.claim_ends_at IS NULL OR s.claim_ends_at >= $4)
          AND (o.starts_at IS NULL OR o.starts_at <= $4)
          AND (o.ends_at IS NULL OR o.ends_at >= $4)
        ORDER BY pc.organization_id DESC NULLS LAST, pc.id ASC
        LIMIT 1
        FOR UPDATE OF pc, s
        "#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(&command.code)
    .bind(now)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| store_error("failed to load promotion code", error))?
    .ok_or_else(|| CommerceServiceError::conflict("promotion code is invalid or unavailable"))?;

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
        code_max_claims: integer_cell(&row, "code_max_claims"),
        code_claimed_quantity: integer_cell(&row, "code_claimed_quantity"),
        expires_at: optional_string_cell(&row, "expires_at"),
    })
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
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $2 IS NULL) OR (organization_id = '0' AND $2 IS NULL))
          AND subject_type = $3
          AND subject_id = CAST($4 AS TEXT)
          AND code_id = $5
          AND status NOT IN ('expired', 'disabled', 'voided', 'cancelled')
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
    if received_count > 0 {
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
    .bind(command.organization_id.as_deref())
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
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $2 IS NULL) OR (organization_id = '0' AND $2 IS NULL))
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
    .bind(command.organization_id.as_deref())
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
    .bind(now)
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
    .bind(command.organization_id.as_deref())
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
    .bind(command.organization_id.as_deref())
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
    .bind(command.organization_id.as_deref())
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
    let row = sqlx::query(
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
               COALESCE(s.claim_ends_at, o.ends_at) AS expires_at
        FROM promotion_offer o
        JOIN promotion_coupon_stock s
          ON s.tenant_id = o.tenant_id
         AND s.offer_id = o.id
        JOIN promotion_offer_version v
          ON v.tenant_id = s.tenant_id
         AND v.id = s.offer_version_id
        WHERE o.tenant_id = CAST($1 AS TEXT)
          AND ((o.organization_id = CAST($2 AS TEXT)) OR (o.organization_id IS NULL AND $3 IS NULL) OR (o.organization_id = '0' AND $3 IS NULL))
          AND o.id = CAST($4 AS TEXT)
          AND o.status = 'active'
          AND o.deleted_at IS NULL
          AND s.status = 'active'
          AND v.lifecycle_status = 'published'
          AND (s.claim_starts_at IS NULL OR s.claim_starts_at <= $5)
          AND (s.claim_ends_at IS NULL OR s.claim_ends_at >= $6)
          AND (o.starts_at IS NULL OR o.starts_at <= $7)
          AND (o.ends_at IS NULL OR o.ends_at >= $8)
        ORDER BY s.created_at ASC, s.id ASC
        LIMIT 1

"#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&command.offer_id)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(now)
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
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $3 IS NULL) OR (organization_id = '0' AND $3 IS NULL))
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
    let row = sqlx::query(
        r#"

        SELECT id AS code_id, promotion_code AS coupon_code, expires_at AS expires_at
        FROM promotion_code
        WHERE tenant_id = CAST($1 AS TEXT)
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $3 IS NULL) OR (organization_id = '0' AND $3 IS NULL))
          AND stock_id = $4
          AND status = 'active'
          AND claimed_quantity = 0
          AND (starts_at IS NULL OR starts_at <= $5)
          AND (expires_at IS NULL OR expires_at >= $6)
        ORDER BY created_at ASC, id ASC
        LIMIT 1
        FOR UPDATE

"#,
    )
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(command.organization_id.as_deref())
    .bind(&promotion.stock_id)
    .bind(now)
    .bind(now)
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
                  AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $3 IS NULL) OR (organization_id = '0' AND $3 IS NULL))
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
            (id, tenant_id, organization_id, coupon_no, stock_id, code_id, offer_id,
             offer_version_id, subject_type, subject_id, owner_user_id, coupon_code,
             status, claimed_at, valid_from, expires_at, redeemed_at, disabled_at,
             request_no, idempotency_key, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'claimed', $13, $14, $15, NULL, NULL, $16, $17, $18, $19)

"#,
    )
    .bind(coupon_id)
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
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
    .bind(now)
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
    let balance_after = (promotion.available_quantity - 1).max(0);
    sqlx::query(
        r#"

        INSERT INTO promotion_coupon_ledger_entry
            (id, tenant_id, organization_id, ledger_no, user_coupon_id, stock_id, offer_id,
             subject_type, subject_id, direction, quantity_delta, balance_after, business_type,
             source_type, source_id, request_no, idempotency_key, created_at, updated_at)
        VALUES
            ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'debit', -1, $10, 'claim',
             $11, $12, $13, $14, $15, $16)

"#,
    )
    .bind(coupon_ledger_entry_id)
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
    .bind(claim_coupon_ledger_no(command))
    .bind(coupon_id)
    .bind(&promotion.stock_id)
    .bind(&promotion.offer_id)
    .bind(USER_SUBJECT_TYPE)
    .bind(&command.owner_user_id)
    .bind(balance_after)
    .bind(PROMOTION_USER_COUPON_SOURCE_TYPE)
    .bind(coupon_id)
    .bind(&command.request_no)
    .bind(&command.idempotency_key)
    .bind(now)
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

fn money_cents(value: &str) -> Result<i64, CommerceServiceError> {
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

fn stable_storage_id(parts: &[&str]) -> String {
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
        // 生命周期扫描：advisory lock 防多实例并发
        assert!(source.contains("pg_try_advisory_lock"));
        assert!(source.contains("pg_advisory_unlock"));
        assert!(source.contains("activate_due_member_cards"));
        assert!(source.contains("expire_due_member_cards"));
        // 排期开卡：scheduled 状态由 starts_at 决定
        assert!(source.contains("scheduled"));
        assert!(source.contains("starts_at_value"));
        // 兑换预览：不落库的 preview 链路
        assert!(source.contains("preview_promotion_code"));
        assert!(source.contains("map_redemption_preview"));
        // 兑换响应携带券标识
        assert!(source.contains("with_coupon"));
        assert!(source.contains("issued_coupon_code"));
        // 每人每码限领排除失效券（与 claim 口径一致）
        assert!(source.contains("AND status NOT IN ('expired', 'disabled', 'voided', 'cancelled')"));
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
    // 排期生效：starts_at 在未来 → scheduled，由生命周期 worker 激活；否则立即 active
    let (status, starts_at_value) = match &command.starts_at {
        Some(starts_at) if starts_at.as_str() > now.as_str() => ("scheduled", starts_at.clone()),
        _ => ("active", now.clone()),
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
             $17, to_char((CASE WHEN $18::TIMESTAMP IS NULL THEN CURRENT_TIMESTAMP
                 ELSE $18::TIMESTAMP END) AT TIME ZONE 'UTC' + make_interval(days => $19),
                 'YYYY-MM-DD HH24:MI:SS'), 0, $20, $20)
        ON CONFLICT (tenant_id, user_coupon_id) DO NOTHING
        "#,
    )
    .bind(&card_id)
    .bind(&card_uuid)
    .bind(&command.tenant_id)
    .bind(command.organization_id.as_deref())
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
    .bind(&command.starts_at)
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
    .bind(subject.organization_id)
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
          AND ((organization_id = CAST($2 AS TEXT)) OR (organization_id IS NULL AND $2 IS NULL) OR (organization_id = '0' AND $2 IS NULL))
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
    .bind(subject.organization_id)
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

/// advisory lock 键：会员卡生命周期扫描（防多实例并发）。
const MEMBER_CARD_LIFECYCLE_SWEEP_LOCK_KEY: i64 = 71_091_238_410;

/// 会员卡生命周期扫描结果。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MemberCardLifecycleSweepOutcome {
    pub activated: i64,
    pub expired: i64,
    /// true 表示本轮被 advisory lock 跳过（另一实例正在执行）。
    pub skipped: bool,
}
