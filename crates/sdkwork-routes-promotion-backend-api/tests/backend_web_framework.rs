use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_commerce_promotion_service::{
    CodeReissueOutcome, PromotionAdminFuture, PromotionAdminListQuery, PromotionAdminPage,
    PromotionAdminRepositoryPort, PromotionAdminScope, PromotionAdminUserCouponItem,
    PromotionCampaignInput, PromotionCampaignItem, PromotionCodeBatchDisableItem,
    PromotionCodeBatchInput, PromotionCodeBatchItem, PromotionCodeDisableItem,
    PromotionCodeExportItem, PromotionCodeItem, PromotionCodeRevealItem,
    PromotionCouponLedgerItem, PromotionCouponStockInput, PromotionCouponStockItem,
    PromotionDiscountApplicationItem, PromotionDiscountApplicationReleaseItem,
    PromotionDiscountApplicationSettlementItem, PromotionDistributionInput,
    PromotionDistributionTaskItem, PromotionOfferInput, PromotionOfferItem, PromotionOverview,
    PromotionRedemptionAttemptItem, PromotionRedemptionAttemptListQuery, PromotionReport,
    PromotionReportCounts, PromotionReportCurrencyRow, PromotionReportTopOffer,
    PromotionUserCouponExtendItem, PromotionUserCouponVoidItem,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_routes_promotion_backend_api::{
    build_backend_promotion_router, wrap_router_with_web_framework,
};
use serde_json::{json, Value};
use tower::ServiceExt;

// These tests run in parallel threads of one process, so `SDKWORK_ENVIRONMENT`
// and `SDKWORK_IAM_ENVIRONMENT` are set idempotently per test and never removed:
// removing one while another test is mid-request made that request fail as an
// unavailable IAM database session.


#[derive(Default)]
struct RecordingPromotionAdminRepository {
    scopes: Mutex<Vec<PromotionAdminScope>>,
    /// Attempts returned by `list_redemption_attempts`; empty means an empty page.
    attempts: Mutex<Vec<PromotionRedemptionAttemptItem>>,
    /// Queries the redemption attempt list received.
    queries: Mutex<Vec<PromotionRedemptionAttemptListQuery>>,
    /// Result of `reveal_code`; `None` means "not in the operator's scope".
    reveal: Mutex<Option<PromotionCodeRevealItem>>,
    /// `(code_id, trace_id)` the reveal route passed to the repository.
    reveals: Mutex<Vec<(String, String)>>,
    /// Result of `export_code_batch`; `None` means "not in scope".
    export: Mutex<Option<PromotionCodeExportItem>>,
    /// `(batch_id, trace_id)` the export route passed to the repository.
    exports: Mutex<Vec<(String, String)>>,
    /// Result of `reissue_code`; `None` means "not in the operator's scope".
    reissue: Mutex<Option<CodeReissueOutcome>>,
    /// `(code_id, reason, explicit code)` the reissue route passed through.
    reissues: Mutex<Vec<(String, String, Option<String>)>>,
    /// `(code_id, reason)` the single-code disable route passed through.
    disables: Mutex<Vec<(String, String)>>,
    /// `(batch_id, reason)` the batch disable route passed through.
    batch_disables: Mutex<Vec<(String, String)>>,
    /// `(user_coupon_id, reason)` the void route passed through.
    voids: Mutex<Vec<(String, String)>>,
    /// `(user_coupon_id, expires_at)` the extension route passed through.
    extensions: Mutex<Vec<(String, String)>>,
    /// `(application_id, trace_id)` the settlement route passed through.
    settlements: Mutex<Vec<(String, String)>>,
    /// `(application_id, reason)` the release route passed through.
    releases: Mutex<Vec<(String, String)>>,
    /// `(from, to)` the reports route passed through, already normalized.
    reports: Mutex<Vec<(String, String)>>,
}

impl PromotionAdminRepositoryPort for RecordingPromotionAdminRepository {
    fn retrieve_overview<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
    ) -> PromotionAdminFuture<'a, PromotionOverview> {
        Box::pin(async move {
            self.scopes.lock().expect("scopes").push(scope.clone());
            Ok(PromotionOverview {
                active_offers: 2,
                total_offers: 3,
                total_coupon_stock: 100,
                available_coupons: 80,
                claimed_coupons: 15,
                redeemed_coupons: 5,
                active_codes: 4,
                discount_applications: 6,
            })
        })
    }

    fn list_campaigns<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCampaignItem>> {
        empty_page()
    }

    fn retrieve_reports<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        from: &'a str,
        to: &'a str,
    ) -> PromotionAdminFuture<'a, PromotionReport> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.reports
            .lock()
            .expect("reports")
            .push((from.to_owned(), to.to_owned()));
        Box::pin(async move {
            let counts = PromotionReportCounts {
                issued_codes: 120,
                claimed_coupons: 90,
                redeemed_coupons: 45,
                expired_coupons: 3,
                voided_coupons: 2,
                disabled_codes: 1,
                applications_applied: 80,
                applications_settled: 70,
                applications_released: 5,
            };
            let rates =
                sdkwork_commerce_promotion_service::report_rates(&counts);
            Ok(PromotionReport {
                from: from.to_owned(),
                to: to.to_owned(),
                counts,
                rates,
                by_currency: vec![PromotionReportCurrencyRow {
                    currency_code: "CNY".to_owned(),
                    discount_applied_minor: 125_000,
                    discount_settled_minor: 100_000,
                    budget_total_minor: 500_000,
                    budget_reserved_minor: 15_000,
                    budget_used_minor: 100_000,
                }],
                top_offers: vec![PromotionReportTopOffer {
                    offer_id: "4001".to_owned(),
                    offer_no: "OFF-20261111000000-0001".to_owned(),
                    display_name: "Welcome coupon".to_owned(),
                    redeemed: 45,
                    discount_settled_minor: 100_000,
                    currency_code: "CNY".to_owned(),
                }],
            })
        })
    }

    fn retrieve_campaign<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>> {
        unreachable_future()
    }
    fn create_campaign<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionCampaignInput,
    ) -> PromotionAdminFuture<'a, PromotionCampaignItem> {
        unreachable_future()
    }
    fn update_campaign<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
        _: &'a PromotionCampaignInput,
    ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>> {
        unreachable_future()
    }
    fn delete_campaign<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
    ) -> PromotionAdminFuture<'a, bool> {
        unreachable_future()
    }

    fn list_offers<'a>(
        &'a self,
        _scope: &'a PromotionAdminScope,
        _query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionOfferItem>> {
        empty_page()
    }

    fn retrieve_offer<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>> {
        unreachable_future()
    }
    fn create_offer<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionOfferInput,
    ) -> PromotionAdminFuture<'a, PromotionOfferItem> {
        unreachable_future()
    }
    fn update_offer<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
        _: &'a PromotionOfferInput,
    ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>> {
        unreachable_future()
    }
    fn delete_offer<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: String,
    ) -> PromotionAdminFuture<'a, bool> {
        unreachable_future()
    }

    fn update_offer_status<'a>(
        &'a self,
        _scope: &'a PromotionAdminScope,
        _offer_id: String,
        _status: &'a str,
    ) -> PromotionAdminFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }

    fn list_coupon_stocks<'a>(
        &'a self,
        _scope: &'a PromotionAdminScope,
        _query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponStockItem>> {
        empty_page()
    }

    fn create_coupon_stock<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionCouponStockInput,
    ) -> PromotionAdminFuture<'a, PromotionCouponStockItem> {
        unreachable_future()
    }
    fn list_code_batches<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeBatchItem>> {
        empty_page()
    }
    fn create_code_batch<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionCodeBatchInput,
    ) -> PromotionAdminFuture<'a, PromotionCodeBatchItem> {
        unreachable_future()
    }

    fn list_codes<'a>(
        &'a self,
        _scope: &'a PromotionAdminScope,
        _query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeItem>> {
        empty_page()
    }

    fn list_distribution_tasks<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDistributionTaskItem>> {
        empty_page()
    }
    fn create_distribution_task<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionDistributionInput,
    ) -> PromotionAdminFuture<'a, PromotionDistributionTaskItem> {
        unreachable_future()
    }
    fn list_user_coupons<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionAdminUserCouponItem>> {
        empty_page()
    }
    fn list_coupon_ledger<'a>(
        &'a self,
        _: &'a PromotionAdminScope,
        _: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponLedgerItem>> {
        empty_page()
    }

    fn list_discount_applications<'a>(
        &'a self,
        _scope: &'a PromotionAdminScope,
        _query: &'a PromotionAdminListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDiscountApplicationItem>> {
        empty_page()
    }

    fn list_redemption_attempts<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        query: &'a PromotionRedemptionAttemptListQuery,
    ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionRedemptionAttemptItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.queries.lock().expect("queries").push(query.clone());
        let items = self.attempts.lock().expect("attempts").clone();
        let total_items = items.len() as i64;
        Box::pin(async move { Ok(PromotionAdminPage { items, total_items }) })
    }

    fn reveal_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeRevealItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.reveals
            .lock()
            .expect("reveals")
            .push((code_id, trace_id));
        let item = self.reveal.lock().expect("reveal").clone();
        Box::pin(async move { Ok(item) })
    }

    fn export_code_batch<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        batch_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeExportItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.exports
            .lock()
            .expect("exports")
            .push((batch_id, trace_id));
        let export = self.export.lock().expect("export").clone();
        Box::pin(async move { Ok(export) })
    }

    fn disable_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        reason: String,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeDisableItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.disables
            .lock()
            .expect("disables")
            .push((code_id.clone(), reason));
        Box::pin(async move {
            Ok(Some(PromotionCodeDisableItem {
                code_id,
                status: "disabled".to_owned(),
            }))
        })
    }

    fn disable_code_batch<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        batch_id: String,
        reason: String,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionCodeBatchDisableItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.batch_disables
            .lock()
            .expect("batch disables")
            .push((batch_id, reason));
        Box::pin(async {
            Ok(Some(PromotionCodeBatchDisableItem {
                disabled_count: 7,
                skipped_count: 3,
            }))
        })
    }

    fn reissue_code<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        code_id: String,
        reason: String,
        explicit_code: Option<String>,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<CodeReissueOutcome>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.reissues
            .lock()
            .expect("reissues")
            .push((code_id.clone(), reason, explicit_code));
        let outcome = self.reissue.lock().expect("reissue").clone();
        // The route must never see the redeemable value: this double can only
        // answer with the masked-only outcome the real repository returns.
        Box::pin(async move { Ok(outcome) })
    }

    fn void_user_coupon<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        user_coupon_id: String,
        reason: String,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionUserCouponVoidItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.voids
            .lock()
            .expect("voids")
            .push((user_coupon_id.clone(), reason));
        Box::pin(async move {
            Ok(Some(PromotionUserCouponVoidItem {
                user_coupon_id,
                status: "voided".to_owned(),
            }))
        })
    }

    fn extend_user_coupon<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        user_coupon_id: String,
        expires_at: String,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionUserCouponExtendItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.extensions
            .lock()
            .expect("extensions")
            .push((user_coupon_id.clone(), expires_at));
        Box::pin(async move {
            Ok(Some(PromotionUserCouponExtendItem {
                user_coupon_id,
                // The repository owns canonicalization; this double mimics the
                // canonical output shape the route must relay.
                expires_at: "2026-12-01T00:00:00.000Z".to_owned(),
            }))
        })
    }

    fn settle_discount_application<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        application_id: String,
        trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionDiscountApplicationSettlementItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.settlements
            .lock()
            .expect("settlements")
            .push((application_id.clone(), trace_id));
        // The state-machine conflict the route must propagate as 409.
        if application_id == "already-settled" {
            return Box::pin(async {
                Err(CommerceServiceError::conflict(
                    "promotion discount application has already been settled",
                ))
            });
        }
        Box::pin(async move {
            Ok(Some(PromotionDiscountApplicationSettlementItem {
                application_id,
                status: "settled".to_owned(),
                settled_amount_minor: 1_250,
            }))
        })
    }

    fn release_discount_application<'a>(
        &'a self,
        scope: &'a PromotionAdminScope,
        application_id: String,
        reason: String,
        _trace_id: String,
    ) -> PromotionAdminFuture<'a, Option<PromotionDiscountApplicationReleaseItem>> {
        self.scopes.lock().expect("scopes").push(scope.clone());
        self.releases
            .lock()
            .expect("releases")
            .push((application_id.clone(), reason));
        if application_id == "already-released" {
            return Box::pin(async {
                Err(CommerceServiceError::conflict(
                    "promotion discount application has already been released",
                ))
            });
        }
        Box::pin(async move {
            Ok(Some(PromotionDiscountApplicationReleaseItem {
                application_id,
                status: "released".to_owned(),
                released_amount_minor: 1_250,
            }))
        })
    }
}

