use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use sdkwork_commerce_promotion_repository_sqlx::{
    PostgresCommerceExchangeStore, PostgresCommercePromotionStore,
};
use sdkwork_commerce_promotion_service::{
    ApplyPromotionDiscountCommand, ClaimPromotionUserCouponCommand, ConsumeMemberCardCommand,
    GrantMemberCardCommand, MemberCardConsumptionOutcome, MemberCardListQuery, PointsBalance,
    PromotionCodeRedemptionPreview,
    PointsBalanceQuery, PointsHistoryItem, PointsHistoryQuery, PromotionCodeRedemptionCommand,
    PromotionCodeRedemptionOutcome, PromotionMemberCard, PromotionUserCouponItem,
    PromotionUserCouponListQuery, RetrieveMemberCardQuery, ReversePromotionDiscountCommand,
    PromotionOfferCatalogItem, PromotionOfferCatalogQuery, PromotionCouponBenefit,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_iam_context_service::IamAppContext;
use sdkwork_web_core::WebRequestContext;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::command_headers::validate_app_write_payload;
use crate::subject::{app_runtime_subject_from_extension, AppRuntimeSubject};

const MAX_PROMOTION_CODE_LEN: usize = 128;

pub type CommercePromotionFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, CommerceServiceError>> + Send + 'a>>;

pub trait CommercePromotionStore: Send + Sync {
    fn list_promotion_user_coupons<'a>(
        &'a self,
        query: PromotionUserCouponListQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionUserCouponItem>>;

    fn list_promotion_offer_catalog<'a>(
        &'a self,
        query: PromotionOfferCatalogQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionOfferCatalogItem>>;

    fn retrieve_promotion_offer_catalog<'a>(
        &'a self,
        query: PromotionOfferCatalogQuery,
        offer_id: &'a str,
    ) -> CommercePromotionFuture<'a, Option<PromotionOfferCatalogItem>>;

    fn retrieve_points_balance<'a>(
        &'a self,
        query: PointsBalanceQuery,
    ) -> CommercePromotionFuture<'a, PointsBalance>;

    fn list_points_history<'a>(
        &'a self,
        query: PointsHistoryQuery,
    ) -> CommercePromotionFuture<'a, Vec<PointsHistoryItem>>;

    fn preview_promotion_code<'a>(
        &'a self,
        command: PromotionCodeRedemptionCommand,
    ) -> CommercePromotionFuture<'a, PromotionCodeRedemptionPreview>;

    fn redeem_promotion_code<'a>(
        &'a self,
        command: PromotionCodeRedemptionCommand,
    ) -> CommercePromotionFuture<'a, PromotionCodeRedemptionOutcome>;

    fn claim_promotion_user_coupon<'a>(
        &'a self,
        command: ClaimPromotionUserCouponCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem>;

    fn apply_promotion_discount<'a>(
        &'a self,
        command: ApplyPromotionDiscountCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem>;

    fn reverse_promotion_discount<'a>(
        &'a self,
        command: ReversePromotionDiscountCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem>;

    fn grant_member_card<'a>(
        &'a self,
        command: GrantMemberCardCommand,
    ) -> CommercePromotionFuture<'a, PromotionMemberCard>;

    fn consume_member_card<'a>(
        &'a self,
        command: ConsumeMemberCardCommand,
    ) -> CommercePromotionFuture<'a, MemberCardConsumptionOutcome>;

    fn list_member_cards<'a>(
        &'a self,
        query: MemberCardListQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionMemberCard>>;

    fn retrieve_member_card<'a>(
        &'a self,
        query: RetrieveMemberCardQuery,
    ) -> CommercePromotionFuture<'a, Option<PromotionMemberCard>>;
}

#[derive(Clone)]
struct AppPromotionState {
    store: Arc<dyn CommercePromotionStore>,
}

