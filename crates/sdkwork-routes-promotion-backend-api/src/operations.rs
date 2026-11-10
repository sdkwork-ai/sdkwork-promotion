use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use sdkwork_commerce_promotion_service::{
    PromotionAdminListQuery, PromotionAdminPage, PromotionAdminService,
    PromotionAdminUserCouponItem, PromotionCampaignInput, PromotionCampaignItem,
    PromotionCodeBatchDisableItem, PromotionCodeBatchInput, PromotionCodeBatchItem,
    PromotionCodeDisableItem, PromotionCodeExportItem, PromotionCodeItem, PromotionCodeReissueItem,
    PromotionCodeRevealItem, PromotionCouponBenefit, PromotionCouponLedgerItem,
    PromotionCouponStockInput, PromotionCouponStockItem, PromotionDiscountApplicationItem,
    PromotionDiscountApplicationReleaseItem, PromotionDiscountApplicationSettlementItem,
    PromotionDistributionInput, PromotionDistributionTaskItem, PromotionOfferInput,
    PromotionOfferItem, PromotionOverview, PromotionRedemptionAttemptItem,
    PromotionRedemptionAttemptListQuery, PromotionReport, PromotionReportCounts,
    PromotionReportCurrencyRow, PromotionReportRates, PromotionReportTopOffer,
    PromotionSubscriptionPeriod, PromotionUserCouponExtendItem, PromotionUserCouponVoidItem,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_iam_context_service::IamAppContext;
use sdkwork_utils_rust::OffsetListPageParams;
use sdkwork_web_core::WebRequestContext;
use serde::{Deserialize, Serialize};

use crate::api_response::{
    conflict, forbidden, internal_error, invalid_state, no_content, not_found, parse_page,
    success_command, success_created, success_item, success_items, unauthorized, validation,
};
use crate::backend_acl::require_backend_operator;
use crate::code_mask::{mask_promotion_code, promotion_code_last4};

const READ_PERMISSION: &str = "commerce.marketing.read";
const MANAGE_PERMISSION: &str = "commerce.marketing.manage";

#[derive(Clone)]
struct PromotionAdminState {
    service: Arc<PromotionAdminService>,
}

#[derive(Debug, Deserialize)]
struct ListParams {
    page: Option<i64>,
    page_size: Option<i64>,
    q: Option<String>,
    status: Option<String>,
    code_batch_id: Option<i64>,
    stock_id: Option<i64>,
    /// Discount-application filter: promotion code id (TEXT, never numeric).
    code_id: Option<String>,
    /// Discount-application filter: `user_coupon_id`.
    user_coupon_id: Option<String>,
    /// Discount-application filter: `order_id`.
    order_id: Option<String>,
}

/// Query for the redemption attempt ledger.
///
/// Separate from [`ListParams`] because the ledger filters on `outcome` and
/// `code_id` and has no `status`; reusing the shared struct would silently accept
/// (and ignore) parameters the operation does not document.
#[derive(Debug, Deserialize)]
struct RedemptionAttemptListParams {
    page: Option<i64>,
    page_size: Option<i64>,
    q: Option<String>,
    outcome: Option<String>,
    code_id: Option<String>,
}

/// Query of the coupon-centre operating report.
///
/// Both bounds are optional and accept RFC 3339 or the canonical server form
/// `YYYY-MM-DD HH:MM:SS`; omitting them asks for the last 30 days ending now.
/// The service normalizes and orders them, and an unparseable bound or
/// `from > to` is a `400`.
#[derive(Debug, Deserialize)]
struct ReportParams {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CampaignRequest {
    campaign_code: Option<String>,
    display_name: String,
    description: Option<String>,
    channel_scope: String,
    audience_scope: String,
    starts_at: String,
    ends_at: Option<String>,
    status: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfferRequest {
    campaign_id: Option<String>,
    offer_code: Option<String>,
    offer_type: String,
    display_name: String,
    description: Option<String>,
    audience_scope: String,
    combinability: String,
    goods_scope: String,
    priority: i32,
    starts_at: String,
    ends_at: Option<String>,
    status: String,
    discount_type: String,
    discount_value: String,
    minimum_amount: String,
    maximum_discount_amount: Option<String>,
    currency_code: String,
    coupon_benefit: Option<CouponBenefitRequest>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum CouponBenefitRequest {
    TokenBankCredit {
        grant_amount: String,
        bonus_amount: Option<String>,
    },
    PointsCredit {
        grant_points: String,
    },
    CashCredit {
        grant_amount: String,
    },
    Subscription {
        period: String,
        duration_days: String,
        daily_quota: String,
        total_quota: String,
        /// Membership package identity; required by the order-side redemption.
        product_id: Option<String>,
        sku_id: Option<String>,
        package_id: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CouponStockRequest {
    offer_id: String,
    stock_type: String,
    code_issue_mode: Option<String>,
    total_quantity: String,
    per_user_limit: i32,
    claim_starts_at: Option<String>,
    claim_ends_at: Option<String>,
    status: String,
    /// Optional hard monetary cap for this stock, as an int64 decimal string of
    /// integer minor units (API_SPEC §13.6). Omitted/blank means "no cap" and
    /// keeps the historical uncapped behaviour.
    budget_amount_minor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeBatchRequest {
    stock_id: String,
    code_type: String,
    requested_quantity: String,
    /// Only meaningful for generated codes; defaults to 16 when omitted.
    code_length: Option<i32>,
    /// Only meaningful for generated codes; defaults to empty when omitted.
    code_prefix: Option<String>,
    /// Full coupon codes defined by the admin; when present the batch stores
    /// these verbatim instead of generating prefix + random suffixes.
    custom_codes: Option<Vec<String>>,
    /// Redemption cap per code; defaults to 1 (single use).
    max_claims: Option<i32>,
    starts_at: Option<String>,
    expires_at: Option<String>,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionRequest {
    stock_id: String,
    owner_user_ids: Vec<String>,
    idempotency_key: String,
}

#[derive(Debug, Deserialize)]
struct UpdateStatusRequest {
    status: String,
}

/// Shared stop-loss request body: the operator's reason.
///
/// Trimmed and bounded by the service (`1..=256` characters); the actor is never
/// part of the body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopLossReasonRequest {
    reason: String,
}

/// Body of a code reissue.
///
/// `code` is optional: when omitted, the replacement is generated from the
/// batch's prefix/length. When present it is normalized and validated exactly
/// like a custom batch code (4–64 characters, `A-Z0-9-_`, upper-cased), and its
/// uniqueness is enforced by the code hash unique constraint (`409` on a
/// collision). The actor is never part of the body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeReissueRequest {
    reason: String,
    code: Option<String>,
}

/// Body of a wallet-coupon extension.
///
/// Accepts the two promotion timestamp shapes; the service gates the wire shape
/// and the repository canonicalizes it and compares it with the database clock.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserCouponExtensionRequest {
    expires_at: String,
}

/// Body of a discount-application release.
///
/// Trimmed and bounded by the service (`1..=256` characters); the actor is never
/// part of the body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscountApplicationReleaseRequest {
    reason: String,
}

macro_rules! response_try {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(response) => return response,
        }
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverviewResponse {
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    active_offers: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    total_offers: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    total_coupon_stock: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    available_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    claimed_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    redeemed_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    active_codes: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    discount_applications: i64,
}
impl From<PromotionOverview> for OverviewResponse {
    fn from(v: PromotionOverview) -> Self {
        Self {
            active_offers: v.active_offers,
            total_offers: v.total_offers,
            total_coupon_stock: v.total_coupon_stock,
            available_coupons: v.available_coupons,
            claimed_coupons: v.claimed_coupons,
            redeemed_coupons: v.redeemed_coupons,
            active_codes: v.active_codes,
            discount_applications: v.discount_applications,
        }
    }
}

// ---------------------------------------------------------------------------
// Coupon-centre operating report (`promotionReports.retrieve`).
//
// Every int64 is an int64-as-decimal-string on the wire (API_SPEC §13.6) and
// every money field is integer minor units (API_SPEC §13.2): no JSON number can
// be rounded by a browser or rendered 100x too large.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportResponse {
    from: String,
    to: String,
    counts: ReportCountsResponse,
    rates: ReportRatesResponse,
    by_currency: Vec<ReportCurrencyResponse>,
    top_offers: Vec<ReportTopOfferResponse>,
}
impl From<PromotionReport> for ReportResponse {
    fn from(v: PromotionReport) -> Self {
        Self {
            from: v.from,
            to: v.to,
            counts: ReportCountsResponse::from(v.counts),
            rates: ReportRatesResponse::from(v.rates),
            by_currency: v
                .by_currency
                .into_iter()
                .map(ReportCurrencyResponse::from)
                .collect(),
            top_offers: v
                .top_offers
                .into_iter()
                .map(ReportTopOfferResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportCountsResponse {
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    issued_codes: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    claimed_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    redeemed_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    expired_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    voided_coupons: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    disabled_codes: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    applications_applied: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    applications_settled: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    applications_released: i64,
}
impl From<PromotionReportCounts> for ReportCountsResponse {
    fn from(v: PromotionReportCounts) -> Self {
        Self {
            issued_codes: v.issued_codes,
            claimed_coupons: v.claimed_coupons,
            redeemed_coupons: v.redeemed_coupons,
            expired_coupons: v.expired_coupons,
            voided_coupons: v.voided_coupons,
            disabled_codes: v.disabled_codes,
            applications_applied: v.applications_applied,
            applications_settled: v.applications_settled,
            applications_released: v.applications_released,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportRatesResponse {
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    claim_rate_bp: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    redemption_rate_bp: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    void_rate_bp: i64,
}
impl From<PromotionReportRates> for ReportRatesResponse {
    fn from(v: PromotionReportRates) -> Self {
        Self {
            claim_rate_bp: v.claim_rate_bp,
            redemption_rate_bp: v.redemption_rate_bp,
            void_rate_bp: v.void_rate_bp,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportCurrencyResponse {
    currency_code: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    discount_applied_minor: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    discount_settled_minor: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    budget_total_minor: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    budget_reserved_minor: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    budget_used_minor: i64,
}
impl From<PromotionReportCurrencyRow> for ReportCurrencyResponse {
    fn from(v: PromotionReportCurrencyRow) -> Self {
        Self {
            currency_code: v.currency_code,
            discount_applied_minor: v.discount_applied_minor,
            discount_settled_minor: v.discount_settled_minor,
            budget_total_minor: v.budget_total_minor,
            budget_reserved_minor: v.budget_reserved_minor,
            budget_used_minor: v.budget_used_minor,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReportTopOfferResponse {
    offer_id: String,
    offer_no: String,
    display_name: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    redeemed: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    discount_settled_minor: i64,
    currency_code: String,
}
impl From<PromotionReportTopOffer> for ReportTopOfferResponse {
    fn from(v: PromotionReportTopOffer) -> Self {
        Self {
            offer_id: v.offer_id,
            offer_no: v.offer_no,
            display_name: v.display_name,
            redeemed: v.redeemed,
            discount_settled_minor: v.discount_settled_minor,
            currency_code: v.currency_code,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CampaignResponse {
    id: String,
    campaign_no: String,
    campaign_code: Option<String>,
    display_name: String,
    description: Option<String>,
    channel_scope: String,
    audience_scope: String,
    starts_at: String,
    ends_at: Option<String>,
    status: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    version: i64,
    updated_at: String,
}
impl From<PromotionCampaignItem> for CampaignResponse {
    fn from(v: PromotionCampaignItem) -> Self {
        Self {
            id: v.id,
            campaign_no: v.campaign_no,
            campaign_code: v.campaign_code,
            display_name: v.display_name,
            description: v.description,
            channel_scope: v.channel_scope,
            audience_scope: v.audience_scope,
            starts_at: v.starts_at,
            ends_at: v.ends_at,
            status: v.status,
            version: v.version,
            updated_at: v.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfferResponse {
    id: String,
    campaign_id: Option<String>,
    offer_no: String,
    offer_code: Option<String>,
    offer_type: String,
    audience_scope: String,
    combinability: String,
    goods_scope: String,
    display_name: String,
    description: Option<String>,
    priority: i32,
    starts_at: String,
    ends_at: Option<String>,
    status: String,
    discount_type: Option<String>,
    discount_value: Option<String>,
    minimum_amount: Option<String>,
    maximum_discount_amount: Option<String>,
    currency_code: Option<String>,
    coupon_benefit: Option<CouponBenefitResponse>,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    version: i64,
    updated_at: String,
}
impl From<PromotionOfferItem> for OfferResponse {
    fn from(v: PromotionOfferItem) -> Self {
        Self {
            id: v.id,
            campaign_id: v.campaign_id,
            offer_no: v.offer_no,
            offer_code: v.offer_code,
            offer_type: v.offer_type,
            audience_scope: v.audience_scope,
            combinability: v.combinability,
            goods_scope: v.goods_scope,
            display_name: v.display_name,
            description: v.description,
            priority: v.priority,
            starts_at: v.starts_at,
            ends_at: v.ends_at,
            status: v.status,
            discount_type: v.discount_type,
            discount_value: v.discount_value,
            minimum_amount: v.minimum_amount,
            maximum_discount_amount: v.maximum_discount_amount,
            currency_code: v.currency_code,
            coupon_benefit: v.coupon_benefit.map(CouponBenefitResponse::from),
            version: v.version,
            updated_at: v.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum CouponBenefitResponse {
    TokenBankCredit {
        target_asset: &'static str,
        grant_amount: String,
        bonus_amount: Option<String>,
    },
    PointsCredit {
        grant_points: String,
    },
    CashCredit {
        grant_amount: String,
    },
    Subscription {
        period: &'static str,
        duration_days: i64,
        daily_quota: String,
        total_quota: String,
        product_id: Option<String>,
        sku_id: Option<String>,
        package_id: Option<String>,
    },
}

impl From<PromotionCouponBenefit> for CouponBenefitResponse {
    fn from(value: PromotionCouponBenefit) -> Self {
        match value {
            PromotionCouponBenefit::TokenBankCredit {
                grant_amount,
                bonus_amount,
            } => Self::TokenBankCredit {
                target_asset: "token_bank",
                grant_amount: grant_amount.to_string(),
                bonus_amount: (bonus_amount > 0).then(|| bonus_amount.to_string()),
            },
            PromotionCouponBenefit::PointsCredit { grant_points } => Self::PointsCredit {
                grant_points: grant_points.to_string(),
            },
            PromotionCouponBenefit::CashCredit { grant_amount } => Self::CashCredit {
                grant_amount: grant_amount.to_string(),
            },
            PromotionCouponBenefit::Subscription {
                product_id,
                sku_id,
                package_id,
                period,
                duration_days,
                daily_quota,
                total_quota,
            } => Self::Subscription {
                period: period.as_str(),
                duration_days,
                daily_quota: daily_quota.to_string(),
                total_quota: total_quota.to_string(),
                product_id: (!product_id.is_empty()).then_some(product_id),
                sku_id: (!sku_id.is_empty()).then_some(sku_id),
                package_id: (package_id > 0).then(|| package_id.to_string()),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StockResponse {
    id: String,
    offer_id: String,
    stock_no: String,
    stock_type: String,
    code_issue_mode: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    total_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    available_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    claimed_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    redeemed_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    locked_quantity: i64,
    per_user_limit: i32,
    claim_starts_at: Option<String>,
    claim_ends_at: Option<String>,
    status: String,
    /// Budget snapshot of the stock's `promotion_budget_account`, integer minor
    /// units. All four are `null` when the stock has no budget account.
    #[serde(with = "sdkwork_utils_rust::serde_int64::option")]
    budget_amount_minor: Option<i64>,
    #[serde(with = "sdkwork_utils_rust::serde_int64::option")]
    budget_available_minor: Option<i64>,
    #[serde(with = "sdkwork_utils_rust::serde_int64::option")]
    budget_reserved_minor: Option<i64>,
    #[serde(with = "sdkwork_utils_rust::serde_int64::option")]
    budget_used_minor: Option<i64>,
}
impl From<PromotionCouponStockItem> for StockResponse {
    fn from(v: PromotionCouponStockItem) -> Self {
        Self {
            id: v.id,
            offer_id: v.offer_id,
            stock_no: v.stock_no,
            stock_type: v.stock_type,
            code_issue_mode: v.code_issue_mode,
            total_quantity: v.total_quantity,
            available_quantity: v.available_quantity,
            claimed_quantity: v.claimed_quantity,
            redeemed_quantity: v.redeemed_quantity,
            locked_quantity: v.locked_quantity,
            per_user_limit: v.per_user_limit,
            claim_starts_at: v.claim_starts_at,
            claim_ends_at: v.claim_ends_at,
            status: v.status,
            budget_amount_minor: v.budget_amount_minor,
            budget_available_minor: v.budget_available_minor,
            budget_reserved_minor: v.budget_reserved_minor,
            budget_used_minor: v.budget_used_minor,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeBatchResponse {
    id: String,
    stock_id: String,
    offer_id: String,
    batch_no: String,
    code_type: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    requested_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    generated_quantity: i64,
    code_length: i32,
    code_prefix: String,
    starts_at: Option<String>,
    expires_at: Option<String>,
    status: String,
    created_at: String,
}
impl From<PromotionCodeBatchItem> for CodeBatchResponse {
    fn from(v: PromotionCodeBatchItem) -> Self {
        Self {
            id: v.id,
            stock_id: v.stock_id,
            offer_id: v.offer_id,
            batch_no: v.batch_no,
            code_type: v.code_type,
            requested_quantity: v.requested_quantity,
            generated_quantity: v.generated_quantity,
            code_length: v.code_length,
            code_prefix: v.code_prefix,
            starts_at: v.starts_at,
            expires_at: v.expires_at,
            status: v.status,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeResponse {
    id: String,
    stock_id: String,
    offer_id: String,
    code_no: String,
    /// Deprecated on the wire: kept for backward compatibility, but it is the
    /// redeemable value and a list must not be a distribution channel. Clients
    /// migrate to `codeMasked`/`codeLast4` for display and to the audited
    /// reveal/export operations for the full value.
    promotion_code: String,
    /// Display form of `promotionCode`; never equal to the full code.
    code_masked: String,
    /// Last four characters of the code (fewer when the code is shorter).
    code_last4: String,
    code_type: String,
    max_claims: i32,
    claimed_quantity: i32,
    starts_at: Option<String>,
    expires_at: Option<String>,
    status: String,
}
impl From<PromotionCodeItem> for CodeResponse {
    fn from(v: PromotionCodeItem) -> Self {
        // Masking happens at the wire boundary only; the stored value and the
        // deprecated `promotionCode` field stay untouched.
        let code_masked = mask_promotion_code(&v.promotion_code);
        let code_last4 = promotion_code_last4(&v.promotion_code);
        Self {
            id: v.id,
            stock_id: v.stock_id,
            offer_id: v.offer_id,
            code_no: v.code_no,
            promotion_code: v.promotion_code,
            code_masked,
            code_last4,
            code_type: v.code_type,
            max_claims: v.max_claims,
            claimed_quantity: v.claimed_quantity,
            starts_at: v.starts_at,
            expires_at: v.expires_at,
            status: v.status,
        }
    }
}

/// One audited reveal result.
///
/// The full code leaves the service only through this shape (and through the
/// export equivalent), which is always preceded by an audit-evidence row.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeRevealResponse {
    code_no: String,
    code: String,
}
impl From<PromotionCodeRevealItem> for CodeRevealResponse {
    fn from(v: PromotionCodeRevealItem) -> Self {
        Self {
            code_no: v.code_no,
            code: v.code,
        }
    }
}

/// One audited batch export result.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeExportResponse {
    items: Vec<CodeRevealResponse>,
    count: i32,
    truncated: bool,
}
impl From<PromotionCodeExportItem> for CodeExportResponse {
    fn from(v: PromotionCodeExportItem) -> Self {
        Self {
            items: v.items.into_iter().map(CodeRevealResponse::from).collect(),
            count: v.count,
            truncated: v.truncated,
        }
    }
}

/// Stop-loss result for one disabled code.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeDisableResponse {
    code_id: String,
    status: String,
}
impl From<PromotionCodeDisableItem> for CodeDisableResponse {
    fn from(v: PromotionCodeDisableItem) -> Self {
        Self {
            code_id: v.code_id,
            status: v.status,
        }
    }
}

/// Result of one code reissue: the new code's identity and display mask plus the
/// id of the code it replaced.
///
/// The new redeemable value is deliberately absent — this operation never reveals
/// it, and the audited reveal/export operations remain the only read paths.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeReissueResponse {
    /// Id of the newly inserted replacement.
    code_id: String,
    code_no: String,
    /// Display mask of the replacement; never equal to its value.
    code_masked: String,
    code_last4: String,
    /// Id of the disabled original.
    replaced_code_id: String,
}
impl From<PromotionCodeReissueItem> for CodeReissueResponse {
    fn from(v: PromotionCodeReissueItem) -> Self {
        Self {
            code_id: v.code_id,
            code_no: v.code_no,
            code_masked: v.code_masked,
            code_last4: v.code_last4,
            replaced_code_id: v.replaced_code_id,
        }
    }
}

/// Stop-loss result for one batch: how many codes were stopped and how many were
/// skipped (already claimed or no longer active).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeBatchDisableResponse {
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    disabled_count: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    skipped_count: i64,
}
impl From<PromotionCodeBatchDisableItem> for CodeBatchDisableResponse {
    fn from(v: PromotionCodeBatchDisableItem) -> Self {
        Self {
            disabled_count: v.disabled_count,
            skipped_count: v.skipped_count,
        }
    }
}

/// Stop-loss result for one voided wallet coupon.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserCouponVoidResponse {
    user_coupon_id: String,
    status: String,
}
impl From<PromotionUserCouponVoidItem> for UserCouponVoidResponse {
    fn from(v: PromotionUserCouponVoidItem) -> Self {
        Self {
            user_coupon_id: v.user_coupon_id,
            status: v.status,
        }
    }
}

/// Result of extending one wallet coupon's expiry (canonical RFC 3339 shape).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserCouponExtensionResponse {
    user_coupon_id: String,
    expires_at: String,
}
impl From<PromotionUserCouponExtendItem> for UserCouponExtensionResponse {
    fn from(v: PromotionUserCouponExtendItem) -> Self {
        Self {
            user_coupon_id: v.user_coupon_id,
            expires_at: v.expires_at,
        }
    }
}

/// Settlement lifecycle result for one discount application.
///
/// `settledAmountMinor` is the amount that left the reservation pool and became
/// spent, as an int64 decimal string of integer minor units (API_SPEC §13.6).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettlementResponse {
    application_id: String,
    status: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    settled_amount_minor: i64,
}
impl From<PromotionDiscountApplicationSettlementItem> for SettlementResponse {
    fn from(v: PromotionDiscountApplicationSettlementItem) -> Self {
        Self {
            application_id: v.application_id,
            status: v.status,
            settled_amount_minor: v.settled_amount_minor,
        }
    }
}

/// Release result for one discount application.
///
/// `releasedAmountMinor` is the reservation that went back to the available pool,
/// as an int64 decimal string of integer minor units (API_SPEC §13.6).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseResponse {
    application_id: String,
    status: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    released_amount_minor: i64,
}
impl From<PromotionDiscountApplicationReleaseItem> for ReleaseResponse {
    fn from(v: PromotionDiscountApplicationReleaseItem) -> Self {
        Self {
            application_id: v.application_id,
            status: v.status,
            released_amount_minor: v.released_amount_minor,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DistributionResponse {
    id: String,
    stock_id: String,
    offer_id: String,
    task_no: String,
    distribution_type: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    requested_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    succeeded_quantity: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    failed_quantity: i64,
    status: String,
    created_at: String,
    completed_at: Option<String>,
}
impl From<PromotionDistributionTaskItem> for DistributionResponse {
    fn from(v: PromotionDistributionTaskItem) -> Self {
        Self {
            id: v.id,
            stock_id: v.stock_id,
            offer_id: v.offer_id,
            task_no: v.task_no,
            distribution_type: v.distribution_type,
            requested_quantity: v.requested_quantity,
            succeeded_quantity: v.succeeded_quantity,
            failed_quantity: v.failed_quantity,
            status: v.status,
            created_at: v.created_at,
            completed_at: v.completed_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserCouponResponse {
    id: String,
    coupon_no: String,
    stock_id: String,
    offer_id: String,
    owner_user_id: String,
    coupon_code: String,
    status: String,
    claimed_at: String,
    valid_from: String,
    expires_at: Option<String>,
    redeemed_at: Option<String>,
    source_type: Option<String>,
    source_id: Option<String>,
}
impl From<PromotionAdminUserCouponItem> for UserCouponResponse {
    fn from(v: PromotionAdminUserCouponItem) -> Self {
        Self {
            id: v.id,
            coupon_no: v.coupon_no,
            stock_id: v.stock_id,
            offer_id: v.offer_id,
            owner_user_id: v.owner_user_id,
            coupon_code: v.coupon_code,
            status: v.status,
            claimed_at: v.claimed_at,
            valid_from: v.valid_from,
            expires_at: v.expires_at,
            redeemed_at: v.redeemed_at,
            source_type: v.source_type,
            source_id: v.source_id,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LedgerResponse {
    id: String,
    stock_id: String,
    user_coupon_id: Option<String>,
    offer_id: String,
    subject_id: Option<String>,
    direction: String,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    quantity_delta: i64,
    #[serde(with = "sdkwork_utils_rust::serde_int64")]
    balance_after: i64,
    business_type: String,
    business_no: String,
    created_at: String,
}
impl From<PromotionCouponLedgerItem> for LedgerResponse {
    fn from(v: PromotionCouponLedgerItem) -> Self {
        Self {
            id: v.id,
            stock_id: v.stock_id,
            user_coupon_id: v.user_coupon_id,
            offer_id: v.offer_id,
            subject_id: v.subject_id,
            direction: v.direction,
            quantity_delta: v.quantity_delta,
            balance_after: v.balance_after,
            business_type: v.business_type,
            business_no: v.business_no,
            created_at: v.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplicationResponse {
    id: String,
    application_no: String,
    order_id: String,
    order_no: Option<String>,
    offer_id: String,
    discount_type: String,
    discount_amount: String,
    currency_code: String,
    status: String,
    applied_at: String,
    settled_at: Option<String>,
    released_at: Option<String>,
    rolled_back_at: Option<String>,
}
impl From<PromotionDiscountApplicationItem> for ApplicationResponse {
    fn from(v: PromotionDiscountApplicationItem) -> Self {
        Self {
            id: v.id,
            application_no: v.application_no,
            order_id: v.order_id,
            order_no: v.order_no,
            offer_id: v.offer_id,
            discount_type: v.discount_type,
            discount_amount: v.discount_amount,
            currency_code: v.currency_code,
            status: v.status,
            applied_at: v.applied_at,
            settled_at: v.settled_at,
            released_at: v.released_at,
            rolled_back_at: v.rolled_back_at,
        }
    }
}

/// One redemption attempt on the wire.
///
/// Every field is a string (or nullable string): ids are TEXT in this schema and
/// `grantedAmount` is a decimal string, so no int64 numeric field is exposed and
/// no value can be rounded by a browser.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RedemptionAttemptResponse {
    id: String,
    code_id: Option<String>,
    submitted_code_suffix: String,
    subject_type: String,
    subject_id: String,
    outcome: String,
    failure_code: Option<String>,
    failure_detail: Option<String>,
    benefit_kind: Option<String>,
    granted_amount: Option<String>,
    currency_code: Option<String>,
    request_no: String,
    order_id: Option<String>,
    trace_id: String,
    occurred_at: String,
}
impl From<PromotionRedemptionAttemptItem> for RedemptionAttemptResponse {
    fn from(v: PromotionRedemptionAttemptItem) -> Self {
        Self {
            id: v.id,
            code_id: v.code_id,
            submitted_code_suffix: v.submitted_code_suffix,
            subject_type: v.subject_type,
            subject_id: v.subject_id,
            outcome: v.outcome,
            failure_code: v.failure_code,
            failure_detail: v.failure_detail,
            benefit_kind: v.benefit_kind,
            granted_amount: v.granted_amount,
            currency_code: v.currency_code,
            request_no: v.request_no,
            order_id: v.order_id,
            trace_id: v.trace_id,
            occurred_at: v.occurred_at,
        }
    }
}

pub fn build_backend_promotion_router(service: Arc<PromotionAdminService>) -> Router {
    Router::new()
        .route("/backend/v3/api/promotions/overview", get(get_overview))
        .route("/backend/v3/api/promotions/reports", get(get_reports))
        .route(
            "/backend/v3/api/promotions/campaigns",
            get(list_campaigns).post(create_campaign),
        )
        .route(
            "/backend/v3/api/promotions/campaigns/{campaignId}",
            get(retrieve_campaign)
                .patch(update_campaign)
                .delete(delete_campaign),
        )
        .route(
            "/backend/v3/api/promotions/offers",
            get(list_offers).post(create_offer),
        )
        .route(
            "/backend/v3/api/promotions/offers/{offerId}",
            get(retrieve_offer).patch(update_offer).delete(delete_offer),
        )
        .route(
            "/backend/v3/api/promotions/offers/{offerId}/status",
            patch(update_offer_status),
        )
        .route(
            "/backend/v3/api/promotions/coupon_stocks",
            get(list_stocks).post(create_stock),
        )
        .route(
            "/backend/v3/api/promotions/code_batches",
            get(list_code_batches).post(create_code_batch),
        )
        .route("/backend/v3/api/promotions/codes", get(list_codes))
        .route(
            "/backend/v3/api/promotions/codes/{codeId}/reveals",
            post(create_code_reveal),
        )
        .route(
            "/backend/v3/api/promotions/code_batches/{batchId}/code_exports",
            post(create_code_export),
        )
        .route(
            "/backend/v3/api/promotions/codes/{codeId}/disables",
            post(create_code_disable),
        )
        .route(
            "/backend/v3/api/promotions/codes/{codeId}/reissues",
            post(create_code_reissue),
        )
        .route(
            "/backend/v3/api/promotions/code_batches/{batchId}/disables",
            post(create_code_batch_disable),
        )
        .route(
            "/backend/v3/api/promotions/user_coupons/{userCouponId}/voids",
            post(create_user_coupon_void),
        )
        .route(
            "/backend/v3/api/promotions/user_coupons/{userCouponId}/extensions",
            post(create_user_coupon_extension),
        )
        .route(
            "/backend/v3/api/promotions/distribution_tasks",
            get(list_distributions).post(create_distribution),
        )
        .route(
            "/backend/v3/api/promotions/user_coupons",
            get(list_user_coupons),
        )
        .route(
            "/backend/v3/api/promotions/coupon_ledger_entries",
            get(list_ledger),
        )
        .route(
            "/backend/v3/api/promotions/discount_applications",
            get(list_applications),
        )
        .route(
            "/backend/v3/api/promotions/discount_applications/{applicationId}/settlements",
            post(create_discount_application_settlement),
        )
        .route(
            "/backend/v3/api/promotions/discount_applications/{applicationId}/releases",
            post(create_discount_application_release),
        )
        .route(
            "/backend/v3/api/promotions/redemption_attempts",
            get(list_redemption_attempts),
        )
        .with_state(PromotionAdminState { service })
}

async fn get_overview(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    match s.service.retrieve_overview(&scope).await {
        Ok(v) => success_item(Some(&c), OverviewResponse::from(v)),
        Err(e) => service_error(Some(&c), "retrieve overview", e),
    }
}

/// Coupon-centre operating report for `[from, to]` (default: last 30 days).
///
/// The route stays thin: tenant/organization/actor come from IAM through
/// [`read_scope`], the window is normalized and validated by the service (so
/// `from > to` and unparseable bounds are `400` before any query), and the
/// response is a single `data.item` envelope.
async fn get_reports(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ReportParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    match s
        .service
        .retrieve_reports(&scope, q.from.as_deref(), q.to.as_deref())
        .await
    {
        Ok(v) => success_item(Some(&c), ReportResponse::from(v)),
        Err(e) => service_error(Some(&c), "retrieve promotion reports", e),
    }
}

async fn list_campaigns(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_campaigns(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, CampaignResponse::from),
        Err(e) => service_error(Some(&c), "list campaigns", e),
    }
}
async fn retrieve_campaign(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "campaignId"));
    match s.service.retrieve_campaign(&scope, id).await {
        Ok(Some(v)) => success_item(Some(&c), CampaignResponse::from(v)),
        Ok(None) => not_found(Some(&c), "campaign not found"),
        Err(e) => service_error(Some(&c), "retrieve campaign", e),
    }
}
async fn create_campaign(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Json(b): Json<CampaignRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let input = response_try!(campaign_input(b).map_err(|e| validation(Some(&c), e.message())));
    match s.service.create_campaign(&scope, &input).await {
        Ok(v) => success_created(Some(&c), CampaignResponse::from(v)),
        Err(e) => service_error(Some(&c), "create campaign", e),
    }
}
async fn update_campaign(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
    Json(b): Json<CampaignRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "campaignId"));
    let input = response_try!(campaign_input(b).map_err(|e| validation(Some(&c), e.message())));
    match s.service.update_campaign(&scope, id, &input).await {
        Ok(Some(v)) => success_item(Some(&c), CampaignResponse::from(v)),
        Ok(None) => not_found(Some(&c), "campaign not found or version conflict"),
        Err(e) => service_error(Some(&c), "update campaign", e),
    }
}
async fn delete_campaign(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "campaignId"));
    match s.service.delete_campaign(&scope, id).await {
        Ok(true) => no_content(Some(&c)),
        Ok(false) => not_found(Some(&c), "draft campaign not found or already in use"),
        Err(e) => service_error(Some(&c), "delete campaign", e),
    }
}

async fn list_offers(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_offers(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, OfferResponse::from),
        Err(e) => service_error(Some(&c), "list offers", e),
    }
}
async fn retrieve_offer(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "offerId"));
    match s.service.retrieve_offer(&scope, id).await {
        Ok(Some(v)) => success_item(Some(&c), OfferResponse::from(v)),
        Ok(None) => not_found(Some(&c), "offer not found"),
        Err(e) => service_error(Some(&c), "retrieve offer", e),
    }
}
async fn create_offer(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Json(b): Json<OfferRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let input = response_try!(offer_input(b).map_err(|e| validation(Some(&c), e.message())));
    match s.service.create_offer(&scope, &input).await {
        Ok(v) => success_created(Some(&c), OfferResponse::from(v)),
        Err(e) => service_error(Some(&c), "create offer", e),
    }
}
async fn update_offer(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
    Json(b): Json<OfferRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "offerId"));
    let input = response_try!(offer_input(b).map_err(|e| validation(Some(&c), e.message())));
    match s.service.update_offer(&scope, id, &input).await {
        Ok(Some(v)) => success_item(Some(&c), OfferResponse::from(v)),
        Ok(None) => not_found(Some(&c), "offer not found or version conflict"),
        Err(e) => service_error(Some(&c), "update offer", e),
    }
}
async fn update_offer_status(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
    Json(b): Json<UpdateStatusRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let parsed = response_try!(parse_id(Some(&c), &id, "offerId"));
    let status = b.status;
    match s.service.update_offer_status(&scope, parsed, &status).await {
        Ok(true) => success_command(Some(&c), id, status),
        Ok(false) => not_found(Some(&c), "offer not found"),
        Err(e) => service_error(Some(&c), "update offer status", e),
    }
}
async fn delete_offer(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(id): Path<String>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let id = response_try!(parse_id(Some(&c), &id, "offerId"));
    match s.service.delete_offer(&scope, id).await {
        Ok(true) => no_content(Some(&c)),
        Ok(false) => not_found(Some(&c), "inactive unused offer not found"),
        Err(e) => service_error(Some(&c), "delete offer", e),
    }
}

async fn list_stocks(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_coupon_stocks(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, StockResponse::from),
        Err(e) => service_error(Some(&c), "list stocks", e),
    }
}
async fn create_stock(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Json(b): Json<CouponStockRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let is_unlimited = b.stock_type.trim().eq_ignore_ascii_case("UNLIMITED");
    let total_quantity = if is_unlimited {
        // 无限库存：总量仅作统计，允许 0
        b.total_quantity.trim().parse::<i64>().unwrap_or(0).max(0)
    } else {
        response_try!(parse_i64_field(&b.total_quantity, "totalQuantity")
            .map_err(|e| validation(Some(&c), e.message())))
    };
    let input = PromotionCouponStockInput {
        offer_id: b.offer_id,
        stock_type: b.stock_type,
        code_issue_mode: b.code_issue_mode.unwrap_or_else(|| {
            sdkwork_commerce_promotion_service::CODE_ISSUE_MODE_REALTIME.to_owned()
        }),
        total_quantity,
        per_user_limit: b.per_user_limit,
        claim_starts_at: b.claim_starts_at,
        claim_ends_at: b.claim_ends_at,
        status: b.status,
        budget_amount_minor: response_try!(
            parse_optional_i64(b.budget_amount_minor.as_deref(), "budgetAmountMinor")
                .map_err(|e| validation(Some(&c), e.message()))
        ),
    };
    match s.service.create_coupon_stock(&scope, &input).await {
        Ok(v) => success_created(Some(&c), StockResponse::from(v)),
        Err(e) => service_error(Some(&c), "create stock", e),
    }
}
async fn list_code_batches(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_code_batches(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, CodeBatchResponse::from),
        Err(e) => service_error(Some(&c), "list code batches", e),
    }
}
async fn create_code_batch(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Json(b): Json<CodeBatchRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let input = PromotionCodeBatchInput {
        stock_id: b.stock_id,
        code_type: b.code_type,
        quantity: response_try!(
            parse_i64_field(&b.requested_quantity, "requestedQuantity").map_err(|e| validation(Some(&c), e.message()))
        ),
        code_length: b.code_length.unwrap_or(16),
        code_prefix: b.code_prefix.unwrap_or_default(),
        custom_codes: b.custom_codes.unwrap_or_default(),
        max_claims: b.max_claims.unwrap_or(1),
        starts_at: b.starts_at,
        expires_at: b.expires_at,
        idempotency_key: b.idempotency_key,
    };
    match s.service.create_code_batch(&scope, &input).await {
        Ok(v) => success_created(Some(&c), CodeBatchResponse::from(v)),
        Err(e) => service_error(Some(&c), "create code batch", e),
    }
}
async fn list_codes(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_codes(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, CodeResponse::from),
        Err(e) => service_error(Some(&c), "list codes", e),
    }
}

/// Audited reveal of one redeemable code.
///
/// The repository writes the `promotion_audit_event` row inside the same
/// transaction as the code read, and fails the operation when that write fails:
/// a redeemable code never leaves the service without evidence. The actor comes
/// only from the IAM-derived [`PromotionAdminScope`]; no body or query field can
/// select it, and a non-operator session is rejected by `manage_scope` before the
/// service is reached.
async fn create_code_reveal(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(code_id): Path<String>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let code_id = response_try!(parse_id(Some(&c), &code_id, "codeId"));
    let trace_id = request_trace_id(&c);
    match s.service.reveal_code(&scope, code_id, &trace_id).await {
        Ok(Some(v)) => success_created(Some(&c), CodeRevealResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion code not found"),
        Err(e) => service_error(Some(&c), "reveal promotion code", e),
    }
}

/// Audited export of one batch's redeemable codes (capped at 5000 rows).
///
/// Same evidence contract as [`create_code_reveal`]; the batch must be inside
/// the operator's tenant/organization scope or the response is `404`.
async fn create_code_export(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(batch_id): Path<String>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let batch_id = response_try!(parse_id(Some(&c), &batch_id, "batchId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .export_code_batch(&scope, batch_id, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), CodeExportResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion code batch not found"),
        Err(e) => service_error(Some(&c), "export promotion code batch", e),
    }
}

/// Trace id of the current request, used as audit evidence.
///
/// Read from the framework-injected request context (W3C `traceparent`), never
/// from a request body or query parameter.
fn request_trace_id(c: &WebRequestContext) -> String {
    c.trace_id.as_deref().unwrap_or_default().trim().to_owned()
}

/// Stop-loss: disables one leaked promotion code.
///
/// The repository writes the `codes.disable` audit row inside the same
/// transaction as the mutation and fails the operation when that write fails.
/// The actor is the IAM-derived [`PromotionAdminScope`] (never a body field); a
/// claimed or non-active code is a `409` conflict.
async fn create_code_disable(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(code_id): Path<String>,
    Json(b): Json<StopLossReasonRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let code_id = response_try!(parse_id(Some(&c), &code_id, "codeId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .disable_code(&scope, code_id, &b.reason, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), CodeDisableResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion code not found"),
        Err(e) => service_error(Some(&c), "disable promotion code", e),
    }
}

/// Stop-loss: reissues one unused promotion code.
///
/// The original must be `active` and never claimed; a claimed or non-active code
/// is a `409` conflict. The repository writes the `codes.reissue` audit row
/// (`subject_count = 2`, both ids and the reason), disables the original with
/// `replaced_by_code_id`, and inserts the replacement with
/// `reissued_from_code_id` inside one transaction. The response names the new
/// code and its mask only — the redeemable value is never returned here.
async fn create_code_reissue(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(code_id): Path<String>,
    Json(b): Json<CodeReissueRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let code_id = response_try!(parse_id(Some(&c), &code_id, "codeId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .reissue_code(&scope, code_id, &b.reason, b.code.as_deref(), &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), CodeReissueResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion code not found"),
        Err(e) => service_error(Some(&c), "reissue promotion code", e),
    }
}

/// Stop-loss: disables every eligible code of one batch.
///
/// Same evidence contract as [`create_code_disable`]; the batch must be inside
/// the operator's tenant/organization scope or the response is `404`.
async fn create_code_batch_disable(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(batch_id): Path<String>,
    Json(b): Json<StopLossReasonRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let batch_id = response_try!(parse_id(Some(&c), &batch_id, "batchId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .disable_code_batch(&scope, batch_id, &b.reason, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), CodeBatchDisableResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion code batch not found"),
        Err(e) => service_error(Some(&c), "disable promotion code batch", e),
    }
}

/// Stop-loss: voids one wallet coupon (claimed/active/pending only).
async fn create_user_coupon_void(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(user_coupon_id): Path<String>,
    Json(b): Json<StopLossReasonRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let user_coupon_id = response_try!(parse_id(Some(&c), &user_coupon_id, "userCouponId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .void_user_coupon(&scope, user_coupon_id, &b.reason, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), UserCouponVoidResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion user coupon not found"),
        Err(e) => service_error(Some(&c), "void promotion user coupon", e),
    }
}

/// Stop-loss: extends one wallet coupon's expiry.
///
/// The service gates the wire shape and the repository canonicalizes the value
/// and compares it with `CURRENT_TIMESTAMP`, so a past or unparseable target is a
/// `400` and never a mutation.
async fn create_user_coupon_extension(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(user_coupon_id): Path<String>,
    Json(b): Json<UserCouponExtensionRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let user_coupon_id = response_try!(parse_id(Some(&c), &user_coupon_id, "userCouponId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .extend_user_coupon(&scope, user_coupon_id, &b.expires_at, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), UserCouponExtensionResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion user coupon not found"),
        Err(e) => service_error(Some(&c), "extend promotion user coupon", e),
    }
}

async fn list_distributions(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_distribution_tasks(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, DistributionResponse::from),
        Err(e) => service_error(Some(&c), "list distributions", e),
    }
}
async fn create_distribution(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Json(b): Json<DistributionRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let owner_user_ids =
        response_try!(validate_owner_user_ids(&b.owner_user_ids)
            .map_err(|e| validation(Some(&c), e.message())));
    let input = PromotionDistributionInput {
        stock_id: b.stock_id,
        owner_user_ids,
        idempotency_key: b.idempotency_key,
    };
    match s.service.create_distribution_task(&scope, &input).await {
        Ok(v) => success_created(Some(&c), DistributionResponse::from(v)),
        Err(e) => service_error(Some(&c), "create distribution", e),
    }
}
async fn list_user_coupons(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_user_coupons(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, UserCouponResponse::from),
        Err(e) => service_error(Some(&c), "list user coupons", e),
    }
}
async fn list_ledger(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_coupon_ledger(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, LedgerResponse::from),
        Err(e) => service_error(Some(&c), "list coupon ledger", e),
    }
}
async fn list_applications(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<ListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(list_query(Some(&c), &q));
    match s.service.list_discount_applications(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, ApplicationResponse::from),
        Err(e) => service_error(Some(&c), "list applications", e),
    }
}
/// Settlement lifecycle: settles one `applied` discount application.
///
/// The repository writes the `discount_applications.settle` audit row inside the
/// same transaction as the mutation and before it; if that write fails the
/// operation fails and nothing changes. The actor is the IAM-derived
/// [`PromotionAdminScope`] (never a body field), and a non-`applied` application
/// is a `409`.
async fn create_discount_application_settlement(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(application_id): Path<String>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let application_id = response_try!(parse_id(Some(&c), &application_id, "applicationId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .settle_discount_application(&scope, application_id, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), SettlementResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion discount application not found"),
        Err(e) => service_error(Some(&c), "settle promotion discount application", e),
    }
}

/// Settlement lifecycle: releases one `applied` discount application.
///
/// Same evidence contract as [`create_discount_application_settlement`]; the
/// operator's reason is required (`1..=256` characters) and is stored on the row.
async fn create_discount_application_release(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Path(application_id): Path<String>,
    Json(b): Json<DiscountApplicationReleaseRequest>,
) -> Response {
    let scope = response_try!(manage_scope(&c, i));
    let application_id = response_try!(parse_id(Some(&c), &application_id, "applicationId"));
    let trace_id = request_trace_id(&c);
    match s
        .service
        .release_discount_application(&scope, application_id, &b.reason, &trace_id)
        .await
    {
        Ok(Some(v)) => success_created(Some(&c), ReleaseResponse::from(v)),
        Ok(None) => not_found(Some(&c), "promotion discount application not found"),
        Err(e) => service_error(Some(&c), "release promotion discount application", e),
    }
}

async fn list_redemption_attempts(
    State(s): State<PromotionAdminState>,
    Extension(i): Extension<IamAppContext>,
    Extension(c): Extension<WebRequestContext>,
    Query(q): Query<RedemptionAttemptListParams>,
) -> Response {
    let scope = response_try!(read_scope(&c, i));
    let (page, q) = response_try!(redemption_attempt_query(Some(&c), &q));
    match s.service.list_redemption_attempts(&scope, &q).await {
        Ok(v) => page_response(Some(&c), v, page, RedemptionAttemptResponse::from),
        Err(e) => service_error(Some(&c), "list redemption attempts", e),
    }
}

fn campaign_input(b: CampaignRequest) -> Result<PromotionCampaignInput, CommerceServiceError> {
    Ok(PromotionCampaignInput {
        campaign_code: b.campaign_code,
        display_name: b.display_name,
        description: b.description,
        channel_scope: b.channel_scope,
        audience_scope: b.audience_scope,
        starts_at: b.starts_at,
        ends_at: b.ends_at,
        status: b.status,
        version: parse_optional_i64(b.version.as_deref(), "version")?,
    })
}
fn offer_input(b: OfferRequest) -> Result<PromotionOfferInput, CommerceServiceError> {
    Ok(PromotionOfferInput {
        campaign_id: b.campaign_id,
        offer_code: b.offer_code,
        offer_type: b.offer_type,
        display_name: b.display_name,
        description: b.description,
        audience_scope: b.audience_scope,
        combinability: b.combinability,
        goods_scope: b.goods_scope,
        priority: b.priority,
        starts_at: b.starts_at,
        ends_at: b.ends_at,
        status: b.status,
        discount_type: b.discount_type,
        discount_value: b.discount_value,
        minimum_amount: b.minimum_amount,
        maximum_discount_amount: b.maximum_discount_amount,
        currency_code: b.currency_code,
        coupon_benefit: b.coupon_benefit.map(coupon_benefit_input).transpose()?,
        version: parse_optional_i64(b.version.as_deref(), "version")?,
    })
}

fn coupon_benefit_input(
    benefit: CouponBenefitRequest,
) -> Result<PromotionCouponBenefit, CommerceServiceError> {
    match benefit {
        CouponBenefitRequest::TokenBankCredit {
            grant_amount,
            bonus_amount,
        } => PromotionCouponBenefit::token_bank_credit_with_bonus(
            parse_i64_field(&grant_amount, "couponBenefit.grantAmount")?,
            parse_optional_i64(bonus_amount.as_deref(), "couponBenefit.bonusAmount")?
                .unwrap_or(0),
        ),
        CouponBenefitRequest::PointsCredit { grant_points } => PromotionCouponBenefit::points_credit(
            parse_i64_field(&grant_points, "couponBenefit.grantPoints")?,
        ),
        CouponBenefitRequest::CashCredit { grant_amount } => PromotionCouponBenefit::cash_credit(
            parse_i64_field(&grant_amount, "couponBenefit.grantAmount")?,
        ),
        CouponBenefitRequest::Subscription {
            period,
            duration_days,
            daily_quota,
            total_quota,
            product_id,
            sku_id,
            package_id,
        } => PromotionCouponBenefit::subscription_with_package(
            product_id.as_deref().unwrap_or_default(),
            sku_id.as_deref().unwrap_or_default(),
            match package_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
                Some(value) => parse_i64_field(value, "couponBenefit.packageId")?,
                None => 0,
            },
            PromotionSubscriptionPeriod::parse(&period)?,
            parse_i64_field(&duration_days, "couponBenefit.durationDays")?,
            parse_i64_field(&daily_quota, "couponBenefit.dailyQuota")?,
            parse_i64_field(&total_quota, "couponBenefit.totalQuota")?,
        ),
    }
}

fn parse_i64_field(value: &str, name: &str) -> Result<i64, CommerceServiceError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            CommerceServiceError::validation(format!("{name} must be a positive integer string"))
        })
}

fn validate_owner_user_ids(values: &[String]) -> Result<Vec<String>, CommerceServiceError> {
    if values.is_empty() {
        return Err(CommerceServiceError::validation(
            "ownerUserIds must not be empty",
        ));
    }
    for value in values {
        if value.trim().is_empty() {
            return Err(CommerceServiceError::validation(
                "ownerUserIds must not contain blank entries",
            ));
        }
    }
    Ok(values.to_vec())
}

fn parse_optional_i64(
    value: Option<&str>,
    name: &str,
) -> Result<Option<i64>, CommerceServiceError> {
    value.map(|value| parse_i64_field(value, name)).transpose()
}

fn read_scope(
    c: &WebRequestContext,
    i: IamAppContext,
) -> Result<sdkwork_commerce_promotion_service::PromotionAdminScope, Response> {
    require_backend_operator(Some(c), i, READ_PERMISSION).map_err(|r| *r)
}
fn manage_scope(
    c: &WebRequestContext,
    i: IamAppContext,
) -> Result<sdkwork_commerce_promotion_service::PromotionAdminScope, Response> {
    require_backend_operator(Some(c), i, MANAGE_PERMISSION).map_err(|r| *r)
}
fn parse_id(c: Option<&WebRequestContext>, value: &str, name: &str) -> Result<String, Response> {
    let value = value.trim();
    if value.is_empty() {
        return Err(validation(c, format!("{name} must not be blank")));
    }
    Ok(value.to_owned())
}
fn list_query(
    c: Option<&WebRequestContext>,
    q: &ListParams,
) -> Result<(OffsetListPageParams, PromotionAdminListQuery), Response> {
    let page = parse_page(c, q.page, q.page_size).map_err(|r| *r)?;
    let query = PromotionAdminListQuery::new(
        page.page,
        page.page_size,
        q.q.as_deref(),
        q.status.as_deref(),
        q.code_batch_id,
        q.stock_id,
        q.code_id.as_deref(),
        q.user_coupon_id.as_deref(),
        q.order_id.as_deref(),
    )
    .map_err(|e| validation(c, e.message()))?;
    Ok((page, query))
}
/// Builds the redemption attempt ledger query.
///
/// `outcome` is validated by [`PromotionRedemptionAttemptListQuery::new`], so an
/// unknown value becomes a 400 instead of an unfiltered list.
fn redemption_attempt_query(
    c: Option<&WebRequestContext>,
    q: &RedemptionAttemptListParams,
) -> Result<(OffsetListPageParams, PromotionRedemptionAttemptListQuery), Response> {
    let page = parse_page(c, q.page, q.page_size).map_err(|r| *r)?;
    let query = PromotionRedemptionAttemptListQuery::new(
        page.page,
        page.page_size,
        q.q.as_deref(),
        q.outcome.as_deref(),
        q.code_id.as_deref(),
    )
    .map_err(|e| validation(c, e.message()))?;
    Ok((page, query))
}
fn page_response<T, R, F>(
    c: Option<&WebRequestContext>,
    result: PromotionAdminPage<T>,
    page: OffsetListPageParams,
    map: F,
) -> Response
where
    R: Serialize,
    F: FnMut(T) -> R,
{
    success_items(
        c,
        result.items.into_iter().map(map).collect(),
        result.total_items,
        page,
    )
}
fn service_error(
    c: Option<&WebRequestContext>,
    operation: &'static str,
    error: CommerceServiceError,
) -> Response {
    tracing::error!(
        operation,
        error_code = error.code(),
        error = error.message(),
        "promotion backend operation failed"
    );
    match error.code() {
        "validation" => validation(c, error.message()),
        "not-found" => not_found(c, error.message()),
        // A domain conflict (stale `version`, resource in use, state that
        // contradicts the request) is NOT a server fault: reporting it as 500
        // made a normal concurrent admin edit look like an outage.
        "conflict" | "unsupported-capability" => conflict(c, error.message()),
        "invalid-state" => invalid_state(c, error.message()),
        "unauthenticated" => unauthorized(c, "authentication is required"),
        "unauthorized" => forbidden(c, "permission is required"),
        _ => internal_error(c, "promotion data operation failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn domain_conflicts_map_to_client_errors_not_server_faults() {
        // Regression: `conflict`/`invalid-state` used to fall into the `_` arm
        // and surface as 500/50001 with the message discarded, so a normal
        // concurrent admin edit looked like an outage.
        let cases = [
            (CommerceServiceError::validation("bad"), 400u16),
            (CommerceServiceError::not_found("gone"), 404),
            (CommerceServiceError::conflict("stale version"), 409),
            (CommerceServiceError::invalid_state("cannot apply"), 422),
        ];
        for (error, expected) in cases {
            let response = service_error(None, "test", error);
            assert_eq!(response.status().as_u16(), expected);
        }
    }

    #[test]
    fn subscription_coupon_request_maps_to_validated_domain_benefit() {
        let request: CouponBenefitRequest = serde_json::from_value(json!({
            "kind": "subscription",
            "period": "month",
            "durationDays": "30",
            "dailyQuota": "1000",
            "totalQuota": "30000"
        }))
        .expect("coupon request");
        assert!(matches!(
            coupon_benefit_input(request).expect("coupon benefit"),
            PromotionCouponBenefit::Subscription {
                daily_quota: 1000,
                total_quota: 30000,
                ..
            }
        ));
    }

    #[test]
    fn token_bank_coupon_response_keeps_target_asset_server_controlled() {
        let response = CouponBenefitResponse::from(
            PromotionCouponBenefit::token_bank_credit(500).expect("coupon benefit"),
        );
        let payload = serde_json::to_value(response).expect("coupon response");
        assert_eq!(payload["kind"], "token_bank_credit");
        assert_eq!(payload["targetAsset"], "token_bank");
        assert_eq!(payload["grantAmount"], "500");
    }

    #[test]
    fn code_list_keeps_the_full_code_and_adds_masked_display_fields() {
        let payload = serde_json::to_value(CodeResponse::from(PromotionCodeItem {
            id: "7011".to_owned(),
            stock_id: "5001".to_owned(),
            offer_id: "4001".to_owned(),
            code_no: "COD-20261111000000-0001".to_owned(),
            promotion_code: "3C76A1B2C3D44E21".to_owned(),
            code_type: "single".to_owned(),
            max_claims: 1,
            claimed_quantity: 0,
            starts_at: None,
            expires_at: None,
            status: "active".to_owned(),
        }))
        .expect("code response");

        // Backward compatibility: the deprecated field is still emitted with the
        // full value.
        assert_eq!(payload["promotionCode"], "3C76A1B2C3D44E21");
        // The two additive display fields.
        assert_eq!(payload["codeMasked"], "3C76****4E21");
        assert_eq!(payload["codeLast4"], "4E21");
        // The mask is never the full code.
        assert_ne!(payload["codeMasked"], payload["promotionCode"]);
    }

    #[test]
    fn coupon_stock_response_wires_budget_as_nullable_minor_strings() {
        fn stock(
            budget: Option<i64>,
        ) -> StockResponse {
            StockResponse::from(PromotionCouponStockItem {
                id: "9001".to_owned(),
                offer_id: "4001".to_owned(),
                stock_no: "STK-20261111000000-0001".to_owned(),
                stock_type: "limited".to_owned(),
                code_issue_mode: "realtime".to_owned(),
                total_quantity: 100,
                available_quantity: 80,
                claimed_quantity: 15,
                redeemed_quantity: 5,
                locked_quantity: 0,
                per_user_limit: 1,
                claim_starts_at: None,
                claim_ends_at: None,
                status: "active".to_owned(),
                budget_amount_minor: budget,
                budget_available_minor: budget.map(|_| 30_000),
                budget_reserved_minor: budget.map(|_| 15_000),
                budget_used_minor: budget.map(|_| 5_000),
            })
        }

        // Money stays an int64 decimal string on the wire, never a JSON number.
        let payload = serde_json::to_value(stock(Some(50_000))).expect("stock response");
        assert_eq!(payload["budgetAmountMinor"], "50000");
        assert_eq!(payload["budgetAvailableMinor"], "30000");
        assert_eq!(payload["budgetReservedMinor"], "15000");
        assert_eq!(payload["budgetUsedMinor"], "5000");

        // No budget account: all four are explicit nulls, never an invented 0.
        let no_budget = serde_json::to_value(stock(None)).expect("stock response");
        for field in [
            "budgetAmountMinor",
            "budgetAvailableMinor",
            "budgetReservedMinor",
            "budgetUsedMinor",
        ] {
            assert!(
                no_budget.get(field).is_some_and(serde_json::Value::is_null),
                "{field} must be present and null when the stock has no budget account"
            );
        }
    }

    #[test]
    fn audited_code_access_routes_are_manage_gated_and_take_the_actor_from_iam() {
        let manifest = include_str!("http_route_manifest.rs");
        // Both operations are registered as `manage` routes, which binds
        // MANAGE_PERMISSION = commerce.marketing.manage.
        assert!(
            manifest.contains("const MANAGE_PERMISSION: &str = \"commerce.marketing.manage\";"),
            "the manage permission constant must stay commerce.marketing.manage"
        );
        for (path, operation_id) in [
            (
                "/backend/v3/api/promotions/codes/{codeId}/reveals",
                "codes.reveals.create",
            ),
            (
                "/backend/v3/api/promotions/code_batches/{batchId}/code_exports",
                "codeBatches.codeExports.create",
            ),
        ] {
            assert!(manifest.contains(path), "manifest must declare {path}");
            let at = manifest
                .find(operation_id)
                .unwrap_or_else(|| panic!("manifest must declare {operation_id}"));
            // The nearest route constructor before the operation id is `manage(`
            // (a `read(` registration would only require commerce.marketing.read).
            let window = &manifest[at.saturating_sub(160)..at];
            assert!(
                window.contains("manage("),
                "{operation_id} must be registered through manage()"
            );
            assert!(
                !window.contains("read("),
                "{operation_id} must not be a read-permission route"
            );
        }

        let source = include_str!("operations.rs");
        for (marker, end_marker, service_call) in [
            (
                "async fn create_code_reveal",
                "/// Audited export of one batch's redeemable codes",
                "reveal_code",
            ),
            (
                "async fn create_code_export",
                "/// Trace id of the current request",
                "export_code_batch",
            ),
        ] {
            let start = source.find(marker).expect("handler");
            let rest = &source[start..];
            let body = &rest[..rest.find(end_marker).expect("handler end")];
            // Tenant/organization/actor all come from the IAM context.
            assert!(
                body.contains("let scope = response_try!(manage_scope(&c, i));"),
                "{marker} must derive the operator scope from IAM via manage_scope"
            );
            // No body extractor: the actor can never be supplied by the caller.
            assert!(
                !body.contains("Json("),
                "{marker} must not accept a request body"
            );
            assert!(
                !body.contains("actor_id"),
                "{marker} must not read an actor id from the request"
            );
            // The audit trace id comes from the request context, not the payload.
            assert!(
                body.contains("let trace_id = request_trace_id(&c);"),
                "{marker} must take the trace id from the request context"
            );
            assert!(
                body.contains(".service") && body.contains(service_call),
                "{marker} must call the service, which enforces actor_id()"
            );
        }

        // The router mounts the same paths the manifest declares.
        let router = include_str!("operations.rs");
        assert!(router.contains("\"/backend/v3/api/promotions/codes/{codeId}/reveals\""));
        assert!(
            router.contains("\"/backend/v3/api/promotions/code_batches/{batchId}/code_exports\"")
        );
    }

    #[test]
    fn stop_loss_routes_are_manage_gated_and_take_the_actor_from_iam() {
        let manifest = include_str!("http_route_manifest.rs");
        let source = include_str!("operations.rs");
        // (path, operationId, handler marker, handler end marker, service call)
        for (path, operation_id, marker, end_marker, service_call) in [
            (
                "/backend/v3/api/promotions/codes/{codeId}/disables",
                "codes.disables.create",
                "async fn create_code_disable",
                "/// Stop-loss: disables every eligible code of one batch.",
                "disable_code(",
            ),
            (
                "/backend/v3/api/promotions/codes/{codeId}/reissues",
                "codes.reissues.create",
                "async fn create_code_reissue",
                "/// Stop-loss: disables every eligible code of one batch.",
                "reissue_code(",
            ),
            (
                "/backend/v3/api/promotions/code_batches/{batchId}/disables",
                "codeBatches.disables.create",
                "async fn create_code_batch_disable",
                "/// Stop-loss: voids one wallet coupon",
                "disable_code_batch(",
            ),
            (
                "/backend/v3/api/promotions/user_coupons/{userCouponId}/voids",
                "userCoupons.voids.create",
                "async fn create_user_coupon_void",
                "/// Stop-loss: extends one wallet coupon's expiry.",
                "void_user_coupon(",
            ),
            (
                "/backend/v3/api/promotions/user_coupons/{userCouponId}/extensions",
                "userCoupons.extensions.create",
                "async fn create_user_coupon_extension",
                "\nfn campaign_input(",
                "extend_user_coupon(",
            ),
        ] {
            assert!(manifest.contains(path), "manifest must declare {path}");
            let at = manifest
                .find(operation_id)
                .unwrap_or_else(|| panic!("manifest must declare {operation_id}"));
            let window = &manifest[at.saturating_sub(160)..at];
            assert!(
                window.contains("manage("),
                "{operation_id} must be registered through manage()"
            );
            assert!(
                !window.contains("read("),
                "{operation_id} must not be a read-permission route"
            );

            let start = source
                .find(marker)
                .unwrap_or_else(|| panic!("missing handler {marker}"));
            let rest = &source[start..];
            let body = &rest[..rest
                .find(end_marker)
                .unwrap_or_else(|| panic!("missing handler end for {marker}"))];
            assert!(
                body.contains("let scope = response_try!(manage_scope(&c, i));"),
                "{marker} must derive the operator scope from IAM via manage_scope"
            );
            // The actor/tenant/organization never come from the body.
            assert!(!body.contains("actor_id"), "{marker}");
            assert!(!body.contains("owner_user_id: b."), "{marker}");
            assert!(!body.contains("tenant_id: b."), "{marker}");
            assert!(
                body.contains("let trace_id = request_trace_id(&c);"),
                "{marker} must take the trace id from the request context"
            );
            assert!(
                body.contains(".service") && body.contains(service_call),
                "{marker} must call the service, which enforces actor_id()"
            );
            // The router mounts the same path the manifest declares.
            assert!(
                source.contains(&format!("\"{path}\"")),
                "router must mount {path}"
            );
        }
    }

    /// The reissue response is a masked-only shape: no field can carry the new
    /// redeemable value, and the handler is manage-gated and audit-driven.
    #[test]
    fn code_reissue_response_is_masked_only() {
        let source = include_str!("operations.rs");
        let start = source
            .find("struct CodeReissueResponse {")
            .expect("missing CodeReissueResponse");
        let rest = &source[start..];
        let declaration = &rest[..rest.find('}').expect("unterminated response")];
        for field in [
            "code_id",
            "code_no",
            "code_masked",
            "code_last4",
            "replaced_code_id",
        ] {
            assert!(
                declaration.contains(&format!("{field}: String")),
                "the reissue response must carry `{field}`"
            );
        }
        // No plaintext field, whatever it would be called.
        assert!(!declaration.contains("promotion_code"));
        assert!(!declaration.contains("promotionCode"));
        assert!(!declaration.contains("code: String"));
        // The handler passes the explicit code through the service, which
        // normalizes it; the route never stores or returns the value itself.
        let handler_start = source
            .find("async fn create_code_reissue")
            .expect("missing reissue handler");
        let handler_rest = &source[handler_start..];
        let handler = &handler_rest[..handler_rest
            .find("/// Stop-loss: disables every eligible code of one batch.")
            .expect("missing handler end")];
        assert!(handler.contains("manage_scope(&c, i)"));
        assert!(handler.contains("reissue_code("));
        assert!(!handler.contains("actor_id"));
        assert!(handler.contains("b.code.as_deref()"));
    }

    /// The report is an operator read: it is registered through `read(`
    /// (`commerce.marketing.read`), mounted at the documented path, and takes
    /// its scope from IAM instead of the query string.
    #[test]
    fn promotion_report_route_is_read_gated_and_mounted() {
        let manifest = include_str!("http_route_manifest.rs");
        let at = manifest
            .find("promotionReports.retrieve")
            .expect("manifest must declare promotionReports.retrieve");
        let window = &manifest[at.saturating_sub(160)..at];
        assert!(
            window.contains("read("),
            "the report must be registered through read()"
        );
        assert!(
            !window.contains("manage("),
            "the report must not require the manage permission"
        );
        assert!(manifest.contains("/backend/v3/api/promotions/reports"));

        let source = include_str!("operations.rs");
        assert!(source.contains("\"/backend/v3/api/promotions/reports\""));
        let start = source.find("async fn get_reports").expect("reports handler");
        let rest = &source[start..];
        let body = &rest[..rest.find("async fn list_campaigns").unwrap_or(rest.len())];
        assert!(
            body.contains("read_scope(&c, i)"),
            "the report handler must take its scope from IAM"
        );
        assert!(
            body.contains("retrieve_reports("),
            "the report handler must call the service"
        );
        assert!(
            !body.contains("Json("),
            "the report handler must not accept a request body"
        );
        assert!(
            !body.contains("tenant_id"),
            "the tenant must never come from the query string"
        );
        assert!(
            !body.contains("organization_id"),
            "the organization must never come from the query string"
        );
    }
}