fn empty_page<'a, T: Send + 'a>() -> PromotionAdminFuture<'a, PromotionAdminPage<T>> {
    Box::pin(async {
        Ok(PromotionAdminPage {
            items: Vec::new(),
            total_items: 0,
        })
    })
}

fn unreachable_future<'a, T: Send + 'a>() -> PromotionAdminFuture<'a, T> {
    Box::pin(async { unreachable!("mutation is not used by this route test") })
}

fn organization_tokens() -> (String, String) {
    organization_tokens_with_permission("commerce.marketing.read")
}

fn manage_tokens() -> (String, String) {
    organization_tokens_with_permission("commerce.marketing.manage")
}

fn organization_tokens_with_permission(permission: &str) -> (String, String) {
    let shared = json!({
        "tenant_id": "100001",
        "organization_id": "300001",
        "user_id": "900001",
        "session_id": "session-1",
        "app_id": "sdkwork-manager-pc",
        "environment": "dev",
        "deployment_mode": "saas",
        "login_scope": "ORGANIZATION"
    });
    let mut auth = shared.clone();
    auth["token_type"] = json!("auth");
    auth["auth_level"] = json!("mfa");
    // token-claims-gate: legacy-fixture --constructs a pre-slimming credential so this
    // test can assert the claim is no longer an authorization source.
    auth["permission_scope"] = json!(permission);
    let mut access = shared;
    access["token_type"] = json!("access");
    (
        sdkwork_web_core::encode_unsigned_test_jwt(auth),
        sdkwork_web_core::encode_unsigned_test_jwt(access),
    )
}

