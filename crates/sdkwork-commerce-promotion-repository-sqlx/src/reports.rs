//! Coupon-centre operating report: windowed counts, money per currency, and the
//! top offers of the window.
//!
//! The legacy overview returns eight lifetime counters. An operating review needs
//! "what happened between these two instants, in which currency, for which
//! offer", so this module runs three focused, indexable read queries in one
//! read-only transaction:
//!
//! 1. [`counts_sql`] — nine windowed counters, one scan per source table.
//! 2. [`money_sql`] — discount money and budget snapshot, grouped by currency.
//! 3. [`top_offers_sql`] — the five offers with the most windowed redemptions.
//!
//! Rules shared by all three:
//!
//! * the window predicate is the SQL instant expression from
//!   [`crate::timestamps::instant`], with inclusive bounds
//!   (`>= $from AND <= $to`), so a stored `TEXT` timestamp is never compared as
//!   text — a `'2026-11-10T09:12:00.000Z'` value must not be dropped just
//!   because `'T'` sorts after `' '`;
//! * tenant/organization filtering keeps the exact admin-list semantics
//!   (`tenant_id = $1 AND organization_id = $2` on raw columns, so the existing
//!   tenant-leading indexes still constrain each scan; `'0'` is the platform
//!   sentinel, so platform rows are visible only to a platform-scoped operator);
//! * money is summed per currency, never across currencies;
//! * budget figures are a **current snapshot** and are therefore not
//!   window-filtered.

use sdkwork_commerce_promotion_service::{
    report_rates, PromotionAdminScope, PromotionReport, PromotionReportCounts,
    PromotionReportCurrencyRow, PromotionReportTopOffer,
};
use sdkwork_contract_service::CommerceServiceError;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::timestamps;

/// Number of offers in the report's `topOffers` section.
pub const REPORT_TOP_OFFER_LIMIT: i64 = 5;

/// Inclusive instant-window predicate for one `TEXT` timestamp column.
///
/// The bound parameters are `$3` (from) and `$4` (to) in every report query, so
/// one set of bounds serves all three queries of the report transaction.
fn window_between(column: &str) -> String {
    let instant = timestamps::instant(column);
    format!("{instant} >= CAST($3 AS timestamptz) AND {instant} <= CAST($4 AS timestamptz)")
}

/// Admin-list tenant/organization predicate for one table alias.
fn scope_predicate(alias: &str) -> String {
    format!("{alias}.tenant_id = $1 AND {alias}.organization_id = $2")
}

/// Converts one stored `TEXT` discount amount to integer minor units.
///
/// `promotion_discount_application.discount_amount` is written as an integer
/// minor-unit decimal string (`"1250"`) by the apply path. Rows written before
/// that rule carry a decimal-major string (`"12.50"`); they are converted at
/// read time so the report cannot silently drop historical money, and a value
/// that is neither shape contributes `0` instead of failing the whole report.
fn discount_amount_minor(column: &str) -> String {
    format!(
        "(CASE \
             WHEN BTRIM({column}) ~ '^[0-9]+$' THEN BTRIM({column})::BIGINT \
             WHEN BTRIM({column}) ~ '^[0-9]+(\\.[0-9]{{1,2}})?$' \
                 THEN ROUND(BTRIM({column})::NUMERIC * 100)::BIGINT \
             ELSE 0 \
         END)"
    )
}

