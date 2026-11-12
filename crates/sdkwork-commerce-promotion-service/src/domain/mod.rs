use sdkwork_contract_service::{CommerceMoney, CommerceServiceError, PromotionCouponStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromotionDiscount {
    FixedAmount(CommerceMoney),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOfferDraft {
    pub offer_code: String,
    pub offer_no: String,
    pub offer_type: String,
    pub tenant_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOfferVersionDraft {
    pub discount: PromotionDiscount,
    pub offer_id: String,
    pub tenant_id: String,
    pub version_no: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionUserCouponTransition {
    from: PromotionCouponStatus,
    to: PromotionCouponStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionUserCouponClaimDraft {
    pub idempotency_key: String,
    pub offer_id: String,
    pub subject_id: String,
    pub subject_type: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDiscountApplicationDraft {
    pub idempotency_key: String,
    pub order_id: String,
    pub subject_id: String,
    pub subject_type: String,
    pub tenant_id: String,
    pub user_coupon_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionUserCouponItem {
    pub id: String,
    pub code: String,
    pub amount: CommerceMoney,
    pub date: String,
    pub status: String,
    /// Frozen asset benefit kind (`token_bank_credit`, `points_credit`,
    /// `cash_credit`, `subscription`) derived from the offer version.
    pub benefit_kind: Option<String>,
    /// Benefit quantity in its own unit: whole Compute Credits, whole
    /// membership points, cash minor units, or subscription quota.
    pub benefit_amount: Option<String>,
    /// ISO-4217 currency for a cash benefit; `None` for non-cash units.
    pub benefit_currency_code: Option<String>,
}

/// One claimable catalogue offer for the app surface. It intentionally exposes
/// only what the coupon centre needs: display copy, claim window, remaining
/// stock, per-user limit, and the frozen benefit terms.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOfferCatalogItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub offer_type: String,
    pub combinability: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub discount_value: Option<String>,
    pub minimum_amount: Option<String>,
    pub currency_code: Option<String>,
    pub coupon_benefit: Option<PromotionCouponBenefit>,
    pub total_quantity: Option<i64>,
    pub available_quantity: i64,
    pub per_user_limit: i64,
    pub claim_starts_at: Option<String>,
    pub claim_ends_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAssetGrantSummary {
    /// `token_bank_credit`, `points_credit`, `cash_credit`, or `subscription`.
    pub benefit_kind: String,
    /// ISO-4217 code for a cash grant; `None` for non-currency units.
    pub currency_code: Option<String>,
    pub grant_count: i64,
    /// Sum in the benefit's own unit: micro-points, whole points, cash minor
    /// units, or subscription quota.
    pub total_amount: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAssetGrantReportQuery {
    pub tenant_id: String,
    pub organization_id: Option<String>,
    pub from: String,
    pub to: String,
}

impl PromotionAssetGrantReportQuery {
    pub fn new(
        tenant_id: &str,
        organization_id: Option<&str>,
        from: &str,
        to: &str,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("tenant_id", tenant_id)?;
        Ok(Self {
            tenant_id: tenant_id.trim().to_string(),
            organization_id: organization_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            from: from.trim().to_string(),
            to: to.trim().to_string(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOfferCatalogQuery {
    pub tenant_id: String,
    pub organization_id: Option<String>,
}

impl PromotionOfferCatalogQuery {
    pub fn new(
        tenant_id: &str,
        organization_id: Option<&str>,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("tenant_id", tenant_id)?;
        Ok(Self {
            tenant_id: tenant_id.trim().to_string(),
            organization_id: organization_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsBalance {
    pub available_points: i64,
    pub frozen_points: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointsHistoryItem {
    pub id: String,
    pub amount: i64,
    pub direction: String,
    pub balance_after: i64,
    pub business_type: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppCommerceExchangeRuleItem {
    pub id: String,
    pub rate: String,
    pub source_asset_type: String,
    pub status: String,
    pub target_asset_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeRedemptionOutcome {
    pub message: String,
    pub amount: CommerceMoney,
    pub credited_points: i64,
    pub balance: i64,
    /// 兑现的权益类型（token_bank_credit/points_credit/cash_credit/subscription）。
    pub benefit_kind: Option<String>,
    /// 入账金额（最小单位字符串；会员卡场景为空）。
    pub credited_amount: Option<String>,
    /// 入账资产类型（token_bank/points/cash；会员卡场景为空）。
    pub asset_type: Option<String>,
    pub member_card_id: Option<String>,
    pub member_card_no: Option<String>,
    /// 兑换生成的用户券 ID 与券码。
    pub user_coupon_id: String,
    pub coupon_code: String,
}

/// 兑换码预览结果：不落库、不消耗库存，展示可得权益。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeRedemptionPreview {
    pub benefit_kind: Option<String>,
    pub credited_amount: Option<String>,
    pub asset_type: Option<String>,
    pub period: Option<String>,
    pub duration_days: Option<i64>,
    pub daily_quota: Option<i64>,
    pub total_quota: Option<i64>,
    pub expires_at: Option<String>,
}

/// 会员卡（订阅权益券兑现的独立载体）：承载每日使用限额与总额度。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionMemberCard {
    pub id: String,
    pub card_no: String,
    pub offer_id: String,
    pub offer_version_id: String,
    pub user_coupon_id: String,
    pub owner_user_id: String,
    pub period: PromotionSubscriptionPeriod,
    pub duration_days: i64,
    pub daily_quota: i64,
    pub total_quota: i64,
    pub total_used: i64,
    pub status: String,
    pub starts_at: String,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 会员卡额度消耗流水。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionMemberCardConsumption {
    pub id: String,
    pub card_id: String,
    pub amount: i64,
    pub balance_after: i64,
    pub business_type: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub occurred_at: String,
}

/// 会员卡额度消耗结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberCardConsumptionOutcome {
    pub accepted: bool,
    pub replayed: bool,
    pub card_id: String,
    pub consumed_amount: i64,
    pub used_today: i64,
    pub daily_quota: i64,
    pub total_used: i64,
    pub total_quota: i64,
    pub balance: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionSubscriptionPeriod {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromotionCouponBenefit {
    TokenBankCredit {
        grant_amount: i64,
        bonus_amount: i64,
    },
    PointsCredit {
        grant_points: i64,
    },
    CashCredit {
        grant_amount: i64,
    },
    Subscription {
        /// Membership package identity the coupon grants. Empty when the admin
        /// has not pinned one yet; the order-side redemption path requires it.
        product_id: String,
        sku_id: String,
        package_id: i64,
        period: PromotionSubscriptionPeriod,
        duration_days: i64,
        daily_quota: i64,
        total_quota: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromotionOrderCouponBenefitKind {
    TokenBankCredit {
        /// Token Bank account **minor units** (micro-points, 1 point =
        /// 1_000_000 micro). The coupon rule stores whole points; the
        /// repository scales grant + bonus once when building this value.
        grant_units: i64,
        currency_code: String,
    },
    PointsCredit {
        grant_points: i64,
    },
    CashCredit {
        grant_units: i64,
        currency_code: String,
    },
    Subscription {
        product_id: String,
        sku_id: String,
        package_id: i64,
        period: PromotionSubscriptionPeriod,
        duration_days: i64,
        daily_quota: i64,
        total_quota: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOrderCouponBenefit {
    pub kind: PromotionOrderCouponBenefitKind,
    pub replayed: bool,
}

impl PromotionSubscriptionPeriod {
    pub fn parse(value: &str) -> Result<Self, CommerceServiceError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "day" | "daily" => Ok(Self::Day),
            "week" | "weekly" => Ok(Self::Week),
            "month" | "monthly" => Ok(Self::Month),
            "quarter" | "quarterly" => Ok(Self::Quarter),
            "year" | "yearly" => Ok(Self::Year),
            _ => Err(CommerceServiceError::validation(
                "promotion subscription coupon period must be day, week, month, quarter, or year",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Quarter => "quarter",
            Self::Year => "year",
        }
    }

    pub fn validate_duration_days(self, duration_days: i64) -> Result<(), CommerceServiceError> {
        let valid = match self {
            Self::Day => duration_days == 1,
            Self::Week => duration_days == 7,
            Self::Month => (28..=31).contains(&duration_days),
            Self::Quarter => (90..=92).contains(&duration_days),
            Self::Year => (365..=366).contains(&duration_days),
        };
        if valid {
            Ok(())
        } else {
            Err(CommerceServiceError::validation(format!(
                "promotion {} subscription coupon duration_days is invalid",
                self.as_str()
            )))
        }
    }
}

impl PromotionCouponBenefit {
    pub fn token_bank_credit(grant_amount: i64) -> Result<Self, CommerceServiceError> {
        Self::token_bank_credit_with_bonus(grant_amount, 0)
    }

    pub fn token_bank_credit_with_bonus(
        grant_amount: i64,
        bonus_amount: i64,
    ) -> Result<Self, CommerceServiceError> {
        let benefit = Self::TokenBankCredit {
            grant_amount,
            bonus_amount,
        };
        benefit.validate()?;
        Ok(benefit)
    }

    pub fn points_credit(grant_points: i64) -> Result<Self, CommerceServiceError> {
        let benefit = Self::PointsCredit { grant_points };
        benefit.validate()?;
        Ok(benefit)
    }

    pub fn cash_credit(grant_amount: i64) -> Result<Self, CommerceServiceError> {
        let benefit = Self::CashCredit { grant_amount };
        benefit.validate()?;
        Ok(benefit)
    }

    pub fn subscription(
        period: PromotionSubscriptionPeriod,
        duration_days: i64,
        daily_quota: i64,
        total_quota: i64,
    ) -> Result<Self, CommerceServiceError> {
        Self::subscription_with_package(
            "",
            "",
            0,
            period,
            duration_days,
            daily_quota,
            total_quota,
        )
    }

    /// Subscription coupon pinned to a concrete membership package.
    ///
    /// The order-side redemption path turns this benefit into a membership
    /// grant and therefore needs the package identity; a coupon without it is
    /// redeemable in promotion but rejected before the coupon is consumed.
    pub fn subscription_with_package(
        product_id: &str,
        sku_id: &str,
        package_id: i64,
        period: PromotionSubscriptionPeriod,
        duration_days: i64,
        daily_quota: i64,
        total_quota: i64,
    ) -> Result<Self, CommerceServiceError> {
        let benefit = Self::Subscription {
            product_id: product_id.trim().to_owned(),
            sku_id: sku_id.trim().to_owned(),
            package_id,
            period,
            duration_days,
            daily_quota,
            total_quota,
        };
        benefit.validate()?;
        Ok(benefit)
    }

    /// Membership package identity of a subscription coupon, when pinned.
    pub fn subscription_package(&self) -> Option<(&str, &str, i64)> {
        match self {
            Self::Subscription {
                product_id,
                sku_id,
                package_id,
                ..
            } if !product_id.is_empty() && !sku_id.is_empty() && *package_id > 0 => {
                Some((product_id.as_str(), sku_id.as_str(), *package_id))
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), CommerceServiceError> {
        match self {
            Self::TokenBankCredit {
                grant_amount,
                bonus_amount,
            } => {
                if *grant_amount <= 0 {
                    return Err(CommerceServiceError::validation(
                        "promotion Token Bank coupon grant amount must be greater than zero",
                    ));
                }
                if *bonus_amount < 0 {
                    return Err(CommerceServiceError::validation(
                        "promotion Token Bank coupon bonus amount must not be negative",
                    ));
                }
            }
            Self::PointsCredit { grant_points } => {
                if *grant_points <= 0 {
                    return Err(CommerceServiceError::validation(
                        "promotion points coupon grant points must be greater than zero",
                    ));
                }
            }
            Self::CashCredit { grant_amount } => {
                if *grant_amount <= 0 {
                    return Err(CommerceServiceError::validation(
                        "promotion cash coupon grant amount must be greater than zero",
                    ));
                }
            }
            Self::Subscription {
                product_id,
                sku_id,
                package_id,
                period,
                duration_days,
                daily_quota,
                total_quota,
            } => {
                // Package identity is optional, but a partially specified one is
                // a configuration error: the order path needs all three or none.
                let package_declared = !product_id.trim().is_empty()
                    || !sku_id.trim().is_empty()
                    || *package_id != 0;
                if package_declared
                    && (product_id.trim().is_empty()
                        || sku_id.trim().is_empty()
                        || *package_id <= 0)
                {
                    return Err(CommerceServiceError::validation(
                        "promotion subscription coupon package requires productId, skuId, and a positive packageId",
                    ));
                }
                period.validate_duration_days(*duration_days)?;
                if *daily_quota <= 0 || *total_quota <= 0 {
                    return Err(CommerceServiceError::validation(
                        "promotion subscription coupon quotas must be greater than zero",
                    ));
                }
                if total_quota < daily_quota {
                    return Err(CommerceServiceError::validation(
                        "promotion subscription coupon total quota must not be less than daily quota",
                    ));
                }
                let maximum_usable_quota =
                    daily_quota.checked_mul(*duration_days).ok_or_else(|| {
                        CommerceServiceError::validation(
                            "promotion subscription coupon quota exceeds the supported range",
                        )
                    })?;
                if *total_quota > maximum_usable_quota {
                    return Err(CommerceServiceError::validation(
                        "promotion subscription coupon total quota must not exceed daily quota multiplied by duration days",
                    ));
                }
                if *period == PromotionSubscriptionPeriod::Day && total_quota != daily_quota {
                    return Err(CommerceServiceError::validation(
                        "daily subscription coupon total quota must equal its daily quota",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl PromotionOrderCouponBenefit {
    pub fn new(
        grant_units: i64,
        currency_code: &str,
        replayed: bool,
    ) -> Result<Self, CommerceServiceError> {
        if grant_units <= 0 {
            return Err(CommerceServiceError::validation(
                "promotion order coupon grant must be greater than zero",
            ));
        }
        require_non_empty_service("currency_code", currency_code)?;
        Ok(Self {
            kind: PromotionOrderCouponBenefitKind::TokenBankCredit {
                grant_units,
                currency_code: currency_code.trim().to_ascii_uppercase(),
            },
            replayed,
        })
    }

    pub fn points_credit(
        grant_points: i64,
        replayed: bool,
    ) -> Result<Self, CommerceServiceError> {
        if grant_points <= 0 {
            return Err(CommerceServiceError::validation(
                "promotion order coupon grant points must be greater than zero",
            ));
        }
        Ok(Self {
            kind: PromotionOrderCouponBenefitKind::PointsCredit { grant_points },
            replayed,
        })
    }

    pub fn cash_credit(
        grant_units: i64,
        currency_code: &str,
        replayed: bool,
    ) -> Result<Self, CommerceServiceError> {
        if grant_units <= 0 {
            return Err(CommerceServiceError::validation(
                "promotion order coupon grant must be greater than zero",
            ));
        }
        require_non_empty_service("currency_code", currency_code)?;
        Ok(Self {
            kind: PromotionOrderCouponBenefitKind::CashCredit {
                grant_units,
                currency_code: currency_code.trim().to_ascii_uppercase(),
            },
            replayed,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn subscription(
        product_id: &str,
        sku_id: &str,
        package_id: i64,
        period: PromotionSubscriptionPeriod,
        duration_days: i64,
        daily_quota: i64,
        total_quota: i64,
        replayed: bool,
    ) -> Result<Self, CommerceServiceError> {
        // 复用营销券订阅权益的额度契约校验（周期/时长/每日与总额度）
        PromotionCouponBenefit::subscription(
            period,
            duration_days,
            daily_quota,
            total_quota,
        )?;
        Ok(Self {
            kind: PromotionOrderCouponBenefitKind::Subscription {
                product_id: product_id.trim().to_owned(),
                sku_id: sku_id.trim().to_owned(),
                package_id,
                period,
                duration_days,
                daily_quota,
                total_quota,
            },
            replayed,
        })
    }
}

impl PromotionDiscount {
    pub fn fixed_amount(amount: CommerceMoney) -> Result<Self, CommerceServiceError> {
        if amount.as_str() == "0" || amount.as_str() == "0.0" || amount.as_str() == "0.00" {
            return Err(CommerceServiceError::validation(
                "promotion fixed discount must be greater than zero",
            ));
        }

        Ok(Self::FixedAmount(amount))
    }
}

impl PromotionOfferDraft {
    pub fn new(
        tenant_id: &str,
        offer_no: &str,
        offer_code: &str,
        name: &str,
        offer_type: &str,
    ) -> Result<Self, CommerceServiceError> {
        crate::validation::require_non_empty("tenant_id", tenant_id)?;
        crate::validation::require_non_empty("offer_no", offer_no)?;
        crate::validation::require_non_empty("offer_code", offer_code)?;
        crate::validation::require_non_empty("name", name)?;
        crate::validation::require_non_empty("offer_type", offer_type)?;

        Ok(Self {
            offer_code: offer_code.trim().to_string(),
            offer_no: offer_no.trim().to_string(),
            offer_type: offer_type.trim().to_string(),
            tenant_id: tenant_id.trim().to_string(),
            name: name.trim().to_string(),
        })
    }
}

impl PromotionOfferVersionDraft {
    pub fn new(
        tenant_id: &str,
        offer_id: &str,
        version_no: i32,
        discount: PromotionDiscount,
    ) -> Result<Self, CommerceServiceError> {
        crate::validation::require_non_empty("tenant_id", tenant_id)?;
        crate::validation::require_non_empty("offer_id", offer_id)?;
        if version_no <= 0 {
            return Err(CommerceServiceError::validation(
                "promotion offer version_no must be greater than zero",
            ));
        }

        Ok(Self {
            discount,
            offer_id: offer_id.trim().to_string(),
            tenant_id: tenant_id.trim().to_string(),
            version_no,
        })
    }
}

impl PromotionUserCouponTransition {
    pub fn new(from: PromotionCouponStatus, to: PromotionCouponStatus) -> Self {
        Self { from, to }
    }

    pub fn validate(&self) -> Result<(), CommerceServiceError> {
        match (&self.from, &self.to) {
            (PromotionCouponStatus::Draft, PromotionCouponStatus::Active)
            | (PromotionCouponStatus::Active, PromotionCouponStatus::Redeemed)
            | (PromotionCouponStatus::Active, PromotionCouponStatus::Expired)
            | (PromotionCouponStatus::Active, PromotionCouponStatus::Disabled)
            | (PromotionCouponStatus::Draft, PromotionCouponStatus::Disabled) => Ok(()),
            _ => Err(CommerceServiceError::invalid_state(
                "invalid promotion user coupon status transition",
            )),
        }
    }
}

impl PromotionUserCouponClaimDraft {
    pub fn new(
        tenant_id: &str,
        offer_id: &str,
        subject_type: &str,
        subject_id: &str,
        idempotency_key: &str,
    ) -> Result<Self, CommerceServiceError> {
        crate::validation::require_non_empty("tenant_id", tenant_id)?;
        crate::validation::require_non_empty("offer_id", offer_id)?;
        crate::validation::require_non_empty("subject_type", subject_type)?;
        crate::validation::require_non_empty("subject_id", subject_id)?;
        crate::validation::require_non_empty("idempotency_key", idempotency_key)?;

        Ok(Self {
            idempotency_key: idempotency_key.trim().to_string(),
            offer_id: offer_id.trim().to_string(),
            subject_id: subject_id.trim().to_string(),
            subject_type: subject_type.trim().to_string(),
            tenant_id: tenant_id.trim().to_string(),
        })
    }
}

impl PromotionDiscountApplicationDraft {
    pub fn new(
        tenant_id: &str,
        user_coupon_id: &str,
        order_id: &str,
        subject_type: &str,
        subject_id: &str,
        idempotency_key: &str,
    ) -> Result<Self, CommerceServiceError> {
        crate::validation::require_non_empty("tenant_id", tenant_id)?;
        crate::validation::require_non_empty("user_coupon_id", user_coupon_id)?;
        crate::validation::require_non_empty("order_id", order_id)?;
        crate::validation::require_non_empty("subject_type", subject_type)?;
        crate::validation::require_non_empty("subject_id", subject_id)?;
        crate::validation::require_non_empty("idempotency_key", idempotency_key)?;

        Ok(Self {
            idempotency_key: idempotency_key.trim().to_string(),
            order_id: order_id.trim().to_string(),
            subject_id: subject_id.trim().to_string(),
            subject_type: subject_type.trim().to_string(),
            tenant_id: tenant_id.trim().to_string(),
            user_coupon_id: user_coupon_id.trim().to_string(),
        })
    }
}

impl PromotionUserCouponItem {
    pub fn new(
        id: &str,
        code: &str,
        amount: &str,
        date: &str,
        status: &str,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("id", id)?;
        require_non_empty_service("code", code)?;
        require_non_empty_service("date", date)?;
        require_non_empty_service("status", status)?;

        Ok(Self {
            id: id.to_string(),
            code: code.to_string(),
            amount: CommerceMoney::new(amount).map_err(CommerceServiceError::validation)?,
            date: date.to_string(),
            status: status.to_string(),
            benefit_kind: None,
            benefit_amount: None,
            benefit_currency_code: None,
        })
    }

    #[must_use]
    pub fn with_benefit(
        mut self,
        kind: Option<&str>,
        amount: Option<&str>,
        currency_code: Option<&str>,
    ) -> Self {
        self.benefit_kind = kind.map(str::to_string);
        self.benefit_amount = amount.map(str::to_string);
        self.benefit_currency_code = currency_code.map(str::to_string);
        self
    }
}

impl PointsBalance {
    pub fn new(available_points: i64, frozen_points: i64) -> Result<Self, CommerceServiceError> {
        if available_points < 0 || frozen_points < 0 {
            return Err(CommerceServiceError::validation(
                "points balance must not be negative",
            ));
        }

        Ok(Self {
            available_points,
            frozen_points,
        })
    }
}

impl PointsHistoryItem {
    pub fn new(
        id: &str,
        amount: i64,
        direction: &str,
        balance_after: i64,
        business_type: &str,
        created_at: &str,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("id", id)?;
        require_non_empty_service("direction", direction)?;
        require_non_empty_service("business_type", business_type)?;
        require_non_empty_service("created_at", created_at)?;
        if amount < 0 || balance_after < 0 {
            return Err(CommerceServiceError::validation(
                "points history amounts must not be negative",
            ));
        }

        Ok(Self {
            id: id.to_string(),
            amount,
            direction: direction.to_string(),
            balance_after,
            business_type: business_type.to_string(),
            created_at: created_at.to_string(),
        })
    }
}

impl AppCommerceExchangeRuleItem {
    pub fn new(
        id: &str,
        source_asset_type: &str,
        target_asset_type: &str,
        rate: &str,
        status: &str,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("id", id)?;
        require_non_empty_service("source_asset_type", source_asset_type)?;
        require_non_empty_service("target_asset_type", target_asset_type)?;
        require_non_empty_service("rate", rate)?;
        require_non_empty_service("status", status)?;

        Ok(Self {
            id: id.to_string(),
            rate: rate.to_string(),
            source_asset_type: source_asset_type.to_string(),
            status: status.to_string(),
            target_asset_type: target_asset_type.to_string(),
        })
    }
}

impl PromotionCodeRedemptionOutcome {
    pub fn new(
        message: &str,
        amount: &str,
        credited_points: i64,
        balance: i64,
    ) -> Result<Self, CommerceServiceError> {
        require_non_empty_service("message", message)?;
        if credited_points < 0 || balance < 0 {
            return Err(CommerceServiceError::validation(
                "promotion code redemption outcome points must not be negative",
            ));
        }

        Ok(Self {
            message: message.to_string(),
            amount: CommerceMoney::new(amount).map_err(CommerceServiceError::validation)?,
            credited_points,
            balance,
            benefit_kind: None,
            credited_amount: None,
            asset_type: None,
            member_card_id: None,
            member_card_no: None,
            user_coupon_id: String::new(),
            coupon_code: String::new(),
        })
    }

    /// 附加兑换生成的券标识（用户券 ID 与券码）。
    pub fn with_coupon(mut self, user_coupon_id: String, coupon_code: String) -> Self {
        self.user_coupon_id = user_coupon_id;
        self.coupon_code = coupon_code;
        self
    }

    /// 附加权益兑现信息（按权益类型分发的入账结果）。
    #[allow(clippy::too_many_arguments)]
    pub fn with_benefit_credit(
        mut self,
        benefit_kind: Option<String>,
        credited_amount: Option<String>,
        asset_type: Option<String>,
        member_card_id: Option<String>,
        member_card_no: Option<String>,
    ) -> Self {
        self.benefit_kind = benefit_kind;
        self.credited_amount = credited_amount;
        self.asset_type = asset_type;
        self.member_card_id = member_card_id;
        self.member_card_no = member_card_no;
        self
    }
}

fn require_non_empty_service(field_name: &str, value: &str) -> Result<(), CommerceServiceError> {
    crate::validation::require_non_empty(field_name, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_coupon_rejects_quota_that_cannot_be_consumed_before_expiry() {
        assert!(PromotionCouponBenefit::subscription(
            PromotionSubscriptionPeriod::Week,
            7,
            100,
            701,
        )
        .is_err());
    }

    #[test]
    fn daily_subscription_coupon_requires_equal_daily_and_total_quota() {
        assert!(PromotionCouponBenefit::subscription(
            PromotionSubscriptionPeriod::Day,
            1,
            100,
            99,
        )
        .is_err());
    }

    #[test]
    fn quarter_subscription_period_validates_90_to_92_days() {
        assert!(PromotionSubscriptionPeriod::parse("quarter").is_ok());
        assert!(PromotionSubscriptionPeriod::parse("quarterly").is_ok());
        for days in [90, 91, 92] {
            assert!(PromotionCouponBenefit::subscription(
                PromotionSubscriptionPeriod::Quarter,
                days,
                100,
                100 * days,
            )
            .is_ok());
        }
        assert!(PromotionCouponBenefit::subscription(
            PromotionSubscriptionPeriod::Quarter,
            89,
            100,
            8900,
        )
        .is_err());
        assert!(PromotionCouponBenefit::subscription(
            PromotionSubscriptionPeriod::Quarter,
            93,
            100,
            9300,
        )
        .is_err());
    }

    #[test]
    fn token_bank_coupon_accepts_bonus_amount() {
        let benefit = PromotionCouponBenefit::token_bank_credit_with_bonus(500, 50)
            .expect("token bank benefit with bonus");
        assert_eq!(
            benefit,
            PromotionCouponBenefit::TokenBankCredit {
                grant_amount: 500,
                bonus_amount: 50,
            }
        );
    }

    #[test]
    fn token_bank_coupon_rejects_negative_bonus_amount() {
        assert!(PromotionCouponBenefit::token_bank_credit_with_bonus(500, -1).is_err());
    }

    #[test]
    fn points_coupon_rejects_non_positive_grant() {
        assert!(PromotionCouponBenefit::points_credit(0).is_err());
        assert!(PromotionCouponBenefit::points_credit(-100).is_err());
        assert_eq!(
            PromotionCouponBenefit::points_credit(1000).expect("points benefit"),
            PromotionCouponBenefit::PointsCredit { grant_points: 1000 }
        );
    }

    #[test]
    fn cash_coupon_rejects_non_positive_grant() {
        assert!(PromotionCouponBenefit::cash_credit(0).is_err());
        assert_eq!(
            PromotionCouponBenefit::cash_credit(100).expect("cash benefit"),
            PromotionCouponBenefit::CashCredit { grant_amount: 100 }
        );
    }

    #[test]
    fn order_points_credit_rejects_non_positive_grant() {
        assert!(PromotionOrderCouponBenefit::points_credit(0, false).is_err());
        let benefit =
            PromotionOrderCouponBenefit::points_credit(1000, true).expect("order points benefit");
        assert!(benefit.replayed);
        assert_eq!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::PointsCredit { grant_points: 1000 }
        );
    }

    #[test]
    fn order_cash_credit_requires_currency_code() {
        assert!(PromotionOrderCouponBenefit::cash_credit(100, "", false).is_err());
        let benefit =
            PromotionOrderCouponBenefit::cash_credit(100, "cny", false).expect("order cash benefit");
        assert_eq!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::CashCredit {
                grant_units: 100,
                currency_code: "CNY".to_owned(),
            }
        );
    }
}
