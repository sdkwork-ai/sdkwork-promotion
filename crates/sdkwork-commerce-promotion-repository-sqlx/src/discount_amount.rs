//! Pure monetary discount computation for one discount application.
//!
//! The offer version stores its terms as decimal-major strings
//! (`discount_type`, `discount_value`, `minimum_amount`,
//! `maximum_discount_amount`). Applying them used to copy `discount_value`
//! straight into the application amount, so a percentage coupon recorded its
//! percentage number as money. This module is the single arithmetic boundary
//! between those terms and the amount the order is actually discounted by.
//!
//! Rules, all evaluated in integer minor units (see [`money_cents`]):
//!
//! - `order_total_minor < minimum_amount` rejects the application.
//! - `NONE` discounts nothing (a benefit-only coupon).
//! - `FIXED` is `min(discount_value, order_total)`.
//! - `PERCENTAGE` is `round_half_up(order_total_minor * pct_basis_points / 10_000)`,
//!   then capped by `maximum_discount_amount` when present, then clamped to the
//!   order total. Basis points are the parsed `discount_value`, so `"12.5"`
//!   becomes `1250` bp = `12.50%`.
//!
//! The order total arrives as **canonical integer minor units**: the shared
//! order schema stores `commerce_order_amount_breakdown.original_amount` as
//! `NUMERIC(20,0)`, so it must not be run through `money_cents` again.
//!
//! No `f64` is ever constructed: every term goes through `money_cents`, and the
//! one multiplication uses `i128` intermediates so a large order total cannot
//! overflow before the result is clamped.

use sdkwork_contract_service::CommerceServiceError;

use crate::postgres_promotion::money_cents;

/// Basis points in one whole unit (`100%` = `10_000` bp = `1.0000`).
const PERCENT_BASIS_POINTS_DENOMINATOR: i128 = 10_000;
/// Added before the basis-point division: integer `round_half_up`.
const PERCENT_ROUNDING_HALF: i128 = PERCENT_BASIS_POINTS_DENOMINATOR / 2;

/// Computes the discount for one order, in integer minor units.
///
/// `order_total_minor` is already in minor units (order schema convention);
/// `minimum_amount`, `discount_value` and `maximum_discount_amount` are the
/// offer version's decimal-major terms.
///
/// The caller must have validated the offer terms first (a `FIXED` value of
/// `0` is only legal for a benefit-only coupon, a percentage is `0 < p <= 100`,
/// and a cap is only legal for `PERCENTAGE`). This function is intentionally
/// total over those inputs: an unknown type is a storage error because it can
/// only come from a row the validation never admitted.
pub fn compute_discount_amount(
    discount_type: &str,
    discount_value: &str,
    minimum_amount: &str,
    maximum_discount_amount: Option<&str>,
    order_total_minor: i64,
) -> Result<i64, CommerceServiceError> {
    let order_cents = order_total_minor.max(0);
    let minimum_cents = money_cents(minimum_amount)?;
    if order_cents < minimum_cents {
        return Err(CommerceServiceError::conflict(
            "order amount does not reach the coupon minimum",
        ));
    }

    let normalized_type = discount_type.trim().to_ascii_uppercase();
    let amount_minor = match normalized_type.as_str() {
        "NONE" => 0,
        "FIXED" => {
            let discount_cents = money_cents(discount_value)?;
            discount_cents.min(order_cents)
        }
        "PERCENTAGE" => {
            // `discount_value` is a percentage here; parsed as minor units it is
            // exactly its basis-point count ("12.5" -> 1250 bp).
            let basis_points = money_cents(discount_value)?;
            let numerator = i128::from(order_cents) * i128::from(basis_points);
            let rounded = (numerator + PERCENT_ROUNDING_HALF) / PERCENT_BASIS_POINTS_DENOMINATOR;
            let mut amount = i64::try_from(rounded).map_err(|_| {
                CommerceServiceError::storage("computed promotion discount amount is too large")
            })?;
            if let Some(cap) = maximum_discount_amount
                .map(str::trim)
                .filter(|cap| !cap.is_empty())
            {
                amount = amount.min(money_cents(cap)?);
            }
            amount.min(order_cents)
        }
        other => {
            return Err(CommerceServiceError::storage(format!(
                "unsupported promotion discount type: {other}"
            )))
        }
    };
    // A discount is never negative; a zero discount is valid and is still
    // recorded by the caller.
    Ok(amount_minor.max(0))
}

#[cfg(test)]
mod tests {
    use super::compute_discount_amount;

    fn discount(
        discount_type: &str,
        discount_value: &str,
        minimum_amount: &str,
        maximum_discount_amount: Option<&str>,
        order_total_minor: i64,
    ) -> i64 {
        compute_discount_amount(
            discount_type,
            discount_value,
            minimum_amount,
            maximum_discount_amount,
            order_total_minor,
        )
        .expect("valid discount terms")
    }

