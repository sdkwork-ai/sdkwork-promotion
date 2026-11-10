use sdkwork_commerce_promotion_service::{
    PromotionAdminFuture, PromotionAdminListQuery, PromotionAdminPage,
    PromotionAdminRepositoryPort, PromotionAdminScope, PromotionAdminUserCouponItem,
    PromotionCampaignInput, PromotionCampaignItem, PromotionCodeBatchInput, PromotionCodeBatchItem,
    PromotionCodeItem, PromotionCouponLedgerItem, PromotionCouponStockInput,
    PromotionCouponStockItem, PromotionDiscountApplicationItem, PromotionDistributionInput,
    PromotionDistributionTaskItem, PromotionOfferInput, PromotionOfferItem, PromotionOverview,
};
use sdkwork_contract_service::CommerceServiceError;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

const POSTGRES_OVERVIEW_SQL: &str = r#"
SELECT
    COUNT(*)::BIGINT AS total_offers,
    COALESCE(SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END), 0)::BIGINT AS active_offers,
    COALESCE((SELECT SUM(total_quantity) FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2), 0)::BIGINT AS total_coupon_stock,
    COALESCE((SELECT SUM(available_quantity) FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2), 0)::BIGINT AS available_coupons,
    COALESCE((SELECT SUM(claimed_quantity) FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2), 0)::BIGINT AS claimed_coupons,
    COALESCE((SELECT SUM(redeemed_quantity) FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2), 0)::BIGINT AS redeemed_coupons,
    COALESCE((SELECT COUNT(*) FROM promotion_code WHERE tenant_id = $1 AND organization_id = $2 AND status = 'active'), 0)::BIGINT AS active_codes,
    COALESCE((SELECT COUNT(*) FROM promotion_discount_application WHERE tenant_id = $1 AND organization_id = $2), 0)::BIGINT AS discount_applications
FROM promotion_offer
WHERE tenant_id = $1 AND organization_id = $2
"#;

const POSTGRES_OFFER_COUNT_SQL: &str = "SELECT COUNT(*) FROM promotion_offer WHERE tenant_id = $1 AND organization_id = $2 AND deleted_at IS NULL AND ($3 = '%%' OR LOWER(display_name) LIKE $3 OR LOWER(COALESCE(offer_code, '')) LIKE $3) AND ($4 IS NULL OR status = $4)";
const POSTGRES_OFFER_LIST_SQL: &str = "SELECT o.id, o.campaign_id, o.offer_no, o.offer_code, o.offer_type, o.audience_scope, o.combinability, o.goods_scope, o.display_name, o.description, o.priority, o.starts_at AS starts_at, o.ends_at AS ends_at, o.status, v.discount_type, v.discount_value AS discount_value, v.minimum_amount AS minimum_amount, v.maximum_discount_amount AS maximum_discount_amount, v.currency_code, v.rule_json AS rule_json, o.version, o.updated_at AS updated_at FROM promotion_offer o LEFT JOIN promotion_offer_version v ON v.id = o.current_offer_version_id AND v.tenant_id = o.tenant_id WHERE o.tenant_id = $1 AND o.organization_id = $2 AND o.deleted_at IS NULL AND ($3 = '%%' OR LOWER(o.display_name) LIKE $3 OR LOWER(COALESCE(o.offer_code, '')) LIKE $3) AND ($4 IS NULL OR o.status = $4) ORDER BY o.priority DESC, o.starts_at DESC LIMIT $5 OFFSET $6";



const POSTGRES_STOCK_COUNT_SQL: &str = "SELECT COUNT(*) FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(stock_no) LIKE $3) AND ($4 IS NULL OR status = $4)";
const POSTGRES_STOCK_LIST_SQL: &str = "SELECT id, offer_id, stock_no, stock_type, code_issue_mode, total_quantity, available_quantity, claimed_quantity, redeemed_quantity, locked_quantity, per_user_limit, claim_starts_at AS claim_starts_at, claim_ends_at AS claim_ends_at, status FROM promotion_coupon_stock WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(stock_no) LIKE $3) AND ($4 IS NULL OR status = $4) ORDER BY created_at DESC LIMIT $5 OFFSET $6";



