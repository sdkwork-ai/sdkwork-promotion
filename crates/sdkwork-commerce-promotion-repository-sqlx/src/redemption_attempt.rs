//! Evidence ledger for promotion-code redemption attempts.
//!
//! Every redemption attempt is written to `promotion_code_redemption_attempt`,
//! whether it succeeded or failed, so support can answer "why was this code
//! rejected" without reading application logs.
//!
//! The two halves have very different transaction semantics and live here side
//! by side so the difference stays visible:
//!
//! * [`insert_redemption_attempt_in_tx`] runs **inside** the redemption
//!   transaction, so a successful attempt is atomic with the redemption it
//!   describes. If the redemption rolls back, the attempt disappears with it.
//! * [`record_redemption_failure_attempt`] runs **after** that transaction has
//!   been dropped (i.e. rolled back) on its own auto-commit connection, because
//!   a row written inside the failing transaction would be rolled back too and
//!   the failure would never be observable.
//!
//! The table must never store a redeemable secret: only the SHA-256 hash and the
//! last four characters of the submitted code are persisted (see
//! [`submitted_code_hash`] and [`submitted_code_suffix`]).

use sdkwork_commerce_promotion_service::PromotionCodeRedemptionCommand;
use sdkwork_contract_service::{CommerceServiceError, CommerceServiceErrorKind};
use sqlx::{PgPool, Postgres, Transaction};

use crate::postgres_promotion::organization_binding;

/// `outcome` value for a redemption that completed and granted the benefit.
pub(crate) const OUTCOME_SUCCEEDED: &str = "succeeded";
/// `outcome` value for a redemption that was rejected.
pub(crate) const OUTCOME_FAILED: &str = "failed";

/// `channel` value for a code redeemed directly by the customer.
pub(crate) const CHANNEL_CODE: &str = "code";
/// `channel` value for a code that only reached the read-only preview path.
pub(crate) const CHANNEL_PREVIEW: &str = "preview";

pub(crate) const FAILURE_CODE_UNAVAILABLE: &str = "code_unavailable";
pub(crate) const FAILURE_CODE_ISSUE_LIMIT: &str = "issue_limit_reached";
pub(crate) const FAILURE_CODE_PER_USER_LIMIT: &str = "per_user_limit_reached";
pub(crate) const FAILURE_CODE_IDEMPOTENCY_CONFLICT: &str = "idempotency_conflict";
pub(crate) const FAILURE_CODE_REDEMPTION_IN_PROGRESS: &str = "redemption_in_progress";
pub(crate) const FAILURE_CODE_BENEFIT_INVALID: &str = "benefit_invalid";
pub(crate) const FAILURE_CODE_INTERNAL_ERROR: &str = "internal_error";

/// `promotion_code_redemption_attempt.failure_code` is a closed taxonomy. It is
/// classified in exactly one place — [`classify_redemption_failure`] — so that
/// renaming a rejection message elsewhere in this crate requires editing exactly
/// one table, and the exhaustive test below fails until that table agrees with
/// the source again.
const REDEMPTION_REJECTION_MESSAGES: &[(&str, &str)] = &[
    // `load_promotion_for_redeem`: no active, in-window code matched.
    (
        "promotion code is invalid or unavailable",
        FAILURE_CODE_UNAVAILABLE,
    ),
    // `ensure_promotion_can_be_redeemed`: stock quantity or per-code claim cap.
    (
        "promotion code has reached its issue limit",
        FAILURE_CODE_ISSUE_LIMIT,
    ),
    // `ensure_promotion_can_be_redeemed`: the subject already received its cap.
    (
        "promotion code subject receive limit has been reached",
        FAILURE_CODE_PER_USER_LIMIT,
    ),
    // Replaying one idempotency key with a different request/body.
    (
        "idempotency key was used with a different promotion code redemption request",
        FAILURE_CODE_IDEMPOTENCY_CONFLICT,
    ),
    // Order-side replay of an already-redeemed coupon with a different code.
    (
        "coupon redemption request was replayed with a different code",
        FAILURE_CODE_IDEMPOTENCY_CONFLICT,
    ),
    // Another request currently holds the idempotency lock.
    (
        "another promotion code redemption request is in progress for this idempotency key",
        FAILURE_CODE_REDEMPTION_IN_PROGRESS,
    ),
];

