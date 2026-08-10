use sdkwork_commerce_promotion_service::{
    AppCommerceExchangeRuleItem, AppCommerceExchangeRuleQuery,
};
use sdkwork_contract_service::CommerceServiceError;
use sqlx::{PgPool, Row};

const POINTS_STORAGE_ASSET_TYPE: &str = "points";
const CASH_STORAGE_ASSET_TYPE: &str = "cash";

/// 环境变量名：平台全局兑换规则所属的系统租户 ID。
///
/// 该租户代表"平台默认汇率"的归属方，必须与真实业务租户隔离。
/// 默认值 `100001` 仅为向后兼容历史数据；生产部署应通过环境变量显式配置一个
/// 专属的系统租户标识（例如 `000000` 或 `platform`），避免与业务租户混淆。
const GLOBAL_EXCHANGE_TENANT_ID_ENV: &str = "PROMOTION_GLOBAL_EXCHANGE_TENANT_ID";
const DEFAULT_GLOBAL_EXCHANGE_TENANT_ID: &str = "100001";

/// 环境变量名：平台全局兑换规则所属的系统组织 ID。
///
/// 留空时 SQL 会以平台哨兵组织 `organization_id = '0'` 作为全局匹配条件。
const GLOBAL_EXCHANGE_ORGANIZATION_ID_ENV: &str = "PROMOTION_GLOBAL_EXCHANGE_ORGANIZATION_ID";
const DEFAULT_GLOBAL_EXCHANGE_ORGANIZATION_ID: &str = "0";

fn global_exchange_tenant_id() -> String {
    std::env::var(GLOBAL_EXCHANGE_TENANT_ID_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_GLOBAL_EXCHANGE_TENANT_ID.to_owned())
}

fn global_exchange_organization_id() -> Option<String> {
    match std::env::var(GLOBAL_EXCHANGE_ORGANIZATION_ID_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => Some(DEFAULT_GLOBAL_EXCHANGE_ORGANIZATION_ID.to_owned()),
    }
}

fn legacy_appbase_organization_id(organization_id: Option<&str>) -> String {
    organization_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_GLOBAL_EXCHANGE_ORGANIZATION_ID)
        .to_owned()
}

#[derive(Debug, Clone)]
pub struct PostgresCommerceExchangeStore {
    pool: PgPool,
}

impl PostgresCommerceExchangeStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_exchange_rules(
        &self,
        query: AppCommerceExchangeRuleQuery,
    ) -> Result<Vec<AppCommerceExchangeRuleItem>, CommerceServiceError> {
        let global_tenant = global_exchange_tenant_id();
        let global_org = global_exchange_organization_id();
        let rows = match query.subject.as_ref() {
            Some(subject) => {
                let organization_id =
                    legacy_appbase_organization_id(subject.organization_id.as_deref());
                sqlx::query(EXCHANGE_RULES_FOR_SUBJECT_SQL)
                    .bind(&subject.tenant_id)
                    .bind(&organization_id)
                    .bind(&subject.tenant_id)
                    .bind(&organization_id)
                    .bind(&global_tenant)
                    .bind(global_org.as_deref())
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                sqlx::query(EXCHANGE_RULES_GLOBAL_SQL)
                    .bind(&global_tenant)
                    .bind(global_org.as_deref())
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|error| store_error("failed to list app exchange rules", error))?;

        rows.iter()
            .map(exchange_rule_from_row)
            .filter(|item| match item {
                Ok(item) => exchange_rule_matches_filters(item, &query),
                Err(_) => true,
            })
            .collect()
    }

    pub async fn load_points_exchange_rate(
        &self,
        query: AppCommerceExchangeRuleQuery,
    ) -> Result<Option<AppCommerceExchangeRuleItem>, CommerceServiceError> {
        let global_tenant = global_exchange_tenant_id();
        let global_org = global_exchange_organization_id();
        let row = match query.subject.as_ref() {
            Some(subject) => {
                let organization_id =
                    legacy_appbase_organization_id(subject.organization_id.as_deref());
                sqlx::query(EXCHANGE_RULES_FOR_SUBJECT_SQL)
                    .bind(&subject.tenant_id)
                    .bind(&organization_id)
                    .bind(&subject.tenant_id)
                    .bind(&organization_id)
                    .bind(&global_tenant)
                    .bind(global_org.as_deref())
                    .fetch_optional(&self.pool)
                    .await
            }
            None => {
                sqlx::query(EXCHANGE_RULES_GLOBAL_SQL)
                    .bind(&global_tenant)
                    .bind(global_org.as_deref())
                    .fetch_optional(&self.pool)
                    .await
            }
        }
        .map_err(|error| store_error("failed to load app exchange rate", error))?;

        row.as_ref().map(exchange_rule_from_row).transpose()
    }
}

