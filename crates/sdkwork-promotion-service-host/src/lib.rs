use sdkwork_commerce_promotion_repository_sqlx::{
    PostgresCommercePromotionStore, PostgresPromotionAdminRepository,
};
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_commerce_promotion_service::{PromotionAdminRepositoryPort, PromotionAdminService};
use sdkwork_promotion_database_host::{
    bootstrap_promotion_database_from_env, bootstrap_promotion_database_host_with_pool,
    PromotionDatabaseHost,
};
use std::sync::Arc;

pub struct PromotionServiceHost {
    database: PromotionDatabaseHost,
    promotion_admin: Arc<PromotionAdminService>,
}

impl PromotionServiceHost {
    pub async fn new() -> Self {
        Self::from_env()
            .await
            .expect("promotion service host bootstrap failed")
    }

    pub async fn from_env() -> Result<Self, String> {
        let database = bootstrap_promotion_database_from_env().await?;
        Self::from_database(database)
    }

    /// Build the promotion service host on a shared pool owned by the
    /// consuming host (same-origin dependency composition). Mirrors the
    /// membership `MembershipServiceHost::from_pool` pattern; the consuming
    /// host already owns the database lifecycle for this pool.
    pub async fn from_pool(pool: &DatabasePool) -> Result<Self, String> {
        let database = bootstrap_promotion_database_host_with_pool(pool).await?;
        Self::from_database(database)
    }

    fn from_database(database: PromotionDatabaseHost) -> Result<Self, String> {
        // 服务端权威持久化仅支持 PostgreSQL（DATABASE_SPEC：authoritative-server）
        let DatabasePool::Postgres(pool, _) = database.pool() else {
            return Err("promotion server requires a PostgreSQL database pool".to_owned());
        };
        let repository: Arc<dyn PromotionAdminRepositoryPort> =
            Arc::new(PostgresPromotionAdminRepository::new(pool.clone()));
        Ok(Self {
            database,
            promotion_admin: Arc::new(PromotionAdminService::new(repository)),
        })
    }

    pub fn database_pool(&self) -> &DatabasePool {
        self.database.pool()
    }

    /// 启动会员卡生命周期 worker：按固定间隔扫描排期生效与到期过期的会员卡。
    /// 使用 PostgreSQL advisory lock 防止多实例并发重复执行；首轮立即执行。
    pub fn spawn_member_card_lifecycle_worker(&self) {
        let DatabasePool::Postgres(pool, _) = self.database_pool() else {
            tracing::warn!("member card lifecycle worker requires a PostgreSQL pool; skipped");
            return;
        };
        let pool = pool.clone();
        let interval_seconds = std::env::var("PROMOTION_LIFECYCLE_SWEEP_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_LIFECYCLE_SWEEP_INTERVAL_SECONDS);
        tracing::info!(
            interval_seconds,
            "spawning member card lifecycle worker"
        );
        tokio::spawn(async move {
            let store = PostgresCommercePromotionStore::new(pool.clone());
            loop {
                if let Err(error) = store.run_member_card_lifecycle_sweep().await {
                    tracing::warn!(error = error.message(), "member card lifecycle sweep failed");
                }
                tokio::time::sleep(std::time::Duration::from_secs(interval_seconds)).await;
            }
        });
    }

    pub fn database_module(&self) -> std::sync::Arc<sdkwork_database_spi::DefaultDatabaseModule> {
        self.database.module()
    }

    pub fn promotion_admin_service(&self) -> Arc<PromotionAdminService> {
        self.promotion_admin.clone()
    }
}
/// 会员卡生命周期扫描默认间隔（秒）：300（5 分钟）。
const DEFAULT_LIFECYCLE_SWEEP_INTERVAL_SECONDS: u64 = 300;
