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

    /// 启动生命周期 worker：按固定间隔扫描会员卡与券（钱包券/券码）的生命周期。
    /// 使用 PostgreSQL advisory lock 防止多实例并发重复执行；首轮立即执行。
    /// Each tick runs both sweeps independently: a member-card failure is logged
    /// and does not skip the coupon sweep (and vice versa), because the two use
    /// distinct advisory-lock keys and disjoint tables.
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
                match store.run_member_card_lifecycle_sweep().await {
                    Ok(outcome) => tracing::info!(
                        activated = outcome.activated,
                        expired = outcome.expired,
                        skipped = outcome.skipped,
                        "member card lifecycle sweep completed"
                    ),
                    Err(error) => {
                        tracing::warn!(error = error.message(), "member card lifecycle sweep failed")
                    }
                }
                // Independent of the member-card outcome above.
                match store.run_promotion_lifecycle_sweep().await {
                    Ok(outcome) => tracing::info!(
                        expired_user_coupons = outcome.expired_user_coupons,
                        expired_codes = outcome.expired_codes,
                        skipped = outcome.skipped,
                        "promotion coupon lifecycle sweep completed"
                    ),
                    Err(error) => tracing::warn!(
                        error = error.message(),
                        "promotion coupon lifecycle sweep failed"
                    ),
                }
                // Independent of both sweeps above: the settlement lifecycle's
                // stale-reservation sweep has its own advisory-lock key (`…430`),
                // so a failure or lock contention here cannot skip either of the
                // other sweeps, and vice versa.
                match store.release_stale_discount_applications().await {
                    Ok(outcome) => tracing::info!(
                        released = outcome.released,
                        skipped = outcome.skipped,
                        "stale discount reservation sweep completed"
                    ),
                    Err(error) => tracing::warn!(
                        error = error.message(),
                        "stale discount reservation sweep failed"
                    ),
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

#[cfg(test)]
mod tests {
    #[test]
    fn lifecycle_worker_runs_every_sweep_on_one_timer_independently() {
        let source = include_str!("lib.rs");
        let start = source
            .find("pub fn spawn_member_card_lifecycle_worker")
            .expect("lifecycle worker");
        let rest = &source[start..];
        let body = &rest[..rest
            .find("pub fn database_module")
            .unwrap_or(rest.len())];

        // One timer: the coupon and stale-reservation sweeps extend the existing
        // loop instead of spawning extra scheduled workers.
        assert_eq!(body.matches("tokio::spawn(").count(), 1);
        assert_eq!(body.matches("tokio::time::sleep(").count(), 1);
        assert!(body.contains("store.run_member_card_lifecycle_sweep().await"));
        assert!(body.contains("store.run_promotion_lifecycle_sweep().await"));
        assert!(body.contains("store.release_stale_discount_applications().await"));

        // Each outcome is logged, and each failure is handled on its own: every
        // failure arm closes before the next sweep is called, so one sweep's
        // error can never skip another.
        assert!(body.contains("member card lifecycle sweep failed"));
        assert!(body.contains("promotion coupon lifecycle sweep failed"));
        assert!(body.contains("stale discount reservation sweep failed"));
        assert!(body.contains("expired_user_coupons"));
        assert!(body.contains("expired_codes"));
        assert!(body.contains("released = outcome.released"));
        let member_failure = body
            .find("member card lifecycle sweep failed")
            .expect("member card failure log");
        let coupon_call = body
            .find("run_promotion_lifecycle_sweep")
            .expect("coupon sweep call");
        assert!(
            member_failure < coupon_call,
            "the coupon sweep must run after the member-card outcome is handled"
        );
        let coupon_failure = body
            .find("promotion coupon lifecycle sweep failed")
            .expect("coupon failure log");
        let reservation_call = body
            .find("release_stale_discount_applications")
            .expect("reservation sweep call");
        assert!(
            coupon_failure < reservation_call,
            "the stale-reservation sweep must run after the coupon outcome is handled"
        );
    }
}
