use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use sdkwork_contract_service::CommerceServiceError;

use crate::domain::PromotionCouponBenefit;

pub type PromotionAdminFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, CommerceServiceError>> + Send + 'a>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAdminScope {
    pub tenant_id: i64,
    pub organization_id: i64,
    pub user_id: String,
}

impl PromotionAdminScope {
    pub fn new(
        tenant_id: i64,
        organization_id: i64,
        user_id: impl Into<String>,
    ) -> Result<Self, CommerceServiceError> {
        let user_id = user_id.into();
        if tenant_id <= 0 {
            return Err(CommerceServiceError::validation(
                "tenant_id must be a positive integer",
            ));
        }
        if organization_id < 0 {
            return Err(CommerceServiceError::validation(
                "organization_id must not be negative",
            ));
        }
        if user_id.trim().is_empty() {
            return Err(CommerceServiceError::validation("user_id is required"));
        }
        Ok(Self {
            tenant_id,
            organization_id,
            user_id: user_id.trim().to_owned(),
        })
    }

    pub fn actor_id(&self) -> Result<i64, CommerceServiceError> {
        self.user_id
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| CommerceServiceError::validation("user_id must be numeric"))
    }
}

pub const CODE_ISSUE_MODE_REALTIME: &str = "realtime";
pub const CODE_ISSUE_MODE_BATCH: &str = "batch";

/// `promotion_audit_event.action`: one operator session revealed one code.
pub const CODE_REVEAL_AUDIT_ACTION: &str = "codes.reveal";
/// `promotion_audit_event.action`: one operator session exported a code batch.
pub const CODE_EXPORT_AUDIT_ACTION: &str = "codes.export";
/// `promotion_audit_event.aggregate_type`: a reveal targets one promotion code.
pub const CODE_AUDIT_AGGREGATE_TYPE: &str = "promotion_code";
/// `promotion_audit_event.aggregate_type`: an export targets one code batch.
pub const CODE_BATCH_AUDIT_AGGREGATE_TYPE: &str = "promotion_code_batch";
/// Hard cap on how many redeemable codes one export may return.
pub const CODE_EXPORT_MAX_ROWS: usize = 5000;
/// One row past the cap, so truncation is detectable without a second COUNT.
pub const CODE_EXPORT_FETCH_LIMIT: i64 = CODE_EXPORT_MAX_ROWS as i64 + 1;

// ---------------------------------------------------------------------------
// Stop-loss vocabulary.
//
// A leaked code and an expired wallet coupon both need an operator-visible
// terminal state. Every operator mutation is audited (`promotion_audit_event`)
// and the audit row is written in the same transaction *before* the mutation,
// exactly like the audited reveal/export operations above.
// ---------------------------------------------------------------------------

/// `promotion_audit_event.action`: one operator disabled one promotion code.
pub const CODE_DISABLE_AUDIT_ACTION: &str = "codes.disable";
/// `promotion_audit_event.action`: one operator disabled a batch's eligible codes.
pub const CODE_BATCH_DISABLE_AUDIT_ACTION: &str = "codes.batch_disable";
/// `promotion_audit_event.action`: one operator replaced one unused promotion
/// code with a newly issued code (lost/compromised code reissue). One audit row
/// covers both sides of the lineage, so it records `subject_count = 2`.
pub const CODE_REISSUE_AUDIT_ACTION: &str = "codes.reissue";
/// `promotion_audit_event.action`: one operator voided one wallet coupon.
pub const USER_COUPON_VOID_AUDIT_ACTION: &str = "user_coupons.void";
/// `promotion_audit_event.action`: one operator extended one wallet coupon's expiry.
pub const USER_COUPON_EXTEND_AUDIT_ACTION: &str = "user_coupons.extend";
/// `promotion_audit_event.aggregate_type`: a wallet-coupon operation targets one
/// `promotion_user_coupon` row.
pub const USER_COUPON_AUDIT_AGGREGATE_TYPE: &str = "promotion_user_coupon";
/// Upper bound of a stop-loss reason, matching `void_reason VARCHAR(256)`.
pub const STOP_LOSS_REASON_MAX_CHARS: usize = 256;

// ---------------------------------------------------------------------------
// Settlement lifecycle vocabulary (S4).
//
// An applied discount application used to stay `applied` forever: nothing ever
// moved the reserved budget, so the give-away was never capped and the money was
// never accounted for. The two audited operator operations below close that
// lifecycle, and the system sweep releases abandoned reservations.
//
// The audit row is written in the same transaction as the mutation and before
// it, exactly like the audited stop-loss operations above.
// ---------------------------------------------------------------------------

/// `promotion_audit_event.action`: one operator settled one discount application
/// (the reserved budget became spent).
pub const DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION: &str = "discount_applications.settle";
/// `promotion_audit_event.action`: one operator released one discount application
/// (the reserved budget went back to the available pool).
pub const DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION: &str = "discount_applications.release";
/// `promotion_audit_event.action`: the system sweep released every reservation
/// whose deadline passed. Aggregated in one row.
pub const BUDGET_RELEASE_EXPIRED_AUDIT_ACTION: &str = "budget.release_expired";
/// `promotion_audit_event.aggregate_type`: a settlement-lifecycle operation
/// targets one `promotion_discount_application` row.
pub const DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE: &str = "promotion_discount_application";



#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAdminListQuery {
    pub page: i64,
    pub page_size: i64,
    pub query: Option<String>,
    pub status: Option<String>,
    pub code_batch_id: Option<i64>,
    pub stock_id: Option<i64>,
    /// Discount-application filter: the promotion code that produced the applied
    /// user coupon (`promotion_code.id`, resolved through the user coupon). Only
    /// the discount-application list reads it; other lists ignore it, exactly
    /// like the existing `code_batch_id`/`stock_id` filters.
    pub code_id: Option<String>,
    /// Discount-application filter: `promotion_discount_application.user_coupon_id`.
    pub user_coupon_id: Option<String>,
    /// Discount-application filter: `promotion_discount_application.order_id`.
    pub order_id: Option<String>,
}

impl PromotionAdminListQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page: i64,
        page_size: i64,
        query: Option<&str>,
        status: Option<&str>,
        code_batch_id: Option<i64>,
        stock_id: Option<i64>,
        code_id: Option<&str>,
        user_coupon_id: Option<&str>,
        order_id: Option<&str>,
    ) -> Result<Self, CommerceServiceError> {
        if page < 1 {
            return Err(CommerceServiceError::validation("page must be at least 1"));
        }
        if !(1..=200).contains(&page_size) {
            return Err(CommerceServiceError::validation(
                "page_size must be between 1 and 200",
            ));
        }
        if let Some(id) = code_batch_id {
            if id <= 0 {
                return Err(CommerceServiceError::validation(
                    "code_batch_id must be positive",
                ));
            }
        }
        if let Some(id) = stock_id {
            if id <= 0 {
                return Err(CommerceServiceError::validation(
                    "stock_id must be positive",
                ));
            }
        }
        Ok(Self {
            page,
            page_size,
            query: optional_text(query),
            status: status.map(str::to_owned),
            code_batch_id,
            stock_id,
            // TEXT ids are passed through untouched; a blank value means "no
            // filter", never an empty-string equality match.
            code_id: optional_text(code_id),
            user_coupon_id: optional_text(user_coupon_id),
            order_id: optional_text(order_id),
        })
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }

    pub fn search_pattern(&self) -> String {
        format!(
            "%{}%",
            self.query
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAdminPage<T> {
    pub items: Vec<T>,
    pub total_items: i64,
}

/// `promotion_code_redemption_attempt.outcome`: the redemption completed and
/// granted the benefit.
pub const REDEMPTION_ATTEMPT_OUTCOME_SUCCEEDED: &str = "succeeded";
/// `promotion_code_redemption_attempt.outcome`: the redemption was rejected.
pub const REDEMPTION_ATTEMPT_OUTCOME_FAILED: &str = "failed";

/// Rejects an `outcome` filter outside the closed vocabulary.
///
/// The filter is a closed set (`succeeded`/`failed`). An unknown value is a
/// client error: silently ignoring it would return every attempt while the
/// caller believes it asked for one outcome (for example `outcome=failure`,
/// which used to look like a successful unfiltered query).
pub fn validate_redemption_outcome(value: &str) -> Result<(), CommerceServiceError> {
    match value.trim() {
        REDEMPTION_ATTEMPT_OUTCOME_SUCCEEDED | REDEMPTION_ATTEMPT_OUTCOME_FAILED => Ok(()),
        other => Err(CommerceServiceError::validation(format!(
            "outcome must be one of: {} / {} (found '{}')",
            REDEMPTION_ATTEMPT_OUTCOME_SUCCEEDED, REDEMPTION_ATTEMPT_OUTCOME_FAILED, other
        ))),
    }
}

/// Query for the redemption attempt ledger.
///
/// Separate from [`PromotionAdminListQuery`] because the ledger filters on its
/// own closed vocabulary (`outcome`) and on `code_id`, and has no `status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionRedemptionAttemptListQuery {
    pub page: i64,
    pub page_size: i64,
    /// Free-text search over the code suffix, subject id, and request number.
    pub query: Option<String>,
    pub outcome: Option<String>,
    pub code_id: Option<String>,
}

impl PromotionRedemptionAttemptListQuery {
    pub fn new(
        page: i64,
        page_size: i64,
        query: Option<&str>,
        outcome: Option<&str>,
        code_id: Option<&str>,
    ) -> Result<Self, CommerceServiceError> {
        if page < 1 {
            return Err(CommerceServiceError::validation("page must be at least 1"));
        }
        // Same bound as every other admin list: values outside it are rejected,
        // not clamped, so a caller cannot silently get a different page size.
        if !(1..=200).contains(&page_size) {
            return Err(CommerceServiceError::validation(
                "page_size must be between 1 and 200",
            ));
        }
        let outcome = match outcome.map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => {
                validate_redemption_outcome(value)?;
                Some(value.to_owned())
            }
            None => None,
        };
        Ok(Self {
            page,
            page_size,
            query: optional_text(query),
            outcome,
            code_id: optional_text(code_id),
        })
    }

    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.page_size
    }

    pub fn search_pattern(&self) -> String {
        format!(
            "%{}%",
            self.query
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
        )
    }
}

/// One row of the redemption attempt ledger as the backend API exposes it.
///
/// The submitted code never appears here: only its suffix is persisted and
/// returned, so the ledger cannot leak a redeemable value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionRedemptionAttemptItem {
    pub id: String,
    pub code_id: Option<String>,
    pub submitted_code_suffix: String,
    pub subject_type: String,
    pub subject_id: String,
    pub outcome: String,
    pub failure_code: Option<String>,
    pub failure_detail: Option<String>,
    pub benefit_kind: Option<String>,
    pub granted_amount: Option<String>,
    pub currency_code: Option<String>,
    pub request_no: String,
    pub order_id: Option<String>,
    pub trace_id: String,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionOverview {
    pub active_offers: i64,
    pub total_offers: i64,
    pub total_coupon_stock: i64,
    pub available_coupons: i64,
    pub claimed_coupons: i64,
    pub redeemed_coupons: i64,
    pub active_codes: i64,
    pub discount_applications: i64,
}

// ---------------------------------------------------------------------------
// Coupon-center operating report.
//
// The legacy overview is eight lifetime counters: it cannot answer "how did the
// coupon programme do last month, in which currencies, for which offers". The
// report below is windowed, carries money per currency, and ranks offers.
//
// Contract:
// - the window is `[from, to]`, inclusive, both canonical RFC 3339 instants;
// - counters are window-scoped by the row's own window column (codes by
//   `created_at`, wallet coupons by `claimed_at`, discount applications by
//   `applied_at`);
// - budget figures are a **current snapshot**, not window-filtered;
// - money is summed per currency, never across currencies;
// - every number is an int64 on the wire (decimal string), never a float.
// ---------------------------------------------------------------------------

/// Windowed coupon-centre counters.
///
/// Each counter counts rows whose window column falls inside `[from, to]`, so
/// the nine values together answer "what happened in this window". `issuedCodes`
/// is the shared denominator of [`PromotionReportRates`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromotionReportCounts {
    /// Promotion codes created in the window (`promotion_code.created_at`).
    pub issued_codes: i64,
    /// Promotion codes created in the window that are now `disabled`.
    pub disabled_codes: i64,
    /// Wallet coupons claimed in the window (`promotion_user_coupon.claimed_at`).
    pub claimed_coupons: i64,
    /// Wallet coupons claimed in the window that reached `redeemed`/`used`.
    pub redeemed_coupons: i64,
    /// Wallet coupons claimed in the window that expired.
    pub expired_coupons: i64,
    /// Wallet coupons claimed in the window that an operator voided.
    pub voided_coupons: i64,
    /// Discount applications created in the window
    /// (`promotion_discount_application.applied_at`), any status.
    pub applications_applied: i64,
    /// Applications created in the window that settled.
    pub applications_settled: i64,
    /// Applications created in the window that were released.
    pub applications_released: i64,
}

