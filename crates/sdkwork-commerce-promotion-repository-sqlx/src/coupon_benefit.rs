use sdkwork_commerce_promotion_service::{
    PromotionCouponBenefit, PromotionOrderCouponBenefit, PromotionSubscriptionPeriod,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_utils_rust::token_bank::MICRO_POINTS_PER_POINT;
use serde_json::Value;

/// The coupon rule stores Token Bank grant/bonus in **whole points** (what an
/// operator types and what the admin/redeem preview shows). The order and
/// account ledger carry the same balance in **micro-points** (1 point =
/// 1_000_000 micro), so the conversion happens exactly once here, when the
/// promotion benefit crosses into the order/account domain.
fn token_bank_grant_micros(points: i64) -> Result<i64, CommerceServiceError> {
    points.checked_mul(MICRO_POINTS_PER_POINT).ok_or_else(|| {
        CommerceServiceError::validation(
            "Token Bank coupon grant amount exceeds the supported range",
        )
    })
}

pub(crate) fn parse_order_coupon_benefit(
    rule_json: Option<&str>,
    legacy_discount_value: &str,
    legacy_currency_code: &str,
    replayed: bool,
) -> Result<PromotionOrderCouponBenefit, CommerceServiceError> {
    let Some(rule_json) = rule_json.map(str::trim).filter(|value| !value.is_empty()) else {
        return legacy_token_bank_benefit(legacy_discount_value, legacy_currency_code, replayed);
    };
    let root = parse_rule_json(rule_json)?;
    let benefit = root
        .get("couponBenefit")
        .or_else(|| root.get("benefit"))
        .unwrap_or(&root);
    let Some(kind) = benefit.get("kind").and_then(Value::as_str) else {
        return legacy_token_bank_benefit(legacy_discount_value, legacy_currency_code, replayed);
    };

    match kind.trim().to_ascii_lowercase().as_str() {
        "token_bank_credit" => {
            let grant_units = positive_integer(benefit, "grantAmount")?;
            let target_asset =
                optional_text(benefit, "targetAsset").unwrap_or_else(|| "token_bank".to_owned());
            if target_asset != "token_bank" {
                return Err(CommerceServiceError::validation(
                    "Token Bank coupon targetAsset must be token_bank",
                ));
            }
            // 赠送额度在订单侧合并为总发放额度（legacy 数据无 bonusAmount 时按 0 处理）
            let bonus_points = non_negative_integer(benefit, "bonusAmount")?;
            let grant_points = grant_units.checked_add(bonus_points).ok_or_else(|| {
                CommerceServiceError::validation(
                    "Token Bank coupon grant amount exceeds the supported range",
                )
            })?;
            // Rule amounts are whole points; the order/account benefit is the
            // ledger's micro-point unit.
            let grant_units = token_bank_grant_micros(grant_points)?;
            PromotionOrderCouponBenefit::new(grant_units, legacy_currency_code, replayed)
        }
        "points_credit" => {
            let grant_points = positive_integer(benefit, "grantPoints")?;
            PromotionOrderCouponBenefit::points_credit(grant_points, replayed)
        }
        "cash_credit" => {
            let grant_units = positive_integer(benefit, "grantAmount")?;
            PromotionOrderCouponBenefit::cash_credit(grant_units, legacy_currency_code, replayed)
        }
        "subscription" => {
            let product_id = required_text(benefit, "productId")?;
            let sku_id = required_text(benefit, "skuId")?;
            let package_id = positive_integer(benefit, "packageId")?;
            let period = PromotionSubscriptionPeriod::parse(&required_text(benefit, "period")?)?;
            // The order side turns a subscription coupon into a membership
            // grant, and the membership contract only accepts day/week/month/
            // year. Rejecting a quarter coupon here fails *before* the coupon is
            // consumed instead of after, when the grant is refused.
            if period == PromotionSubscriptionPeriod::Quarter {
                return Err(CommerceServiceError::validation(
                    "subscription coupon period quarter is not supported by membership fulfilment; use month with 90-92 days",
                ));
            }
            let duration_days = positive_integer(benefit, "durationDays")?;
            PromotionOrderCouponBenefit::subscription(
                &product_id,
                &sku_id,
                package_id,
                period,
                duration_days,
                positive_integer(benefit, "dailyQuota")?,
                positive_integer(benefit, "totalQuota")?,
                replayed,
            )
        }
        _ => Err(CommerceServiceError::validation(
            "promotion coupon benefit kind must be token_bank_credit, points_credit, cash_credit, or subscription",
        )),
    }
}

pub(crate) fn parse_admin_coupon_benefit(
    rule_json: Option<&str>,
) -> Result<Option<PromotionCouponBenefit>, CommerceServiceError> {
    let Some(rule_json) = rule_json.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let root = parse_rule_json(rule_json)?;
    let Some(benefit) = root.get("couponBenefit").or_else(|| root.get("benefit")) else {
        return Ok(None);
    };
    let kind = required_text(benefit, "kind")?;
    match kind.trim().to_ascii_lowercase().as_str() {
        "token_bank_credit" => {
            // The order-side parser rejects a non-token-bank target asset; the
            // redemption parser must agree, otherwise a rule that is invalid on
            // one path is honoured on the other.
            if let Some(target_asset) = optional_text(benefit, "targetAsset") {
                if target_asset != "token_bank" {
                    return Err(CommerceServiceError::validation(
                        "Token Bank coupon targetAsset must be token_bank",
                    ));
                }
            }
            Ok(Some(PromotionCouponBenefit::token_bank_credit_with_bonus(
                positive_integer(benefit, "grantAmount")?,
                non_negative_integer(benefit, "bonusAmount")?,
            )?))
        }
        "points_credit" => Ok(Some(PromotionCouponBenefit::points_credit(
            positive_integer(benefit, "grantPoints")?,
        )?)),
        "cash_credit" => Ok(Some(PromotionCouponBenefit::cash_credit(
            positive_integer(benefit, "grantAmount")?,
        )?)),
        "subscription" => Ok(Some(PromotionCouponBenefit::subscription_with_package(
            optional_text(benefit, "productId").unwrap_or_default().as_str(),
            optional_text(benefit, "skuId").unwrap_or_default().as_str(),
            optional_positive_integer(benefit, "packageId")?,
            PromotionSubscriptionPeriod::parse(&required_text(benefit, "period")?)?,
            positive_integer(benefit, "durationDays")?,
            positive_integer(benefit, "dailyQuota")?,
            positive_integer(benefit, "totalQuota")?,
        )?)),
        _ => Err(CommerceServiceError::validation(
            "promotion coupon benefit kind must be token_bank_credit, points_credit, cash_credit, or subscription",
        )),
    }
}

pub(crate) fn serialize_admin_coupon_benefit(
    benefit: Option<&PromotionCouponBenefit>,
) -> Result<Option<String>, CommerceServiceError> {
    let Some(benefit) = benefit else {
        return Ok(None);
    };
    benefit.validate()?;
    let coupon_benefit = match benefit {
        PromotionCouponBenefit::TokenBankCredit {
            grant_amount,
            bonus_amount,
        } => {
            let mut value = serde_json::json!({
                "kind": "token_bank_credit",
                "targetAsset": "token_bank",
                "grantAmount": grant_amount.to_string(),
            });
            if *bonus_amount > 0 {
                value["bonusAmount"] = serde_json::json!(bonus_amount.to_string());
            }
            value
        }
        PromotionCouponBenefit::PointsCredit { grant_points } => serde_json::json!({
            "kind": "points_credit",
            "grantPoints": grant_points.to_string(),
        }),
        PromotionCouponBenefit::CashCredit { grant_amount } => serde_json::json!({
            "kind": "cash_credit",
            "grantAmount": grant_amount.to_string(),
        }),
        PromotionCouponBenefit::Subscription {
            product_id,
            sku_id,
            package_id,
            period,
            duration_days,
            daily_quota,
            total_quota,
        } => {
            let mut value = serde_json::json!({
                "kind": "subscription",
                "period": period.as_str(),
                "durationDays": duration_days,
                "dailyQuota": daily_quota.to_string(),
                "totalQuota": total_quota.to_string(),
            });
            // Persist the membership package identity only when the admin pinned
            // one; the order-side redemption requires it, the claim path does not.
            if !product_id.is_empty() && !sku_id.is_empty() && *package_id > 0 {
                value["productId"] = serde_json::json!(product_id);
                value["skuId"] = serde_json::json!(sku_id);
                value["packageId"] = serde_json::json!(package_id.to_string());
            }
            value
        }
    };
    serde_json::to_string(&serde_json::json!({ "couponBenefit": coupon_benefit }))
        .map(Some)
        .map_err(|error| {
            CommerceServiceError::storage(format!(
                "failed to serialize promotion coupon benefit: {error}"
            ))
        })
}

fn parse_rule_json(rule_json: &str) -> Result<Value, CommerceServiceError> {
    serde_json::from_str(rule_json).map_err(|error| {
        CommerceServiceError::validation(format!(
            "promotion coupon benefit rule is invalid JSON: {error}"
        ))
    })
}

fn legacy_token_bank_benefit(
    discount_value: &str,
    currency_code: &str,
    replayed: bool,
) -> Result<PromotionOrderCouponBenefit, CommerceServiceError> {
    // Legacy rules encode the amount in currency; `legacy_coupon_credit_units`
    // converts it to whole points, then the account boundary applies the
    // point → micro-point scale.
    let grant_points = legacy_coupon_credit_units(discount_value)?;
    PromotionOrderCouponBenefit::new(
        token_bank_grant_micros(grant_points)?,
        currency_code,
        replayed,
    )
}

fn legacy_coupon_credit_units(value: &str) -> Result<i64, CommerceServiceError> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.starts_with('-') || normalized.starts_with('+') {
        return Err(CommerceServiceError::validation(
            "legacy coupon discount value must be a non-negative money amount",
        ));
    }
    let mut parts = normalized.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.chars().all(|character| character.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return Err(CommerceServiceError::validation(
            "legacy coupon discount value must be a non-negative money amount",
        ));
    }
    let whole_cents = whole
        .parse::<i64>()
        .ok()
        .and_then(|value| value.checked_mul(100))
        .ok_or_else(|| {
            CommerceServiceError::validation(
                "legacy coupon discount value exceeds the supported range",
            )
        })?;
    let fraction_cents = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().unwrap_or_default() * 10,
        _ => fraction.parse::<i64>().unwrap_or_default(),
    };
    let cents = whole_cents.checked_add(fraction_cents).ok_or_else(|| {
        CommerceServiceError::validation("legacy coupon discount value exceeds the supported range")
    })?;
    if cents <= 0 {
        return Err(CommerceServiceError::validation(
            "coupon grant amount must be greater than zero",
        ));
    }
    Ok((cents / 10).max(1))
}