const POSTGRES_CODE_COUNT_SQL: &str = "SELECT COUNT(*) FROM promotion_code WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(promotion_code) LIKE $3 OR LOWER(code_no) LIKE $3) AND ($4 IS NULL OR status = $4) AND ($5 IS NULL OR code_batch_id = CAST($5 AS TEXT))";
const POSTGRES_CODE_LIST_SQL: &str = "SELECT id, stock_id, offer_id, code_no, promotion_code, code_type, max_claims, claimed_quantity, starts_at AS starts_at, expires_at AS expires_at, status FROM promotion_code WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(promotion_code) LIKE $3 OR LOWER(code_no) LIKE $3) AND ($4 IS NULL OR status = $4) AND ($5 IS NULL OR code_batch_id = CAST($5 AS TEXT)) ORDER BY created_at DESC LIMIT $6 OFFSET $7";



const POSTGRES_APPLICATION_COUNT_SQL: &str = "SELECT COUNT(*) FROM promotion_discount_application WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(application_no) LIKE $3 OR LOWER(COALESCE(order_no, '')) LIKE $3) AND ($4 IS NULL OR status = $4)";
const POSTGRES_APPLICATION_LIST_SQL: &str = "SELECT id, application_no, order_id, order_no, offer_id, discount_type, CAST(discount_amount AS TEXT) AS discount_amount, currency_code, status, applied_at AS applied_at, settled_at AS settled_at, released_at AS released_at, rolled_back_at AS rolled_back_at FROM promotion_discount_application WHERE tenant_id = $1 AND organization_id = $2 AND ($3 = '%%' OR LOWER(application_no) LIKE $3 OR LOWER(COALESCE(order_no, '')) LIKE $3) AND ($4 IS NULL OR status = $4) ORDER BY applied_at DESC LIMIT $5 OFFSET $6";



#[derive(Clone)]
pub struct PostgresPromotionAdminRepository {
    pool: PgPool,
}

impl PostgresPromotionAdminRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Clone)]
pub(crate) enum AdminPool {
    Postgres(PgPool),
}

enum AdminRow {
    Postgres(PgRow),
}

impl AdminRow {
    fn i32(&self, name: &str) -> Result<i32, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode_error(name, error))
    }

    fn i64(&self, name: &str) -> Result<i64, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode_error(name, error))
    }

    fn string(&self, name: &str) -> Result<String, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode_error(name, error))
    }

    fn optional_string(&self, name: &str) -> Result<Option<String>, CommerceServiceError> {
        match self {
            Self::Postgres(row) => row.try_get(name),

        }
        .map_err(|error| decode_error(name, error))
    }

}