#[derive(Debug, Deserialize)]
struct CouponListQueryParams {
    status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionCodeRedemptionRequest {
    code: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimPromotionUserCouponRequest {
    offer_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPromotionDiscountRequest {
    order_id: String,
    user_coupon_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReversePromotionDiscountRequest {
    user_coupon_id: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionUserCouponItemResponse {
    id: String,
    code: String,
    amount: String,
    date: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_currency_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionOfferCatalogResponse {
    coupon_id: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "type")]
    coupon_type: String,
    status: String,
    can_receive: bool,
    amount: String,
    min_consume: Option<String>,
    total: Option<String>,
    remaining_count: String,
    get_limit: String,
    stackable: bool,
    start_time: String,
    end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    benefit_currency_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointsBalanceResponse {
    available_points: i64,
    frozen_points: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PointsHistoryItemResponse {
    id: String,
    amount: i64,
    direction: String,
    balance_after: i64,
    business_type: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionCodeRedemptionOutcomeResponse {
    message: String,
    amount: String,
    credited_points: i64,
    balance: i64,
    benefit_kind: Option<String>,
    credited_amount: Option<String>,
    asset_type: Option<String>,
    member_card_id: Option<String>,
    member_card_no: Option<String>,
}



impl CommercePromotionStore for PostgresCommercePromotionStore {
    fn list_promotion_user_coupons<'a>(
        &'a self,
        query: PromotionUserCouponListQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionUserCouponItem>> {
        Box::pin(async move { self.list_promotion_user_coupons(query).await })
    }

    fn list_promotion_offer_catalog<'a>(
        &'a self,
        query: PromotionOfferCatalogQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionOfferCatalogItem>> {
        Box::pin(async move { self.list_promotion_offer_catalog(query).await })
    }

    fn retrieve_promotion_offer_catalog<'a>(
        &'a self,
        query: PromotionOfferCatalogQuery,
        offer_id: &'a str,
    ) -> CommercePromotionFuture<'a, Option<PromotionOfferCatalogItem>> {
        Box::pin(async move {
            let items = self.list_promotion_offer_catalog(query).await?;
            Ok(items.into_iter().find(|item| item.id == offer_id))
        })
    }

    fn retrieve_points_balance<'a>(
        &'a self,
        query: PointsBalanceQuery,
    ) -> CommercePromotionFuture<'a, PointsBalance> {
        Box::pin(async move { self.retrieve_points_balance(query).await })
    }

    fn list_points_history<'a>(
        &'a self,
        query: PointsHistoryQuery,
    ) -> CommercePromotionFuture<'a, Vec<PointsHistoryItem>> {
        Box::pin(async move { self.list_points_history(query).await })
    }

    fn preview_promotion_code<'a>(
        &'a self,
        command: PromotionCodeRedemptionCommand,
    ) -> CommercePromotionFuture<'a, PromotionCodeRedemptionPreview> {
        Box::pin(async move { self.preview_promotion_code(command).await })
    }

    fn redeem_promotion_code<'a>(
        &'a self,
        command: PromotionCodeRedemptionCommand,
    ) -> CommercePromotionFuture<'a, PromotionCodeRedemptionOutcome> {
        Box::pin(async move { self.redeem_promotion_code(command).await })
    }

    fn claim_promotion_user_coupon<'a>(
        &'a self,
        command: ClaimPromotionUserCouponCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem> {
        Box::pin(async move { self.claim_promotion_user_coupon(command).await })
    }

    fn apply_promotion_discount<'a>(
        &'a self,
        command: ApplyPromotionDiscountCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem> {
        Box::pin(async move { self.apply_promotion_discount(command).await })
    }

    fn reverse_promotion_discount<'a>(
        &'a self,
        command: ReversePromotionDiscountCommand,
    ) -> CommercePromotionFuture<'a, PromotionUserCouponItem> {
        Box::pin(async move { self.reverse_promotion_discount(command).await })
    }

    fn grant_member_card<'a>(
        &'a self,
        command: GrantMemberCardCommand,
    ) -> CommercePromotionFuture<'a, PromotionMemberCard> {
        Box::pin(async move { self.grant_member_card(command).await })
    }

    fn consume_member_card<'a>(
        &'a self,
        command: ConsumeMemberCardCommand,
    ) -> CommercePromotionFuture<'a, MemberCardConsumptionOutcome> {
        Box::pin(async move { self.consume_member_card(command).await })
    }

    fn list_member_cards<'a>(
        &'a self,
        query: MemberCardListQuery,
    ) -> CommercePromotionFuture<'a, Vec<PromotionMemberCard>> {
        Box::pin(async move { self.list_member_cards(query).await })
    }

    fn retrieve_member_card<'a>(
        &'a self,
        query: RetrieveMemberCardQuery,
    ) -> CommercePromotionFuture<'a, Option<PromotionMemberCard>> {
        Box::pin(async move { self.retrieve_member_card(query).await })
    }
}