    #[test]
    fn fixed_discount_is_below_equal_to_and_above_the_order_total() {
        // Order total 99.99 = 9999 minor units.
        assert_eq!(discount("FIXED", "5.00", "0", None, 9_999), 500);
        // Equal: the order is free, not negative.
        assert_eq!(discount("FIXED", "99.99", "0", None, 9_999), 9_999);
        // Above: clamped to the total instead of overshooting it.
        assert_eq!(discount("FIXED", "500", "0", None, 9_999), 9_999);
        // Zero is a valid fixed amount (benefit-only coupons keep it).
        assert_eq!(discount("FIXED", "0", "0", None, 9_999), 0);
    }

    #[test]
    fn percentage_discount_rounds_half_up_to_minor_units() {
        // 33.33% of 99.99 = 33.326667 -> 33.33 (3333 minor units), not 3332.
        assert_eq!(discount("PERCENTAGE", "33.33", "0", None, 9_999), 3_333);
        // 12.5% of 100.00 = 12.50 exactly.
        assert_eq!(discount("PERCENTAGE", "12.5", "0", None, 10_000), 1_250);
        // 12.5% of 1.00 = 0.125 -> half up rounds to 0.13.
        assert_eq!(discount("PERCENTAGE", "12.5", "0", None, 100), 13);
        // 100% of the order is the order.
        assert_eq!(discount("PERCENTAGE", "100", "0", None, 9_999), 9_999);
        // Below half a minor unit: 0.01% of 1.00 = 0.0001 -> 0.
        assert_eq!(discount("PERCENTAGE", "0.01", "0", None, 100), 0);
    }

    #[test]
    fn percentage_cap_applies_before_the_order_total_clamp() {
        // 50% of 100.00 = 50.00, capped at 10.00.
        assert_eq!(discount("PERCENTAGE", "50", "0", Some("10"), 10_000), 1_000);
        // A cap above the computed amount never inflates the discount.
        assert_eq!(discount("PERCENTAGE", "10", "0", Some("50"), 10_000), 1_000);
        // The cap cannot push the discount past the order total either.
        assert_eq!(discount("PERCENTAGE", "100", "0", Some("500"), 9_999), 9_999);
    }

    #[test]
    fn minimum_threshold_is_inclusive_and_rejects_a_lower_order() {
        // Order total 50.00 equals the 50.00 minimum: allowed.
        assert_eq!(discount("FIXED", "5", "50", None, 5_000), 500);
        let error = compute_discount_amount("FIXED", "5", "50", None, 4_999)
            .expect_err("order below the minimum must be rejected");
        assert_eq!(
            error.message(),
            "order amount does not reach the coupon minimum"
        );
        // The threshold guards a benefit-only coupon as well.
        let error = compute_discount_amount("NONE", "0", "50", None, 4_999)
            .expect_err("the minimum still guards a benefit-only coupon");
        assert_eq!(
            error.message(),
            "order amount does not reach the coupon minimum"
        );
    }

    #[test]
    fn none_discounts_nothing() {
        assert_eq!(discount("NONE", "0", "0", None, 9_999), 0);
        assert_eq!(discount("NONE", "0", "0", None, 0), 0);
    }

    #[test]
    fn zero_and_negative_totals_never_yield_a_discount() {
        assert_eq!(discount("FIXED", "10", "0", None, 0), 0);
        assert_eq!(discount("PERCENTAGE", "100", "0", None, 0), 0);
        assert_eq!(discount("PERCENTAGE", "100", "0", Some("5"), 0), 0);
        // A negative total cannot come from the order schema, but the function
        // must stay total instead of discounting a refund into the negative.
        assert_eq!(discount("PERCENTAGE", "50", "0", None, -100), 0);
    }

    #[test]
    fn large_values_are_computed_in_i128_without_overflow() {
        // Largest representable order total in minor units: 100% takes it all.
        assert_eq!(
            discount("PERCENTAGE", "100", "0", None, i64::MAX),
            i64::MAX
        );
        let amount = discount("PERCENTAGE", "99.99", "0", None, i64::MAX);
        assert!(amount > 0 && amount < i64::MAX);
        // A discount value too large for minor units is a clear error, never a
        // wrap-around or a panic.
        let error =
            compute_discount_amount("FIXED", "99999999999999999", "0", None, i64::MAX)
                .expect_err("an unrepresentable discount value must be rejected");
        assert!(error.message().contains("too large"));
    }

    #[test]
    fn unsupported_discount_type_is_a_storage_error() {
        let error = compute_discount_amount("BOGUS", "1", "0", None, 1_000)
            .expect_err("an unknown type must not silently discount");
        assert!(error
            .message()
            .contains("unsupported promotion discount type"));
    }
}