/// Integer basis-point ratios derived from [`PromotionReportCounts`].
///
/// All three rates share one denominator, `issuedCodes`, so they are directly
/// comparable and sit in `0..=10_000`:
///
/// * `claimRateBp = claimedCoupons * 10_000 / issuedCodes`
/// * `redemptionRateBp = redeemedCoupons * 10_000 / issuedCodes`
/// * `voidRateBp = voidedCoupons * 10_000 / issuedCodes`
///
/// A zero denominator yields `0`; the division is integer division (round down)
/// and clamps at `10_000`, so no float is ever constructed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromotionReportRates {
    pub claim_rate_bp: i64,
    pub redemption_rate_bp: i64,
    pub void_rate_bp: i64,
}

/// One currency row of the report's money section.
///
/// `discountAppliedMinor` / `discountSettledMinor` are windowed application
/// amounts; the three `budget*Minor` fields are a **current snapshot** of the
/// budget accounts (not window-filtered). All values are integer minor units.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromotionReportCurrencyRow {
    pub currency_code: String,
    pub discount_applied_minor: i64,
    pub discount_settled_minor: i64,
    pub budget_total_minor: i64,
    pub budget_reserved_minor: i64,
    pub budget_used_minor: i64,
}

/// One ranked offer of the report's `topOffers` section.
///
/// `redeemed` counts wallet coupons redeemed in the window; the amounts and the
/// currency come from that offer's settled applications, so a row's money always
/// belongs to the currency it reports.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromotionReportTopOffer {
    pub offer_id: String,
    pub offer_no: String,
    pub display_name: String,
    pub redeemed: i64,
    pub discount_settled_minor: i64,
    pub currency_code: String,
}

/// The coupon-centre operating report served by `promotionReports.retrieve`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromotionReport {
    /// Canonical RFC 3339 UTC start of the window (inclusive).
    pub from: String,
    /// Canonical RFC 3339 UTC end of the window (inclusive).
    pub to: String,
    pub counts: PromotionReportCounts,
    pub rates: PromotionReportRates,
    /// One row per currency; never a cross-currency total.
    pub by_currency: Vec<PromotionReportCurrencyRow>,
    /// Top offers by windowed redemption count.
    pub top_offers: Vec<PromotionReportTopOffer>,
}

/// Converts a ratio to integer basis points (`100%` = `10_000`).
///
/// `0` when the denominator is `0` (or negative: a negative denominator cannot
/// describe a share), and the result is clamped to `0..=10_000` so a numerator
/// above its denominator reports `100%` instead of an out-of-range rate. The
/// multiplication happens in `i128`, so `i64::MAX * 10_000` cannot overflow, and
/// the division truncates (rounds down) exactly like integer arithmetic on the
/// wire contract.
#[must_use]
pub fn basis_points(numerator: i64, denominator: i64) -> i64 {
    if numerator <= 0 || denominator <= 0 {
        return 0;
    }
    let scaled = i128::from(numerator) * 10_000 / i128::from(denominator);
    // `clamp` bounds the value to 0..=10_000, which always fits in `i64`.
    scaled.clamp(0, 10_000) as i64
}

/// Derives the three report rates from the windowed counters.
#[must_use]
pub fn report_rates(counts: &PromotionReportCounts) -> PromotionReportRates {
    PromotionReportRates {
        claim_rate_bp: basis_points(counts.claimed_coupons, counts.issued_codes),
        redemption_rate_bp: basis_points(counts.redeemed_coupons, counts.issued_codes),
        void_rate_bp: basis_points(counts.voided_coupons, counts.issued_codes),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCampaignItem {
    pub id: String,
    pub campaign_no: String,
    pub campaign_code: Option<String>,
    pub display_name: String,
    pub description: Option<String>,
    pub channel_scope: String,
    pub audience_scope: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub status: String,
    pub version: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCampaignInput {
    pub campaign_code: Option<String>,
    pub display_name: String,
    pub description: Option<String>,
    pub channel_scope: String,
    pub audience_scope: String,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub status: String,
    pub version: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PromotionOfferItem {
    pub id: String,
    pub campaign_id: Option<String>,
    pub offer_no: String,
    pub offer_code: Option<String>,
    pub offer_type: String,
    pub audience_scope: String,
    pub combinability: String,
    pub goods_scope: String,
    pub display_name: String,
    pub description: Option<String>,
    pub priority: i32,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub status: String,
    pub discount_type: Option<String>,
    pub discount_value: Option<String>,
    pub minimum_amount: Option<String>,
    pub maximum_discount_amount: Option<String>,
    pub currency_code: Option<String>,
    pub coupon_benefit: Option<PromotionCouponBenefit>,
    pub version: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PromotionOfferInput {
    pub campaign_id: Option<String>,
    pub offer_code: Option<String>,
    pub offer_type: String,
    pub display_name: String,
    pub description: Option<String>,
    pub audience_scope: String,
    pub combinability: String,
    pub goods_scope: String,
    pub priority: i32,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub status: String,
    pub discount_type: String,
    pub discount_value: String,
    pub minimum_amount: String,
    pub maximum_discount_amount: Option<String>,
    pub currency_code: String,
    pub coupon_benefit: Option<PromotionCouponBenefit>,
    pub version: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCouponStockItem {
    pub id: String,
    pub offer_id: String,
    pub stock_no: String,
    pub stock_type: String,
    pub code_issue_mode: String,
    pub total_quantity: i64,
    pub available_quantity: i64,
    pub claimed_quantity: i64,
    pub redeemed_quantity: i64,
    pub locked_quantity: i64,
    pub per_user_limit: i32,
    pub claim_starts_at: Option<String>,
    pub claim_ends_at: Option<String>,
    pub status: String,
    /// Hard monetary cap of the stock's budget account, in integer minor units
    /// (`promotion_budget_account.total_amount_minor`).
    ///
    /// All four budget fields are `None` when the stock has no budget account
    /// (the historical shape): the read model never invents a `0` cap. Money
    /// stays integer minor units on the wire — no float and no division by 100.
    pub budget_amount_minor: Option<i64>,
    /// Amount still available to reserve (`available_amount_minor`).
    pub budget_available_minor: Option<i64>,
    /// Amount currently held by live reservations (`reserved_amount_minor`).
    pub budget_reserved_minor: Option<i64>,
    /// Amount already spent (`used_amount_minor`).
    pub budget_used_minor: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCouponStockInput {
    pub offer_id: String,
    pub stock_type: String,
    pub code_issue_mode: String,
    pub total_quantity: i64,
    pub per_user_limit: i32,
    pub claim_starts_at: Option<String>,
    pub claim_ends_at: Option<String>,
    pub status: String,
    /// Optional hard monetary cap for this stock, in integer minor units
    /// (`promotion_coupon_stock.budget_amount_minor`). `None` means "no cap" and
    /// leaves the historical behaviour untouched: apply never reserves.
    ///
    /// Money-unit rule: minor-unit integer only. A caller that holds a
    /// decimal-major amount must convert it at its own boundary; this struct
    /// never carries a float and never divides by 100.
    pub budget_amount_minor: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeItem {
    pub id: String,
    pub stock_id: String,
    pub offer_id: String,
    pub code_no: String,
    pub promotion_code: String,
    pub code_type: String,
    pub max_claims: i32,
    pub claimed_quantity: i32,
    pub starts_at: Option<String>,
    pub expires_at: Option<String>,
    pub status: String,
}

/// One redeemable code returned by an audited reveal or export.
///
/// Only the audited operations return the redeemable value; the plain code list
/// keeps returning the historical `promotion_code` field for compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeRevealItem {
    pub code_no: String,
    pub code: String,
}

/// Result of one audited batch export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeExportItem {
    pub items: Vec<PromotionCodeRevealItem>,
    /// Number of codes actually returned (never more than [`CODE_EXPORT_MAX_ROWS`]).
    pub count: i32,
    /// True when the batch holds more codes than the cap allowed to be returned.
    pub truncated: bool,
}

/// Applies the export cap to the rows the repository fetched.
///
/// The query fetches [`CODE_EXPORT_FETCH_LIMIT`] rows (one past the cap) so
/// truncation is detectable without a second COUNT that could disagree with the
/// page actually returned. Exactly [`CODE_EXPORT_MAX_ROWS`] rows is a complete
/// export; one row more is truncated and the extra row is dropped.
pub fn finalize_code_export(mut items: Vec<PromotionCodeRevealItem>) -> PromotionCodeExportItem {
    let truncated = items.len() > CODE_EXPORT_MAX_ROWS;
    items.truncate(CODE_EXPORT_MAX_ROWS);
    // The cap is 5000, so the conversion can never fail; clamping instead of
    // panicking keeps this helper total for any caller-supplied vector.
    let count = i32::try_from(items.len()).unwrap_or(i32::MAX);
    PromotionCodeExportItem {
        items,
        count,
        truncated,
    }
}

/// Result of disabling one promotion code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeDisableItem {
    pub code_id: String,
    pub status: String,
}

/// Repository outcome of one code reissue.
///
/// The replacement's redeemable value is deliberately absent: a reissue reports
/// the new row's identity plus a display mask only. The audited `codes.reveal` /
/// `codeBatches.codeExports` operations remain the only read paths for the full
/// code, so issuing a replacement never becomes an unaudited reveal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeReissueOutcome {
    /// `promotion_code.id` of the newly inserted replacement.
    pub new_code_id: String,
    /// `promotion_code.code_no` of the newly inserted replacement.
    pub new_code_no: String,
    /// Display mask of the replacement; never equal to its value.
    pub code_masked: String,
    /// Last four characters of the replacement.
    pub code_last4: String,
}

/// Result of reissuing one lost or compromised code.
///
/// `code_id` is the *new* code; `replaced_code_id` is the disabled original, so
/// the response names both sides of the lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeReissueItem {
    pub code_id: String,
    pub code_no: String,
    pub code_masked: String,
    pub code_last4: String,
    pub replaced_code_id: String,
}

impl PromotionCodeReissueItem {
    /// Maps the repository outcome onto the service item, naming the replaced id.
    pub fn from_outcome(replaced_code_id: String, outcome: CodeReissueOutcome) -> Self {
        Self {
            code_id: outcome.new_code_id,
            code_no: outcome.new_code_no,
            code_masked: outcome.code_masked,
            code_last4: outcome.code_last4,
            replaced_code_id,
        }
    }
}

/// Result of disabling every eligible code of one batch.
///
/// `disabled_count` is the number of rows the operation actually stopped;
/// `skipped_count` is the rest of the batch (codes already claimed, or no longer
/// active) and is reported instead of failing the whole batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeBatchDisableItem {
    pub disabled_count: i64,
    pub skipped_count: i64,
}

/// Result of voiding one wallet coupon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionUserCouponVoidItem {
    pub user_coupon_id: String,
    pub status: String,
}

/// Result of extending one wallet coupon's expiry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionUserCouponExtendItem {
    pub user_coupon_id: String,
    pub expires_at: String,
}

/// Result of settling one discount application.
///
/// `settled_amount_minor` is the amount that left the reservation pool and became
/// spent, in integer minor units (int64-as-string on the wire). It is `0` when
/// the application never reserved budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDiscountApplicationSettlementItem {
    pub application_id: String,
    pub status: String,
    pub settled_amount_minor: i64,
}

