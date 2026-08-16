//! sdkwork-promotion 独立部署的 building-block API 服务器。
//!
//! 该二进制仅在 building-block 拓扑下独立运行；生产网关路由由 commerce/app 拓扑拥有。
//! CORS 策略默认拒绝跨域，必须通过 `PROMOTION_CORS_ORIGINS` 环境变量显式配置允许的来源。

use sdkwork_api_promotion_assembly::assemble_api_router_from_env;
use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_env,
};
use sdkwork_web_bootstrap::{infra_public_path_prefixes, ComposedApiAssembly};
use tower_http::trace::TraceLayer;

/// 环境变量名：允许的跨域来源列表，逗号分隔。
///
/// 示例：`https://console.sdkwork.com,https://admin.sdkwork.com`。
/// 留空时不附加 CORS 层，等同拒绝所有跨域请求。
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let assembly = assemble_api_router_from_env()
        .await
        .expect("promotion API assembly failed");
    let framework = build_web_framework_builder(
        iam_web_request_context_resolver_from_env().await,
        assembly.route_manifest.clone(),
        infra_public_path_prefixes(),
    );
    let app = ComposedApiAssembly::try_compose("SDKWork Promotion API", vec![assembly])
        .expect("promotion API composition failed")
        .into_hosted(framework)
        .router
        .layer(TraceLayer::new_for_http());
    let addr = std::env::var("PROMOTION_API_BIND").unwrap_or_else(|_| "0.0.0.0:18097".to_owned());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|error| panic!("promotion-api bind {addr} failed: {error}"));
    tracing::info!(%addr, "promotion api server listening");
    axum::serve(listener, app)
        .await
        .unwrap_or_else(|error| panic!("promotion api serve failed: {error}"));
}