macro_rules! impl_admin_repository {
    ($repository:ty, $variant:ident) => {
        impl PromotionAdminRepositoryPort for $repository {
            fn retrieve_overview<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
            ) -> PromotionAdminFuture<'a, PromotionOverview> {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    retrieve_overview(&pool, scope).await
                })
            }

            fn list_campaigns<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCampaignItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::list_campaigns(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        query,
                    )
                    .await
                })
            }

            fn retrieve_campaign<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                campaign_id: String,
            ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::retrieve_campaign(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        campaign_id,
                    )
                    .await
                })
            }

            fn create_campaign<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                input: &'a PromotionCampaignInput,
            ) -> PromotionAdminFuture<'a, PromotionCampaignItem> {
                Box::pin(async move {
                    crate::promotion_admin_management::create_campaign(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        input,
                    )
                    .await
                })
            }

            fn update_campaign<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                campaign_id: String,
                input: &'a PromotionCampaignInput,
            ) -> PromotionAdminFuture<'a, Option<PromotionCampaignItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::update_campaign(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        campaign_id,
                        input,
                    )
                    .await
                })
            }

            fn delete_campaign<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                campaign_id: String,
            ) -> PromotionAdminFuture<'a, bool> {
                Box::pin(async move {
                    crate::promotion_admin_management::delete_campaign(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        campaign_id,
                    )
                    .await
                })
            }

            fn list_offers<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionOfferItem>> {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    list_offers(&pool, scope, query).await
                })
            }

            fn retrieve_offer<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                offer_id: String,
            ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::retrieve_offer(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        offer_id,
                    )
                    .await
                })
            }

            fn create_offer<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                input: &'a PromotionOfferInput,
            ) -> PromotionAdminFuture<'a, PromotionOfferItem> {
                Box::pin(async move {
                    crate::promotion_admin_management::create_offer(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        input,
                    )
                    .await
                })
            }

            fn update_offer<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                offer_id: String,
                input: &'a PromotionOfferInput,
            ) -> PromotionAdminFuture<'a, Option<PromotionOfferItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::update_offer(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        offer_id,
                        input,
                    )
                    .await
                })
            }

            fn update_offer_status<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                offer_id: String,
                status: &'a str,
            ) -> PromotionAdminFuture<'a, bool> {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    update_offer_status(&pool, scope, offer_id, status).await
                })
            }

            fn delete_offer<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                offer_id: String,
            ) -> PromotionAdminFuture<'a, bool> {
                Box::pin(async move {
                    crate::promotion_admin_management::delete_offer(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        offer_id,
                    )
                    .await
                })
            }

            fn list_coupon_stocks<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponStockItem>> {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    list_coupon_stocks(&pool, scope, query).await
                })
            }

            fn create_coupon_stock<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                input: &'a PromotionCouponStockInput,
            ) -> PromotionAdminFuture<'a, PromotionCouponStockItem> {
                Box::pin(async move {
                    crate::promotion_admin_management::create_coupon_stock(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        input,
                    )
                    .await
                })
            }

            fn list_code_batches<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeBatchItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::list_code_batches(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        query,
                    )
                    .await
                })
            }

            fn create_code_batch<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                input: &'a PromotionCodeBatchInput,
            ) -> PromotionAdminFuture<'a, PromotionCodeBatchItem> {
                Box::pin(async move {
                    crate::promotion_admin_management::create_code_batch(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        input,
                    )
                    .await
                })
            }

            fn list_codes<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCodeItem>> {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    list_codes(&pool, scope, query).await
                })
            }

            fn list_distribution_tasks<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDistributionTaskItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::list_distribution_tasks(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        query,
                    )
                    .await
                })
            }

            fn create_distribution_task<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                input: &'a PromotionDistributionInput,
            ) -> PromotionAdminFuture<'a, PromotionDistributionTaskItem> {
                Box::pin(async move {
                    crate::promotion_admin_management::create_distribution_task(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        input,
                    )
                    .await
                })
            }

            fn list_user_coupons<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionAdminUserCouponItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::list_user_coupons(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        query,
                    )
                    .await
                })
            }

            fn list_coupon_ledger<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionCouponLedgerItem>> {
                Box::pin(async move {
                    crate::promotion_admin_management::list_coupon_ledger(
                        &AdminPool::$variant(self.pool.clone()),
                        scope,
                        query,
                    )
                    .await
                })
            }

            fn list_discount_applications<'a>(
                &'a self,
                scope: &'a PromotionAdminScope,
                query: &'a PromotionAdminListQuery,
            ) -> PromotionAdminFuture<'a, PromotionAdminPage<PromotionDiscountApplicationItem>>
            {
                Box::pin(async move {
                    let pool = AdminPool::$variant(self.pool.clone());
                    list_discount_applications(&pool, scope, query).await
                })
            }
        }
    };
}

impl_admin_repository!(PostgresPromotionAdminRepository, Postgres);