#[tokio::test]
async fn authenticated_overview_receives_iam_context_and_returns_success() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = organization_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/promotions/overview")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");


    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");
    assert_eq!(status, StatusCode::OK, "unexpected response: {payload}");
    assert_eq!(payload["code"], 0);
    // int64 counters are serialized as decimal strings on the wire
    // (API_SPEC §13.6); the assertion matches that contract.
    assert_eq!(payload["data"]["item"]["totalOffers"], "3");
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[PromotionAdminScope::new(100001, 300001, "900001").expect("scope")]
    );
}

/// The operating review consumes this exact payload: one window, nine counters,
/// three basis-point rates, money per currency, and a ranked top-offer list. All
/// int64 values are decimal strings and all money is integer minor units, so a
/// browser cannot round an id or render a minor amount as major units.
#[tokio::test]
async fn promotion_reports_returns_the_windowed_report_shape() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = organization_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/promotions/reports?from=2026-10-12T00:00:00Z&to=2026-11-11T23:59:59Z")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");

    assert_eq!(status, StatusCode::OK, "unexpected response: {payload}");
    assert_eq!(payload["code"], 0);
    let item = &payload["data"]["item"];
    // The window is the canonical RFC 3339 pair the service normalized.
    assert_eq!(item["from"], "2026-10-12T00:00:00.000Z");
    assert_eq!(item["to"], "2026-11-11T23:59:59.000Z");
    // Nine counters, every one an int64 decimal string.
    for (field, expected) in [
        ("issuedCodes", "120"),
        ("claimedCoupons", "90"),
        ("redeemedCoupons", "45"),
        ("expiredCoupons", "3"),
        ("voidedCoupons", "2"),
        ("disabledCodes", "1"),
        ("applicationsApplied", "80"),
        ("applicationsSettled", "70"),
        ("applicationsReleased", "5"),
    ] {
        assert_eq!(item["counts"][field], expected, "counts.{field}");
    }
    // Basis points: integer math over the issued-code denominator.
    assert_eq!(item["rates"]["claimRateBp"], "7500");
    assert_eq!(item["rates"]["redemptionRateBp"], "3750");
    // 2 * 10_000 / 120 = 166.66 -> 166 (round down).
    assert_eq!(item["rates"]["voidRateBp"], "166");
    // Money is per currency and in integer minor units.
    let currency = &item["byCurrency"][0];
    assert_eq!(currency["currencyCode"], "CNY");
    assert_eq!(currency["discountAppliedMinor"], "125000");
    assert_eq!(currency["discountSettledMinor"], "100000");
    assert_eq!(currency["budgetTotalMinor"], "500000");
    assert_eq!(currency["budgetReservedMinor"], "15000");
    assert_eq!(currency["budgetUsedMinor"], "100000");
    // Every ranked offer carries its own currency.
    let offer = &item["topOffers"][0];
    assert_eq!(offer["offerId"], "4001");
    assert_eq!(offer["offerNo"], "OFF-20261111000000-0001");
    assert_eq!(offer["displayName"], "Welcome coupon");
    assert_eq!(offer["redeemed"], "45");
    assert_eq!(offer["discountSettledMinor"], "100000");
    assert_eq!(offer["currencyCode"], "CNY");
    // The route handed the repository the normalized, inclusive window.
    assert_eq!(
        repository.reports.lock().expect("reports").as_slice(),
        &[(
            "2026-10-12T00:00:00.000Z".to_owned(),
            "2026-11-11T23:59:59.000Z".to_owned()
        )]
    );
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[PromotionAdminScope::new(100001, 300001, "900001").expect("scope")]
    );
}