fn positive_integer(value: &Value, field: &str) -> Result<i64, CommerceServiceError> {
    value
        .get(field)
        .and_then(|raw| {
            raw.as_i64().or_else(|| {
                raw.as_str()
                    .and_then(|text| text.trim().parse::<i64>().ok())
            })
        })
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            CommerceServiceError::validation(format!(
                "promotion coupon benefit {field} must be a positive integer"
            ))
        })
}

/// Optional integer that must be positive when present (missing means 0).
fn optional_positive_integer(value: &Value, field: &str) -> Result<i64, CommerceServiceError> {
    match value.get(field) {
        None => Ok(0),
        Some(_) => positive_integer(value, field),
    }
}

fn non_negative_integer(value: &Value, field: &str) -> Result<i64, CommerceServiceError> {    // 可选字段（如 bonusAmount）：缺失视为 0，保持与旧数据兼容
    let Some(raw) = value.get(field) else {
        return Ok(0);
    };
    raw.as_i64()
        .or_else(|| raw.as_str().and_then(|text| text.trim().parse::<i64>().ok()))
        .filter(|value| *value >= 0)
        .ok_or_else(|| {
            CommerceServiceError::validation(format!(
                "promotion coupon benefit {field} must be a non-negative integer"
            ))
        })
}

fn required_text(value: &Value, field: &str) -> Result<String, CommerceServiceError> {
    optional_text(value, field).ok_or_else(|| {
        CommerceServiceError::validation(format!("promotion coupon benefit {field} is required"))
    })
}