/// Classifies a redemption rejection into the `failure_code` taxonomy.
///
/// Two families are intentionally message-based: the redemption guards are the
/// only producers of those exact messages, and those messages are already the
/// contract the HTTP layer shows. Everything else is classified by error kind:
/// any `validation` rejection raised inside the redemption transaction comes
/// from the coupon benefit (`rule_json` parse or benefit application), while
/// storage/transport/state failures are internal errors that the operator
/// cannot fix from the code.
pub(crate) fn classify_redemption_failure(error: &CommerceServiceError) -> &'static str {
    let message = error.message();
    if let Some((_, code)) = REDEMPTION_REJECTION_MESSAGES
        .iter()
        .find(|(text, _)| *text == message)
    {
        return code;
    }
    if matches!(error.kind(), CommerceServiceErrorKind::Validation) {
        return FAILURE_CODE_BENEFIT_INVALID;
    }
    FAILURE_CODE_INTERNAL_ERROR
}

/// SHA-256 hex of the normalized submitted code.
///
/// Normalization is `UPPER(BTRIM(value))`, matching the redemption lookup
/// (`UPPER(BTRIM(pc.promotion_code)) = UPPER(BTRIM($3))` in the documented
/// fallback) and the canonical form admin-defined codes are stored in. The
/// derivation itself lives in [`crate::code_identity::code_identity`], the one
/// function that also produces `promotion_code.code_hash`, so an attempt's hash
/// and the row's indexed hash can never disagree. Promotion codes are restricted
/// to ASCII alphanumerics plus `-`/`_`, so ASCII case folding is the whole story.
pub(crate) fn submitted_code_hash(submitted_code: &str) -> String {
    crate::code_identity::code_identity(submitted_code).hash
}

/// Last four characters of the trimmed submitted code.
///
/// Never the whole value: this table is readable by support staff and must not
/// contain a redeemable code. Four characters are enough to confirm "the
/// customer typed the card we printed" without being enough to redeem. Shared
/// with `promotion_code.code_last4` through
/// [`crate::code_identity::code_last4`].
pub(crate) fn submitted_code_suffix(submitted_code: &str) -> String {
    crate::code_identity::code_last4(submitted_code)
}

/// Truncates a rejection message to `failure_detail VARCHAR(512)`.
///
/// Truncation counts characters, not bytes, so a multi-byte message cannot be
/// cut in the middle of a character (which PostgreSQL would reject).
fn failure_detail(message: &str) -> String {
    message.chars().take(512).collect()
}

/// The code/stock/offer a submission resolved to.
///
/// A failed redemption rolls its transaction back, so these identifiers cannot
/// be read back from the rolled-back rows afterwards; the caller captures them
/// in a variable it still owns once the transaction is gone, and hands them to
/// [`record_redemption_failure_attempt`].
#[derive(Debug, Clone)]
pub(crate) struct ResolvedRedemptionTarget {
    pub code_id: String,
    pub stock_id: String,
    pub offer_id: String,
}

/// One row of `promotion_code_redemption_attempt`.
///
/// Note the absence of any field carrying the submitted code: the record only
/// ever holds the hash and the suffix, so no insert path can leak the secret.
pub(crate) struct RedemptionAttemptRecord<'a> {
    pub tenant_id: &'a str,
    /// Always sentinel-bound: `organization_id` is `TEXT NOT NULL DEFAULT '0'`
    /// and an explicit SQL `NULL` does not fall back to that default, it aborts
    /// the insert. The type keeps `None` from ever reaching the bind.
    pub organization_id: &'a str,
    pub code_id: Option<&'a str>,
    pub stock_id: Option<&'a str>,
    pub offer_id: Option<&'a str>,
    pub user_coupon_id: Option<&'a str>,
    pub order_id: Option<&'a str>,
    pub submitted_code_hash: &'a str,
    pub submitted_code_suffix: &'a str,
    pub subject_type: &'a str,
    pub subject_id: &'a str,
    pub channel: &'a str,
    pub outcome: &'a str,
    pub failure_code: Option<&'a str>,
    pub failure_detail: Option<&'a str>,
    pub benefit_kind: Option<&'a str>,
    pub granted_amount: Option<&'a str>,
    pub currency_code: Option<&'a str>,
    pub request_no: &'a str,
    pub idempotency_key: &'a str,
    pub trace_id: &'a str,
    pub occurred_at: &'a str,
}

