use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sdkwork_commerce_promotion_repository_sqlx::PostgresCommerceExchangeStore;
use sdkwork_commerce_promotion_service::{
    AppCommerceExchangeRuleItem, AppCommerceExchangeRuleQuery, AppCommerceSubject,
};
use sdkwork_contract_service::CommerceServiceError;
use sdkwork_iam_context_service::IamAppContext;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::subject::app_runtime_subject_from_extension;

const MAX_ASSET_TYPE_LEN: usize = 32;
const POINTS_ASSET_TYPE: &str = "POINTS";
const CASH_ASSET_TYPE: &str = "CASH";

pub type CommerceExchangeFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, CommerceServiceError>> + Send + 'a>>;

pub trait CommerceExchangeStore: Send + Sync {
    fn list_exchange_rules<'a>(
        &'a self,
        query: AppCommerceExchangeRuleQuery,
    ) -> CommerceExchangeFuture<'a, Vec<AppCommerceExchangeRuleItem>>;

    fn load_points_exchange_rate<'a>(
        &'a self,
        query: AppCommerceExchangeRuleQuery,
    ) -> CommerceExchangeFuture<'a, Option<AppCommerceExchangeRuleItem>>;
}

#[derive(Clone)]
struct AppExchangeState {
    store: Arc<dyn CommerceExchangeStore>,
}

#[derive(Debug, Deserialize)]
struct ExchangeRulesQueryRequest {
    #[serde(rename = "source_asset_type", alias = "sourceAssetType")]
    source_asset_type: Option<String>,
    #[serde(rename = "target_asset_type", alias = "targetAssetType")]
    target_asset_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppCommerceExchangeApiResult<T: Serialize> {
    code: String,
    msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppCommerceExchangeRuleResponse {
    id: String,
    source_asset_type: String,
    target_asset_type: String,
    rate: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppCommercePointsExchangeRateResponse {
    source_asset_type: String,
    target_asset_type: String,
    rate: String,
}



impl CommerceExchangeStore for PostgresCommerceExchangeStore {
    fn list_exchange_rules<'a>(
        &'a self,
        query: AppCommerceExchangeRuleQuery,
    ) -> CommerceExchangeFuture<'a, Vec<AppCommerceExchangeRuleItem>> {
        Box::pin(async move { self.list_exchange_rules(query).await })
    }

    fn load_points_exchange_rate<'a>(
        &'a self,
        query: AppCommerceExchangeRuleQuery,
    ) -> CommerceExchangeFuture<'a, Option<AppCommerceExchangeRuleItem>> {
        Box::pin(async move { self.load_points_exchange_rate(query).await })
    }
}

impl<T: Serialize> AppCommerceExchangeApiResult<T> {
    fn success(data: T) -> Self {
        Self {
            code: "2000".to_owned(),
            msg: "SUCCESS".to_owned(),
            data: Some(data),
        }
    }
}

impl AppCommerceExchangeApiResult<()> {
    fn error(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            msg: msg.into(),
            data: None,
        }
    }
}



pub fn app_exchange_router_with_postgres_pool(pool: PgPool) -> Router {
    build_app_exchange_router(Arc::new(PostgresCommerceExchangeStore::new(pool)))
}

pub fn build_app_exchange_router(store: Arc<dyn CommerceExchangeStore>) -> Router {
    Router::new()
        .route(
            "/app/v3/api/wallet/exchange_rate",
            get(points_exchange_rate),
        )
        .route(
            "/app/v3/api/wallet/points/exchanges/rules",
            get(points_exchange_rules),
        )
        .with_state(AppExchangeState { store })
}

