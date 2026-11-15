use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest};

// Auth classification follows the promotion app-api OpenAPI contract
// (apis/app-api/promotion/sdkwork-promotion-app-api.openapi.json):
//   - dual_token_or_anonymous: storefront catalogue reads that admit anonymous
//     visitors while still resolving a signed-in principal (offer browsing)
//   - public: display-only configuration with no principal-dependent data
//     (exchange rates, exchange rules)
//   - dual_token: user-owned data and state-changing operations
//     (my coupons, wallet points, redemptions, discount applications,
//     member cards)
const HTTP_ROUTES: &[HttpRoute] = &[
    // --- Offer catalogue (optional credential: guest storefront browsing) --
    dual_token_or_anonymous(
        HttpMethod::Get,
        "/app/v3/api/promotions/offers",
        "promotions.offers.list",
    ),
    dual_token_or_anonymous(
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

const fn dual_token_or_anonymous(
    method: HttpMethod,
    path: &'static str,
    operation_id: &'static str,
) -> HttpRoute {
    HttpRoute::dual_token_or_anonymous(method, path, "promotions", operation_id)
}

pub fn app_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(HTTP_ROUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_web_core::RouteAuth;

    fn assert_optional_credential(method: &str, path: &str) {
        let manifest = app_route_manifest();
        let route = manifest
            .match_route(method, path)
            .unwrap_or_else(|| panic!("{method} {path} must be registered"));
        assert_eq!(
            RouteAuth::DualTokenOrAnonymous,
            route.auth,
            "{method} {path} must admit anonymous storefront visitors while still resolving a signed-in principal"
        );
    }

    fn assert_dual_token(method: &str, path: &str) {
        let manifest = app_route_manifest();
        let route = manifest
            .match_route(method, path)
            .unwrap_or_else(|| panic!("{method} {path} must be registered"));
        assert_eq!(
            RouteAuth::DualToken,
            route.auth,
            "{method} {path} is user-scoped and must stay behind dual-token authentication"
        );
    }

    #[test]
    fn offer_catalogue_routes_allow_anonymous_storefront_browsing() {
        assert_optional_credential("GET", "/app/v3/api/promotions/offers");
        assert_optional_credential("GET", "/app/v3/api/promotions/offers/{offerId}");
        assert_optional_credential("GET", "/app/v3/api/promotions/offers/offer-1");
    }

    #[test]
    fn user_scoped_promotion_routes_stay_dual_token() {
        assert_dual_token("GET", "/app/v3/api/promotions/user_coupons");
        assert_dual_token("POST", "/app/v3/api/promotions/user_coupon_claims");
        assert_dual_token("POST", "/app/v3/api/promotions/codes/redemptions");
        assert_dual_token("GET", "/app/v3/api/promotions/member_cards");
        assert_dual_token("GET", "/app/v3/api/wallet/points");
    }
}
