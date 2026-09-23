use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_commerce_promotion_service::{
    PromotionAdminFuture, PromotionAdminListQuery, PromotionAdminPage,
    PromotionAdminRepositoryPort, PromotionAdminScope, PromotionAdminUserCouponItem,
    PromotionCampaignInput, PromotionCampaignItem, PromotionCodeBatchInput, PromotionCodeBatchItem,
    PromotionCodeItem, PromotionCouponLedgerItem, PromotionCouponStockInput,
    PromotionCouponStockItem, PromotionDiscountApplicationItem, PromotionDistributionInput,
    PromotionDistributionTaskItem, PromotionOfferInput, PromotionOfferItem, PromotionOverview,
};
use sdkwork_iam_web_adapter::IamDatabaseWebRequestContextResolver;
use sdkwork_routes_promotion_backend_api::{
    build_backend_promotion_router, wrap_router_with_web_framework,
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[derive(Default)]
struct RecordingPromotionAdminRepository {
    scopes: Mutex<Vec<PromotionAdminScope>>,
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
    // token-claims-gate: legacy-fixture — constructs a pre-slimming credential so this
    // test can assert the claim is no longer an authorization source.
    auth["permission_scope"] = json!("commerce.marketing.read");
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
        IamDatabaseWebRequestContextResolver::new(None),
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

    std::env::remove_var("SDKWORK_IAM_ENVIRONMENT");
    std::env::remove_var("SDKWORK_ENVIRONMENT");

    let status = response.status();
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body"),
    )
    .expect("response json");
    assert_eq!(status, StatusCode::OK, "unexpected response: {payload}");
    assert_eq!(payload["code"], 0);
    assert_eq!(payload["data"]["item"]["totalOffers"], 3);
    assert_eq!(
        repository.scopes.lock().expect("scopes").as_slice(),
        &[PromotionAdminScope::new(100001, 300001, "900001").expect("scope")]
    );
}