async fn points_exchange_rate(
    State(state): State<AppExchangeState>,
    runtime_context: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match resolve_subject(runtime_context) {
        Ok(subject) => subject,
        Err(response) => return response,
    };
    let query = AppCommerceExchangeRuleQuery {
        subject: Some(subject),
        source_asset_type: Some(POINTS_ASSET_TYPE.to_owned()),
        target_asset_type: Some(CASH_ASSET_TYPE.to_owned()),
    };

    match state.store.load_points_exchange_rate(query).await {
        Ok(Some(item)) => Json(AppCommerceExchangeApiResult::success(
            AppCommercePointsExchangeRateResponse {
                source_asset_type: item.source_asset_type,
                target_asset_type: item.target_asset_type,
                rate: item.rate,
            },
        ))
        .into_response(),
        Ok(None) => not_found_response("exchange rule was not found"),
        Err(error) => commerce_error_response("exchange rule read model is unavailable", error),
    }
}

async fn points_exchange_rules(
    State(state): State<AppExchangeState>,
    Query(params): Query<ExchangeRulesQueryRequest>,
    runtime_context: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match resolve_subject(runtime_context) {
        Ok(subject) => subject,
        Err(response) => return response,
    };
    let source_asset_type = match normalize_optional_asset_type(params.source_asset_type.as_deref())
    {
        Ok(value) => value,
        Err(message) => return validation_response(message),
    };
    let target_asset_type = match normalize_optional_asset_type(params.target_asset_type.as_deref())
    {
        Ok(value) => value,
        Err(message) => return validation_response(message),
    };

    match state
        .store
        .list_exchange_rules(AppCommerceExchangeRuleQuery {
            subject: Some(subject),
            source_asset_type,
            target_asset_type,
        })
        .await
    {
        Ok(items) => Json(AppCommerceExchangeApiResult::success(
            items.into_iter().map(map_exchange_rule).collect::<Vec<_>>(),
        ))
        .into_response(),
        Err(error) => commerce_error_response("exchange rule read model is unavailable", error),
    }
}

fn resolve_subject(
    runtime_context: Option<Extension<IamAppContext>>,
) -> Result<AppCommerceSubject, Response> {
    let subject =
        app_runtime_subject_from_extension(runtime_context).map_err(unauthorized_response)?;
    Ok(AppCommerceSubject {
        tenant_id: subject.tenant_id,
        organization_id: subject.organization_id,
        user_id: subject.user_id,
    })
}

fn normalize_optional_asset_type(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_uppercase();
    if normalized.chars().count() > MAX_ASSET_TYPE_LEN {
        return Err(format!(
            "asset type must be at most {MAX_ASSET_TYPE_LEN} characters"
        ));
    }
    if !normalized
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("asset type may only contain letters, numbers, -, and _".to_owned());
    }
    if normalized != POINTS_ASSET_TYPE && normalized != CASH_ASSET_TYPE {
        return Err("exchange rule currently supports POINTS to CASH only".to_owned());
    }
    Ok(Some(normalized))
}

fn map_exchange_rule(value: AppCommerceExchangeRuleItem) -> AppCommerceExchangeRuleResponse {
    AppCommerceExchangeRuleResponse {
        id: value.id,
        source_asset_type: value.source_asset_type,
        target_asset_type: value.target_asset_type,
        rate: value.rate,
        status: value.status,
    }
}

fn commerce_error_response(context: &str, error: CommerceServiceError) -> Response {
    match error.code() {
        "validation" => validation_response(error.message()),
        "unauthenticated" | "unauthorized" => unauthorized_response(error.message().to_owned()),
        "not-found" => not_found_response(error.message()),
        "conflict" | "invalid-state" | "unsupported-capability" => (
            StatusCode::CONFLICT,
            Json(AppCommerceExchangeApiResult::error("4090", error.message())),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AppCommerceExchangeApiResult::error(
                "5000",
                format!("{context}: {}", error.message()),
            )),
        )
            .into_response(),
    }
}

fn unauthorized_response(message: String) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AppCommerceExchangeApiResult::error("4010", message)),
    )
        .into_response()
}

fn validation_response(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AppCommerceExchangeApiResult::error("4001", message.into())),
    )
        .into_response()
}

fn not_found_response(message: impl Into<String>) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(AppCommerceExchangeApiResult::error("4040", message.into())),
    )
        .into_response()
}
