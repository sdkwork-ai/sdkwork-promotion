use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest};

// Auth classification follows the promotion app-api OpenAPI contract
// (apis/app-api/promotion/sdkwork-promotion-app-api.openapi.json):
//   - public: catalogue/display surfaces visible to anonymous users
//     (offer browsing, exchange rates, exchange rules)
//   - dual_token: user-owned data and state-changing operations
//     (my coupons, wallet points, redemptions, discount applications,
//     member cards)
const HTTP_ROUTES: &[HttpRoute] = &[
    // --- Anonymous display surfaces -------------------------------------
    public(
        HttpMethod::Get,
        "/app/v3/api/promotions/offers",
        "promotions.offers.list",
    ),
    public(
        HttpMethod::Get,
        "/app/v3/api/promotions/offers/{offerId}",
        "promotions.offers.retrieve",
    ),
    public(
        HttpMethod::Get,
        "/app/v3/api/wallet/exchange_rate",
        "wallet.exchangeRate",
    ),
    public(
        HttpMethod::Get,
        "/app/v3/api/wallet/points/exchanges/rules",
        "points.exchangeRules",
    ),
    // --- My coupons (user-owned data) ------------------------------------
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/user_coupons",
        "promotions.userCoupons.list",
    ),
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/user_coupons/{userCouponId}",
        "promotions.userCoupons.retrieve",
    ),
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/user_coupons/wallet",
        "promotions.userCoupons.wallet.list",
    ),
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/user_coupons/wallet/{userCouponId}",
        "promotions.userCoupons.wallet.retrieve",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/user_coupon_claims",
        "promotions.userCoupons.claims.create",
    ),
    // --- Wallet points (user-owned data) ---------------------------------
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/wallet/points",
        "wallet.points.balance",
    ),
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/wallet/points/history",
        "wallet.points.history",
    ),
    // --- Redemptions (user operations) -----------------------------------
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/codes/redemptions/preview",
        "promotions.codes.redemptions.preview",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/codes/redemptions",
        "promotions.codes.redemptions.create",
    ),
    // --- Discount applications (transaction operations) -------------------
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/discount_applications",
        "promotions.discountApplications.create",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/discount_applications/reversals",
        "promotions.discountApplications.reversals.create",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/discount_applications/{applicationId}/releases",
        "promotions.discountApplications.releases.create",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/discount_applications/{applicationId}/rollback",
        "promotions.discountApplications.rollback",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/discount_applications/{applicationId}/settlements",
        "promotions.discountApplications.settlements.create",
    ),
    // --- Member cards (user-owned data) -----------------------------------
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/member_cards",
        "promotions.memberCards.list",
    ),
    dual_token(
        HttpMethod::Get,
        "/app/v3/api/promotions/member_cards/{cardId}",
        "promotions.memberCards.retrieve",
    ),
    dual_token(
        HttpMethod::Post,
        "/app/v3/api/promotions/member_cards/{cardId}/consumptions",
        "promotions.memberCards.consumptions.create",
    ),
];

const fn public(method: HttpMethod, path: &'static str, operation_id: &'static str) -> HttpRoute {
    HttpRoute::public(method, path, "promotions", operation_id)
}

const fn dual_token(
    method: HttpMethod,
    path: &'static str,
    operation_id: &'static str,
) -> HttpRoute {
    HttpRoute::dual_token(method, path, "promotions", operation_id)
}

pub fn promotion_app_api_public_path_prefixes() -> Vec<String> {
    sdkwork_web_bootstrap::infra_public_path_prefixes()
}

pub fn app_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(HTTP_ROUTES)
}

#[cfg(test)]
mod tests {
    use sdkwork_web_core::RouteAuth;

    use super::app_route_manifest;

    #[test]
    fn public_catalogue_routes_skip_auth_token() {
        let manifest = app_route_manifest();
        for (method, path) in [
            ("GET", "/app/v3/api/promotions/offers"),
            ("GET", "/app/v3/api/promotions/offers/demo-offer"),
            ("GET", "/app/v3/api/wallet/exchange_rate"),
            ("GET", "/app/v3/api/wallet/points/exchanges/rules"),
        ] {
            let route = manifest
                .match_route(method, path)
                .unwrap_or_else(|| panic!("{method} {path} must be registered"));
            assert_eq!(
                RouteAuth::Public,
                route.auth,
                "{method} {path} must be Public so anonymous callers skip auth tokens"
            );
        }
    }

    #[test]
    fn user_owned_promotion_routes_remain_dual_token() {
        let manifest = app_route_manifest();
        let coupons = manifest
            .match_route("GET", "/app/v3/api/promotions/user_coupons")
            .expect("user coupons remain protected");
        assert_eq!(RouteAuth::DualToken, coupons.auth);
    }
}