/// Result of releasing one discount application back to the available pool.
///
/// `released_amount_minor` is the reservation that was returned, in integer minor
/// units (int64-as-string on the wire). It is `0` when the application never
/// reserved budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDiscountApplicationReleaseItem {
    pub application_id: String,
    pub status: String,
    pub released_amount_minor: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeBatchItem {

    pub id: String,
    pub stock_id: String,
    pub offer_id: String,
    pub batch_no: String,
    pub code_type: String,
    pub requested_quantity: i64,
    pub generated_quantity: i64,
    pub code_length: i32,
    pub code_prefix: String,
    pub starts_at: Option<String>,
    pub expires_at: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCodeBatchInput {
    pub stock_id: String,
    pub code_type: String,
    pub quantity: i64,
    pub code_length: i32,
    pub code_prefix: String,
    /// Admin-defined full coupon codes. When non-empty the batch stores these
    /// verbatim instead of generating prefix + random suffixes.
    pub custom_codes: Vec<String>,
    /// Redemption cap per code (1 = single use).
    pub max_claims: i32,
    pub starts_at: Option<String>,
    pub expires_at: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDistributionTaskItem {
    pub id: String,
    pub stock_id: String,
    pub offer_id: String,
    pub task_no: String,
    pub distribution_type: String,
    pub requested_quantity: i64,
    pub succeeded_quantity: i64,
    pub failed_quantity: i64,
    pub status: String,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDistributionInput {
    pub stock_id: String,
    pub owner_user_ids: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionAdminUserCouponItem {
    pub id: String,
    pub coupon_no: String,
    pub stock_id: String,
    pub offer_id: String,
    pub owner_user_id: String,
    pub coupon_code: String,
    pub status: String,
    pub claimed_at: String,
    pub valid_from: String,
    pub expires_at: Option<String>,
    pub redeemed_at: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionCouponLedgerItem {
    pub id: String,
    pub stock_id: String,
    pub user_coupon_id: Option<String>,
    pub offer_id: String,
    pub subject_id: Option<String>,
    pub direction: String,
    pub quantity_delta: i64,
    pub balance_after: i64,
    pub business_type: String,
    pub business_no: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionDiscountApplicationItem {
    pub id: String,
    pub application_no: String,
    pub order_id: String,
    pub order_no: Option<String>,
    pub offer_id: String,
    pub discount_type: String,
    pub discount_amount: String,
    pub currency_code: String,
    pub status: String,
    pub applied_at: String,
    pub settled_at: Option<String>,
    pub released_at: Option<String>,
    pub rolled_back_at: Option<String>,
}

pub trait PromotionAdminRepositoryPort: Send + Sync {
    fn retrieve_overview<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
    ) -> PromotionAdminFuture<'a, PromotionOverview>;
    /// Builds the coupon-centre operating report for one operator scope.
    ///
    /// `from` and `to` are the already-normalized canonical RFC 3339 UTC
    /// instants produced by [`crate::timestamps::report_window`]; the
    /// implementation MUST filter every window column through the SQL instant
    /// expression with inclusive bounds (never a text comparison), MUST apply
    /// the same tenant/organization scoping as the admin lists, and MUST sum
    /// money per currency instead of across currencies. Budget figures are a
    /// current snapshot and are therefore not window-filtered.
    fn retrieve_reports<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        from: &'a str,
        to: &'a str,
    ) -> PromotionAdminFuture<'a, PromotionReport>;
    fn list_campaigns<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCampaignItem>>;
    fn retrieve_campaign<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        campaign_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>>;
    fn create_campaign<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        input: &'a PromotionCampaignInput,
    ) -> PromotionAdminFuture<'a, PromotionCampaignItem>;
    fn update_campaign<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        campaign_id: String,
        input: &'a PromotionCampaignInput,
    ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>>;
    fn delete_campaign<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        campaign_id: String,
    ) -> PromotionAdminFuture<'a, bool>;
    fn list_offers<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionOfferItem>>;
    fn retrieve_offer<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        offer_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>>;
    fn create_offer<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        input: &'a PromotionOfferInput,
    ) -> PromotionAdminFuture<'a, PromotionOfferItem>;
    fn update_offer<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        offer_id: String,
        input: &'a PromotionOfferInput,
    ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>>;
    fn update_offer_status<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        offer_id: String,
        status: &'a str,
    ) -> PromotionAdminFuture<'a, bool>;
    fn delete_offer<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        offer_id: String,
    ) -> PromotionAdminFuture<'a, bool>;
    fn list_coupon_stocks<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponStockItem>>;
    fn create_coupon_stock<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        input: &'a PromotionCouponStockInput,
    ) -> PromotionAdminFuture<'a, PromotionCouponStockItem>;
    fn list_code_batches<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeBatchItem>>;
    fn create_code_batch<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        input: &'a PromotionCodeBatchInput,
    ) -> PromotionAdminFuture<'a, PromotionCodeBatchItem>;
    fn list_codes<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeItem>>;
    fn list_distribution_tasks<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDistributionTaskItem>>;
    fn create_distribution_task<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        input: &'a PromotionDistributionInput,
    ) -> PromotionAdminFuture<'a, PromotionDistributionTaskItem>;
    fn list_user_coupons<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionAdminUserCouponItem>>;
    fn list_coupon_ledger<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponLedgerItem>>;
    fn list_discount_applications<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDiscountApplicationItem>>;
    /// Lists code-redemption attempts (successes and failures) for one scope.
    fn list_redemption_attempts<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionRedemptionAttemptListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionRedemptionAttemptItem>>;
    /// Reveals one redeemable code, provisioned by an audit-evidence row.
    ///
    /// The implementation MUST write the `promotion_audit_event` row and read the
    /// code in the same transaction, and MUST fail the operation when the audit
    /// write fails: a redeemable code never leaves the service without evidence.
    /// `None` means the code is not in the operator's scope (404).
    fn reveal_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeRevealItem>>;
    /// Exports up to [`CODE_EXPORT_MAX_ROWS`] redeemable codes of one batch,
    /// provisioned by the same audit-evidence contract as [`Self::reveal_code`].
    /// `None` means the batch is not in the operator's scope (404).
    fn export_code_batch<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        batch_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeExportItem>>;

    /// Disables one active, never-claimed code and records `codes.disable`
    /// evidence in the same transaction.
    ///
    /// The implementation MUST write the audit row before the mutation and MUST
    /// fail the operation when that write fails. `None` means the code is not in
    /// the operator's scope (404); a claimed or non-active code is a conflict.
    fn disable_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        reason: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeDisableItem>>;

    /// Disables every eligible code of one batch and records `codes.batch_disable`
    /// evidence (`subject_count = disabled_count`) in the same transaction.
    /// `None` means the batch is not in the operator's scope (404).
    fn disable_code_batch<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        batch_id: String,
        reason: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeBatchDisableItem>>;

    /// Reissues one unused code: disables the original and inserts a replacement
    /// with lineage, recording `codes.reissue` (`subject_count = 2`, both ids and
    /// the reason in `detail_json`) in the same transaction before either
    /// mutation.
    ///
    /// The replacement inherits the original's stock, offer, offer version, code
    /// batch, code type, validity window, and `max_claims`. `explicit_code` is an
    /// already-normalized operator-supplied value; `None` generates one from the
    /// batch's prefix/length.
    ///
    /// The dedicated `codes.reissue` operation returns the new code's identity
    /// and display mask only, never its redeemable value; the audited reveal and
    /// export operations remain the only read paths. `None` means the code is not
    /// in the operator's scope (404); a claimed or non-active code is a conflict.
    fn reissue_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        reason: String,
        explicit_code: Option<String>,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<CodeReissueOutcome>>;

    /// Voids one claimed/active/pending wallet coupon and records
    /// `user_coupons.void` evidence in the same transaction.
    /// `None` means the coupon is not in the operator's scope (404).
    fn void_user_coupon<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        user_coupon_id: String,
        reason: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionUserCouponVoidItem>>;

    /// Moves one claimed/active/pending wallet coupon's expiry to `expires_at`
    /// and records `user_coupons.extend` evidence (old + new expiry in
    /// `detail_json`) in the same transaction. The future check uses the
    /// database clock. `None` means the coupon is not in the operator's scope
    /// (404).
    fn extend_user_coupon<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        user_coupon_id: String,
        expires_at: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionUserCouponExtendItem>>;

    /// Settles one `applied` discount application and records
    /// `discount_applications.settle` evidence in the same transaction.
    ///
    /// The implementation MUST write the audit row before the mutation and MUST
    /// fail the operation when that write fails. The reserved budget amount moves
    /// from `reserved` to `used` in one guarded `UPDATE`; a non-`applied`
    /// application is a `409`. `None` means the application is not in the
    /// operator's scope (404).
    fn settle_discount_application<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        application_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionDiscountApplicationSettlementItem>>;

    /// Releases one `applied` discount application and records
    /// `discount_applications.release` evidence in the same transaction.
    ///
    /// Same evidence contract as [`Self::settle_discount_application`]; the
    /// reserved budget amount moves from `reserved` back to `available`, and the
    /// reason is stored on the row (`release_reason`). A non-`applied`
    /// application is a `409`.
    fn release_discount_application<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        application_id: String,
        reason: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionDiscountApplicationReleaseItem>>;
}

#[derive(Clone)]
pub struct PromotionAdminService {
    repository: Arc<dyn PromotionAdminRepositoryPort>,
}

impl PromotionAdminService {
    pub fn new(repository: Arc<dyn PromotionAdminRepositoryPort>) -> Self {
        Self { repository }
    }

    pub async fn retrieve_overview(
        &self,
        scope: &PromotionAdminScope,
    ) -> Result<PromotionOverview, CommerceServiceError> {
        self.repository.retrieve_overview(scope).await
    }

