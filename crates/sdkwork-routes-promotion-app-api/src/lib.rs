pub mod api_response;
pub mod command_headers;
pub mod exchange_router;
pub mod http_route_manifest;
pub mod promotion_router;
pub mod routes;
pub mod subject;
pub mod web_bootstrap;

pub use exchange_router::{
    app_exchange_router_with_postgres_pool, build_app_exchange_router, CommerceExchangeFuture,
    CommerceExchangeStore,
};
pub use promotion_router::{
    app_promotion_router_with_postgres_pool, build_app_promotion_router, CommercePromotionFuture,
    CommercePromotionStore,
};
pub use routes::{build_promotion_app_router, build_promotion_app_router_with_framework};
pub use web_bootstrap::wrap_router_with_web_framework_from_env;

pub fn promotion_app_api_route_manifest() -> sdkwork_web_core::HttpRouteManifest {
    http_route_manifest::app_route_manifest()
}

use axum::Router;
use sdkwork_promotion_service_host::PromotionServiceHost;
use std::sync::Arc;

/// Business-only assembly entrypoint: mounts the promotion app router
/// WITHOUT a Web Framework layer. Consuming gateways compose dependency
/// surfaces in-process and install framework/security once on the combined
/// router (API_ASSEMBLY_SPEC §4/§6.1). A nested framework layer would run
/// without the composed route manifest, re-classify every request with
/// `route_auth = None`, and reject anonymous public routes (offers,
/// exchange rate, exchange rules) as missing-access-token (40101) even
/// though the host manifest classifies them as public.
pub async fn gateway_mount_business(host: Arc<PromotionServiceHost>) -> Router {
    build_promotion_app_router(host)
}

pub async fn gateway_mount(host: Arc<PromotionServiceHost>) -> Router {
    gateway_mount_business(host).await
}