/// An invalid window is a client error, not an empty report: `from > to` and an
/// unparseable bound are both rejected before the repository runs.
#[tokio::test]
async fn promotion_reports_rejects_an_invalid_window() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = organization_tokens();

    for uri in [
        // from after to
        "/backend/v3/api/promotions/reports?from=2026-11-11T00:00:00Z&to=2026-11-10T00:00:00Z",
        // unparseable from
        "/backend/v3/api/promotions/reports?from=2026/11/10",
        // unparseable to
        "/backend/v3/api/promotions/reports?to=2026-11-10",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("authorization", format!("Bearer {auth_token}"))
                    .header("access-token", &access_token)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let payload: Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response body"),
        )
        .expect("response json");
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{uri} must be a client error: {payload}"
        );
        assert!(
            content_type.starts_with("application/problem+json"),
            "errors must be problem+json: {content_type}"
        );
        assert!(
            payload["code"].is_number(),
            "ProblemDetail.code must be numeric: {payload}"
        );
    }
    // The repository never saw a query: the service rejected the window first.
    assert!(
        repository.reports.lock().expect("reports").is_empty(),
        "an invalid window must not reach the repository"
    );
}

fn attempt(outcome: &str, failure_code: Option<&str>) -> PromotionRedemptionAttemptItem {
    PromotionRedemptionAttemptItem {
        id: "8844220011".to_owned(),
        code_id: Some("7011".to_owned()),
        submitted_code_suffix: "2026".to_owned(),
        subject_type: "user".to_owned(),
        subject_id: "900001".to_owned(),
        outcome: outcome.to_owned(),
        failure_code: failure_code.map(str::to_owned),
        failure_detail: failure_code.map(|_| "promotion code is invalid or unavailable".to_owned()),
        benefit_kind: failure_code
            .is_none()
            .then(|| "token_bank_credit".to_owned()),
        granted_amount: failure_code.is_none().then(|| "500".to_owned()),
        currency_code: Some("CNY".to_owned()),
        request_no: "REQ-1".to_owned(),
        order_id: None,
        trace_id: String::new(),
        occurred_at: "2026-11-10 09:12:44".to_owned(),
    }
}

