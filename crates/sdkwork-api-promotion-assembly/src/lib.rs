//! Gateway assembly for sdkwork-promotion.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{assemble_api_router, ApiAssembly, assemble_api_router_with_pool, web_module_with_pool};

use sdkwork_database_sqlx::DatabasePool;
use sdkwork_web_bootstrap::{ApiAssemblyContribution, ReadinessCheck, WebModule};
use sdkwork_web_core::DomainContextInjector;
use std::sync::Arc;

/// Backend business-only contribution (levels, campaigns, offers, codes).
///
/// Returns the dependency-owned backend contribution without a Web Framework
/// layer — the consuming host installs framework/security once on the
/// combined router (API_ASSEMBLY_SPEC §4/§6.1).
pub async fn assemble_backend_business_router(
    host: std::sync::Arc<sdkwork_promotion_service_host::PromotionServiceHost>,
) -> ApiAssembly {
    let router = sdkwork_routes_promotion_backend_api::build_promotion_backend_router(host);
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

/// Same-origin dependency composition: build the promotion backend business
/// contribution on a shared pool owned by the consuming host.
pub async fn assemble_backend_business_router_with_pool(
    pool: &DatabasePool,
) -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_pool(pool).await?,
    );
    Ok(assemble_backend_business_router(host).await)
}

/// App business-only contribution (member cards, coupons, offers browsing).
///
/// Returns the dependency-owned app contribution without a Web Framework
/// layer — the consuming host installs framework/security once on the
/// combined router (API_ASSEMBLY_SPEC §4/§6.1).
pub async fn assemble_app_api_contribution_with_pool(
    pool: &DatabasePool,
) -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_pool(pool).await?,
    );
    let router = sdkwork_routes_promotion_app_api::build_promotion_app_router(host);
    let manifest = sdkwork_routes_promotion_app_api::promotion_app_api_route_manifest();
    ApiAssemblyContribution::from_manifest(
        "sdkwork-promotion",
        "SDKWork Promotion App API",
        router,
        manifest,
        Vec::<Arc<dyn DomainContextInjector>>::new(),
        Arc::new(sdkwork_web_bootstrap::AlwaysReady) as Arc<dyn ReadinessCheck>,
    )
    .map_err(|error| format!("promotion app route manifest is invalid: {error}"))
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
/// App-api surface route manifest owned by the dependency assembly.
pub fn app_api_route_manifest() -> sdkwork_web_core::HttpRouteManifest {
    sdkwork_routes_promotion_app_api::promotion_app_api_route_manifest()
}

/// Canonical Web Module definition for this application
/// (API_ASSEMBLY_SPEC §4.1.1): the complete HTTP surface — every route,
/// manifest, and OpenAPI document of this owner — as one installable module.
pub async fn web_module() -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(assemble_api_router_from_env().await?))
}