async fn retrieve_overview(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
) -> Result<PromotionOverview, CommerceServiceError> {
    let row = match pool {
        AdminPool::Postgres(pool) => AdminRow::Postgres(
            sqlx::query(POSTGRES_OVERVIEW_SQL)
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .fetch_one(pool)
                .await
                .map_err(|error| storage_error("retrieve promotion overview", error))?,
        ),
    };
    Ok(PromotionOverview {
        total_offers: row.i64("total_offers")?,
        active_offers: row.i64("active_offers")?,
        total_coupon_stock: row.i64("total_coupon_stock")?,
        available_coupons: row.i64("available_coupons")?,
        claimed_coupons: row.i64("claimed_coupons")?,
        redeemed_coupons: row.i64("redeemed_coupons")?,
        active_codes: row.i64("active_codes")?,
        discount_applications: row.i64("discount_applications")?,
    })
}

async fn list_offers(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionOfferItem>, CommerceServiceError> {
    let (total_items, rows) = query_page(
        pool,
        scope,
        query,
        POSTGRES_OFFER_COUNT_SQL,
        POSTGRES_OFFER_LIST_SQL,
        "list promotion offers",
    )
    .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            Ok(PromotionOfferItem {
                id: row.string("id")?,
                campaign_id: row.optional_string("campaign_id")?,

                offer_no: row.string("offer_no")?,
                offer_code: row.optional_string("offer_code")?,
                offer_type: row.string("offer_type")?,
                audience_scope: row.string("audience_scope")?,
                combinability: row.string("combinability")?,
                goods_scope: row.string("goods_scope")?,
                display_name: row.string("display_name")?,
                description: row.optional_string("description")?,
                priority: row.i32("priority")?,
                starts_at: row.string("starts_at")?,
                ends_at: row.optional_string("ends_at")?,
                status: row.string("status")?,
                discount_type: row.optional_string("discount_type")?,
                discount_value: row.optional_string("discount_value")?,
                minimum_amount: row.optional_string("minimum_amount")?,
                maximum_discount_amount: row.optional_string("maximum_discount_amount")?,
                currency_code: row.optional_string("currency_code")?,
                coupon_benefit: crate::coupon_benefit::parse_admin_coupon_benefit(
                    row.optional_string("rule_json")?.as_deref(),
                )?,
                version: row.i64("version")?,
                updated_at: row.string("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;
    Ok(PromotionAdminPage { items, total_items })
}

async fn list_coupon_stocks(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionCouponStockItem>, CommerceServiceError> {
    let (total_items, rows) = query_page(
        pool,
        scope,
        query,
        POSTGRES_STOCK_COUNT_SQL,
        POSTGRES_STOCK_LIST_SQL,
        "list promotion coupon stocks",
    )
    .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            Ok(PromotionCouponStockItem {
                id: row.string("id")?,
                offer_id: row.string("offer_id")?,
                stock_no: row.string("stock_no")?,
                stock_type: row.string("stock_type")?,
                code_issue_mode: row.string("code_issue_mode")?,
                total_quantity: row.i64("total_quantity")?,
                available_quantity: row.i64("available_quantity")?,
                claimed_quantity: row.i64("claimed_quantity")?,
                redeemed_quantity: row.i64("redeemed_quantity")?,
                locked_quantity: row.i64("locked_quantity")?,
                per_user_limit: row.i32("per_user_limit")?,
                claim_starts_at: row.optional_string("claim_starts_at")?,
                claim_ends_at: row.optional_string("claim_ends_at")?,
                status: row.string("status")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;
    Ok(PromotionAdminPage { items, total_items })
}

async fn list_codes(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionCodeItem>, CommerceServiceError> {
    let search = query.search_pattern();
    let (total_items, rows) = match pool {
        AdminPool::Postgres(pool) => {
            let total_items =
                sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(POSTGRES_CODE_COUNT_SQL))
                    .bind(scope.tenant_id.to_string())
                    .bind(scope.organization_id.to_string())
                    .bind(&search)
                    .bind(query.status.as_deref())
                    .bind(query.code_batch_id)
                    .fetch_one(pool)
                    .await
                    .map_err(|error| storage_error("list promotion codes", error))?;
            let rows: Vec<AdminRow> = sqlx::query(sqlx::AssertSqlSafe(POSTGRES_CODE_LIST_SQL))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.status.as_deref())
                .bind(query.code_batch_id)
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage_error("list promotion codes", error))?
                .into_iter()
                .map(AdminRow::Postgres)
                .collect();
            (total_items, rows)
        }
};
    let items = rows
        .into_iter()
        .map(|row| {
            Ok(PromotionCodeItem {
                id: row.string("id")?,
                stock_id: row.string("stock_id")?,
                offer_id: row.string("offer_id")?,
                code_no: row.string("code_no")?,
                // The admin console masks the value for display but copies and
                // exports it, so the wire value must be the full code. Masking
                // here makes "copy full code" copy the masked text.
                promotion_code: row.string("promotion_code")?,
                code_type: row.string("code_type")?,
                max_claims: row.i32("max_claims")?,
                claimed_quantity: row.i32("claimed_quantity")?,
                starts_at: row.optional_string("starts_at")?,
                expires_at: row.optional_string("expires_at")?,
                status: row.string("status")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;
    Ok(PromotionAdminPage { items, total_items })
}

async fn list_discount_applications(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
) -> Result<PromotionAdminPage<PromotionDiscountApplicationItem>, CommerceServiceError> {
    let (total_items, rows) = query_page(
        pool,
        scope,
        query,
        POSTGRES_APPLICATION_COUNT_SQL,
        POSTGRES_APPLICATION_LIST_SQL,
        "list promotion discount applications",
    )
    .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            Ok(PromotionDiscountApplicationItem {
                id: row.string("id")?,
                application_no: row.string("application_no")?,
                order_id: row.string("order_id")?,
                order_no: row.optional_string("order_no")?,
                offer_id: row.string("offer_id")?,
                discount_type: row.string("discount_type")?,
                discount_amount: row.string("discount_amount")?,
                currency_code: row.string("currency_code")?,
                status: row.string("status")?,
                applied_at: row.string("applied_at")?,
                settled_at: row.optional_string("settled_at")?,
                released_at: row.optional_string("released_at")?,
                rolled_back_at: row.optional_string("rolled_back_at")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;
    Ok(PromotionAdminPage { items, total_items })
}

async fn query_page(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    query: &PromotionAdminListQuery,
    postgres_count_sql: &str,
    postgres_list_sql: &str,
    operation: &str,
) -> Result<(i64, Vec<AdminRow>), CommerceServiceError> {
    let search = query.search_pattern();
    match pool {
        AdminPool::Postgres(pool) => {
            let total_items = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(postgres_count_sql))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.status.as_deref())
                .fetch_one(pool)
                .await
                .map_err(|error| storage_error(operation, error))?;
            let rows = sqlx::query(sqlx::AssertSqlSafe(postgres_list_sql))
                .bind(scope.tenant_id.to_string())
                .bind(scope.organization_id.to_string())
                .bind(&search)
                .bind(query.status.as_deref())
                .bind(query.page_size)
                .bind(query.offset())
                .fetch_all(pool)
                .await
                .map_err(|error| storage_error(operation, error))?
                .into_iter()
                .map(AdminRow::Postgres)
                .collect();
            Ok((total_items, rows))
        }
}
}

async fn update_offer_status(
    pool: &AdminPool,
    scope: &PromotionAdminScope,
    offer_id: String,
    status: &str,
) -> Result<bool, CommerceServiceError> {
    let affected = match pool {
        AdminPool::Postgres(pool) => sqlx::query("UPDATE promotion_offer SET status = $1, version = version + 1, updated_at = TO_CHAR(CURRENT_TIMESTAMP AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') WHERE id = $2 AND tenant_id = $3 AND organization_id = $4")
            .bind(status)
            .bind(offer_id)
            .bind(scope.tenant_id.to_string())
            .bind(scope.organization_id.to_string())
            .execute(pool)
            .await
            .map_err(|error| storage_error("update promotion offer status", error))?
            .rows_affected(),
    };
    Ok(affected == 1)
}

fn storage_error(context: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("{context}: {error}"))
}

fn decode_error(column: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("decode promotion admin column {column}: {error}"))
}
