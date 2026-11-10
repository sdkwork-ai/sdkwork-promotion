use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest};

const READ_PERMISSION: &str = "commerce.marketing.read";
const MANAGE_PERMISSION: &str = "commerce.marketing.manage";

const HTTP_ROUTES: &[HttpRoute] = &[
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/overview",
        "overview.retrieve",
    ),
    // Coupon-centre operating report: a read-gated, windowed replacement for the
    // lifetime counters, with money per currency and a top-offer ranking.
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/reports",
        "promotionReports.retrieve",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/campaigns",
        "campaigns.list",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/campaigns",
        "campaigns.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/campaigns/{campaignId}",
        "campaigns.retrieve",
    ),
    manage(
        HttpMethod::Patch,
        "/backend/v3/api/promotions/campaigns/{campaignId}",
        "campaigns.update",
    ),
    manage(
        HttpMethod::Delete,
        "/backend/v3/api/promotions/campaigns/{campaignId}",
        "campaigns.delete",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/offers",
        "offers.list",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/offers",
        "offers.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/offers/{offerId}",
        "offers.retrieve",
    ),
    manage(
        HttpMethod::Patch,
        "/backend/v3/api/promotions/offers/{offerId}",
        "offers.update",
    ),
    manage(
        HttpMethod::Delete,
        "/backend/v3/api/promotions/offers/{offerId}",
        "offers.delete",
    ),
    manage(
        HttpMethod::Patch,
        "/backend/v3/api/promotions/offers/{offerId}/status",
        "offers.status.update",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/coupon_stocks",
        "couponStocks.list",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/coupon_stocks",
        "couponStocks.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/code_batches",
        "codeBatches.list",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/code_batches",
        "codeBatches.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/codes",
        "codes.list",
    ),
    // Audited code access: the full redeemable value is served only through
    // these two operations, each gated by `commerce.marketing.manage` and each
    // provisioned by an audit-evidence row.
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/codes/{codeId}/reveals",
        "codes.reveals.create",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/code_batches/{batchId}/code_exports",
        "codeBatches.codeExports.create",
    ),
    // Stop-loss: audited mutations that disable leaked codes, void a wallet
    // coupon, or extend one. Every one is manage-gated and takes its actor from
    // IAM, never from the body.
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/codes/{codeId}/disables",
        "codes.disables.create",
    ),
    // Reissue replaces one unused code with a freshly issued one. It is a
    // manage-gated, audited mutation (`codes.reissue`, `subject_count = 2`) and
    // returns the new code's mask, never its redeemable value.
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/codes/{codeId}/reissues",
        "codes.reissues.create",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/code_batches/{batchId}/disables",
        "codeBatches.disables.create",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/user_coupons/{userCouponId}/voids",
        "userCoupons.voids.create",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/user_coupons/{userCouponId}/extensions",
        "userCoupons.extensions.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/distribution_tasks",
        "distributionTasks.list",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/distribution_tasks",
        "distributionTasks.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/user_coupons",
        "userCoupons.list",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/coupon_ledger_entries",
        "couponLedgerEntries.list",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/discount_applications",
        "discountApplications.list",
    ),
    // Settlement lifecycle: audited, manage-gated mutations that settle or
    // release one applied discount application. Their actor comes from IAM, never
    // from the body.
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/discount_applications/{applicationId}/settlements",
        "discountApplications.settlements.create",
    ),
    manage(
        HttpMethod::Post,
        "/backend/v3/api/promotions/discount_applications/{applicationId}/releases",
        "discountApplications.releases.create",
    ),
    read(
        HttpMethod::Get,
        "/backend/v3/api/promotions/redemption_attempts",
        "redemptionAttempts.list",
    ),
];

const fn read(method: HttpMethod, path: &'static str, operation_id: &'static str) -> HttpRoute {
    HttpRoute::dual_token(method, path, "promotions", operation_id)
        .with_required_permission(READ_PERMISSION)
}

const fn manage(method: HttpMethod, path: &'static str, operation_id: &'static str) -> HttpRoute {
    HttpRoute::dual_token(method, path, "promotions", operation_id)
        .with_required_permission(MANAGE_PERMISSION)
}

pub fn backend_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(HTTP_ROUTES)
}