/// The UI builds against this exact payload: every field is a string (or null),
/// the list lives under `data.items`, and paging metadata sits beside it.
#[tokio::test]
async fn redemption_attempt_list_returns_the_contract_shape() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    repository.attempts.lock().expect("attempts").extend([
        attempt("succeeded", None),
        attempt("failed", Some("code_unavailable")),
    ]);
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = organization_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/promotions/redemption_attempts?page=1&page_size=20&outcome=failed&code_id=7011")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");


    assert_eq!(status, StatusCode::OK, "unexpected response: {payload}");
    assert_eq!(payload["code"], 0);
    let items = payload["data"]["items"]
        .as_array()
        .expect("data.items must be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(
        payload["data"]["pageInfo"]["mode"], "offset",
        "paging metadata must stay beside the items: {payload}"
    );
    let succeeded = &items[0];
    for field in [
        "id",
        "codeId",
        "submittedCodeSuffix",
        "subjectType",
        "subjectId",
        "outcome",
        "failureCode",
        "failureDetail",
        "benefitKind",
        "grantedAmount",
        "currencyCode",
        "requestNo",
        "orderId",
        "traceId",
        "occurredAt",
    ] {
        assert!(
            succeeded.get(field).is_some(),
            "redemption attempt items must always carry `{field}`: {succeeded}"
        );
    }
    assert_eq!(succeeded["id"], "8844220011");
    assert_eq!(succeeded["submittedCodeSuffix"], "2026");
    assert_eq!(succeeded["outcome"], "succeeded");
    assert_eq!(succeeded["grantedAmount"], "500");
    assert!(succeeded["failureCode"].is_null());
    assert_eq!(items[1]["outcome"], "failed");
    assert_eq!(items[1]["failureCode"], "code_unavailable");
    // The submitted code itself is never returned.
    assert!(succeeded.get("promotionCode").is_none());
    // The outcome/code filters reach the repository.
    let queries = repository.queries.lock().expect("queries");
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].outcome.as_deref(), Some("failed"));
    assert_eq!(queries[0].code_id.as_deref(), Some("7011"));
    assert_eq!(queries[0].page_size, 20);
}

/// An unknown `outcome` is a client error, not an unfiltered list.
#[tokio::test]
async fn redemption_attempt_list_rejects_an_unknown_outcome() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = organization_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/promotions/redemption_attempts?outcome=failure")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");


    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "unexpected response: {payload}"
    );
    assert!(
        content_type.starts_with("application/problem+json"),
        "errors must be problem+json: {content_type}"
    );
    assert!(
        payload["code"].is_number(),
        "ProblemDetail.code must be numeric: {payload}"
    );
    assert!(
        payload["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("outcome must be one of"),
        "unexpected detail: {payload}"
    );
    // The repository was never reached: no silent unfiltered list.
    assert!(repository.queries.lock().expect("queries").is_empty());
}

/// The audited reveal returns the full code in `data.item`, records exactly the
/// operator scope from IAM, and passes the path id through unchanged.
#[tokio::test]
async fn audited_code_reveal_returns_the_item_shape_and_the_operator_scope() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    *repository.reveal.lock().expect("reveal") = Some(PromotionCodeRevealItem {
        code_no: "COD-20261111000000-0001".to_owned(),
        code: "3C76A1B2C3D44E21".to_owned(),
    });
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = manage_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backend/v3/api/promotions/codes/7011/reveals")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");


    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unexpected response: {payload}"
    );
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["codeNo"], "COD-20261111000000-0001");
    assert_eq!(payload["data"]["item"]["code"], "3C76A1B2C3D44E21");

    // Tenant, organization, and actor all came from the IAM context.
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[PromotionAdminScope::new(100001, 300001, "900001").expect("scope")]
    );
    let reveals = repository.reveals.lock().expect("reveals");
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].0, "7011");
}

