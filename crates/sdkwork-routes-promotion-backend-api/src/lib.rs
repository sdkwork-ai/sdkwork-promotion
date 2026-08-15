mod api_response;
mod backend_acl;
pub mod http_route_manifest;
mod operations;
pub mod routes;
mod subject;
pub mod web_bootstrap;

pub use operations::build_backend_promotion_router;
pub use routes::{build_promotion_backend_router, build_promotion_backend_router_with_framework};
pub use web_bootstrap::{
    promotion_backend_api_public_path_prefixes, wrap_router_with_web_framework,
    wrap_router_with_web_framework_from_env,
};

use axum::Router;
use sdkwork_promotion_service_host::PromotionServiceHost;
use std::sync::Arc;

pub fn gateway_route_manifest() -> sdkwork_web_core::HttpRouteManifest {
    http_route_manifest::backend_route_manifest()
}

/// Business-only assembly entrypoint: mounts the promotion backend router
/// WITHOUT a Web Framework layer. Consuming gateways compose dependency
/// surfaces in-process and install framework/security once on the combined
/// router (API_ASSEMBLY_SPEC §4/§6.1); a nested layer would re-classify the
/// request after the host injected trusted-subject context and reject it as
/// client identity projection (40001).
pub async fn gateway_mount_business(host: Arc<PromotionServiceHost>) -> Router {
    build_promotion_backend_router(host)
}

pub async fn gateway_mount(host: Arc<PromotionServiceHost>) -> Router {
    build_promotion_backend_router_with_framework(host).await
}