const INSERT_ATTEMPT_SQL: &str = r#"
INSERT INTO promotion_code_redemption_attempt
    (id, uuid, tenant_id, organization_id, code_id, stock_id, offer_id,
     user_coupon_id, order_id, submitted_code_hash, submitted_code_suffix,
     subject_type, subject_id, channel, outcome, failure_code, failure_detail,
     benefit_kind, granted_amount, currency_code, request_no, idempotency_key,
     trace_id, occurred_at, created_at)
VALUES
    ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
     $18, $19, $20, $21, $22, $23, $24, $24)
"#;

fn bind_attempt<'q>(
    query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    record: &'q RedemptionAttemptRecord<'q>,
) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(sdkwork_utils_rust::uuid())
        .bind(sdkwork_utils_rust::uuid())
        .bind(record.tenant_id)
        .bind(record.organization_id)
        .bind(record.code_id)
        .bind(record.stock_id)
        .bind(record.offer_id)
        .bind(record.user_coupon_id)
        .bind(record.order_id)
        .bind(record.submitted_code_hash)
        .bind(record.submitted_code_suffix)
        .bind(record.subject_type)
        .bind(record.subject_id)
        .bind(record.channel)
        .bind(record.outcome)
        .bind(record.failure_code)
        .bind(record.failure_detail)
        .bind(record.benefit_kind)
        .bind(record.granted_amount)
        .bind(record.currency_code)
        .bind(record.request_no)
        .bind(record.idempotency_key)
        .bind(record.trace_id)
        .bind(record.occurred_at)
}

/// Records a **successful** attempt inside the redemption transaction.
///
/// This must stay inside the transaction: the attempt and the redemption it
/// describes are one atomic unit, and a rolled-back redemption must not leave a
/// "succeeded" attempt behind.
pub(crate) async fn insert_redemption_attempt_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    record: &RedemptionAttemptRecord<'_>,
) -> Result<(), CommerceServiceError> {
    bind_attempt(sqlx::query(INSERT_ATTEMPT_SQL), record)
        .execute(&mut **tx)
        .await
        .map_err(|error| {
            CommerceServiceError::storage(format!(
                "failed to record promotion code redemption attempt: {error}"
            ))
        })?;
    Ok(())
}