/// The audited export returns `{ items, count, truncated }` under `data.item`,
/// with the batch id taken from the path.
#[tokio::test]
async fn audited_code_export_returns_items_count_and_truncated() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    *repository.export.lock().expect("export") = Some(PromotionCodeExportItem {
        items: vec![
            PromotionCodeRevealItem {
                code_no: "COD-0001".to_owned(),
                code: "3C76A1B2C3D44E21".to_owned(),
            },
            PromotionCodeRevealItem {
                code_no: "COD-0002".to_owned(),
                code: "3C76A1B2C3D44E22".to_owned(),
            },
        ],
        count: 2,
        truncated: false,
    });
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = manage_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backend/v3/api/promotions/code_batches/8801/code_exports")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");


    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unexpected response: {payload}"
    );
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["count"], 2);
    assert_eq!(payload["data"]["item"]["truncated"], false);
    let items = payload["data"]["item"]["items"]
        .as_array()
        .expect("data.item.items must be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["codeNo"], "COD-0001");
    assert_eq!(items[0]["code"], "3C76A1B2C3D44E21");

    let exports = repository.exports.lock().expect("exports");
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0].0, "8801");
}

/// A code outside the operator scope is a 404, not an empty reveal.
#[tokio::test]
async fn audited_code_reveal_404s_when_the_code_is_outside_the_scope() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = manage_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backend/v3/api/promotions/codes/7011/reveals")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");


    assert_eq!(status, StatusCode::NOT_FOUND, "unexpected: {payload}");
    assert!(payload["code"].is_number(), "{payload}");
    assert_eq!(
        repository.scopes.lock().expect("scopes").len(),
        1,
        "the service was reached with the operator scope"
    );
}

/// A batch outside the operator scope is a 404, not an empty export.
#[tokio::test]
async fn audited_code_export_404s_when_the_batch_is_outside_the_scope() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository.clone()),
    );
    let app = wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    );
    let (auth_token, access_token) = manage_tokens();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backend/v3/api/promotions/code_batches/8801/code_exports")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");


    assert_eq!(status, StatusCode::NOT_FOUND, "unexpected: {payload}");
    assert!(payload["code"].is_number(), "{payload}");
    assert_eq!(
        repository.scopes.lock().expect("scopes").len(),
        1,
        "the service was reached with the operator scope"
    );
}

fn stop_loss_app(repository: Arc<RecordingPromotionAdminRepository>) -> axum::Router {
    let service = Arc::new(
        sdkwork_commerce_promotion_service::PromotionAdminService::new(repository),
    );
    wrap_router_with_web_framework(
        IamWebRequestContextResolver::new(None),
        build_backend_promotion_router(service),
    )
}

async fn post_json(
    app: axum::Router,
    uri: &str,
    body: Value,
    (auth_token, access_token): &(String, String),
) -> (StatusCode, String, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {auth_token}"))
                .header("access-token", access_token)
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");
    (status, content_type, payload)
}

/// Stop-loss code routes: `data.item` carries `{ codeId, status }` and the batch
/// route carries `{ disabledCount, skippedCount }` as int64 strings.
#[tokio::test]
async fn stop_loss_code_disable_routes_return_the_contract_shapes() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    let (status, content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/codes/7011/disables",
        json!({ "reason": "  leaked on a public forum  " }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert!(content_type.starts_with("application/json"));
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["codeId"], "7011");
    assert_eq!(payload["data"]["item"]["status"], "disabled");
    assert!(payload["traceId"].as_str().is_some_and(|value| !value.is_empty()));

    let (status, _content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/code_batches/8801/disables",
        json!({ "reason": "batch leaked" }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert_eq!(payload["code"], 0);
    // int64 wire fields stay decimal strings.
    assert_eq!(payload["data"]["item"]["disabledCount"], "7");
    assert_eq!(payload["data"]["item"]["skippedCount"], "3");

    // The actor/tenancy came from IAM, never from the body.
    let expected_scope = PromotionAdminScope::new(100001, 300001, "900001").expect("scope");
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[expected_scope.clone(), expected_scope]
    );
    let disables = repository.disables.lock().expect("disables");
    assert_eq!(disables.len(), 1);
    assert_eq!(disables[0].0, "7011");
    // The service trimmed the reason before the repository saw it.
    assert_eq!(disables[0].1, "leaked on a public forum");
    assert_eq!(
        repository.batch_disables.lock().expect("batch disables")[0].0,
        "8801"
    );

}

/// Reissue returns the new code's identity and mask plus the replaced id under
/// `data.item`; the redeemable value is never part of the payload.
#[tokio::test]
async fn code_reissue_route_returns_the_new_code_masked_only() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    *repository.reissue.lock().expect("reissue") = Some(CodeReissueOutcome {
        new_code_id: "880011".to_owned(),
        new_code_no: "COD-20261111000000-0009".to_owned(),
        code_masked: "3C76****4E21".to_owned(),
        code_last4: "4E21".to_owned(),
    });
    let tokens = manage_tokens();
    let (status, content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/codes/7011/reissues",
        json!({ "reason": "  customer lost the printed card  ", "code": "welcome-2026" }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert!(content_type.starts_with("application/json"));
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["codeId"], "880011");
    assert_eq!(
        payload["data"]["item"]["codeNo"],
        "COD-20261111000000-0009"
    );
    assert_eq!(payload["data"]["item"]["codeMasked"], "3C76****4E21");
    assert_eq!(payload["data"]["item"]["codeLast4"], "4E21");
    assert_eq!(payload["data"]["item"]["replacedCodeId"], "7011");
    // The new redeemable value is not part of the response: the audited
    // reveal/export operations remain its only read paths.
    for forbidden in ["code", "promotionCode", "promotion_code", "value"] {
        assert!(
            payload["data"]["item"].get(forbidden).is_none(),
            "reissue must not return the plaintext code as `{forbidden}`: {payload}"
        );
    }
    assert_ne!(
        payload["data"]["item"]["codeMasked"], payload["data"]["item"]["codeId"],
        "the masked value is never the code identifier"
    );

    // The service trimmed the reason and normalized the explicit code before the
    // repository saw them; the actor/tenancy came from IAM.
    assert_eq!(
        repository.reissues.lock().expect("reissues")[0],
        (
            "7011".to_owned(),
            "customer lost the printed card".to_owned(),
            Some("WELCOME-2026".to_owned())
        )
    );
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[PromotionAdminScope::new(100001, 300001, "900001").expect("scope")]
    );
}