/// Nine windowed counters in one statement (one scan per source table).
///
/// Codes are windowed by `created_at`, wallet coupons by `claimed_at`, and
/// discount applications by `applied_at`; every predicate goes through
/// [`window_between`]. `disabledCodes`, `redeemedCoupons`, `expiredCoupons`,
/// `voidedCoupons`, and the settled/released counters are conditional counts of
/// the same window, so a status change after the window does not move a row
/// between queries.
pub(crate) fn counts_sql() -> String {
    let code_window = window_between("c.created_at");
    let coupon_window = window_between("uc.claimed_at");
    let application_window = window_between("a.applied_at");
    format!(
        r#"
        SELECT
            (SELECT COUNT(*)::BIGINT FROM promotion_code c
              WHERE {code_scope} AND {code_window}) AS issued_codes,
            (SELECT COUNT(*)::BIGINT FROM promotion_code c
              WHERE {code_scope} AND LOWER(c.status) = 'disabled' AND {code_window}) AS disabled_codes,
            (SELECT COUNT(*)::BIGINT FROM promotion_user_coupon uc
              WHERE {coupon_scope} AND {coupon_window}) AS claimed_coupons,
            (SELECT COUNT(*)::BIGINT FROM promotion_user_coupon uc
              WHERE {coupon_scope} AND LOWER(uc.status) IN ('redeemed', 'used') AND {coupon_window}) AS redeemed_coupons,
            (SELECT COUNT(*)::BIGINT FROM promotion_user_coupon uc
              WHERE {coupon_scope} AND LOWER(uc.status) = 'expired' AND {coupon_window}) AS expired_coupons,
            (SELECT COUNT(*)::BIGINT FROM promotion_user_coupon uc
              WHERE {coupon_scope} AND LOWER(uc.status) = 'voided' AND {coupon_window}) AS voided_coupons,
            (SELECT COUNT(*)::BIGINT FROM promotion_discount_application a
              WHERE {application_scope} AND {application_window}) AS applications_applied,
            (SELECT COUNT(*)::BIGINT FROM promotion_discount_application a
              WHERE {application_scope} AND LOWER(a.status) = 'settled' AND {application_window}) AS applications_settled,
            (SELECT COUNT(*)::BIGINT FROM promotion_discount_application a
              WHERE {application_scope} AND LOWER(a.status) = 'released' AND {application_window}) AS applications_released
        "#,
        code_scope = scope_predicate("c"),
        coupon_scope = scope_predicate("uc"),
        application_scope = scope_predicate("a"),
    )
}

/// Discount money and budget figures, one row per currency.
///
/// Two independently grouped CTEs (`applied` and `budget`) are functionally full
/// outer joined on `currency_code`, so a currency that only has budget accounts
/// (no application yet) and a currency that only has applications both appear.
/// The application side is window-filtered; the budget side is the current
/// snapshot.
pub(crate) fn money_sql() -> String {
    let application_window = window_between("a.applied_at");
    let amount = discount_amount_minor("a.discount_amount");
    format!(
        r#"
        WITH applied AS (
            SELECT a.currency_code AS currency_code,
                   COALESCE(SUM({amount}), 0)::BIGINT AS discount_applied_minor,
                   COALESCE(SUM(CASE WHEN LOWER(a.status) = 'settled' THEN {amount} ELSE 0 END), 0)::BIGINT AS discount_settled_minor
            FROM promotion_discount_application a
            WHERE {application_scope} AND {application_window}
            GROUP BY a.currency_code
        ),
        budget AS (
            SELECT b.currency_code AS currency_code,
                   COALESCE(SUM(b.total_amount_minor), 0)::BIGINT AS budget_total_minor,
                   COALESCE(SUM(b.reserved_amount_minor), 0)::BIGINT AS budget_reserved_minor,
                   COALESCE(SUM(b.used_amount_minor), 0)::BIGINT AS budget_used_minor
            FROM promotion_budget_account b
            WHERE {budget_scope}
            GROUP BY b.currency_code
        )
        SELECT COALESCE(a.currency_code, b.currency_code) AS currency_code,
               COALESCE(a.discount_applied_minor, 0)::BIGINT AS discount_applied_minor,
               COALESCE(a.discount_settled_minor, 0)::BIGINT AS discount_settled_minor,
               COALESCE(b.budget_total_minor, 0)::BIGINT AS budget_total_minor,
               COALESCE(b.budget_reserved_minor, 0)::BIGINT AS budget_reserved_minor,
               COALESCE(b.budget_used_minor, 0)::BIGINT AS budget_used_minor
        FROM applied a
        FULL OUTER JOIN budget b ON b.currency_code = a.currency_code
        ORDER BY currency_code ASC
        "#,
        application_scope = scope_predicate("a"),
        budget_scope = scope_predicate("b"),
    )
}

