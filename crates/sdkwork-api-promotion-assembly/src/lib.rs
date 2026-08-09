//! Gateway assembly for sdkwork-promotion.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{assemble_api_router, assemble_api_router_with_pool, ApiAssembly};

use sdkwork_web_bootstrap::{ApiAssemblyContribution, ReadinessCheck};
use sdkwork_web_core::DomainContextInjector;
use std::sync::Arc;

pub async fn assemble_backend_business_router(
    host: std::sync::Arc<sdkwork_promotion_service_host::PromotionServiceHost>,
) -> ApiAssembly {
    let router = sdkwork_routes_promotion_backend_api::gateway_mount_business(host).await;
    let manifest = sdkwork_routes_promotion_backend_api::gateway_route_manifest();
    ApiAssemblyContribution::from_manifest(
        "sdkwork-promotion",
        "SDKWork Promotion Backend API",
        router,
        manifest,
        Vec::<Arc<dyn DomainContextInjector>>::new(),
        Arc::new(sdkwork_web_bootstrap::AlwaysReady) as Arc<dyn ReadinessCheck>,
    )
    .expect("promotion backend route manifest is valid")
}

pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_env().await?,
    );
    Ok(assemble_api_router(host).await)
}

pub async fn assemble_backend_business_router_from_env() -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_env().await?,
    );
    Ok(assemble_backend_business_router(host).await)
}

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