/// Records a **failed** attempt after the redemption transaction rolled back.
///
/// Runs on a separate pooled connection in auto-commit mode: the calling
/// function owns no transaction here, and the redemption transaction has already
/// been dropped, so this row survives the rollback.
///
/// A write failure must never replace the business rejection the client is about
/// to receive, so the error is logged and swallowed — the original
/// `CommerceServiceError` still travels back to the caller unchanged.
pub(crate) async fn record_redemption_failure_attempt(
    pool: &PgPool,
    command: &PromotionCodeRedemptionCommand,
    resolved: Option<&ResolvedRedemptionTarget>,
    channel: &str,
    subject_type: &str,
    occurred_at: &str,
    error: &CommerceServiceError,
) {
    let detail = failure_detail(error.message());
    let record = RedemptionAttemptRecord {
        tenant_id: &command.tenant_id,
        organization_id: organization_binding(command.organization_id.as_deref()),
        code_id: resolved.map(|target| target.code_id.as_str()),
        stock_id: resolved.map(|target| target.stock_id.as_str()),
        offer_id: resolved.map(|target| target.offer_id.as_str()),
        // The wallet coupon row is rolled back with the redemption.
        user_coupon_id: None,
        order_id: None,
        submitted_code_hash: &submitted_code_hash(&command.code),
        submitted_code_suffix: &submitted_code_suffix(&command.code),
        subject_type,
        subject_id: &command.owner_user_id,
        channel,
        outcome: OUTCOME_FAILED,
        failure_code: Some(classify_redemption_failure(error)),
        failure_detail: Some(&detail),
        benefit_kind: None,
        granted_amount: None,
        currency_code: None,
        request_no: &command.request_no,
        idempotency_key: &command.idempotency_key,
        // `PromotionCodeRedemptionCommand` carries no trace id yet; the column
        // defaults to '' and the read API exposes it as-is.
        trace_id: "",
        occurred_at,
    };
    if let Err(write_error) = bind_attempt(sqlx::query(INSERT_ATTEMPT_SQL), &record)
        .execute(pool)
        .await
    {
        tracing::error!(
            operation = "record_redemption_failure_attempt",
            error = %write_error,
            "failed to record promotion code redemption failure attempt; keeping the original rejection"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Table-driven classification: every rejection message the redemption path
    /// can raise maps to the documented failure code, and each documented code is
    /// reachable (no dead taxonomy entry).
    #[test]
    fn every_redemption_rejection_maps_to_its_documented_failure_code() {
        let source = include_str!("postgres_promotion.rs");

        let expected: &[(&str, &str)] = &[
            (
                "promotion code is invalid or unavailable",
                FAILURE_CODE_UNAVAILABLE,
            ),
            (
                "promotion code has reached its issue limit",
                FAILURE_CODE_ISSUE_LIMIT,
            ),
            (
                "promotion code subject receive limit has been reached",
                FAILURE_CODE_PER_USER_LIMIT,
            ),
            (
                "idempotency key was used with a different promotion code redemption request",
                FAILURE_CODE_IDEMPOTENCY_CONFLICT,
            ),
            (
                "coupon redemption request was replayed with a different code",
                FAILURE_CODE_IDEMPOTENCY_CONFLICT,
            ),
            (
                "another promotion code redemption request is in progress for this idempotency key",
                FAILURE_CODE_REDEMPTION_IN_PROGRESS,
            ),
        ];
        // The table above is the whole taxonomy: no entry may be dropped or
        // invented without updating the contract and this assertion.
        assert_eq!(REDEMPTION_REJECTION_MESSAGES, expected);
        for (message, code) in expected {
            // The messages must still exist in the redemption source. A rename
            // there without a rename here would silently degrade every failure
            // to `internal_error`.
            assert!(
                source.contains(message),
                "rejection message is no longer raised by the redemption path: {message}"
            );
            let error = CommerceServiceError::conflict(*message);
            assert_eq!(&classify_redemption_failure(&error), code, "{message}");
        }

        // A benefit that cannot be parsed/applied is a `validation` rejection
        // from inside the redemption transaction.
        let benefit_error = CommerceServiceError::validation(
            "promotion coupon benefit grantAmount must be a positive integer",
        );
        assert_eq!(
            classify_redemption_failure(&benefit_error),
            FAILURE_CODE_BENEFIT_INVALID
        );

        // Everything else — storage faults, locks, unknown states — is internal.
        for error in [
            CommerceServiceError::storage("failed to load promotion code: connection reset"),
            CommerceServiceError::locked("row is locked"),
            CommerceServiceError::invalid_state("promotion idempotency record has no response"),
            CommerceServiceError::transport("upstream unavailable"),
            CommerceServiceError::unknown("boom"),
        ] {
            assert_eq!(
                classify_redemption_failure(&error),
                FAILURE_CODE_INTERNAL_ERROR,
                "{}",
                error.message()
            );
        }
    }

    #[test]
    fn submitted_code_hash_and_suffix_never_carry_the_redeemable_value() {
        // A padded, lower-case paste normalizes to the same evidence as the
        // upper-case value the admin console printed.
        let canonical = submitted_code_hash("WELCOME2026");
        assert_eq!(submitted_code_hash("welcome2026"), canonical);
        assert_eq!(submitted_code_hash("  Welcome2026\t"), canonical);
        // sha256 hex of "WELCOME2026".
        assert_eq!(
            canonical,
            "5edf7240eae6a7f88a96900821c406054d692926b364a393c529ba5f6cf457d7"
        );
        assert_eq!(canonical.len(), 64);
        assert!(canonical
            .chars()
            .all(|character| character.is_ascii_hexdigit()));

        assert_eq!(submitted_code_suffix("  welcome2026  "), "2026");
        assert_eq!(submitted_code_suffix("WELCOME2026"), "2026");
        // Shorter values degrade to the whole (short) trimmed value rather than
        // panicking on a missing four-character tail.
        assert_eq!(submitted_code_suffix(" ab "), "ab");
        // The suffix is never the full redeemable code for a realistic code.
        let code = "ABCDEFGH1234";
        assert_ne!(submitted_code_suffix(code), code);
        assert_eq!(submitted_code_suffix(code).chars().count(), 4);
    }

    #[test]
    fn attempt_insert_writes_only_hash_and_suffix_for_the_submitted_code() {
        let source = include_str!("redemption_attempt.rs");
        // Exactly one insert path into the evidence table. The needle is
        // assembled at runtime so this assertion cannot match its own source.
        let insert_marker = ["INSERT INTO ", "promotion_code_redemption_attempt"].concat();
        assert_eq!(source.matches(insert_marker.as_str()).count(), 1);
        let insert = source
            .find("const INSERT_ATTEMPT_SQL")
            .expect("attempt insert statement");
        let rest = &source[insert..];
        let statement = &rest[..rest.find("\"#;").expect("end of insert statement")];
        assert!(statement.contains("submitted_code_hash"));
        assert!(statement.contains("submitted_code_suffix"));
        // The raw submitted value has no column at all.
        assert!(!statement.contains("submitted_code,"));
        assert!(!statement.contains("submitted_code)"));

        // The bind chain binds the derived values, never `command.code`.
        let bind_start = source.find("fn bind_attempt").expect("attempt bind helper");
        let bind_body = &source[bind_start..];
        let bind_end = bind_body.find("\n}\n").unwrap_or(bind_body.len());
        let binds = &bind_body[..bind_end];
        assert!(binds.contains("record.submitted_code_hash"));
        assert!(binds.contains("record.submitted_code_suffix"));
        assert!(!binds.contains("command.code"));

        // The record itself has no raw-code field.
        let record_start = source
            .find("pub(crate) struct RedemptionAttemptRecord")
            .expect("attempt record");
        let record = &source[record_start..];
        let record_end = record.find("\n}\n").unwrap_or(record.len());
        assert!(!record[..record_end].contains("submitted_code:"));
        assert!(!record[..record_end].contains("code: &'a str"));
    }

    #[test]
    fn failure_detail_truncates_to_the_column_width_in_characters() {
        let short = "promotion code is invalid or unavailable";
        assert_eq!(failure_detail(short), short);

        let long = "错".repeat(600);
        let truncated = failure_detail(&long);
        assert_eq!(truncated.chars().count(), 512);
        // 512 characters, 1536 bytes: the column counts characters, so a byte
        // truncation would have produced half a message and a DB error.
        assert_eq!(truncated.len(), 512 * 3);
    }

    /// Body of one method, from its `fn` marker to the next method or doc block.
    fn method_body<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("missing method: {marker}"));
        let rest = &source[start..];
        let end = rest
            .match_indices("\n    ")
            .map(|(offset, _)| offset)
            .find(|offset| {
                let tail = &rest[offset + 1..];
                tail.starts_with("    ///")
                    || tail.starts_with("    pub async fn ")
                    || tail.starts_with("    pub fn ")
                    || tail.starts_with("    async fn ")
            })
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn success_attempt_is_written_inside_the_redemption_transaction() {
        let source = include_str!("postgres_promotion.rs");
        let body = method_body(source, "async fn redeem_promotion_code_in_tx(");

        let begin = body
            .find("let mut tx = self.pool.begin()")
            .expect("redemption transaction");
        // The idempotent-replay branch commits early, so the *last* commit is the
        // one that ends the redemption unit of work.
        let commit = body.rfind("tx.commit()").expect("redemption commit");
        let insert = body
            .find("insert_redemption_attempt_in_tx(")
            .expect("success attempt insert");
        assert!(
            begin < insert && insert < commit,
            "the success attempt must be written between `let mut tx = ...` and `tx.commit()`:\n{body}"
        );
        assert!(body.contains("outcome: OUTCOME_SUCCEEDED"));
        assert!(body.contains("failure_code: None"));
        // The resolved ids the attempt names come from the locked promotion row.
        assert!(body.contains("code_id: Some(&promotion.code_id)"));
        assert!(body.contains("stock_id: Some(&promotion.stock_id)"));
        assert!(body.contains("offer_id: Some(&promotion.offer_id)"));
        assert!(body.contains("user_coupon_id: Some(&coupon_id)"));

        // The wrapper must not hold a second transaction of its own.
        let wrapper = method_body(source, "pub async fn redeem_promotion_code(");
        assert!(!wrapper.contains("self.pool.begin()"));
    }

    #[test]
    fn failure_attempt_is_written_after_the_redemption_transaction_is_dropped() {
        let source = include_str!("postgres_promotion.rs");
        let wrapper = method_body(source, "pub async fn redeem_promotion_code(");

        let inner = wrapper
            .find("redeem_promotion_code_in_tx(")
            .expect("inner redemption call");
        let recorder = wrapper
            .find("record_redemption_failure_attempt(")
            .expect("failure attempt recorder");
        assert!(
            inner < recorder,
            "the failure recorder must run after the inner function returned and released (rolled back) the transaction:\n{wrapper}"
        );
        // The wrapper owns no transaction: the inner function owns and releases
        // it, so the recorder's insert cannot land inside the rolled-back unit.
        assert!(!wrapper.contains("self.pool.begin()"));
        assert!(!wrapper.contains("tx.commit()"));
        assert!(!wrapper.contains("&mut tx"));
        // The evidence row is attributed to the code that was resolved before the
        // rejection, and the original error is returned unchanged.
        assert!(wrapper.contains("resolved.as_ref()"));
        assert!(wrapper.contains("CHANNEL_CODE"));
        assert!(wrapper.contains("Err(error)"));
        assert!(!wrapper.contains("map_err"));

        // The recorder itself executes on the pool (auto-commit), never on a
        // transaction handle.
        let recorder_source = include_str!("redemption_attempt.rs");
        let recorder_start = recorder_source
            .find("pub(crate) async fn record_redemption_failure_attempt")
            .expect("failure recorder definition");
        let recorder_rest = &recorder_source[recorder_start..];
        let recorder_body =
            &recorder_rest[..recorder_rest.find("\n}\n").unwrap_or(recorder_rest.len())];
        assert!(recorder_body.contains(".execute(pool)"));
        assert!(!recorder_body.contains("Transaction"));
        assert!(recorder_body.contains("tracing::error!"));
        assert!(recorder_body.contains("outcome: OUTCOME_FAILED"));
        assert!(recorder_body.contains("user_coupon_id: None"));
    }

    #[test]
    fn attempt_rows_bind_the_platform_organization_sentinel_instead_of_null() {
        let source = include_str!("redemption_attempt.rs");
        // `organization_id` is TEXT NOT NULL DEFAULT '0'; an explicit SQL NULL
        // does not fall back to the default, it aborts the insert.
        let record_start = source
            .find("pub(crate) struct RedemptionAttemptRecord")
            .expect("attempt record");
        let record = &source[record_start..];
        let record_end = record.find("\n}\n").unwrap_or(record.len());
        assert!(
            record[..record_end].contains("pub organization_id: &'a str,"),
            "the record must carry an already-bound organization, never an Option"
        );
        assert!(!record[..record_end].contains("organization_id: Option"));

        let bind_start = source.find("fn bind_attempt").expect("attempt bind helper");
        let bind_body = &source[bind_start..];
        let bind_end = bind_body.find("\n}\n").unwrap_or(bind_body.len());
        let binds = &bind_body[..bind_end];
        assert!(binds.contains(".bind(record.organization_id)"));
        assert!(!binds.contains("organization_id.as_deref()"));

        // Both record builders (success and failure) go through the sentinel
        // helper shared with every other promotion insert. The needle is
        // assembled at runtime so this assertion cannot match its own source.
        let sentinel_call = ["organization_", "binding("].concat();
        assert_eq!(source.matches(sentinel_call.as_str()).count(), 2);
        let promotion_source = include_str!("postgres_promotion.rs");
        assert!(promotion_source.contains("organization_id: organization_binding("));
    }

    /// Columns of one `CREATE TABLE IF NOT EXISTS <table> ( ... )` block, in
    /// declaration order, excluding table constraints.
    fn create_table_columns(sql: &str, table: &str) -> Vec<String> {
        let marker = format!("CREATE TABLE IF NOT EXISTS {table} (");
        let start = sql
            .find(&marker)
            .unwrap_or_else(|| panic!("missing CREATE TABLE for {table}"))
            + marker.len();
        let rest = &sql[start..];
        let end = rest
            .find("\n);")
            .unwrap_or_else(|| panic!("unterminated CREATE TABLE for {table}"));
        rest[..end]
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.starts_with("CONSTRAINT ") && !line.starts_with("--"))
            .map(|line| {
                line.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches(',')
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn attempt_table_matches_between_the_baseline_and_its_migration() {
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");
        let migration = include_str!(
            "../../../database/migrations/postgres/0003_coupon_redemption_evidence.up.sql"
        );

        let baseline_columns = create_table_columns(baseline, "promotion_code_redemption_attempt");
        let migration_columns =
            create_table_columns(migration, "promotion_code_redemption_attempt");
        assert_eq!(
            baseline_columns, migration_columns,
            "the folded baseline and the 0003 migration must declare the same columns"
        );
        let expected = [
            "id",
            "uuid",
            "tenant_id",
            "organization_id",
            "code_id",
            "stock_id",
            "offer_id",
            "user_coupon_id",
            "order_id",
            "submitted_code_hash",
            "submitted_code_suffix",
            "subject_type",
            "subject_id",
            "channel",
            "outcome",
            "failure_code",
            "failure_detail",
            "benefit_kind",
            "granted_amount",
            "currency_code",
            "request_no",
            "idempotency_key",
            "trace_id",
            "occurred_at",
            "created_at",
        ];
        assert_eq!(baseline_columns, expected, "unexpected attempt column set");
        assert!(
            !baseline_columns
                .iter()
                .any(|column| column == "promotion_code"),
            "the attempt table must not store the redeemable code"
        );

        // Both halves of the migration (and its folded copy) must carry the
        // evidence indexes, including the two design fixes.
        for index in [
            "idx_pca_code",
            "idx_pca_hash",
            "idx_pca_subject",
            "idx_pca_failure",
            "idx_promotion_coupon_ledger_entry_org_created",
        ] {
            assert!(baseline.contains(index), "baseline is missing {index}");
            assert!(
                migration.contains(index),
                "migration 0003 is missing {index}"
            );
        }
        // The normalized expression index this migration once added is
        // superseded by the 0007 unique hash index: 0003 keeps the explanatory
        // note only, and 0007 drops the index where an earlier run created it.
        let normalized_index = ["idx_promotion_code_", "value_normalized"].concat();
        let create_normalized = ["CREATE INDEX IF NOT EXISTS ", normalized_index.as_str()].concat();
        assert!(
            !migration.contains(create_normalized.as_str()),
            "0003 must not create the superseded normalized index: {create_normalized}"
        );
        assert!(
            !baseline.contains(create_normalized.as_str()),
            "the folded baseline must not create the superseded normalized index: {create_normalized}"
        );
        assert!(migration.contains("Superseded by `0007_code_hash`"));
        // The ledger scope column is added with the NOT NULL sentinel so the
        // framework's organization-id contract still holds.
        assert!(baseline.contains(
            "promotion_coupon_ledger_entry ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0'"
        ));
        assert!(migration.contains(
            "promotion_coupon_ledger_entry ADD COLUMN IF NOT EXISTS organization_id TEXT NOT NULL DEFAULT '0'"
        ));
    }
}