pub fn app_promotion_router_with_postgres_pool(pool: PgPool) -> Router {
    build_app_promotion_router(Arc::new(PostgresCommercePromotionStore::new(pool.clone()))).merge(
        crate::exchange_router::build_app_exchange_router(Arc::new(
            PostgresCommerceExchangeStore::new(pool),
        )),
    )
}

pub fn build_app_promotion_router(store: Arc<dyn CommercePromotionStore>) -> Router {
    Router::new()
        .route(
            "/app/v3/api/promotions/user_coupons",
            get(fetch_promotion_user_coupons),
        )
        .route(
            "/app/v3/api/promotions/user_coupons/{userCouponId}",
            get(retrieve_promotion_user_coupon),
        )
        .route(
            "/app/v3/api/promotions/user_coupons/wallet",
            get(fetch_promotion_user_coupon_wallet),
        )
        .route(
            "/app/v3/api/promotions/user_coupons/wallet/{userCouponId}",
            get(retrieve_promotion_user_coupon_wallet_item),
        )
        .route("/app/v3/api/promotions/offers", get(fetch_promotion_offers))
        .route(
            "/app/v3/api/promotions/offers/{offerId}",
            get(retrieve_promotion_offer),
        )
        .route("/app/v3/api/wallet/points", get(fetch_points_balance))
        .route(
            "/app/v3/api/wallet/points/history",
            get(fetch_points_history),
        )
        .route(
            "/app/v3/api/promotions/user_coupon_claims",
            post(claim_promotion_user_coupon),
        )
        .route(
            "/app/v3/api/promotions/codes/redemptions",
            post(redeem_promotion_code),
        )
        .route(
            "/app/v3/api/promotions/codes/redemptions/preview",
            post(preview_promotion_code),
        )
        .route(
            "/app/v3/api/promotions/member_cards",
            get(fetch_my_member_cards),
        )
        .route(
            "/app/v3/api/promotions/member_cards/{cardId}",
            get(retrieve_member_card),
        )
        .route(
            "/app/v3/api/promotions/member_cards/{cardId}/consumptions",
            post(consume_member_card),
        )
        .route(
            "/app/v3/api/promotions/discount_applications",
            post(apply_promotion_discount),
        )
        .route(
            "/app/v3/api/promotions/discount_applications/{applicationId}/releases",
            post(release_promotion_discount),
        )
        .route(
            "/app/v3/api/promotions/discount_applications/{applicationId}/rollback",
            post(rollback_promotion_discount),
        )
        .route(
            "/app/v3/api/promotions/discount_applications/{applicationId}/settlements",
            post(settle_promotion_discount),
        )
        .route(
            "/app/v3/api/promotions/discount_applications/reversals",
            post(reverse_promotion_discount),
        )
        .with_state(AppPromotionState { store })
}

