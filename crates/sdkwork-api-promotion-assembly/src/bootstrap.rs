//! Gateway bootstrap for sdkwork-promotion.
//! Multi-surface merges mount shared infrastructure routes once at the assembly layer
//! so `/healthz`, `/livez`, `/readyz`, and `/metrics` are not duplicated per surface.
//!
//! The assembly exports the indivisible `ApiAssemblyContribution` contract
//! (API_ASSEMBLY_SPEC.md section 4); the platform cloud gateway composes the
//! contribution with its process-shared PostgreSQL pool.

use sdkwork_database_sqlx::DatabasePool;
use sdkwork_web_bootstrap::{
    ApiAssemblyContribution, DatabasePoolReadinessCheck, ReadinessCheck,
};
use sdkwork_web_core::{DomainContextInjector, HttpRouteManifest};
use std::sync::Arc;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract).
pub type ApiAssembly = ApiAssemblyContribution;

fn combined_route_manifest() -> HttpRouteManifest {
    let app_manifest = sdkwork_routes_promotion_app_api::promotion_app_api_route_manifest();
    let backend_manifest = sdkwork_routes_promotion_backend_api::gateway_route_manifest();
    let mut routes = app_manifest.routes().to_vec();
    routes.extend(backend_manifest.routes());
    HttpRouteManifest::new(&routes)
}

pub async fn assemble_api_router(
    host: Arc<sdkwork_promotion_service_host::PromotionServiceHost>,
) -> ApiAssembly {
    let mut router = axum::Router::new();
    router = router.merge(sdkwork_routes_promotion_app_api::gateway_mount(host.clone()).await);
    router = router.merge(sdkwork_routes_promotion_backend_api::gateway_mount(host.clone()).await);
    ApiAssemblyContribution::from_manifest(
        "sdkwork-promotion",
        "SDKWork Promotion API",
        router,
        combined_route_manifest(),
        Vec::<Arc<dyn DomainContextInjector>>::new(),
        Arc::new(sdkwork_web_bootstrap::AlwaysReady) as Arc<dyn ReadinessCheck>,
    )
    .expect("promotion route manifest is valid")
}

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

pub async fn assemble_backend_business_router_from_env() -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_env().await?,
    );
    Ok(assemble_backend_business_router(host).await)
}

/// Assemble the Promotion contribution against a caller-provided database pool so
/// the platform cloud gateway can share its process-wide PostgreSQL pool.
pub async fn assemble_api_router_with_pool(pool: DatabasePool) -> Result<ApiAssembly, String> {
    let host = std::sync::Arc::new(
        sdkwork_promotion_service_host::PromotionServiceHost::from_env().await?,
    );
    let mut router = axum::Router::new();
    router = router.merge(sdkwork_routes_promotion_app_api::gateway_mount(host.clone()).await);
    router = router.merge(sdkwork_routes_promotion_backend_api::gateway_mount(host).await);
    ApiAssemblyContribution::from_manifest(
        "sdkwork-promotion",
        "SDKWork Promotion API",
        router,
        combined_route_manifest(),
        Vec::<Arc<dyn DomainContextInjector>>::new(),
        Arc::new(DatabasePoolReadinessCheck::new(pool)),
    )
}