/// The five offers with the most wallet coupons redeemed inside the window.
///
/// Tie-break: settled discount amount descending, then offer id ascending, so
/// the page is deterministic. Each offer reports one currency — the currency of
/// its largest settled amount (tie broken by currency code) — and only that
/// currency's settled amount, so the row's money and its currency always agree.
/// An offer with no settled application falls back to its current offer
/// version's currency.
pub(crate) fn top_offers_sql() -> String {
    let coupon_window = window_between("uc.claimed_at");
    let application_window = window_between("a.applied_at");
    let amount = discount_amount_minor("a.discount_amount");
    format!(
        r#"
        WITH redeemed AS (
            SELECT uc.offer_id AS offer_id, COUNT(*)::BIGINT AS redeemed
            FROM promotion_user_coupon uc
            WHERE {coupon_scope}
              AND LOWER(uc.status) IN ('redeemed', 'used')
              AND {coupon_window}
            GROUP BY uc.offer_id
        ),
        settled AS (
            SELECT a.offer_id AS offer_id, a.currency_code AS currency_code,
                   COALESCE(SUM({amount}), 0)::BIGINT AS settled_minor
            FROM promotion_discount_application a
            WHERE {application_scope}
              AND LOWER(a.status) = 'settled'
              AND {application_window}
            GROUP BY a.offer_id, a.currency_code
        ),
        dominant AS (
            SELECT offer_id, currency_code, settled_minor,
                   ROW_NUMBER() OVER (
                       PARTITION BY offer_id
                       ORDER BY settled_minor DESC, currency_code ASC
                   ) AS currency_rank
            FROM settled
        )
        SELECT o.id AS offer_id, o.offer_no AS offer_no, o.display_name AS display_name,
               r.redeemed AS redeemed,
               COALESCE(d.settled_minor, 0)::BIGINT AS discount_settled_minor,
               COALESCE(d.currency_code, v.currency_code, '') AS currency_code
        FROM redeemed r
        JOIN promotion_offer o ON o.tenant_id = $1 AND o.organization_id = $2 AND o.id = r.offer_id
        LEFT JOIN dominant d ON d.offer_id = r.offer_id AND d.currency_rank = 1
        LEFT JOIN promotion_offer_version v
               ON v.tenant_id = o.tenant_id AND v.id = o.current_offer_version_id
        ORDER BY r.redeemed DESC, COALESCE(d.settled_minor, 0) DESC, o.id ASC
        LIMIT $5
        "#,
        coupon_scope = scope_predicate("uc"),
        application_scope = scope_predicate("a"),
    )
}

