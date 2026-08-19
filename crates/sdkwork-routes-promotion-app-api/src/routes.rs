use axum::Router;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_promotion_service_host::PromotionServiceHost;
use std::sync::Arc;

use crate::app_promotion_router_with_postgres_pool;
use crate::web_bootstrap::wrap_router_with_web_framework_from_env;

pub fn build_promotion_app_router(host: Arc<PromotionServiceHost>) -> Router {
    // 服务端权威持久化仅支持 PostgreSQL（DATABASE_SPEC：authoritative-server）
    let DatabasePool::Postgres(pool, _) = host.database_pool() else {
        panic!("promotion server requires a PostgreSQL database pool");
    };
    app_promotion_router_with_postgres_pool(pool.clone())
}

pub async fn build_promotion_app_router_with_framework(host: Arc<PromotionServiceHost>) -> Router {
    wrap_router_with_web_framework_from_env(build_promotion_app_router(host)).await
}