    /// Builds the coupon-centre operating report for the last 30 days by
    /// default.
    ///
    /// The window is validated and normalized here, in the service layer, before
    /// any query runs: an unparseable bound or `from > to` is a `400` even for a
    /// scope with no rows, and the repository only ever receives canonical
    /// instants. The repository derives the rates from the counters it read with
    /// the shared integer basis-point helper, so the wire never carries a float.
    pub async fn retrieve_reports(
        &self,
        scope: &PromotionAdminScope,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<PromotionReport, CommerceServiceError> {
        let (from, to) = crate::timestamps::report_window(from, to, &crate::timestamps::now_iso())?;
        self.repository.retrieve_reports(scope, &from, &to).await
    }
    pub async fn list_campaigns(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionCampaignItem>, CommerceServiceError> {
        self.repository.list_campaigns(scope, query).await
    }
    pub async fn retrieve_campaign(
        &self,
        scope: &PromotionAdminScope,
        id: String,
    ) -> Result<Option<PromotionCampaignItem>, CommerceServiceError> {
        positive_id("campaign_id", &id)?;
        self.repository.retrieve_campaign(scope, id).await
    }
    pub async fn create_campaign(
        &self,
        scope: &PromotionAdminScope,
        input: &PromotionCampaignInput,
    ) -> Result<PromotionCampaignItem, CommerceServiceError> {
        scope.actor_id()?;
        validate_campaign(input)?;
        self.repository.create_campaign(scope, input).await
    }
    pub async fn update_campaign(
        &self,
        scope: &PromotionAdminScope,
        id: String,
        input: &PromotionCampaignInput,
    ) -> Result<Option<PromotionCampaignItem>, CommerceServiceError> {
        positive_id("campaign_id", &id)?;
        validate_campaign(input)?;
        if input.version.is_none() {
            return Err(CommerceServiceError::validation("version is required"));
        }
        self.repository.update_campaign(scope, id, input).await
    }
    pub async fn delete_campaign(
        &self,
        scope: &PromotionAdminScope,
        id: String,
    ) -> Result<bool, CommerceServiceError> {
        positive_id("campaign_id", &id)?;
        self.repository.delete_campaign(scope, id).await
    }
    pub async fn list_offers(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionOfferItem>, CommerceServiceError> {
        self.repository.list_offers(scope, query).await
    }
    pub async fn retrieve_offer(
        &self,
        scope: &PromotionAdminScope,
        id: String,
    ) -> Result<Option<PromotionOfferItem>, CommerceServiceError> {
        positive_id("offer_id", &id)?;
        self.repository.retrieve_offer(scope, id).await
    }
    pub async fn create_offer(
        &self,
        scope: &PromotionAdminScope,
        input: &PromotionOfferInput,
    ) -> Result<PromotionOfferItem, CommerceServiceError> {
        validate_offer(input)?;
        self.repository.create_offer(scope, input).await
    }
    pub async fn update_offer(
        &self,
        scope: &PromotionAdminScope,
        id: String,
        input: &PromotionOfferInput,
    ) -> Result<Option<PromotionOfferItem>, CommerceServiceError> {
        positive_id("offer_id", &id)?;
        validate_offer(input)?;
        if input.version.is_none() {
            return Err(CommerceServiceError::validation("version is required"));
        }
        self.repository.update_offer(scope, id, input).await
    }
    pub async fn update_offer_status(
        &self,
        scope: &PromotionAdminScope,
        id: String,
        status: &str,
    ) -> Result<bool, CommerceServiceError> {
        positive_id("offer_id", &id)?;
        validate_status(status)?;
        self.repository.update_offer_status(scope, id, status).await
    }
    pub async fn delete_offer(
        &self,
        scope: &PromotionAdminScope,
        id: String,
    ) -> Result<bool, CommerceServiceError> {
        positive_id("offer_id", &id)?;
        self.repository.delete_offer(scope, id).await
    }
    pub async fn list_coupon_stocks(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionCouponStockItem>, CommerceServiceError> {
        self.repository.list_coupon_stocks(scope, query).await
    }
    pub async fn create_coupon_stock(
        &self,
        scope: &PromotionAdminScope,
        input: &PromotionCouponStockInput,
    ) -> Result<PromotionCouponStockItem, CommerceServiceError> {
        positive_id("offer_id", &input.offer_id)?;
        let is_unlimited = input.stock_type.trim().eq_ignore_ascii_case("unlimited");
        if (!is_unlimited && input.total_quantity <= 0) || input.per_user_limit <= 0 {
            return Err(CommerceServiceError::validation(
                "total_quantity must be positive for limited stock; per_user_limit must be positive",
            ));
        }
        validate_status(&input.status)?;
        if !matches!(
            input.code_issue_mode.as_str(),
            CODE_ISSUE_MODE_REALTIME | CODE_ISSUE_MODE_BATCH
        ) {
            return Err(CommerceServiceError::validation(
                "code_issue_mode must be realtime or batch",
            ));
        }
        // An optional monetary cap is integer minor units and must be a real cap:
        // `0`/negative would create a budget account that can never reserve.
        if input.budget_amount_minor.is_some_and(|value| value <= 0) {
            return Err(CommerceServiceError::validation(
                "budget_amount_minor must be a positive integer when present",
            ));
        }
        self.repository.create_coupon_stock(scope, input).await
    }
    pub async fn list_code_batches(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionCodeBatchItem>, CommerceServiceError> {
        self.repository.list_code_batches(scope, query).await
    }
    pub async fn create_code_batch(
        &self,
        scope: &PromotionAdminScope,
        input: &PromotionCodeBatchInput,
    ) -> Result<PromotionCodeBatchItem, CommerceServiceError> {
        positive_id("stock_id", &input.stock_id)?;
        if !(1..=5000).contains(&input.quantity) {
            return Err(CommerceServiceError::validation(
                "quantity must be between 1 and 5000",
            ));
        }
        if input.max_claims < 1 || input.max_claims > 1_000_000 {
            return Err(CommerceServiceError::validation(
                "max_claims must be between 1 and 1000000",
            ));
        }
        if input.custom_codes.is_empty() {
            if !(12..=32).contains(&input.code_length) {
                return Err(CommerceServiceError::validation(
                    "code_length must be between 12 and 32",
                ));
            }
            validate_code_batch_prefix(&input.code_prefix, input.code_length)?;
        } else {
            validate_custom_codes(&input.custom_codes, input.quantity)?;
            if input.max_claims != 1 {
                return Err(CommerceServiceError::validation(
                    "custom codes are single-use; max_claims must be 1",
                ));
            }
        }
        required("idempotency_key", &input.idempotency_key)?;
        self.repository.create_code_batch(scope, input).await
    }
    pub async fn list_codes(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionCodeItem>, CommerceServiceError> {
        self.repository.list_codes(scope, query).await
    }
    pub async fn list_distribution_tasks(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionDistributionTaskItem>, CommerceServiceError> {
        self.repository.list_distribution_tasks(scope, query).await
    }
    pub async fn create_distribution_task(
        &self,
        scope: &PromotionAdminScope,
        input: &PromotionDistributionInput,
    ) -> Result<PromotionDistributionTaskItem, CommerceServiceError> {
        positive_id("stock_id", &input.stock_id)?;
        required("idempotency_key", &input.idempotency_key)?;
        if input.owner_user_ids.is_empty()
            || input.owner_user_ids.len() > 200
            || input.owner_user_ids.iter().any(|id| id.trim().is_empty())
        {
            return Err(CommerceServiceError::validation(
                "owner_user_ids must contain 1 to 200 non-empty ids",
            ));
        }
        let mut ids = input.owner_user_ids.clone();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != input.owner_user_ids.len() {
            return Err(CommerceServiceError::validation(
                "owner_user_ids must be unique",
            ));
        }
        self.repository.create_distribution_task(scope, input).await
    }
    pub async fn list_user_coupons(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionAdminUserCouponItem>, CommerceServiceError> {
        self.repository.list_user_coupons(scope, query).await
    }
    pub async fn list_coupon_ledger(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionCouponLedgerItem>, CommerceServiceError> {
        self.repository.list_coupon_ledger(scope, query).await
    }
    pub async fn list_discount_applications(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionAdminListQuery,
    ) -> Result<PromotionAdminPage<PromotionDiscountApplicationItem>, CommerceServiceError> {
        self.repository
            .list_discount_applications(scope, query)
            .await
    }
    /// Lists redemption attempts, newest first.
    ///
    /// Re-validates `outcome` here as well as in the query constructor: the
    /// query fields are public, so a direct struct literal must not be able to
    /// smuggle an unknown outcome past the filter.
    pub async fn list_redemption_attempts(
        &self,
        scope: &PromotionAdminScope,
        query: &PromotionRedemptionAttemptListQuery,
    ) -> Result<PromotionAdminPage<PromotionRedemptionAttemptItem>, CommerceServiceError> {
        if let Some(outcome) = query.outcome.as_deref() {
            validate_redemption_outcome(outcome)?;
        }
        if query.page < 1 || !(1..=200).contains(&query.page_size) {
            return Err(CommerceServiceError::validation(
                "page must be at least 1 and page_size must be between 1 and 200",
            ));
        }
        self.repository.list_redemption_attempts(scope, query).await
    }

    /// Reveals one redeemable code for the authenticated operator.
    ///
    /// The actor is the IAM-derived [`PromotionAdminScope`]; no request field can
    /// select it. `scope.actor_id()` is re-checked here so a session that cannot
    /// be attributed never gets a code with an anonymous audit row.
    pub async fn reveal_code(
        &self,
        scope: &PromotionAdminScope,
        code_id: String,
        trace_id: &str,
    ) -> Result<Option<PromotionCodeRevealItem>, CommerceServiceError> {
        positive_id("code_id", &code_id)?;
        scope.actor_id()?;
        self.repository
            .reveal_code(scope, code_id, audit_trace_id(trace_id))
            .await
    }

    /// Exports the redeemable codes of one batch for the authenticated operator.
    pub async fn export_code_batch(
        &self,
        scope: &PromotionAdminScope,
        batch_id: String,
        trace_id: &str,
    ) -> Result<Option<PromotionCodeExportItem>, CommerceServiceError> {
        positive_id("batch_id", &batch_id)?;
        scope.actor_id()?;
        self.repository
            .export_code_batch(scope, batch_id, audit_trace_id(trace_id))
            .await
    }

    /// Disables one leaked code for the authenticated operator.
    ///
    /// The actor is the IAM-derived [`PromotionAdminScope`], never a body field,
    /// and the reason is trimmed/bounded before it reaches the repository, so
    /// `void_reason VARCHAR(256)` can never be overrun.
    pub async fn disable_code(
        &self,
        scope: &PromotionAdminScope,
        code_id: String,
        reason: &str,
        trace_id: &str,
    ) -> Result<Option<PromotionCodeDisableItem>, CommerceServiceError> {
        positive_id("code_id", &code_id)?;
        scope.actor_id()?;
        let reason = validate_stop_loss_reason(reason)?;
        self.repository
            .disable_code(scope, code_id, reason, audit_trace_id(trace_id))
            .await
    }

    /// Disables every eligible code of one batch for the authenticated operator.
    pub async fn disable_code_batch(
        &self,
        scope: &PromotionAdminScope,
        batch_id: String,
        reason: &str,
        trace_id: &str,
    ) -> Result<Option<PromotionCodeBatchDisableItem>, CommerceServiceError> {
        positive_id("batch_id", &batch_id)?;
        scope.actor_id()?;
        let reason = validate_stop_loss_reason(reason)?;
        self.repository
            .disable_code_batch(scope, batch_id, reason, audit_trace_id(trace_id))
            .await
    }

    /// Reissues one unused code (lost, or leaked before use) for the operator.
    ///
    /// The guard is the disable guard ([`code_reissue_decision`]); the reason is
    /// trimmed/bounded and an operator-supplied replacement code is normalized and
    /// gated exactly like a custom batch code. The repository performs the
    /// disable + insert + audit in one transaction and returns the new code's
    /// identity and mask only — the redeemable value is never part of this
    /// operation's result.
    pub async fn reissue_code(
        &self,
        scope: &PromotionAdminScope,
        code_id: String,
        reason: &str,
        explicit_code: Option<&str>,
        trace_id: &str,
    ) -> Result<Option<PromotionCodeReissueItem>, CommerceServiceError> {
        positive_id("code_id", &code_id)?;
        scope.actor_id()?;
        let reason = validate_stop_loss_reason(reason)?;
        let explicit_code = match explicit_code {
            Some(code) => Some(validate_explicit_code(code)?),
            None => None,
        };
        let outcome = self
            .repository
            .reissue_code(
                scope,
                code_id.clone(),
                reason,
                explicit_code,
                audit_trace_id(trace_id),
            )
            .await?;
        Ok(outcome.map(|outcome| PromotionCodeReissueItem::from_outcome(code_id, outcome)))
    }

    /// Voids one wallet coupon for the authenticated operator.
    pub async fn void_user_coupon(
        &self,
        scope: &PromotionAdminScope,
        user_coupon_id: String,
        reason: &str,
        trace_id: &str,
    ) -> Result<Option<PromotionUserCouponVoidItem>, CommerceServiceError> {
        positive_id("user_coupon_id", &user_coupon_id)?;
        scope.actor_id()?;
        let reason = validate_stop_loss_reason(reason)?;
        self.repository
            .void_user_coupon(scope, user_coupon_id, reason, audit_trace_id(trace_id))
            .await
    }

    /// Extends one wallet coupon's expiry for the authenticated operator.
    ///
    /// The wire shape is gated here so a malformed body is a `400` before any
    /// transaction opens; the authoritative parse/canonicalization and the
    /// "is it in the future" decision against the database clock stay in the
    /// repository.
    pub async fn extend_user_coupon(
        &self,
        scope: &PromotionAdminScope,
        user_coupon_id: String,
        expires_at: &str,
        trace_id: &str,
    ) -> Result<Option<PromotionUserCouponExtendItem>, CommerceServiceError> {
        positive_id("user_coupon_id", &user_coupon_id)?;
        scope.actor_id()?;
        validate_extension_expires_at(expires_at)?;
        self.repository
            .extend_user_coupon(
                scope,
                user_coupon_id,
                expires_at.trim().to_owned(),
                audit_trace_id(trace_id),
            )
            .await
    }

    /// Settles one applied discount application for the authenticated operator.
    ///
    /// The actor is the IAM-derived [`PromotionAdminScope`]; no request field can
    /// select it. State validation happens here and again in the repository's
    /// guarded `UPDATE`, so a concurrent release can never be overwritten.
    pub async fn settle_discount_application(
        &self,
        scope: &PromotionAdminScope,
        application_id: String,
        trace_id: &str,
    ) -> Result<Option<PromotionDiscountApplicationSettlementItem>, CommerceServiceError> {
        positive_id("application_id", &application_id)?;
        scope.actor_id()?;
        self.repository
            .settle_discount_application(scope, application_id, audit_trace_id(trace_id))
            .await
    }

    /// Releases one applied discount application for the authenticated operator.
    ///
    /// The reason is trimmed/bounded before it reaches the repository, so
    /// `release_reason VARCHAR(512)` can never be overrun.
    pub async fn release_discount_application(
        &self,
        scope: &PromotionAdminScope,
        application_id: String,
        reason: &str,
        trace_id: &str,
    ) -> Result<Option<PromotionDiscountApplicationReleaseItem>, CommerceServiceError> {
        positive_id("application_id", &application_id)?;
        scope.actor_id()?;
        let reason = validate_release_reason(reason)?;
        self.repository
            .release_discount_application(
                scope,
                application_id,
                reason,
                audit_trace_id(trace_id),
            )
            .await
    }
}

/// Normalizes the trace id that will be persisted as audit evidence.
///
/// The value comes from the request context (W3C `traceparent`), never from the
/// caller's body. The column is `VARCHAR(128)` and a W3C trace id is 32 hex
/// characters, so the clamp only guards against a malformed upstream header.
fn audit_trace_id(trace_id: &str) -> String {
    trace_id.trim().chars().take(128).collect()
}

// ---------------------------------------------------------------------------
// Stop-loss decisions.
//
// Each of these is a pure function so the guard truth table can be unit-tested
// without a database, and so the repository, the SQL predicate, and the tests
// share one definition of "eligible".
// ---------------------------------------------------------------------------

/// Decides whether one code may be disabled.
///
/// A claimed code has already produced a wallet coupon the user legitimately
/// holds, so disabling it would orphan that benefit; that is deliberately a
/// different message from a code that is simply no longer active
/// (`disabled`/`expired`/...). The claimed check comes first because it is the
/// stronger, more specific reason.
pub fn code_disable_decision(status: &str, claimed_quantity: i64) -> Result<(), &'static str> {
    if claimed_quantity > 0 {
        return Err("code has already been claimed and cannot be disabled");
    }
    if !status.trim().eq_ignore_ascii_case("active") {
        return Err("code is not active");
    }
    Ok(())
}

/// Decides whether one code may be reissued.
///
/// A reissue replaces the code with a new value, which is only safe while nobody
/// holds the old one. The rule is deliberately identical to
/// [`code_disable_decision`] — the claimed check first (it is the stronger, more
/// specific reason), then `active` — and the non-active message matches it
/// verbatim; only the claimed message names the operation, because the same
/// "cannot be disabled" wording would mislead an operator who asked for a
/// replacement. The two helpers are cross-checked by the truth-table test so the
/// shared predicate cannot drift.
pub fn code_reissue_decision(status: &str, claimed_quantity: i64) -> Result<(), &'static str> {
    if claimed_quantity > 0 {
        return Err("code has already been claimed and cannot be reissued");
    }
    if !status.trim().eq_ignore_ascii_case("active") {
        return Err("code is not active");
    }
    Ok(())
}

/// Decides whether one wallet coupon may be voided or have its expiry extended.
///
/// Both operations need a coupon the user can still use: `claimed`, `active`, or
/// `pending` (case-insensitive). An already-redeemed/used coupon is a distinct
/// conflict from a coupon that is already voided/expired/cancelled/disabled,
/// because the first must be refunded through the order flow while the second is
/// simply terminal.
pub fn user_coupon_void_decision(status: &str) -> Result<(), &'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "redeemed" | "used" => Err("coupon has already been redeemed"),
        "claimed" | "active" | "pending" => Ok(()),
        _ => Err("coupon is not active"),
    }
}

// ---------------------------------------------------------------------------
// Settlement lifecycle and budget decisions.
//
// Same shape as the stop-loss decisions above: pure functions so the truth
// table is unit-testable without a database, and so the repository's guarded
// `UPDATE ... WHERE <predicate>` and the service-level rejection share one
// definition.
// ---------------------------------------------------------------------------

/// Decides whether a coupon budget can reserve `amount_minor`.
///
/// `available_minor` is the pool the account has not promised to anyone yet and
/// `amount_minor` is the application amount in integer minor units. Both are
/// non-negative integers: this helper never sees a float and never divides by
/// 100. The database enforces the exact same rule as an `UPDATE` predicate
/// (`available_amount_minor >= $amount`), so an exact-boundary reservation
/// (`available == amount`) succeeds on both sides and an over-cap one is a
/// `409` conflict ("coupon budget has been exhausted").
pub fn budget_reserve_decision(available_minor: i64, amount_minor: i64) -> Result<(), &'static str> {
    if amount_minor <= 0 {
        return Err("coupon budget reservation amount must be positive");
    }
    if available_minor < amount_minor {
        return Err("coupon budget has been exhausted");
    }
    Ok(())
}

/// Decides whether one discount application can be settled.
///
/// Only `applied` may settle. Every other state gets its own message so an
/// operator can tell "already settled" (money already spent) from "released"
/// (money already returned) from "rolled back" (the order flow reversed it).
pub fn settle_decision(status: &str) -> Result<(), &'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "applied" => Ok(()),
        "settled" => Err("promotion discount application has already been settled"),
        "released" => Err("promotion discount application has already been released"),
        "rolled_back" => {
            Err("promotion discount application was rolled back and cannot be settled")
        }
        _ => Err("promotion discount application is not in the applied state"),
    }
}

/// Decides whether one discount application can be released.
///
/// Mirror of [`settle_decision`]: only `applied` may release, and each terminal
/// state reports why it is terminal.
pub fn release_decision(status: &str) -> Result<(), &'static str> {
    match status.trim().to_ascii_lowercase().as_str() {
        "applied" => Ok(()),
        "settled" => Err("promotion discount application has already been settled"),
        "released" => Err("promotion discount application has already been released"),
        "rolled_back" => {
            Err("promotion discount application was rolled back and cannot be released")
        }
        _ => Err("promotion discount application is not in the applied state"),
    }
}

/// Validates and normalizes an operator release reason.
///
/// Shares the 256-character bound with the stop-loss reason: both are stored in
/// a `VARCHAR(256)`/`VARCHAR(512)` evidence column, and a silent truncation would
/// lose the operator's explanation.
pub fn validate_release_reason(reason: &str) -> Result<String, CommerceServiceError> {
    validate_stop_loss_reason(reason)
}

/// Validates the operator's stop-loss reason and returns its canonical form.
///
/// Trimmed, 1..=[`STOP_LOSS_REASON_MAX_CHARS`] characters: the value is stored in
/// `void_reason VARCHAR(256)`, so a longer reason is a client error rather than a
/// silent truncation that loses the audit explanation.
pub fn validate_stop_loss_reason(reason: &str) -> Result<String, CommerceServiceError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(CommerceServiceError::validation("reason is required"));
    }
    let length = reason.chars().count();
    if length > STOP_LOSS_REASON_MAX_CHARS {
        return Err(CommerceServiceError::validation(format!(
            "reason must be at most {STOP_LOSS_REASON_MAX_CHARS} characters"
        )));
    }
    Ok(reason.to_owned())
}