/// Runs the three report queries in one read-only transaction.
///
/// `from`/`to` are canonical RFC 3339 instants produced by the service layer's
/// [`crate::timestamps`] normalization; they are bound as `timestamptz`, never
/// spliced into the SQL text.
pub(crate) async fn retrieve_reports(
    pool: &PgPool,
    scope: &PromotionAdminScope,
    from: &str,
    to: &str,
) -> Result<PromotionReport, CommerceServiceError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| storage_error("begin promotion report", error))?;
    // A report is read-only by construction; the session cannot accidentally
    // mutate evidence while it is open.
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|error| storage_error("mark promotion report read-only", error))?;

    let tenant_id = scope.tenant_id.to_string();
    let organization_id = scope.organization_id.to_string();

    // The three statements are built once, so the exact SQL the tests inspect is
    // the SQL that runs.
    let counts_sql = counts_sql();
    let money_sql = money_sql();
    let offers_sql = top_offers_sql();

    let counts_row = sqlx::query(sqlx::AssertSqlSafe(counts_sql.as_str()))
        .bind(&tenant_id)
        .bind(&organization_id)
        .bind(from)
        .bind(to)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| storage_error("count promotion report rows", error))?;
    let counts = PromotionReportCounts {
        issued_codes: i64_column(&counts_row, "issued_codes")?,
        disabled_codes: i64_column(&counts_row, "disabled_codes")?,
        claimed_coupons: i64_column(&counts_row, "claimed_coupons")?,
        redeemed_coupons: i64_column(&counts_row, "redeemed_coupons")?,
        expired_coupons: i64_column(&counts_row, "expired_coupons")?,
        voided_coupons: i64_column(&counts_row, "voided_coupons")?,
        applications_applied: i64_column(&counts_row, "applications_applied")?,
        applications_settled: i64_column(&counts_row, "applications_settled")?,
        applications_released: i64_column(&counts_row, "applications_released")?,
    };

    let money_rows = sqlx::query(sqlx::AssertSqlSafe(money_sql.as_str()))
        .bind(&tenant_id)
        .bind(&organization_id)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| storage_error("sum promotion report money", error))?;
    let by_currency = money_rows
        .iter()
        .map(|row| {
            Ok(PromotionReportCurrencyRow {
                currency_code: string_column(row, "currency_code")?,
                discount_applied_minor: i64_column(row, "discount_applied_minor")?,
                discount_settled_minor: i64_column(row, "discount_settled_minor")?,
                budget_total_minor: i64_column(row, "budget_total_minor")?,
                budget_reserved_minor: i64_column(row, "budget_reserved_minor")?,
                budget_used_minor: i64_column(row, "budget_used_minor")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;

    let offer_rows = sqlx::query(sqlx::AssertSqlSafe(offers_sql.as_str()))
        .bind(&tenant_id)
        .bind(&organization_id)
        .bind(from)
        .bind(to)
        .bind(REPORT_TOP_OFFER_LIMIT)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| storage_error("rank promotion report offers", error))?;
    let top_offers = offer_rows
        .iter()
        .map(|row| {
            Ok(PromotionReportTopOffer {
                offer_id: string_column(row, "offer_id")?,
                offer_no: string_column(row, "offer_no")?,
                display_name: string_column(row, "display_name")?,
                redeemed: i64_column(row, "redeemed")?,
                discount_settled_minor: i64_column(row, "discount_settled_minor")?,
                currency_code: string_column(row, "currency_code")?,
            })
        })
        .collect::<Result<Vec<_>, CommerceServiceError>>()?;

    tx.commit()
        .await
        .map_err(|error| storage_error("commit promotion report", error))?;

    Ok(PromotionReport {
        from: from.to_owned(),
        to: to.to_owned(),
        // One definition of the rates, shared with every other caller.
        rates: report_rates(&counts),
        counts,
        by_currency,
        top_offers,
    })
}

fn i64_column(row: &PgRow, column: &str) -> Result<i64, CommerceServiceError> {
    row.try_get(column)
        .map_err(|error| decode_error(column, error))
}

fn string_column(row: &PgRow, column: &str) -> Result<String, CommerceServiceError> {
    row.try_get(column)
        .map_err(|error| decode_error(column, error))
}

fn storage_error(context: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("{context}: {error}"))
}

fn decode_error(column: &str, error: sqlx::Error) -> CommerceServiceError {
    CommerceServiceError::storage(format!("decode promotion report column {column}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `SUM(...)` must sit inside a `GROUP BY currency_code` block; a sum
    /// after the final grouping would silently total across currencies.
    fn assert_no_ungrouped_sum(sql: &str) {
        let last_group = sql.rfind("GROUP BY").expect("money query must group");
        assert!(
            !sql[last_group..].contains("SUM("),
            "no SUM may appear after the last GROUP BY: {sql}"
        );
    }

    #[test]
    fn counts_query_windows_every_table_through_the_instant_expression() {
        let sql = counts_sql();

        // Source scan: every window predicate is produced by the one helper that
        // wraps the column in the shared instant expression, and each windowed
        // column goes through it.
        let source = include_str!("reports.rs");
        assert!(source.contains("timestamps::instant(column)"));
        for window_column in ["c.created_at", "uc.claimed_at", "a.applied_at"] {
            assert!(
                source.contains(&format!("window_between(\"{window_column}\")")),
                "report counts must window {window_column} through the instant helper"
            );
        }

        // Built SQL: an instant comparison, never a text comparison.
        assert!(sql.contains("::timestamptz"));
        assert!(sql.contains("AT TIME ZONE 'UTC'"));
        for column in ["created_at", "claimed_at", "applied_at"] {
            assert!(
                !sql.contains(&format!("{column} >=")),
                "{column} must not be compared as text: {sql}"
            );
            assert!(
                !sql.contains(&format!("{column} <=")),
                "{column} must not be compared as text: {sql}"
            );
        }

        // Both bounds are inclusive, which is what `window_between` promises.
        assert!(sql.contains(">= CAST($3 AS timestamptz)"));
        assert!(sql.contains("<= CAST($4 AS timestamptz)"));

        // All nine counters are present exactly once.
        for counter in [
            "issued_codes",
            "disabled_codes",
            "claimed_coupons",
            "redeemed_coupons",
            "expired_coupons",
            "voided_coupons",
            "applications_applied",
            "applications_settled",
            "applications_released",
        ] {
            assert_eq!(
                sql.matches(&format!("AS {counter}")).count(),
                1,
                "{counter} must be counted once"
            );
        }
    }

    #[test]
    fn report_queries_keep_the_admin_list_tenant_and_organization_scoping() {
        let counts = counts_sql();
        for alias in ["c", "uc", "a"] {
            assert!(
                counts.contains(&format!("{alias}.tenant_id = $1")),
                "counts must scope {alias}.tenant_id"
            );
            assert!(
                counts.contains(&format!("{alias}.organization_id = $2")),
                "counts must scope {alias}.organization_id"
            );
        }

        let money = money_sql();
        for alias in ["a", "b"] {
            assert!(money.contains(&format!("{alias}.tenant_id = $1")));
            assert!(money.contains(&format!("{alias}.organization_id = $2")));
        }

        let offers = top_offers_sql();
        for alias in ["uc", "a"] {
            assert!(offers.contains(&format!("{alias}.tenant_id = $1")));
            assert!(offers.contains(&format!("{alias}.organization_id = $2")));
        }
        // The offer join is scoped too (a coupon can reference an offer id from
        // another tenant only through corrupt data).
        assert!(offers.contains("o.tenant_id = $1 AND o.organization_id = $2"));

        // The predicates stay on raw columns so the tenant-leading indexes remain
        // usable; wrapping them in a function would defeat the index prefix.
        for sql in [&counts, &money, &offers] {
            for alias in ["c", "uc", "a", "b", "o"] {
                assert!(!sql.contains(&format!("LOWER({alias}.organization_id)")));
                assert!(!sql.contains(&format!("CAST({alias}.organization_id")));
                assert!(!sql.contains(&format!("LOWER({alias}.tenant_id)")));
            }
        }
    }

    #[test]
    fn money_query_groups_by_currency_and_never_sums_across_currencies() {
        let sql = money_sql();

        // One group per source, both on the currency column.
        assert!(sql.contains("GROUP BY a.currency_code"));
        assert!(sql.contains("GROUP BY b.currency_code"));
        assert_eq!(sql.matches("GROUP BY").count(), 2);
        assert_no_ungrouped_sum(&sql);

        // The budget figures are the current snapshot: the budget CTE carries no
        // window bound at all.
        let budget_start = sql.find("FROM promotion_budget_account").expect("budget");
        let budget_end = budget_start
            + sql[budget_start..]
                .find("GROUP BY")
                .expect("budget grouping");
        assert!(
            !sql[budget_start..budget_end].contains("CAST($3"),
            "the budget snapshot must not be window-filtered: {sql}"
        );

        // The discount money *is* window-filtered through the instant expression.
        let applied_start = sql
            .find("FROM promotion_discount_application")
            .expect("applied");
        assert!(sql[applied_start..budget_start].contains(">= CAST($3 AS timestamptz)"));
        assert!(sql[applied_start..budget_start].contains("<= CAST($4 AS timestamptz)"));

        // Discount amounts are integer minor units; legacy decimal-major rows are
        // converted, never summed as text.
        assert!(sql.contains("BTRIM(a.discount_amount)::BIGINT"));
        assert!(sql.contains("NUMERIC * 100"));
        assert!(sql.contains("'settled'"));
    }

    #[test]
    fn top_offers_rank_by_windowed_redemptions_and_pin_one_currency_per_offer() {
        let sql = top_offers_sql();

        // Ranking: redeemed desc, settled amount desc, offer id asc.
        assert!(
            sql.contains("ORDER BY r.redeemed DESC, COALESCE(d.settled_minor, 0) DESC, o.id ASC")
        );
        assert!(
            sql.contains("LIMIT $5"),
            "the page size is bound, not spliced"
        );

        // Exactly one currency per offer: the dominant one is picked by settled
        // amount, and only that currency's amount is reported.
        assert!(sql.contains("ROW_NUMBER() OVER ("));
        assert!(sql.contains("PARTITION BY offer_id"));
        assert!(sql.contains("ORDER BY settled_minor DESC, currency_code ASC"));
        assert!(
            sql.contains("LEFT JOIN dominant d ON d.offer_id = r.offer_id AND d.currency_rank = 1")
        );
        assert!(sql.contains("COALESCE(d.currency_code, v.currency_code, '')"));
        assert!(sql.contains("LOWER(a.status) = 'settled'"));

        // The redemption count is windowed by `claimed_at`, the settled amount by
        // `applied_at`, both through the instant expression.
        assert!(sql.contains("uc.claimed_at"));
        assert!(sql.contains("a.applied_at"));
        assert!(sql.contains("AT TIME ZONE 'UTC'"));
        assert_eq!(sql.matches("<= CAST($4 AS timestamptz)").count(), 2);
    }

    #[test]
    fn report_reads_share_one_read_only_transaction() {
        let source = include_str!("reports.rs");
        // Scan the production half only: this test's own marker strings must not
        // be counted.
        let production = &source[..source.find("#[cfg(test)]").expect("test module")];
        assert!(production.contains("SET TRANSACTION READ ONLY"));
        assert!(production.contains(".begin()"));
        // Three focused queries, one transaction.
        assert_eq!(production.matches(".fetch_one(&mut *tx)").count(), 1);
        assert_eq!(production.matches(".fetch_all(&mut *tx)").count(), 2);
        assert_eq!(production.matches("tx.commit()").count(), 1);
    }
}