async fn fetch_promotion_user_coupons(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    Query(query): Query<CouponListQueryParams>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PromotionUserCouponListQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        query.status.as_deref(),
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.list_promotion_user_coupons(query).await {
        Ok(items) => crate::api_response::success_items(
            ctx,
            items
                .into_iter()
                .map(map_promotion_user_coupon)
                .collect::<Vec<_>>(),
            1,
            20,
        ),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn find_promotion_user_coupon(
    store: &dyn CommercePromotionStore,
    subject: &AppRuntimeSubject,
    user_coupon_id: &str,
) -> Result<Option<PromotionUserCouponItem>, CommerceServiceError> {
    let query = PromotionUserCouponListQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        None,
    )?;
    let items = store.list_promotion_user_coupons(query).await?;
    Ok(items.into_iter().find(|item| item.id == user_coupon_id))
}

async fn fetch_points_balance(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PointsBalanceQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.retrieve_points_balance(query).await {
        Ok(balance) => crate::api_response::success_item(ctx, map_points_balance(balance)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn fetch_points_history(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PointsHistoryQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.list_points_history(query).await {
        Ok(items) => crate::api_response::success_items(
            ctx,
            items
                .into_iter()
                .map(map_points_history)
                .collect::<Vec<_>>(),
            1,
            20,
        ),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn claim_promotion_user_coupon(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    headers: HeaderMap,
    body: Json<ClaimPromotionUserCouponRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let write_headers = match validate_app_write_payload(
        &headers,
        "promotions.userCoupons.claim",
        &*body,
        |idempotency_key| fallback_request_no(&subject, &body.offer_id, idempotency_key),
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let command = match ClaimPromotionUserCouponCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &body.offer_id,
        &write_headers.request_no,
        &write_headers.idempotency_key,
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.claim_promotion_user_coupon(command).await {
        Ok(item) => crate::api_response::success_item(ctx, map_promotion_user_coupon(item)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn apply_promotion_discount(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    headers: HeaderMap,
    body: Json<ApplyPromotionDiscountRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let write_headers = match validate_app_write_payload(
        &headers,
        "promotions.discounts.apply",
        &*body,
        |idempotency_key| {
            fallback_request_no(
                &subject,
                &format!("{}-{}", body.order_id, body.user_coupon_id),
                idempotency_key,
            )
        },
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let command = match ApplyPromotionDiscountCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &body.order_id,
        &body.user_coupon_id,
        &write_headers.request_no,
        &write_headers.idempotency_key,
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.apply_promotion_discount(command).await {
        Ok(item) => crate::api_response::success_item(ctx, map_promotion_user_coupon(item)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn reverse_promotion_discount(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    headers: HeaderMap,
    body: Json<ReversePromotionDiscountRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let write_headers = match validate_app_write_payload(
        &headers,
        "promotions.discounts.reverse",
        &*body,
        |idempotency_key| fallback_request_no(&subject, &body.user_coupon_id, idempotency_key),
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let command = match ReversePromotionDiscountCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &body.user_coupon_id,
        &write_headers.request_no,
        &write_headers.idempotency_key,
        body.reason.as_deref(),
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.reverse_promotion_discount(command).await {
        Ok(item) => crate::api_response::success_item(ctx, map_promotion_user_coupon(item)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromotionCodeRedemptionPreviewResponse {
    benefit_kind: Option<String>,
    credited_amount: Option<String>,
    asset_type: Option<String>,
    period: Option<String>,
    duration_days: Option<i64>,
    daily_quota: Option<i64>,
    total_quota: Option<i64>,
    expires_at: Option<String>,
}

fn map_redemption_preview(value: PromotionCodeRedemptionPreview) -> PromotionCodeRedemptionPreviewResponse {
    PromotionCodeRedemptionPreviewResponse {
        benefit_kind: value.benefit_kind,
        credited_amount: value.credited_amount,
        asset_type: value.asset_type,
        period: value.period,
        duration_days: value.duration_days,
        daily_quota: value.daily_quota,
        total_quota: value.total_quota,
        expires_at: value.expires_at,
    }
}

async fn preview_promotion_code(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    Json(request): Json<PromotionCodeRedemptionRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let code = match validate_promotion_code_redemption_request(request) {
        Ok(code) => code,
        Err(message) => return crate::api_response::validation(ctx, message),
    };
    let command = match PromotionCodeRedemptionCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &code,
        &fallback_request_no(&subject, &code, "preview"),
        &format!("preview-{}", code),
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.preview_promotion_code(command).await {
        Ok(preview) => crate::api_response::success_item(ctx, map_redemption_preview(preview)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn redeem_promotion_code(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    headers: HeaderMap,
    Json(request): Json<PromotionCodeRedemptionRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let write_headers = match validate_app_write_payload(
        &headers,
        "promotions.codes.redeem",
        &request,
        |idempotency_key| {
            fallback_request_no(
                &subject,
                request.code.as_deref().unwrap_or_default(),
                idempotency_key,
            )
        },
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let code = match validate_promotion_code_redemption_request(request) {
        Ok(code) => code,
        Err(message) => return crate::api_response::validation(ctx, message),
    };
    let command = match PromotionCodeRedemptionCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &code,
        &write_headers.request_no,
        &write_headers.idempotency_key,
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.redeem_promotion_code(command).await {
        Ok(outcome) => {
            crate::api_response::success_item(ctx, map_promotion_code_redemption_outcome(outcome))
        }
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn fetch_promotion_offers(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PromotionOfferCatalogQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.list_promotion_offer_catalog(query).await {
        Ok(items) => crate::api_response::success_items(
            ctx,
            items
                .into_iter()
                .map(map_promotion_offer_catalog)
                .collect::<Vec<_>>(),
            1,
            50,
        ),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn retrieve_promotion_offer(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(offer_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PromotionOfferCatalogQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.retrieve_promotion_offer_catalog(query, &offer_id).await {
        Ok(Some(item)) => {
            crate::api_response::success_item(ctx, map_promotion_offer_catalog(item))
        }
        Ok(None) => crate::api_response::not_found(ctx, "promotion offer not found"),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn retrieve_promotion_user_coupon(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(user_coupon_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    match find_promotion_user_coupon(&*state.store, &subject, &user_coupon_id).await {
        Ok(Some(item)) => crate::api_response::success_item(ctx, map_promotion_user_coupon(item)),
        Ok(None) => crate::api_response::not_found(ctx, "promotion user coupon not found"),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn fetch_promotion_user_coupon_wallet(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    Query(query): Query<CouponListQueryParams>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match PromotionUserCouponListQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        query.status.as_deref(),
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };

    match state.store.list_promotion_user_coupons(query).await {
        Ok(items) => crate::api_response::success_items(
            ctx,
            items
                .into_iter()
                .map(map_promotion_user_coupon)
                .collect::<Vec<_>>(),
            1,
            20,
        ),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn retrieve_promotion_user_coupon_wallet_item(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(user_coupon_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    match find_promotion_user_coupon(&*state.store, &subject, &user_coupon_id).await {
        Ok(Some(item)) => crate::api_response::success_item(ctx, map_promotion_user_coupon(item)),
        Ok(None) => crate::api_response::not_found(ctx, "promotion wallet coupon not found"),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn release_promotion_discount(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(application_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let _subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let _ = (state, application_id);
    crate::api_response::not_found(ctx, "discount application not found")
}

async fn rollback_promotion_discount(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(application_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let _subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let _ = (state, application_id);
    crate::api_response::not_found(ctx, "discount application not found")
}

async fn settle_promotion_discount(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    axum::extract::Path(application_id): axum::extract::Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let _subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let _ = (state, application_id);
    crate::api_response::not_found(ctx, "discount application not found")
}

fn validate_promotion_code_redemption_request(
    request: PromotionCodeRedemptionRequest,
) -> Result<String, String> {
    let code = request.code.unwrap_or_default().trim().to_owned();
    if code.is_empty() {
        return Err("promotion code must not be empty".to_owned());
    }
    if code.chars().count() > MAX_PROMOTION_CODE_LEN {
        return Err(format!(
            "promotion code length must not exceed {MAX_PROMOTION_CODE_LEN} characters"
        ));
    }
    if !code.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err("promotion code must contain only visible ASCII characters".to_owned());
    }
    Ok(code)
}

fn map_promotion_offer_catalog(value: PromotionOfferCatalogItem) -> PromotionOfferCatalogResponse {
    let coupon_type = if value.coupon_benefit.is_some() {
        "GIFT"
    } else if value.discount_value.is_some() {
        "DISCOUNT"
    } else {
        "UNKNOWN"
    };
    let currency_code = value.currency_code.clone();
    let (benefit_kind, benefit_amount, benefit_currency_code) = match value.coupon_benefit.as_ref()
    {
        Some(PromotionCouponBenefit::TokenBankCredit {
            grant_amount,
            bonus_amount,
        }) => (
            Some("token_bank_credit".to_owned()),
            Some(grant_amount.saturating_add(*bonus_amount).to_string()),
            None,
        ),
        Some(PromotionCouponBenefit::PointsCredit { grant_points }) => (
            Some("points_credit".to_owned()),
            Some(grant_points.to_string()),
            None,
        ),
        Some(PromotionCouponBenefit::CashCredit { grant_amount }) => (
            Some("cash_credit".to_owned()),
            Some(grant_amount.to_string()),
            currency_code.clone(),
        ),
        Some(PromotionCouponBenefit::Subscription { total_quota, .. }) => (
            Some("subscription".to_owned()),
            Some(total_quota.to_string()),
            None,
        ),
        None => (None, None, None),
    };
    PromotionOfferCatalogResponse {
        coupon_id: value.id,
        name: value.name,
        description: value.description,
        coupon_type: coupon_type.to_owned(),
        status: "available".to_owned(),
        can_receive: value.available_quantity > 0,
        amount: value.discount_value.unwrap_or_else(|| "0".to_owned()),
        min_consume: value.minimum_amount,
        total: value.total_quantity.map(|total| total.to_string()),
        remaining_count: value.available_quantity.to_string(),
        get_limit: value.per_user_limit.to_string(),
        stackable: value.combinability.eq_ignore_ascii_case("COMBINABLE"),
        start_time: value.starts_at,
        end_time: value.ends_at,
        benefit_kind,
        benefit_amount,
        benefit_currency_code,
    }
}

/// Converts the repository's integer minor-unit amount into the major-unit
/// decimal string the app coupon centre renders with `formatMoney`.
///
/// The admin surface intentionally keeps minor units (`formatMarketingAmountMinor`);
/// the app surface historically received a major decimal. Sending minor units to
/// `formatMoney` renders a 100x amount, so the conversion belongs here.
fn minor_units_to_major_string(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "0.00".to_owned();
    }
    if trimmed.contains('.') {
        return trimmed.to_owned();
    }
    match trimmed.parse::<i64>() {
        Ok(minor) => {
            let sign = if minor < 0 { "-" } else { "" };
            let absolute = minor.unsigned_abs();
            format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
        }
        Err(_) => trimmed.to_owned(),
    }
}

fn map_promotion_user_coupon(value: PromotionUserCouponItem) -> PromotionUserCouponItemResponse {
    PromotionUserCouponItemResponse {
        id: value.id,
        code: value.code,
        amount: minor_units_to_major_string(value.amount.as_str()),
        date: value.date,
        status: value.status,
        benefit_kind: value.benefit_kind,
        benefit_amount: value.benefit_amount,
        benefit_currency_code: value.benefit_currency_code,
    }
}

fn map_points_balance(value: PointsBalance) -> PointsBalanceResponse {
    PointsBalanceResponse {
        available_points: value.available_points,
        frozen_points: value.frozen_points,
    }
}

fn map_points_history(value: PointsHistoryItem) -> PointsHistoryItemResponse {
    PointsHistoryItemResponse {
        id: value.id,
        amount: value.amount,
        direction: value.direction,
        balance_after: value.balance_after,
        business_type: value.business_type,
        created_at: value.created_at,
    }
}

fn map_promotion_code_redemption_outcome(
    value: PromotionCodeRedemptionOutcome,
) -> PromotionCodeRedemptionOutcomeResponse {
    PromotionCodeRedemptionOutcomeResponse {
        message: value.message,
        amount: value.amount.as_str().to_owned(),
        credited_points: value.credited_points,
        balance: value.balance,
        benefit_kind: value.benefit_kind,
        credited_amount: value.credited_amount,
        asset_type: value.asset_type,
        member_card_id: value.member_card_id,
        member_card_no: value.member_card_no,
    }
}


#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberCardResponse {
    id: String,
    card_no: String,
    offer_id: String,
    offer_version_id: String,
    user_coupon_id: String,
    period: String,
    duration_days: i64,
    daily_quota: i64,
    total_quota: i64,
    total_used: i64,
    balance: i64,
    status: String,
    starts_at: String,
    expires_at: Option<String>,
    created_at: String,
}

fn map_member_card(value: PromotionMemberCard) -> MemberCardResponse {
    MemberCardResponse {
        id: value.id,
        card_no: value.card_no,
        offer_id: value.offer_id,
        offer_version_id: value.offer_version_id,
        user_coupon_id: value.user_coupon_id,
        period: value.period.as_str().to_owned(),
        duration_days: value.duration_days,
        daily_quota: value.daily_quota,
        total_quota: value.total_quota,
        total_used: value.total_used,
        balance: value.total_quota - value.total_used,
        status: value.status,
        starts_at: value.starts_at,
        expires_at: value.expires_at,
        created_at: value.created_at,
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberCardConsumptionRequest {
    amount: String,
    source_type: Option<String>,
    source_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberCardConsumptionResponse {
    accepted: bool,
    replayed: bool,
    card_id: String,
    consumed_amount: i64,
    used_today: i64,
    daily_quota: i64,
    total_used: i64,
    total_quota: i64,
    balance: i64,
}

fn map_member_card_consumption(value: MemberCardConsumptionOutcome) -> MemberCardConsumptionResponse {
    MemberCardConsumptionResponse {
        accepted: value.accepted,
        replayed: value.replayed,
        card_id: value.card_id,
        consumed_amount: value.consumed_amount,
        used_today: value.used_today,
        daily_quota: value.daily_quota,
        total_used: value.total_used,
        total_quota: value.total_quota,
        balance: value.balance,
    }
}

async fn fetch_my_member_cards(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match MemberCardListQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.list_member_cards(query).await {
        Ok(items) => {
            let cards = items.into_iter().map(map_member_card).collect::<Vec<_>>();
            let page_size = cards.len() as i64;
            crate::api_response::success_items(ctx, cards, 1, page_size)
        }
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn retrieve_member_card(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    Path(card_id): Path<String>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let query = match RetrieveMemberCardQuery::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &card_id,
    ) {
        Ok(query) => query,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.retrieve_member_card(query).await {
        Ok(Some(card)) => crate::api_response::success_item(ctx, map_member_card(card)),
        Ok(None) => crate::api_response::not_found(ctx, "member card is not found"),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

async fn consume_member_card(
    State(state): State<AppPromotionState>,
    runtime_context: Option<Extension<IamAppContext>>,
    request_context: Option<Extension<WebRequestContext>>,
    headers: HeaderMap,
    Path(card_id): Path<String>,
    body: Json<MemberCardConsumptionRequest>,
) -> Response {
    let ctx = request_context.as_ref().map(|ext| &ext.0);
    let subject = match app_runtime_subject_from_extension(runtime_context) {
        Ok(subject) => subject,
        Err(message) => return crate::api_response::unauthorized(ctx, message),
    };
    let write_headers = match validate_app_write_payload(
        &headers,
        "promotions.memberCards.consume",
        &*body,
        |idempotency_key| fallback_request_no(&subject, &card_id, idempotency_key),
    ) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let amount: i64 = match body.amount.trim().parse() {
        Ok(value) if value > 0 => value,
        _ => return crate::api_response::validation(ctx, "member card consumption amount must be a positive integer"),
    };
    let command = match ConsumeMemberCardCommand::new(
        &subject.tenant_id,
        subject.organization_id.as_deref(),
        &subject.user_id,
        &card_id,
        amount,
        body.source_type.as_deref(),
        body.source_id.as_deref(),
        &write_headers.request_no,
        &write_headers.idempotency_key,
    ) {
        Ok(command) => command,
        Err(error) => return crate::api_response::map_service_error(ctx, error),
    };
    match state.store.consume_member_card(command).await {
        Ok(outcome) => crate::api_response::success_item(ctx, map_member_card_consumption(outcome)),
        Err(error) => crate::api_response::map_service_error(ctx, error),
    }
}

fn fallback_request_no(subject: &AppRuntimeSubject, code: &str, idempotency_key: &str) -> String {
    let code_part = code
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!(
        "promotion-code-redemption-{}-{}-{}",
        subject.user_id,
        code_part,
        stable_header_token(idempotency_key),
    )
}

fn stable_header_token(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_commerce_promotion_service::PromotionCouponBenefit;

    fn catalog_offer(
        coupon_benefit: Option<PromotionCouponBenefit>,
        discount_value: Option<&str>,
    ) -> PromotionOfferCatalogItem {
        PromotionOfferCatalogItem {
            id: "offer-1".to_owned(),
            name: "Token Bank coupon".to_owned(),
            description: Some("110 credits".to_owned()),
            offer_type: "coupon".to_owned(),
            combinability: "COMBINABLE".to_owned(),
            starts_at: "2026-11-01T00:00:00.000Z".to_owned(),
            ends_at: Some("2026-11-30T00:00:00.000Z".to_owned()),
            discount_value: discount_value.map(str::to_owned),
            minimum_amount: Some("0".to_owned()),
            currency_code: Some("CNY".to_owned()),
            coupon_benefit,
            total_quantity: Some(100),
            available_quantity: 20,
            per_user_limit: 1,
            claim_starts_at: None,
            claim_ends_at: None,
        }
    }

    #[test]
    fn catalog_maps_asset_coupon_with_string_quantities() {
        let response = map_promotion_offer_catalog(catalog_offer(
            Some(PromotionCouponBenefit::TokenBankCredit {
                grant_amount: 110,
                bonus_amount: 0,
            }),
            Some("0"),
        ));
        assert_eq!(response.coupon_id, "offer-1");
        assert_eq!(response.coupon_type, "GIFT");
        assert!(response.can_receive);
        assert_eq!(response.remaining_count, "20");
        assert_eq!(response.get_limit, "1");
        assert_eq!(response.total.as_deref(), Some("100"));
        assert!(response.stackable);
        assert_eq!(response.benefit_kind.as_deref(), Some("token_bank_credit"));
        assert_eq!(response.benefit_amount.as_deref(), Some("110"));
        assert_eq!(response.benefit_currency_code, None);
    }

    #[test]
    fn catalog_maps_discount_coupon_and_zero_stock_is_not_claimable() {
        let mut offer = catalog_offer(None, Some("300"));
        offer.available_quantity = 0;
        let response = map_promotion_offer_catalog(offer);
        assert_eq!(response.coupon_type, "DISCOUNT");
        assert!(!response.can_receive);
        assert_eq!(response.remaining_count, "0");
        assert_eq!(response.amount, "300");
    }

    #[test]
    fn user_coupon_response_keeps_the_frozen_benefit_snapshot() {
        let item = PromotionUserCouponItem::new(
            "uc-1",
            "CL-1",
            "0",
            "2026-11-12T00:00:00.000Z",
            "claimed",
        )
        .expect("user coupon")
        .with_benefit(Some("token_bank_credit"), Some("130"), None);
        let response = map_promotion_user_coupon(item);
        assert_eq!(response.code, "CL-1");
        assert_eq!(response.amount, "0.00");
        assert_eq!(response.benefit_kind.as_deref(), Some("token_bank_credit"));
        assert_eq!(response.benefit_amount.as_deref(), Some("130"));
        assert_eq!(response.benefit_currency_code, None);
    }

    #[test]
    fn minor_units_are_rendered_as_major_units_for_the_app_surface() {
        assert_eq!(minor_units_to_major_string("8000"), "80.00");
        assert_eq!(minor_units_to_major_string("10050"), "100.50");
        assert_eq!(minor_units_to_major_string("5"), "0.05");
        assert_eq!(minor_units_to_major_string("-250"), "-2.50");
        assert_eq!(minor_units_to_major_string("80.00"), "80.00");
        assert_eq!(minor_units_to_major_string(""), "0.00");
    }
}