/// Boundary shape gate for a caller-supplied wallet-coupon expiry.
///
/// Accepts the two promotion timestamp shapes — RFC 3339
/// (`YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`) and the canonical server form
/// (`YYYY-MM-DD HH:MM:SS`). This is only a wire gate: the authoritative parse,
/// canonicalization (`YYYY-MM-DDTHH:MM:SS.sssZ`), and the
/// "later than the database clock" decision live in the repository, which
/// compares against `CURRENT_TIMESTAMP` so process/database clock skew can never
/// extend a coupon into the past.
pub fn validate_extension_expires_at(value: &str) -> Result<(), CommerceServiceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CommerceServiceError::validation("expiresAt is required"));
    }
    let bytes = value.as_bytes();
    let digits = |start: usize, end: usize| {
        value
            .get(start..end)
            .is_some_and(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    };
    let shaped = bytes.len() >= 19
        && digits(0, 4)
        && bytes[4] == b'-'
        && digits(5, 7)
        && bytes[7] == b'-'
        && digits(8, 10)
        && matches!(bytes[10], b'T' | b't' | b' ')
        && digits(11, 13)
        && bytes[13] == b':'
        && digits(14, 16)
        && bytes[16] == b':'
        && digits(17, 19);
    if !shaped {
        return Err(CommerceServiceError::validation(
            "expiresAt must be an RFC 3339 timestamp such as 2026-11-10T09:12:00.000Z",
        ));
    }
    Ok(())
}

fn validate_campaign(input: &PromotionCampaignInput) -> Result<(), CommerceServiceError> {
    required("display_name", &input.display_name)?;
    required("starts_at", &input.starts_at)?;
    if !matches!(
        input.status.as_str(),
        "draft" | "scheduled" | "active" | "paused" | "ended" | "cancelled" | "archived"
    ) {
        return Err(CommerceServiceError::validation("invalid campaign status"));
    }
    Ok(())
}

/// 券码前缀必须为字母数字且为随机部分至少保留 8 个字符（防枚举猜测）。
fn validate_code_batch_prefix(
    code_prefix: &str,
    code_length: i32,
) -> Result<(), CommerceServiceError> {
    if code_prefix.chars().count() > code_length as usize - 8 {
        return Err(CommerceServiceError::validation(
            "code_prefix must leave at least 8 random characters for code guessing resistance",
        ));
    }
    Ok(())
}

/// Validates and normalizes an operator-supplied replacement code.
///
/// Exactly the custom-code contract: trimmed and upper-cased (the redemption
/// lookup compares upper-cased values), 4 to 64 characters, ASCII alphanumerics
/// plus `-`/`_`. Uniqueness is not a property of the value — it is enforced by
/// `uk_promotion_code_hash` and surfaced as a `409` by the insert path — so this
/// helper only owns the canonical shape and the charset.
pub fn validate_explicit_code(code: &str) -> Result<String, CommerceServiceError> {
    let normalized = code.trim().to_ascii_uppercase();
    let length = normalized.chars().count();
    if !(4..=64).contains(&length) {
        return Err(CommerceServiceError::validation(
            "code must be 4 to 64 characters long",
        ));
    }
    if !normalized
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    {
        return Err(CommerceServiceError::validation(
            "code may only contain letters, digits, '-' and '_'",
        ));
    }
    Ok(normalized)
}