fn optional_text(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_commerce_promotion_service::PromotionOrderCouponBenefitKind;

    #[test]
    fn parses_subscription_coupon_with_daily_and_total_quota() {
        let rule = r#"{"couponBenefit":{"kind":"subscription","productId":"membership","skuId":"sku-pro-month","packageId":"1002","period":"month","durationDays":30,"dailyQuota":"1000","totalQuota":"30000"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect("subscription benefit");
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::Subscription {
                package_id: 1002,
                daily_quota: 1000,
                total_quota: 30000,
                ..
            }
        ));
    }

    #[test]
    fn rejects_daily_coupon_when_total_differs_from_daily_quota() {
        let rule = r#"{"couponBenefit":{"kind":"subscription","productId":"membership","skuId":"sku-day","packageId":1001,"period":"day","durationDays":1,"dailyQuota":100,"totalQuota":200}}"#;
        assert!(parse_order_coupon_benefit(Some(rule), "0", "CNY", false).is_err());
    }

    #[test]
    fn keeps_legacy_empty_rule_as_token_bank_credit() {
        let benefit =
            parse_order_coupon_benefit(Some("{}"), "500.00", "CNY", true).expect("legacy benefit");
        assert!(benefit.replayed);
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::TokenBankCredit {
                grant_units: 5_000_000_000,
                ..
            }
        ));
    }

    #[test]
    fn admin_subscription_package_reaches_the_order_parser() {
        // The admin console must be able to pin the membership package, because
        // the order-side redemption requires productId/skuId/packageId. Before
        // this field set existed, every admin-created subscription coupon was
        // rejected by the order path with "productId is required".
        let benefit = PromotionCouponBenefit::subscription_with_package(
            "membership",
            "sku-pro-month",
            1002,
            PromotionSubscriptionPeriod::Month,
            30,
            1000,
            30000,
        )
        .expect("admin benefit with package");
        let rule_json = serialize_admin_coupon_benefit(Some(&benefit))
            .expect("serialize benefit")
            .expect("rule json");
        assert!(rule_json.contains("\"productId\":\"membership\""), "{rule_json}");
        assert!(rule_json.contains("\"skuId\":\"sku-pro-month\""), "{rule_json}");
        assert!(rule_json.contains("\"packageId\":\"1002\""), "{rule_json}");
        assert_eq!(
            parse_admin_coupon_benefit(Some(&rule_json)).expect("parse benefit"),
            Some(benefit)
        );
        let order_benefit = parse_order_coupon_benefit(Some(&rule_json), "0", "CNY", false)
            .expect("order side accepts the pinned package");
        assert!(matches!(
            order_benefit.kind,
            PromotionOrderCouponBenefitKind::Subscription {
                package_id: 1002,
                ..
            }
        ));
    }

    #[test]
    fn partial_subscription_package_is_rejected_at_write_time() {
        let error = PromotionCouponBenefit::subscription_with_package(
            "membership",
            "",
            1002,
            PromotionSubscriptionPeriod::Month,
            30,
            1000,
            30000,
        )
        .expect_err("partial package must be rejected");
        assert_eq!("validation", error.code());
    }

    #[test]
    fn order_path_rejects_quarter_subscription_before_consuming_the_coupon() {
        // Membership fulfilment has no quarter period; failing at parse time
        // keeps the coupon unconsumed.
        let rule = r#"{"couponBenefit":{"kind":"subscription","productId":"membership","skuId":"sku-q","packageId":"1003","period":"quarter","durationDays":90,"dailyQuota":100,"totalQuota":9000}}"#;
        let error = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect_err("quarter must be rejected on the order path");
        assert_eq!("validation", error.code());
        // The promotion-side parser still accepts it for the member-card path.
        assert!(parse_admin_coupon_benefit(Some(rule)).is_ok());
    }

    #[test]
    fn admin_subscription_benefit_round_trips_through_canonical_rule_json() {        let benefit = PromotionCouponBenefit::subscription(
            PromotionSubscriptionPeriod::Month,
            30,
            1000,
            30000,
        )
        .expect("admin benefit");
        let rule_json = serialize_admin_coupon_benefit(Some(&benefit))
            .expect("serialize benefit")
            .expect("rule json");
        assert_eq!(
            parse_admin_coupon_benefit(Some(&rule_json)).expect("parse benefit"),
            Some(benefit)
        );
    }

    #[test]
    fn admin_parser_accepts_legacy_subscription_rule_with_package_fields() {
        // 存量订阅券规则含套餐字段（productId/skuId/packageId），新模型解析时忽略
        let rule = r#"{"couponBenefit":{"kind":"subscription","productId":"membership","skuId":"sku-pro-month","packageId":"1002","period":"month","durationDays":30,"dailyQuota":"1000","totalQuota":"30000"}}"#;
        let benefit = parse_admin_coupon_benefit(Some(rule))
            .expect("legacy subscription benefit")
            .expect("benefit");
        assert!(matches!(
            benefit,
            PromotionCouponBenefit::Subscription {
                period: PromotionSubscriptionPeriod::Month,
                duration_days: 30,
                daily_quota: 1000,
                total_quota: 30000,
                ..
            }
        ));
    }

    #[test]
    fn admin_parser_preserves_legacy_rule_without_typed_coupon_benefit() {
        assert_eq!(
            parse_admin_coupon_benefit(Some(r#"{"minimumQuantity":2}"#)).expect("legacy rule"),
            None
        );
    }

    #[test]
    fn token_bank_bonus_is_merged_into_order_grant_units() {
        let rule = r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"500","bonusAmount":"50"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect("token bank benefit with bonus");
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::TokenBankCredit {
                grant_units: 550_000_000,
                ..
            }
        ));
    }

    #[test]
    fn token_bank_grant_is_converted_from_points_to_account_micro_units() {
        // The reported incident: an operator authored a 110-point coupon and
        // the wallet credited 0.00011 points because 110 was written straight
        // into the micro-point ledger. The order benefit must carry
        // 110 * 1_000_000 micro so the account credits 110 points.
        let rule = r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect("token bank benefit without bonus");
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::TokenBankCredit {
                grant_units: 110_000_000,
                ..
            }
        ));
    }

    #[test]
    fn token_bank_grant_with_bonus_scales_the_merged_total() {
        let rule = r#"{"couponBenefit":{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"110","bonusAmount":"20"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect("token bank benefit with bonus");
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::TokenBankCredit {
                grant_units: 130_000_000,
                ..
            }
        ));
    }

    #[test]
    fn token_bank_grant_rejects_a_points_overflow_before_consuming_the_coupon() {
        let rule = format!(
            r#"{{"couponBenefit":{{"kind":"token_bank_credit","targetAsset":"token_bank","grantAmount":"{}"}}}}"#,
            i64::MAX
        );
        let error = parse_order_coupon_benefit(Some(&rule), "0", "CNY", false)
            .expect_err("a point value that overflows micro-points must be rejected");
        assert_eq!("validation", error.code());
    }

    #[test]
    fn order_points_credit_parses_grant_points() {
        let rule = r#"{"couponBenefit":{"kind":"points_credit","grantPoints":"1000"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "CNY", false)
            .expect("points benefit");
        assert!(matches!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::PointsCredit { grant_points: 1000 }
        ));
    }

    #[test]
    fn order_cash_credit_parses_grant_amount_with_offer_currency() {
        let rule = r#"{"couponBenefit":{"kind":"cash_credit","grantAmount":"100"}}"#;
        let benefit = parse_order_coupon_benefit(Some(rule), "0", "USD", false)
            .expect("cash benefit");
        assert_eq!(
            benefit.kind,
            PromotionOrderCouponBenefitKind::CashCredit {
                grant_units: 100,
                currency_code: "USD".to_owned(),
            }
        );
    }

    #[test]
    fn admin_points_and_cash_benefits_round_trip_through_canonical_rule_json() {
        for benefit in [
            PromotionCouponBenefit::points_credit(1000).expect("points benefit"),
            PromotionCouponBenefit::cash_credit(100).expect("cash benefit"),
            PromotionCouponBenefit::token_bank_credit_with_bonus(500, 50)
                .expect("token bank benefit"),
        ] {
            let rule_json = serialize_admin_coupon_benefit(Some(&benefit))
                .expect("serialize benefit")
                .expect("rule json");
            assert_eq!(
                parse_admin_coupon_benefit(Some(&rule_json)).expect("parse benefit"),
                Some(benefit)
            );
        }
    }
}