const EXCHANGE_RULES_FOR_SUBJECT_SQL: &str = r#"
SELECT
    id,
    source_asset_type,
    target_asset_type,
    rate,
    status,
    tenant_id,
    organization_id
FROM commerce_exchange_rule
WHERE ((tenant_id = $1::text AND organization_id = $2::text)
       OR (tenant_id = $5::text AND (organization_id = $6::text OR organization_id = '0')))
  AND source_asset_type = 'points'
  AND target_asset_type = 'cash'
  AND status = 'active'
ORDER BY CASE WHEN tenant_id = $3::text AND organization_id = $4::text THEN 0 ELSE 1 END,
         updated_at DESC,
         id DESC
LIMIT 500
"#;

const EXCHANGE_RULES_GLOBAL_SQL: &str = r#"
SELECT
    id,
    source_asset_type,
    target_asset_type,
    rate,
    status,
    tenant_id,
    organization_id
FROM commerce_exchange_rule
WHERE tenant_id = $1::text
  AND (organization_id = $2::text OR organization_id = '0')
  AND source_asset_type = 'points'
  AND target_asset_type = 'cash'
  AND status = 'active'
ORDER BY updated_at DESC, id DESC
LIMIT 500
"#;

fn exchange_rule_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AppCommerceExchangeRuleItem, CommerceServiceError> {
    AppCommerceExchangeRuleItem::new(
        &string_cell(row, "id"),
        &display_asset_type(&string_cell(row, "source_asset_type"))?,
        &display_asset_type(&string_cell(row, "target_asset_type"))?,
        &canonical_decimal_string(&string_cell(row, "rate"), 6, "exchange rule rate")?,
        &string_cell(row, "status"),
    )
}

fn display_asset_type(value: &str) -> Result<String, CommerceServiceError> {
    match value.trim() {
        POINTS_STORAGE_ASSET_TYPE => Ok("POINTS".to_owned()),
        CASH_STORAGE_ASSET_TYPE => Ok("CASH".to_owned()),
        value => Err(CommerceServiceError::validation(format!(
            "unsupported app exchange asset type: {value}"
        ))),
    }
}

fn exchange_rule_matches_filters(
    item: &AppCommerceExchangeRuleItem,
    query: &AppCommerceExchangeRuleQuery,
) -> bool {
    query
        .source_asset_type
        .as_deref()
        .map(|value| value == item.source_asset_type)
        .unwrap_or(true)
        && query
            .target_asset_type
            .as_deref()
            .map(|value| value == item.target_asset_type)
            .unwrap_or(true)
}

fn canonical_decimal_string(
    value: &str,
    scale: usize,
    field_name: &str,
) -> Result<String, CommerceServiceError> {
    let value = value.trim().replace(',', "");
    if value.is_empty() || value.starts_with('-') || value.starts_with('+') {
        return Err(CommerceServiceError::validation(format!(
            "invalid {field_name}: {value}"
        )));
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .unwrap_or_default()
        .trim_start_matches('0')
        .to_owned();
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || whole.chars().any(|ch| !ch.is_ascii_digit())
        || fraction.chars().any(|ch| !ch.is_ascii_digit())
        || fraction.len() > scale
    {
        return Err(CommerceServiceError::validation(format!(
            "invalid {field_name}: {value}"
        )));
    }
    let whole = if whole.is_empty() { "0" } else { &whole };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        Ok(whole.to_owned())
    } else {
        Ok(format!("{whole}.{fraction}"))
    }
}

fn optional_string_cell(row: &sqlx::postgres::PgRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column)
        .ok()
        .flatten()
        .or_else(|| row.try_get::<String, _>(column).ok())
        .or_else(|| {
            row.try_get::<Option<i64>, _>(column)
                .ok()
                .flatten()
                .map(|value| value.to_string())
        })
        .or_else(|| {
            row.try_get::<i64, _>(column)
                .ok()
                .map(|value| value.to_string())
        })
}

fn string_cell(row: &sqlx::postgres::PgRow, column: &str) -> String {
    optional_string_cell(row, column).unwrap_or_default()
}

fn store_error(context: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("{context}: {error}"))
}