/// Validates admin-defined full coupon codes.
///
/// A custom code is stored verbatim (trimmed and upper-cased, the same
/// canonical form generated codes use), so it must be unambiguous, safe to
/// paste from a printed card, and unique inside the batch. The redemption
/// lookup compares upper-cased trimmed values, so anything outside this
/// charset would be unredeemable or ambiguous. The shape/charset rule is shared
/// with [`validate_explicit_code`] (the reissue path), so a value accepted for a
/// batch is accepted for a replacement and vice versa.
fn validate_custom_codes(codes: &[String], quantity: i64) -> Result<(), CommerceServiceError> {
    if codes.len() as i64 != quantity {
        return Err(CommerceServiceError::validation(
            "custom_codes length must equal quantity",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for code in codes {
        let normalized = validate_explicit_code(code)?;
        if !seen.insert(normalized) {
            return Err(CommerceServiceError::validation(
                "custom codes must be unique inside the batch",
            ));
        }
    }
    Ok(())
}

fn validate_offer(input: &PromotionOfferInput) -> Result<(), CommerceServiceError> {
    required("display_name", &input.display_name)?;
    required("offer_type", &input.offer_type)?;
    required("discount_type", &input.discount_type)?;
    required("discount_value", &input.discount_value)?;
    required("starts_at", &input.starts_at)?;
    validate_status(&input.status)?;
    validate_discount_terms(input)?;
    if let Some(coupon_benefit) = &input.coupon_benefit {
        coupon_benefit.validate()?;
    }
    Ok(())
}

/// Validates the monetary discount terms of one offer.
///
/// `discount_type` is matched case-insensitively against a closed set; the
/// stored form is upper case. A coupon offer always carries a `coupon_benefit`
/// and may keep `discount_value = 0`, which is why `FIXED` + `0` stays valid
/// for those rows while a lone `FIXED` + `0` is rejected:
///
/// - `FIXED`: `discount_value` is money; `0` is only legal with a
///   `coupon_benefit`.
/// - `PERCENTAGE`: `discount_value` is a percentage in `(0, 100]`;
///   `maximum_discount_amount`, when present, must be greater than `0`.
/// - `NONE`: a benefit-only coupon with `discount_value == 0` and a
///   `coupon_benefit`; it must not carry a cap.
///
/// Every decimal field (`discount_value`, `minimum_amount`, and the optional
/// `maximum_discount_amount`) must be a plain non-negative decimal with at most
/// two fraction digits — no sign, exponent, comma, or extra precision. Because
/// the format itself forbids a sign, `minimum_amount >= 0` holds by
/// construction.
fn validate_discount_terms(input: &PromotionOfferInput) -> Result<(), CommerceServiceError> {
    let discount_type = input.discount_type.trim().to_ascii_uppercase();
    let discount_value = plain_decimal_minor_units("discount_value", &input.discount_value)?;
    plain_decimal_minor_units("minimum_amount", &input.minimum_amount)?;
    let maximum_discount_amount =
        optional_plain_decimal_minor_units(input.maximum_discount_amount.as_deref())?;
    let has_coupon_benefit = input.coupon_benefit.is_some();

    match discount_type.as_str() {
        "FIXED" => {
            if discount_value == 0 && !has_coupon_benefit {
                return Err(CommerceServiceError::validation(
                    "a FIXED discount_value of 0 is only valid when a coupon_benefit is present",
                ));
            }
            reject_maximum_discount_amount(&discount_type, maximum_discount_amount)?;
        }
        "PERCENTAGE" => {
            if discount_value <= 0 || discount_value > PERCENTAGE_MAX_MINOR_UNITS {
                return Err(CommerceServiceError::validation(
                    "a PERCENTAGE discount_value must be greater than 0 and at most 100",
                ));
            }
            if let Some(maximum) = maximum_discount_amount {
                if maximum <= 0 {
                    return Err(CommerceServiceError::validation(
                        "maximum_discount_amount must be greater than 0",
                    ));
                }
            }
        }
        "NONE" => {
            if discount_value != 0 {
                return Err(CommerceServiceError::validation(
                    "a NONE discount must have discount_value 0",
                ));
            }
            if !has_coupon_benefit {
                return Err(CommerceServiceError::validation(
                    "a NONE discount is only valid when a coupon_benefit is present",
                ));
            }
            reject_maximum_discount_amount(&discount_type, maximum_discount_amount)?;
        }
        _ => {
            return Err(CommerceServiceError::validation(format!(
                "discount_type {discount_type} is not supported; expected FIXED, PERCENTAGE or NONE"
            )))
        }
    }
    Ok(())
}

/// `100` as minor units, so a percentage can be compared without a float.
const PERCENTAGE_MAX_MINOR_UNITS: i64 = 10_000;

fn reject_maximum_discount_amount(
    discount_type: &str,
    maximum_discount_amount: Option<i64>,
) -> Result<(), CommerceServiceError> {
    if maximum_discount_amount.is_some() {
        return Err(CommerceServiceError::validation(format!(
            "maximum_discount_amount is only valid for a PERCENTAGE discount, not {discount_type}"
        )));
    }
    Ok(())
}

fn optional_plain_decimal_minor_units(
    value: Option<&str>,
) -> Result<Option<i64>, CommerceServiceError> {
    match value {
        // A blank optional cap is the same as "no cap": the repository stores
        // `NULL` for it.
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => plain_decimal_minor_units("maximum_discount_amount", value).map(Some),
        None => Ok(None),
    }
}

/// Parses a plain non-negative decimal into minor units (`"12.5"` -> `1250`).
///
/// The accepted shape is exactly `^\d+(\.\d{1,2})?$` after trimming: signs,
/// exponents, thousands separators, an empty integer part, and more than two
/// fraction digits are all rejected rather than silently rounded.
fn plain_decimal_minor_units(name: &str, value: &str) -> Result<i64, CommerceServiceError> {
    let normalized = value.trim();
    let (integer, fraction) = match normalized.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (normalized, None),
    };
    let valid_fraction = match fraction {
        None => true,
        Some(fraction) => {
            !fraction.is_empty()
                && fraction.len() <= 2
                && fraction.chars().all(|character| character.is_ascii_digit())
        }
    };
    if integer.is_empty()
        || !integer.chars().all(|character| character.is_ascii_digit())
        || !valid_fraction
    {
        return Err(invalid_discount_decimal(name));
    }
    let integer = integer
        .parse::<i64>()
        .map_err(|_| invalid_discount_decimal(name))?;
    let fraction = match fraction {
        None => 0,
        // One digit is tenths ("5" -> 50 minor), two digits are hundredths.
        Some(fraction) => {
            let hundredths = fraction
                .parse::<i64>()
                .map_err(|_| invalid_discount_decimal(name))?;
            if fraction.len() == 1 {
                hundredths * 10
            } else {
                hundredths
            }
        }
    };
    integer
        .checked_mul(100)
        .and_then(|minor| minor.checked_add(fraction))
        .ok_or_else(|| invalid_discount_decimal(name))
}

fn invalid_discount_decimal(name: &str) -> CommerceServiceError {
    CommerceServiceError::validation(format!(
        "{name} must be a plain non-negative decimal with at most 2 fraction digits"
    ))
}

fn positive_id(name: &str, value: &str) -> Result<(), CommerceServiceError> {
    if value.trim().is_empty() {
        Err(CommerceServiceError::validation(format!(
            "{name} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_status(status: &str) -> Result<(), CommerceServiceError> {
    if matches!(status, "active" | "disabled") {
        Ok(())
    } else {
        Err(CommerceServiceError::validation(
            "status must be 'active' or 'disabled'",
        ))
    }
}

fn required(name: &str, value: &str) -> Result<(), CommerceServiceError> {
    if value.trim().is_empty() {
        Err(CommerceServiceError::validation(format!(
            "{name} is required"
        )))
    } else {
        Ok(())
    }
}

fn optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{
        audit_trace_id, basis_points, budget_reserve_decision, code_disable_decision,
        code_reissue_decision, finalize_code_export, release_decision, report_rates,
        settle_decision, user_coupon_void_decision,
        validate_code_batch_prefix, validate_custom_codes, validate_discount_terms,
        validate_explicit_code, validate_extension_expires_at, validate_offer,
        validate_redemption_outcome, validate_release_reason, validate_stop_loss_reason,
        CodeReissueOutcome, PromotionAdminListQuery, PromotionCodeReissueItem,
        PromotionCodeRevealItem, PromotionCouponBenefit, PromotionCouponStockInput,
        PromotionOfferInput, PromotionRedemptionAttemptListQuery, PromotionReportCounts,
        BUDGET_RELEASE_EXPIRED_AUDIT_ACTION, CODE_AUDIT_AGGREGATE_TYPE,
        CODE_BATCH_AUDIT_AGGREGATE_TYPE, CODE_BATCH_DISABLE_AUDIT_ACTION,
        CODE_DISABLE_AUDIT_ACTION, CODE_EXPORT_AUDIT_ACTION, CODE_EXPORT_FETCH_LIMIT,
        CODE_EXPORT_MAX_ROWS, CODE_REISSUE_AUDIT_ACTION, CODE_REVEAL_AUDIT_ACTION,
        DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE, DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
        DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION, REDEMPTION_ATTEMPT_OUTCOME_FAILED,
        REDEMPTION_ATTEMPT_OUTCOME_SUCCEEDED, STOP_LOSS_REASON_MAX_CHARS,
        USER_COUPON_AUDIT_AGGREGATE_TYPE, USER_COUPON_EXTEND_AUDIT_ACTION,
        USER_COUPON_VOID_AUDIT_ACTION,
    };

    /// Minimal offer input whose monetary terms are the only thing under test.
    fn offer_terms(
        discount_type: &str,
        discount_value: &str,
        minimum_amount: &str,
        maximum_discount_amount: Option<&str>,
        with_coupon_benefit: bool,
    ) -> PromotionOfferInput {
        PromotionOfferInput {
            campaign_id: None,
            offer_code: None,
            offer_type: "coupon".to_owned(),
            display_name: "test offer".to_owned(),
            description: None,
            audience_scope: "all".to_owned(),
            combinability: "EXCLUSIVE".to_owned(),
            goods_scope: "all".to_owned(),
            priority: 0,
            starts_at: "2026-01-01 00:00:00".to_owned(),
            ends_at: None,
            status: "active".to_owned(),
            discount_type: discount_type.to_owned(),
            discount_value: discount_value.to_owned(),
            minimum_amount: minimum_amount.to_owned(),
            maximum_discount_amount: maximum_discount_amount.map(str::to_owned),
            currency_code: "CNY".to_owned(),
            coupon_benefit: with_coupon_benefit.then(|| {
                PromotionCouponBenefit::token_bank_credit_with_bonus(500, 50)
                    .expect("valid coupon benefit")
            }),
            version: None,
        }
    }

    #[test]
    fn discount_terms_truth_table() {
        // Every row is one rule: `None` accepts, `Some(needle)` rejects with a
        // message containing `needle`.
        let cases: Vec<(&str, PromotionOfferInput, Option<&str>)> = vec![
            // ---- accepted ----
            (
                "fixed amount",
                offer_terms("FIXED", "12.50", "0", None, false),
                None,
            ),
            (
                "fixed type is case-insensitive",
                offer_terms("fixed", "12.50", "0", None, false),
                None,
            ),
            (
                "fixed 0 with a coupon benefit (every existing coupon-code offer)",
                offer_terms("FIXED", "0", "0", None, true),
                None,
            ),
            (
                "percentage in (0, 100]",
                offer_terms("PERCENTAGE", "12.5", "0", None, false),
                None,
            ),
            (
                "percentage at 100",
                offer_terms("PERCENTAGE", "100", "0", None, false),
                None,
            ),
            (
                "percentage with a cap",
                offer_terms("PERCENTAGE", "33.33", "0", Some("20.00"), false),
                None,
            ),
            (
                "percentage with a coupon benefit",
                offer_terms("PERCENTAGE", "10", "0", None, true),
                None,
            ),
            (
                "benefit-only coupon",
                offer_terms("NONE", "0", "0", None, true),
                None,
            ),
            (
                "positive minimum",
                offer_terms("FIXED", "5", "99.99", None, false),
                None,
            ),
            (
                "blank optional cap means no cap",
                offer_terms("FIXED", "5", "0", Some("  "), false),
                None,
            ),
            // ---- rejected: closed type set ----
            (
                "unknown discount type",
                offer_terms("BOGUS", "1", "0", None, false),
                Some("is not supported"),
            ),
            (
                "empty discount type",
                offer_terms("  ", "1", "0", None, false),
                Some("is not supported"),
            ),
            // ---- rejected: FIXED ----
            (
                "fixed 0 without a coupon benefit",
                offer_terms("FIXED", "0", "0", None, false),
                Some("only valid when a coupon_benefit is present"),
            ),
            (
                "fixed with a cap",
                offer_terms("FIXED", "5", "0", Some("10"), false),
                Some("only valid for a PERCENTAGE"),
            ),
            // ---- rejected: PERCENTAGE ----
            (
                "percentage 0",
                offer_terms("PERCENTAGE", "0", "0", None, false),
                Some("greater than 0"),
            ),
            (
                "percentage above 100",
                offer_terms("PERCENTAGE", "100.01", "0", None, false),
                Some("at most 100"),
            ),
            (
                "percentage cap 0",
                offer_terms("PERCENTAGE", "10", "0", Some("0"), false),
                Some("greater than 0"),
            ),
            // ---- rejected: NONE ----
            (
                "none without a coupon benefit",
                offer_terms("NONE", "0", "0", None, false),
                Some("only valid when a coupon_benefit is present"),
            ),
            (
                "none with a nonzero discount",
                offer_terms("NONE", "5", "0", None, true),
                Some("must have discount_value 0"),
            ),
            (
                "none with a cap",
                offer_terms("NONE", "0", "0", Some("10"), true),
                Some("only valid for a PERCENTAGE"),
            ),
            // ---- rejected: decimal shape ----
            (
                "negative discount value",
                offer_terms("FIXED", "-1", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "signed discount value",
                offer_terms("FIXED", "+1", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "exponent discount value",
                offer_terms("FIXED", "1e3", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "comma discount value",
                offer_terms("FIXED", "1,00", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "three fraction digits",
                offer_terms("FIXED", "1.234", "0", None, false),
                Some("at most 2 fraction digits"),
            ),
            (
                "empty fraction",
                offer_terms("FIXED", "1.", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "empty discount value",
                offer_terms("FIXED", "", "0", None, false),
                Some("discount_value must be a plain non-negative decimal"),
            ),
            (
                "negative minimum",
                offer_terms("FIXED", "5", "-1", None, false),
                Some("minimum_amount must be a plain non-negative decimal"),
            ),
            (
                "exponent minimum",
                offer_terms("FIXED", "5", "1e3", None, false),
                Some("minimum_amount must be a plain non-negative decimal"),
            ),
            (
                "cap with three fraction digits",
                offer_terms("PERCENTAGE", "10", "0", Some("1.234"), false),
                Some("maximum_discount_amount must be a plain non-negative decimal"),
            ),
        ];

        for (label, input, expected) in cases {
            match (validate_discount_terms(&input), expected) {
                (Ok(()), None) => {}
                (Ok(()), Some(expected)) => {
                    panic!("{label}: expected rejection containing {expected:?}, got Ok(())")
                }
                (Err(error), None) => panic!("{label}: expected acceptance, got {error:?}"),
                (Err(error), Some(expected)) => assert!(
                    error.message().contains(expected),
                    "{label}: expected an error containing {expected:?}, got {:?}",
                    error.message()
                ),
            }
        }

        // A legacy coupon-code offer (FIXED + 0 + benefit) stays valid through
        // the whole offer validation, not just the terms helper.
        let legacy_coupon_offer = offer_terms("FIXED", "0", "0", None, true);
        assert!(validate_offer(&legacy_coupon_offer).is_ok());
    }

    fn codes(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn reveal_item(index: usize) -> PromotionCodeRevealItem {
        PromotionCodeRevealItem {
            code_no: format!("COD-{index}"),
            code: format!("CODE{index}"),
        }
    }

    #[test]
    fn code_export_limit_is_capped_at_5000_and_flags_truncation() {
        // The fetch limit sits one row past the cap so truncation can be proven
        // without a second COUNT that could disagree with the page.
        assert_eq!(CODE_EXPORT_FETCH_LIMIT, 5001);
        assert_eq!(CODE_EXPORT_MAX_ROWS, 5000);

        // Exactly the cap: a complete export.
        let exact = finalize_code_export((0..CODE_EXPORT_MAX_ROWS).map(reveal_item).collect());
        assert!(!exact.truncated);
        assert_eq!(exact.count, 5000);
        assert_eq!(exact.items.len(), 5000);
        assert_eq!(exact.items[0].code_no, "COD-0");

        // One row past the cap: truncated, and the extra sentinel row is dropped.
        let over = finalize_code_export((0..CODE_EXPORT_MAX_ROWS + 1).map(reveal_item).collect());
        assert!(over.truncated);
        assert_eq!(over.count, 5000);
        assert_eq!(over.items.len(), CODE_EXPORT_MAX_ROWS);
        assert_eq!(
            over.items.last().map(|item| item.code_no.as_str()),
            Some("COD-4999")
        );

        // Below the cap: complete, count mirrors the rows.
        let under = finalize_code_export((0..4999).map(reveal_item).collect());
        assert!(!under.truncated);
        assert_eq!(under.count, 4999);
        assert_eq!(under.items.len(), 4999);

        // An empty batch is a complete, empty export.
        let empty = finalize_code_export(Vec::new());
        assert!(!empty.truncated);
        assert_eq!(empty.count, 0);
        assert!(empty.items.is_empty());
    }

    #[test]
    fn discount_application_filters_are_optional_and_trimmed() {
        let query = PromotionAdminListQuery::new(
            2,
            20,
            Some("ORD-1"),
            Some("applied"),
            None,
            None,
            Some(" 7011 "),
            Some(" 8801 "),
            Some(" 9901 "),
        )
        .expect("valid query");
        assert_eq!(query.code_id.as_deref(), Some("7011"));
        assert_eq!(query.user_coupon_id.as_deref(), Some("8801"));
        assert_eq!(query.order_id.as_deref(), Some("9901"));
        // The existing q/status behaviour is untouched.
        assert_eq!(query.query.as_deref(), Some("ORD-1"));
        assert_eq!(query.status.as_deref(), Some("applied"));
        assert_eq!(query.search_pattern(), "%ord-1%");
        assert_eq!(query.offset(), 20);

        // A blank filter is "no filter", never an empty-string equality match.
        let blank = PromotionAdminListQuery::new(
            1,
            20,
            None,
            None,
            None,
            None,
            Some("   "),
            Some(""),
            None,
        )
        .expect("blank filters are allowed");
        assert_eq!(blank.code_id, None);
        assert_eq!(blank.user_coupon_id, None);
        assert_eq!(blank.order_id, None);

        // Pagination bounds are unchanged.
        assert!(
            PromotionAdminListQuery::new(0, 20, None, None, None, None, None, None, None).is_err()
        );
        assert!(
            PromotionAdminListQuery::new(1, 0, None, None, None, None, None, None, None).is_err()
        );
    }

    #[test]
    fn audit_action_and_aggregate_vocabulary_matches_the_check_constraint() {
        // `chk_pae_action` (widened by 0005_coupon_stop_loss and again by
        // 0006_budget_and_settlement) admits exactly these ten values; the
        // aggregate types are the closed set the repository writes.
        assert_eq!(CODE_REVEAL_AUDIT_ACTION, "codes.reveal");
        assert_eq!(CODE_EXPORT_AUDIT_ACTION, "codes.export");
        assert_eq!(CODE_DISABLE_AUDIT_ACTION, "codes.disable");
        assert_eq!(CODE_BATCH_DISABLE_AUDIT_ACTION, "codes.batch_disable");
        assert_eq!(USER_COUPON_VOID_AUDIT_ACTION, "user_coupons.void");
        assert_eq!(USER_COUPON_EXTEND_AUDIT_ACTION, "user_coupons.extend");
        assert_eq!(
            DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION,
            "discount_applications.settle"
        );
        assert_eq!(
            DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
            "discount_applications.release"
        );
        assert_eq!(BUDGET_RELEASE_EXPIRED_AUDIT_ACTION, "budget.release_expired");
        assert_eq!(CODE_AUDIT_AGGREGATE_TYPE, "promotion_code");
        assert_eq!(CODE_BATCH_AUDIT_AGGREGATE_TYPE, "promotion_code_batch");
        assert_eq!(USER_COUPON_AUDIT_AGGREGATE_TYPE, "promotion_user_coupon");
        assert_eq!(
            DISCOUNT_APPLICATION_AUDIT_AGGREGATE_TYPE,
            "promotion_discount_application"
        );
        // The lifecycle sweep action is reserved by the same check; it has no
        // tenant-scoped writer here because the sweep is system-level.
        for action in [
            CODE_REVEAL_AUDIT_ACTION,
            CODE_EXPORT_AUDIT_ACTION,
            CODE_DISABLE_AUDIT_ACTION,
            CODE_BATCH_DISABLE_AUDIT_ACTION,
            USER_COUPON_VOID_AUDIT_ACTION,
            USER_COUPON_EXTEND_AUDIT_ACTION,
            DISCOUNT_APPLICATION_SETTLE_AUDIT_ACTION,
            DISCOUNT_APPLICATION_RELEASE_AUDIT_ACTION,
            BUDGET_RELEASE_EXPIRED_AUDIT_ACTION,
            "lifecycle.expire",
        ] {
            assert!(
                !action.trim().is_empty() && action.len() <= 64,
                "chk_pae_action admits {action}"
            );
        }

        // The trace id is bounded to the VARCHAR(128) column.
        assert_eq!(audit_trace_id("  "), "");
        assert_eq!(
            audit_trace_id(" 4bf92f3577b34da6a3ce929d0e0e4736 "),
            "4bf92f3577b34da6a3ce929d0e0e4736"
        );
        assert_eq!(audit_trace_id(&"a".repeat(200)).chars().count(), 128);
    }

    #[test]
    fn budget_reserve_decision_truth_table() {
        // Exact-boundary success: the account can promise exactly what it has.
        assert_eq!(budget_reserve_decision(1000, 1000), Ok(()));
        assert_eq!(budget_reserve_decision(1001, 1000), Ok(()));
        // One minor unit short is a conflict, and the message is the wire text.
        assert_eq!(
            budget_reserve_decision(999, 1000),
            Err("coupon budget has been exhausted")
        );
        assert_eq!(
            budget_reserve_decision(0, 1),
            Err("coupon budget has been exhausted")
        );
        // A non-positive application amount is a programming error, never a
        // free discount: it is rejected before the comparison.
        for amount in [0, -1, i64::MIN] {
            assert_eq!(
                budget_reserve_decision(i64::MAX, amount),
                Err("coupon budget reservation amount must be positive"),
                "amount={amount}"
            );
        }
        // No cap ("no budget account") is decided by the caller, not here.
        assert_eq!(budget_reserve_decision(i64::MAX, i64::MAX), Ok(()));
    }

    #[test]
    fn settle_decision_truth_table() {
        for accepted in ["applied", "APPLIED", " applied "] {
            assert_eq!(settle_decision(accepted), Ok(()), "status={accepted}");
        }
        assert_eq!(
            settle_decision("settled"),
            Err("promotion discount application has already been settled")
        );
        assert_eq!(
            settle_decision("released"),
            Err("promotion discount application has already been released")
        );
        assert_eq!(
            settle_decision("rolled_back"),
            Err("promotion discount application was rolled back and cannot be settled")
        );
        // Distinct messages: the three terminal states never collapse into one.
        let messages = [
            settle_decision("settled").expect_err("settled"),
            settle_decision("released").expect_err("released"),
            settle_decision("rolled_back").expect_err("rolled back"),
        ];
        assert_eq!(
            messages.iter().collect::<std::collections::BTreeSet<_>>().len(),
            3
        );
        assert_eq!(
            settle_decision("pending"),
            Err("promotion discount application is not in the applied state")
        );
    }

    #[test]
    fn release_decision_truth_table() {
        for accepted in ["applied", "APPLIED", " applied "] {
            assert_eq!(release_decision(accepted), Ok(()), "status={accepted}");
        }
        assert_eq!(
            release_decision("settled"),
            Err("promotion discount application has already been settled")
        );
        assert_eq!(
            release_decision("released"),
            Err("promotion discount application has already been released")
        );
        assert_eq!(
            release_decision("rolled_back"),
            Err("promotion discount application was rolled back and cannot be released")
        );
        assert_eq!(
            release_decision("voided"),
            Err("promotion discount application is not in the applied state")
        );
        // Settle and release only disagree on the rolled-back wording.
        assert_ne!(
            settle_decision("rolled_back"),
            release_decision("rolled_back")
        );
        assert_eq!(settle_decision("applied"), release_decision("applied"));
    }

    #[test]
    fn release_reason_shares_the_evidence_column_bound() {
        assert_eq!(
            validate_release_reason("  order cancelled  ").expect("reason"),
            "order cancelled"
        );
        assert_eq!("validation", validate_release_reason("   ").expect_err("blank").code());
        assert_eq!(
            "validation",
            validate_release_reason(&"x".repeat(STOP_LOSS_REASON_MAX_CHARS + 1))
                .expect_err("over limit")
                .code()
        );
        assert!(validate_release_reason(&"x".repeat(STOP_LOSS_REASON_MAX_CHARS)).is_ok());
    }

    #[test]
    fn coupon_stock_input_carries_an_optional_minor_unit_cap() {
        // The cap is an integer minor-unit value: no float, no division. `None`
        // keeps the historical "no cap" behaviour.
        let uncapped = PromotionCouponStockInput {
            offer_id: "1".to_owned(),
            stock_type: "limited".to_owned(),
            code_issue_mode: "realtime".to_owned(),
            total_quantity: 10,
            per_user_limit: 1,
            claim_starts_at: None,
            claim_ends_at: None,
            status: "active".to_owned(),
            budget_amount_minor: None,
        };
        assert_eq!(uncapped.budget_amount_minor, None);
        let capped = PromotionCouponStockInput {
            budget_amount_minor: Some(12_345),
            ..uncapped
        };
        assert_eq!(capped.budget_amount_minor, Some(12_345));
    }

    #[test]
    fn code_disable_decision_truth_table() {
        // (status, claimed_quantity, expected)
        for (status, claimed, expected) in [
            ("active", 0, Ok(())),
            ("ACTIVE", 0, Ok(())),
            (" active ", 0, Ok(())),
            // A claimed code is the stronger conflict and has its own message.
            ("active", 1, Err("code has already been claimed and cannot be disabled")),
            ("active", 42, Err("code has already been claimed and cannot be disabled")),
            ("disabled", 0, Err("code is not active")),
            ("expired", 0, Err("code is not active")),
            ("voided", 0, Err("code is not active")),
            ("active", -1, Ok(())),
        ] {
            assert_eq!(
                code_disable_decision(status, claimed),
                expected,
                "status={status} claimed={claimed}"
            );
        }
    }

    #[test]
    fn code_reissue_decision_truth_table_matches_the_disable_guard() {
        // (status, claimed_quantity, accepted)
        for (status, claimed, accepted) in [
            ("active", 0, true),
            ("ACTIVE", 0, true),
            (" active ", 0, true),
            ("active", 1, false),
            ("active", 42, false),
            ("disabled", 0, false),
            ("expired", 0, false),
            ("voided", 0, false),
        ] {
            assert_eq!(
                code_reissue_decision(status, claimed).is_ok(),
                accepted,
                "status={status} claimed={claimed}"
            );
            // The reissue guard and the disable guard admit exactly the same
            // rows, so the two operations cannot disagree about eligibility.
            assert_eq!(
                code_reissue_decision(status, claimed).is_ok(),
                code_disable_decision(status, claimed).is_ok(),
                "reissue and disable guards diverged at status={status} claimed={claimed}"
            );
        }
        assert_eq!(code_reissue_decision("active", 0), Ok(()));
        assert_eq!(
            code_reissue_decision("active", 3),
            Err("code has already been claimed and cannot be reissued")
        );
        assert_eq!(
            code_reissue_decision("disabled", 0),
            Err("code is not active")
        );
        // A claimed code is the stronger, more specific conflict, even when the
        // row is also no longer active.
        assert_eq!(
            code_reissue_decision("disabled", 3),
            Err("code has already been claimed and cannot be reissued")
        );
    }

    #[test]
    fn explicit_reissue_code_is_normalized_and_gated_like_a_custom_code() {
        assert_eq!(
            validate_explicit_code("  welcome-2026  ").expect("valid"),
            "WELCOME-2026"
        );
        assert_eq!(validate_explicit_code("a1-_").expect("valid"), "A1-_");
        let at_limit = "A".repeat(64);
        assert_eq!(
            validate_explicit_code(&at_limit)
                .expect("at limit")
                .chars()
                .count(),
            64
        );
        let too_long = "A".repeat(65);
        for rejected in [
            "ABC",
            "",
            "   ",
            "AB CD",
            "ABC*",
            "优惠券码",
            too_long.as_str(),
        ] {
            let error = validate_explicit_code(rejected).expect_err("must be rejected");
            assert_eq!("validation", error.code(), "{rejected:?}");
        }

        // The custom-code path and the reissue path share one shape/charset
        // rule, so a code accepted for a batch is accepted as a replacement and
        // vice versa.
        for code in [
            "WELCOME2026",
            "A1-_",
            "abcd",
            at_limit.as_str(),
            "ABC",
            "AB CD",
            "ABC*",
            too_long.as_str(),
        ] {
            assert_eq!(
                validate_custom_codes(&[code.to_owned()], 1).is_ok(),
                validate_explicit_code(code).is_ok(),
                "{code:?} must be judged identically by both code paths"
            );
        }
    }

    #[test]
    fn code_reissue_audit_vocabulary_is_stable() {
        assert_eq!(CODE_REISSUE_AUDIT_ACTION, "codes.reissue");
        assert_eq!(CODE_DISABLE_AUDIT_ACTION, "codes.disable");
    }

    #[test]
    fn reissue_item_names_the_new_code_and_the_replaced_code_without_plaintext() {
        let item = PromotionCodeReissueItem::from_outcome(
            "7011".to_owned(),
            CodeReissueOutcome {
                new_code_id: "7012".to_owned(),
                new_code_no: "COD-20261111000000-0002".to_owned(),
                code_masked: "3C76****4E21".to_owned(),
                code_last4: "4E21".to_owned(),
            },
        );
        assert_eq!(item.code_id, "7012");
        assert_eq!(item.code_no, "COD-20261111000000-0002");
        assert_eq!(item.code_masked, "3C76****4E21");
        assert_eq!(item.code_last4, "4E21");
        assert_eq!(item.replaced_code_id, "7011");
        // Neither the repository outcome nor the service item has a plaintext
        // field: reissue is not a reveal path.
        assert_ne!(item.code_masked, item.code_id);
        for declaration in [
            "pub struct CodeReissueOutcome {",
            "pub struct PromotionCodeReissueItem {",
        ] {
            let source = include_str!("backend_admin.rs");
            let start = source
                .find(declaration)
                .unwrap_or_else(|| panic!("missing {declaration}"));
            let rest = &source[start..];
            let fields = &rest[..rest.find('}').expect("unterminated struct")];
            // Doc comments may name the `promotion_code` columns; only the field
            // declarations matter for "no plaintext field".
            let fields_only = fields
                .lines()
                .filter(|line| line.trim_start().starts_with("pub "))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!fields_only.contains("promotion_code"), "{declaration}");
            assert!(!fields_only.contains("pub code:"), "{declaration}");
            assert!(!fields_only.contains("pub value:"), "{declaration}");
        }
    }

    #[test]
    fn user_coupon_void_decision_truth_table() {
        // Every status the wallet persists is a closed set here; a redeemed/used
        // coupon must be refunded through the order flow, never voided silently.
        for accepted in ["claimed", "active", "pending", "CLAIMED", " Pending "] {
            assert_eq!(
                user_coupon_void_decision(accepted),
                Ok(()),
                "status={accepted}"
            );
        }
        for status in ["redeemed", "used", "REDEEMED", " Used "] {
            assert_eq!(
                user_coupon_void_decision(status),
                Err("coupon has already been redeemed"),
                "status={status}"
            );
        }
        for status in ["voided", "expired", "cancelled", "disabled", "frozen"] {
            assert_eq!(
                user_coupon_void_decision(status),
                Err("coupon is not active"),
                "status={status}"
            );
        }
    }

    #[test]
    fn stop_loss_reason_is_trimmed_and_bounded() {
        assert_eq!(
            validate_stop_loss_reason("  leaked on a public forum  ").expect("reason"),
            "leaked on a public forum"
        );
        for rejected in ["", "   ", "\n\t"] {
            let error = validate_stop_loss_reason(rejected).expect_err("empty reason");
            assert_eq!("validation", error.code(), "{rejected:?}");
        }
        // Exactly the column width is accepted, one more character is not.
        let at_limit = "x".repeat(STOP_LOSS_REASON_MAX_CHARS);
        assert_eq!(
            validate_stop_loss_reason(&at_limit)
                .expect("at limit")
                .chars()
                .count(),
            STOP_LOSS_REASON_MAX_CHARS
        );
        let over_limit = "x".repeat(STOP_LOSS_REASON_MAX_CHARS + 1);
        let error = validate_stop_loss_reason(&over_limit).expect_err("over limit");
        assert_eq!("validation", error.code());
        assert!(error.message().contains("256"), "{error:?}");
        // Counted in characters, not bytes: a 256-character CJK reason fits the
        // VARCHAR(256) column.
        let cjk = "泄".repeat(STOP_LOSS_REASON_MAX_CHARS);
        assert!(validate_stop_loss_reason(&cjk).is_ok());
    }

    #[test]
    fn extension_expiry_shape_gate_rejects_unparseable_values() {
        for accepted in [
            "2026-11-10T09:12:00.000Z",
            "2026-11-10T17:12:00+08:00",
            "2026-11-10 09:12:00",
            " 2026-11-10T09:12:00Z ",
        ] {
            assert!(
                validate_extension_expires_at(accepted).is_ok(),
                "{accepted} must pass the wire gate"
            );
        }
        for rejected in ["", "   ", "not-a-timestamp", "2026-11-10", "2026/11/10 09:12:00", "tomorrow"] {
            let error =
                validate_extension_expires_at(rejected).expect_err("must be rejected");
            assert_eq!("validation", error.code(), "{rejected:?}");
        }
        
    }

    #[test]
    fn redemption_attempt_outcome_filter_is_a_closed_set() {
        for accepted in [
            REDEMPTION_ATTEMPT_OUTCOME_SUCCEEDED,
            REDEMPTION_ATTEMPT_OUTCOME_FAILED,
            // Query strings are trimmed before validation, so a trailing newline
            // from a hand-written client still works.
            " failed ",
        ] {
            assert!(validate_redemption_outcome(accepted).is_ok(), "{accepted}");
        }
        for rejected in [
            "",
            "failure",
            "SUCCEEDED",
            "all",
            "succeeded,failed",
            "null",
        ] {
            let error = validate_redemption_outcome(rejected).expect_err("must be rejected");
            assert_eq!("validation", error.code(), "{rejected}");
        }

        // The query constructor applies the same rule, so an unknown outcome can
        // never reach the repository as an ignored filter.
        let error = PromotionRedemptionAttemptListQuery::new(1, 20, None, Some("failure"), None)
            .expect_err("unknown outcome must be rejected at construction");
        assert_eq!("validation", error.code());
        let accepted = PromotionRedemptionAttemptListQuery::new(
            1,
            20,
            Some("  ABC  "),
            Some(" failed "),
            Some(" 42 "),
        )
        .expect("valid query");
        assert_eq!(accepted.query.as_deref(), Some("ABC"));
        assert_eq!(accepted.outcome.as_deref(), Some("failed"));
        assert_eq!(accepted.code_id.as_deref(), Some("42"));
        assert_eq!(accepted.search_pattern(), "%abc%");
        assert_eq!(accepted.offset(), 0);
        // An empty outcome means "no filter", not "invalid".
        assert_eq!(
            PromotionRedemptionAttemptListQuery::new(1, 20, None, Some("  "), None)
                .expect("blank outcome is no filter")
                .outcome,
            None
        );

        // Pagination bounds match every other admin list.
        assert!(PromotionRedemptionAttemptListQuery::new(0, 20, None, None, None).is_err());
        assert!(PromotionRedemptionAttemptListQuery::new(1, 0, None, None, None).is_err());
        assert!(PromotionRedemptionAttemptListQuery::new(1, 201, None, None, None).is_err());
        assert!(PromotionRedemptionAttemptListQuery::new(2, 200, None, None, None).is_ok());
        assert_eq!(
            PromotionRedemptionAttemptListQuery::new(3, 20, None, None, None)
                .expect("query")
                .offset(),
            40
        );
    }

    #[test]
    fn code_prefix_validation_requires_at_least_eight_random_characters() {
        assert!(validate_code_batch_prefix("", 12).is_ok());
        assert!(validate_code_batch_prefix("WELCOME", 16).is_ok());
        assert!(validate_code_batch_prefix("WELCOME2026", 20).is_ok());
        // 前缀占满长度时随机部分不足 8 位，必须拒绝（防枚举）
        let error = validate_code_batch_prefix("ABCDEFGHIJK", 12).expect_err("short random tail");
        assert_eq!("validation", error.code());
        let error = validate_code_batch_prefix("WELCOMENEW", 16).expect_err("nine char prefix");
        assert_eq!("validation", error.code());
    }

    #[test]
    fn code_prefix_validation_is_unaffected_by_unicode_width() {
        // 按字符数（chars）计算，而不是字节数
        assert!(validate_code_batch_prefix("优惠", 12).is_ok());
    }

    #[test]
    fn custom_codes_accept_short_and_long_admin_defined_values() {
        assert!(validate_custom_codes(&codes(&["WELCOME2026"]), 1).is_ok());
        assert!(validate_custom_codes(&codes(&["VIP888"]), 1).is_ok());
        assert!(validate_custom_codes(&codes(&["A1-_"]), 1).is_ok());
        let long = "A".repeat(64);
        assert!(validate_custom_codes(&codes(&[long.as_str()]), 1).is_ok());
    }

    #[test]
    fn custom_codes_reject_length_quantity_and_charset_violations() {
        for (values, quantity) in [
            (codes(&["ABC"]), 1),                  // too short
            (codes(&["ABCDEFGHI"]), 2),            // length != quantity
            (codes(&["ABCDE"]), 0),                // empty batch
            (codes(&["AB CD"]), 1),                // space
            (codes(&["ABC*"]), 1),                 // disallowed symbol
            (codes(&["WELCOME", "WELCOME"]), 2),   // duplicate
            (codes(&["welcome", "WELCOME"]), 2),   // duplicate ignoring case
        ] {
            let error = validate_custom_codes(&values, quantity).expect_err("must be rejected");
            assert_eq!("validation", error.code(), "{values:?} / {quantity}");
        }
        let too_long = "A".repeat(65);
        assert!(validate_custom_codes(&codes(&[too_long.as_str()]), 1).is_err());
    }

    #[test]
    fn basis_points_truth_table() {
        // (numerator, denominator, expected basis points)
        for (numerator, denominator, expected) in [
            // A zero denominator has no meaningful share: 0, never a division
            // fault and never an invented 100%.
            (0, 0, 0),
            (5, 0, 0),
            (i64::MAX, 0, 0),
            // Zero numerator.
            (0, 10, 0),
            // Rounding down: 1/3 is 33.33%, truncated to 3333 bp.
            (1, 3, 3333),
            (2, 3, 6666),
            // Exact half / whole ratios.
            (1, 2, 5000),
            (3, 4, 7500),
            (10, 10, 10_000),
            // Above 100% clamps instead of reporting an out-of-range rate.
            (11, 10, 10_000),
            (25, 10, 10_000),
            (i64::MAX, 1, 10_000),
            // A tiny share truncates to zero.
            (1, 20_000, 0),
            (1, i64::MAX, 0),
            // Large numerator * 10_000 must not overflow: computed in i128.
            (i64::MAX, i64::MAX, 10_000),
            // Negative guards: a negative numerator or denominator cannot
            // describe a share.
            (-1, 10, 0),
            (-10, -10, 0),
            (10, -10, 0),
        ] {
            assert_eq!(
                basis_points(numerator, denominator),
                expected,
                "basis_points({numerator}, {denominator})"
            );
        }
        // The contract range is always respected.
        for numerator in [-5, 0, 1, 7, 10_000, i64::MAX] {
            for denominator in [-5, 0, 1, 3, 10_000, i64::MAX] {
                let value = basis_points(numerator, denominator);
                assert!((0..=10_000).contains(&value), "{numerator}/{denominator}");
            }
        }
    }

    #[test]
    fn report_rates_share_the_issued_code_denominator() {
        let counts = PromotionReportCounts {
            issued_codes: 200,
            claimed_coupons: 150,
            redeemed_coupons: 100,
            voided_coupons: 1,
            ..PromotionReportCounts::default()
        };
        let rates = report_rates(&counts);
        assert_eq!(rates.claim_rate_bp, 7_500);
        assert_eq!(rates.redemption_rate_bp, 5_000);
        assert_eq!(rates.void_rate_bp, 50);

        // No issued codes in the window: every rate is 0 instead of a division
        // fault.
        let empty = report_rates(&PromotionReportCounts::default());
        assert_eq!(
            empty,
            super::PromotionReportRates::default(),
            "an empty window reports zero rates"
        );

        // More redemptions than issues (a coupon can outlive its code window)
        // clamps at 100% instead of exceeding it.
        let over = report_rates(&PromotionReportCounts {
            issued_codes: 2,
            redeemed_coupons: 5,
            ..PromotionReportCounts::default()
        });
        assert_eq!(over.redemption_rate_bp, 10_000);
    }
}