/// A code outside the operator scope is a 404, not a masked success.
#[tokio::test]
async fn code_reissue_route_404s_when_the_code_is_outside_the_scope() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    let (status, content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/codes/7011/reissues",
        json!({ "reason": "lost" }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unexpected: {payload}");
    assert!(
        content_type.starts_with("application/problem+json"),
        "{content_type}"
    );
    assert!(payload["code"].is_number(), "{payload}");
    // The service still reached the repository with the IAM scope; the absence
    // of a row is what produced the 404.
    assert_eq!(repository.scopes.lock().expect("scopes").len(), 1);
}

/// A blank reason or a malformed explicit code is a 400 before the repository.
#[tokio::test]
async fn code_reissue_route_rejects_invalid_input_before_the_repository() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    for body in [
        json!({ "reason": "   " }),
        json!({ "reason": "lost", "code": "ABC" }),
        json!({ "reason": "lost", "code": "AB CD" }),
        json!({ "reason": "lost", "code": "ABC*" }),
        json!({ "reason": "lost", "code": "A".repeat(65) }),
    ] {
        let (status, content_type, payload) = post_json(
            stop_loss_app(repository.clone()),
            "/backend/v3/api/promotions/codes/7011/reissues",
            body.clone(),
            &tokens,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {payload}");
        assert!(
            content_type.starts_with("application/problem+json"),
            "{body}: {content_type}"
        );
        assert!(payload["code"].is_number(), "{body}: {payload}");
    }
    assert!(repository.reissues.lock().expect("reissues").is_empty());
}

/// Stop-loss wallet-coupon routes: `{ userCouponId, status }` and
/// `{ userCouponId, expiresAt }` under `data.item`.
#[tokio::test]
async fn stop_loss_user_coupon_routes_return_the_contract_shapes() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    let (status, _content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/user_coupons/9901/voids",
        json!({ "reason": "fraudulent claim" }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert_eq!(payload["data"]["item"]["userCouponId"], "9901");
    assert_eq!(payload["data"]["item"]["status"], "voided");

    let (status, _content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/user_coupons/9901/extensions",
        json!({ "expiresAt": "2026-12-01 00:00:00" }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert_eq!(payload["data"]["item"]["userCouponId"], "9901");
    // The repository's canonical shape is relayed untouched.
    assert_eq!(
        payload["data"]["item"]["expiresAt"],
        "2026-12-01T00:00:00.000Z"
    );

    assert_eq!(
        repository.voids.lock().expect("voids")[0],
        ("9901".to_owned(), "fraudulent claim".to_owned())
    );
    assert_eq!(
        repository.extensions.lock().expect("extensions")[0],
        ("9901".to_owned(), "2026-12-01 00:00:00".to_owned())
    );

}

/// An empty (or blank) `reason` is a 400 before the repository is reached.
#[tokio::test]
async fn stop_loss_routes_reject_an_empty_reason() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    for uri in [
        "/backend/v3/api/promotions/codes/7011/disables",
        "/backend/v3/api/promotions/code_batches/8801/disables",
        "/backend/v3/api/promotions/user_coupons/9901/voids",
    ] {
        let (status, content_type, payload) = post_json(
            stop_loss_app(repository.clone()),
            uri,
            json!({ "reason": "   " }),
            &tokens,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {payload}");
        assert!(
            content_type.starts_with("application/problem+json"),
            "{uri}: {content_type}"
        );
        assert!(payload["code"].is_number(), "{uri}: {payload}");
        assert!(
            payload["detail"].as_str().unwrap_or_default().contains("reason"),
            "{uri}: {payload}"
        );
    }
    assert!(repository.disables.lock().expect("disables").is_empty());
    assert!(repository
        .batch_disables
        .lock()
        .expect("batch disables")
        .is_empty());
    assert!(repository.voids.lock().expect("voids").is_empty());

}

/// A malformed `expiresAt` is a 400 before the repository is reached.
#[tokio::test]
async fn stop_loss_extension_rejects_a_bad_expires_at() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    for expires_at in ["", "   ", "not-a-timestamp", "2026-11-10"] {
        let (status, content_type, payload) = post_json(
            stop_loss_app(repository.clone()),
            "/backend/v3/api/promotions/user_coupons/9901/extensions",
            json!({ "expiresAt": expires_at }),
            &tokens,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{expires_at:?}: {payload}");
        assert!(
            content_type.starts_with("application/problem+json"),
            "{content_type}"
        );
        assert!(payload["code"].is_number(), "{payload}");
        assert!(
            payload["detail"]
                .as_str()
                .unwrap_or_default()
                .contains("expiresAt"),
            "{payload}"
        );
    }
    assert!(repository
        .extensions
        .lock()
        .expect("extensions")
        .is_empty());

}

/// Settlement lifecycle routes: `data.item` carries
/// `{ applicationId, status, settledAmountMinor }` / `...releasedAmountMinor`,
/// with the int64 amount as a decimal string.
#[tokio::test]
async fn discount_application_settlement_routes_return_the_contract_shapes() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    // The settlement operation takes no body at all.
    let (status, content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/discount_applications/6101/settlements",
        json!({}),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert!(content_type.starts_with("application/json"));
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["applicationId"], "6101");
    assert_eq!(payload["data"]["item"]["status"], "settled");
    // int64 wire fields stay decimal strings.
    assert_eq!(payload["data"]["item"]["settledAmountMinor"], "1250");
    assert!(payload["traceId"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));

    let (status, _content_type, payload) = post_json(
        stop_loss_app(repository.clone()),
        "/backend/v3/api/promotions/discount_applications/6101/releases",
        json!({ "reason": "  order cancelled by buyer  " }),
        &tokens,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "unexpected: {payload}");
    assert_eq!(payload["data"]["item"]["applicationId"], "6101");
    assert_eq!(payload["data"]["item"]["status"], "released");
    assert_eq!(payload["data"]["item"]["releasedAmountMinor"], "1250");

    // The service trimmed the release reason before the repository saw it.
    assert_eq!(
        repository.releases.lock().expect("releases")[0],
        ("6101".to_owned(), "order cancelled by buyer".to_owned())
    );
    // The actor/tenancy came from IAM, never from the body.
    let expected_scope = PromotionAdminScope::new(100001, 300001, "900001").expect("scope");
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[expected_scope.clone(), expected_scope]
    );
}

/// An empty (or blank) release `reason` is a 400 before the repository is reached.
#[tokio::test]
async fn discount_application_release_rejects_an_empty_reason() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    for reason in ["", "   ", "\n\t"] {
        let (status, content_type, payload) = post_json(
            stop_loss_app(repository.clone()),
            "/backend/v3/api/promotions/discount_applications/6101/releases",
            json!({ "reason": reason }),
            &tokens,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{reason:?}: {payload}");
        assert!(
            content_type.starts_with("application/problem+json"),
            "{content_type}"
        );
        assert!(payload["code"].is_number(), "{payload}");
        assert!(
            payload["detail"]
                .as_str()
                .unwrap_or_default()
                .contains("reason"),
            "{payload}"
        );
    }
    assert!(repository.releases.lock().expect("releases").is_empty());
}

/// A non-`applied` application is a 409 (state-machine conflict), not a 500.
#[tokio::test]
async fn discount_application_lifecycle_conflicts_propagate_as_409() {
    std::env::set_var("SDKWORK_ENVIRONMENT", "dev");
    std::env::set_var("SDKWORK_IAM_ENVIRONMENT", "development");

    let repository = Arc::new(RecordingPromotionAdminRepository::default());
    let tokens = manage_tokens();
    for (uri, body, expected_message) in [
        (
            "/backend/v3/api/promotions/discount_applications/already-settled/settlements",
            json!({}),
            "already been settled",
        ),
        (
            "/backend/v3/api/promotions/discount_applications/already-released/releases",
            json!({ "reason": "duplicate release" }),
            "already been released",
        ),
    ] {
        let (status, content_type, payload) =
            post_json(stop_loss_app(repository.clone()), uri, body, &tokens).await;
        assert_eq!(status, StatusCode::CONFLICT, "{uri}: {payload}");
        assert!(
            content_type.starts_with("application/problem+json"),
            "{uri}: {content_type}"
        );
        assert!(payload["code"].is_number(), "{uri}: {payload}");
        assert!(
            payload["detail"]
                .as_str()
                .unwrap_or_default()
                .contains(expected_message),
            "{uri}: {payload}"
        );
    }
}
