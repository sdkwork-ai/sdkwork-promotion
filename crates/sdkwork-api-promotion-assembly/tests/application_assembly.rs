use sdkwork_api_promotion_assembly::app_api_route_manifest;
use sdkwork_web_core::RouteAuth;

#[test]
fn application_manifest_exports_public_catalogue_surface() {
    let manifest = app_api_route_manifest();
    for (method, path, operation_id) in [
        (
            "GET",
            "/app/v3/api/promotions/offers",
            "promotions.offers.list",
        ),
        (
            "GET",
            "/app/v3/api/wallet/exchange_rate",
            "wallet.exchangeRate",
        ),
        (
            "GET",
            "/app/v3/api/wallet/points/exchanges/rules",
            "points.exchangeRules",
        ),
    ] {
        let route = manifest
            .match_route(method, path)
            .unwrap_or_else(|| panic!("{method} {path} must be exported by the assembly"));
        assert_eq!(RouteAuth::Public, route.auth);
        assert_eq!(operation_id, route.operation_id);
    }

    let coupons = manifest
        .match_route("GET", "/app/v3/api/promotions/user_coupons")
        .expect("user coupons remain protected");
    assert_eq!(RouteAuth::DualToken, coupons.auth);
}

#[test]
fn promotion_assembly_merges_unwrapped_business_routers() {
    let source = include_str!("../src/bootstrap.rs");
    assert!(
        source.contains("gateway_mount_business"),
        "promotion assembly must merge unwrapped business routers so the host owns the single Web Framework layer"
    );
    assert!(
        !source.contains("build_promotion_app_router_with_framework"),
        "promotion assembly must not nest a Web Framework wrap inside the contribution router"
    );
    assert!(
        !source.contains("::gateway_mount("),
        "promotion assembly must not call the wrapped gateway_mount entrypoint"
    );
}
