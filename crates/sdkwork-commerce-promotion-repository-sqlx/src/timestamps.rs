//! Promotion time-window SQL instant handling.
//!
//! Promotion rows persist time windows (`starts_at`, `ends_at`, `expires_at`,
//! `claim_starts_at`, `claim_ends_at`, `valid_from`, ...) as `TEXT`. Two shapes
//! reach the database:
//!
//! * the canonical server form `YYYY-MM-DD HH:MM:SS` (UTC, produced by
//!   `current_timestamp_string` and by `created_at` / `updated_at`), and
//! * the RFC 3339 form the admin console sends, e.g.
//!   `2026-11-10T09:12:00.000Z`.
//!
//! Text comparison between those shapes is not chronological: `'T'` (0x54) sorts
//! after `' '` (0x20), so `starts_at <= '2026-11-10 14:09:44'` is FALSE for a
//! window that already started. That defect made every promotion code whose
//! offer carried a `starts_at` unredeemable (`409 promotion code is invalid or
//! unavailable`).
//!
//! The contract enforced here:
//!
//! 1. Every window column is written through [`canonical_window`], so new rows
//!    carry one RFC 3339 UTC shape. Unparseable input is rejected, never stored.
//! 2. Every window predicate compares instants through [`instant`] /
//!    [`window_started`] / [`window_not_ended`], never raw text.
//! 3. Every Rust-side window comparison goes through [`epoch_seconds`], never
//!    `str` ordering.
//!
//! Audit columns (`created_at`, `updated_at`, `claimed_at`, `redeemed_at`) stay
//! in the canonical server form because they are ordered and displayed, never
//! compared against a caller-supplied value. Reporting windows still compare
//! them through [`instant`] so a mixed-shape row is never mis-classified.
//!
//! The pure parser/formatter primitives live in the service crate
//! (`sdkwork_commerce_promotion_service::timestamps`) so the service layer can
//! validate and normalize a window before a transaction opens; this module
//! re-exports them and owns the SQL-side expression builders.

pub(crate) use sdkwork_commerce_promotion_service::timestamps::{
    epoch_seconds, format_iso_epoch, now_iso, server_form_to_iso, CANONICAL_SHAPE_HINT,
};

use sdkwork_contract_service::CommerceServiceError;

/// Normalizes a `TEXT` timestamp column to an absolute instant.
///
/// The expression keeps `NULL` / blank values as `NULL` (an absent window
/// bound) and returns the parsed instant for both accepted shapes. Values the
/// parser does not recognize are treated as absent rather than compared as
/// text, so a malformed row can never silently satisfy a text predicate.
pub(crate) fn instant(column: &str) -> String {
    format!(
        "(CASE \
             WHEN NULLIF(BTRIM({column}), '') IS NULL THEN NULL \
             WHEN BTRIM({column}) ~ '(Z|z|[+-][0-9]{{2}}:?[0-9]{{2}})$' \
                 THEN BTRIM({column})::timestamptz \
             WHEN BTRIM({column}) ~ '^[0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}[ T][0-9]{{2}}:[0-9]{{2}}' \
                 THEN BTRIM({column})::timestamp AT TIME ZONE 'UTC' \
             ELSE NULL \
         END)"
    )
}

/// Predicate: the window bound is absent or already reached.
pub(crate) fn window_started(column: &str) -> String {
    let instant = instant(column);
    format!("({instant} IS NULL OR {instant} <= NOW())")
}

/// Predicate: the window bound is absent or not yet passed.
pub(crate) fn window_not_ended(column: &str) -> String {
    let instant = instant(column);
    format!("({instant} IS NULL OR {instant} >= NOW())")
}

/// Normalizes a caller-supplied time window to the canonical RFC 3339 UTC shape.
///
/// Accepts the RFC 3339 shapes an admin console sends (with `Z` or a numeric
/// offset, optional fractional seconds) and the canonical server form
/// `YYYY-MM-DD HH:MM:SS`, which is treated as UTC. Blank input means "no bound".
pub(crate) fn canonical_window(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>, CommerceServiceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let epoch = epoch_seconds(value).ok_or_else(|| {
        CommerceServiceError::validation(format!(
            "{field} must be an RFC 3339 timestamp such as {CANONICAL_SHAPE_HINT}"
        ))
    })?;
    Ok(Some(format_iso_epoch(epoch)))
}

/// Normalizes a required caller-supplied instant (a wallet-coupon extension).
///
/// Same parser as [`canonical_window`], but an absent value is a client error
/// instead of "no bound": extending a coupon to *no* expiry is not a supported
/// operation.
pub(crate) fn required_instant(
    value: &str,
    field: &str,
) -> Result<String, CommerceServiceError> {
    canonical_window(Some(value), field)?.ok_or_else(|| {
        CommerceServiceError::validation(format!("{field} is required"))
    })
}

/// Maps the SQL-evaluated "is the new instant later than the database clock?"
/// predicate to the client error for an extension target.
///
/// The comparison itself is `(<canonical>)::timestamptz > CURRENT_TIMESTAMP`
/// inside the mutation statement — never a Rust `SystemTime` read — so a skewed
/// application process can never extend a coupon into the past. This helper only
/// turns the authoritative boolean into the wire error.
pub(crate) fn extension_future_decision(
    expires_in_future: bool,
) -> Result<(), CommerceServiceError> {
    if expires_in_future {
        Ok(())
    } else {
        Err(CommerceServiceError::validation(
            "expiresAt must be in the future",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_parses_both_promotion_shapes_to_the_same_instant() {
        let iso = epoch_seconds("2026-11-10T09:12:00.000Z").expect("iso shape");
        let canonical = epoch_seconds("2026-11-10 09:12:00").expect("canonical shape");
        assert_eq!(iso, canonical);
        assert_eq!(
            format_iso_epoch(iso),
            "2026-11-10T09:12:00.000Z",
            "both shapes must normalize to one canonical wire shape"
        );
    }

    #[test]
    fn canonical_window_normalizes_and_rejects() {
        assert_eq!(
            canonical_window(Some("2026-11-10T17:12:00+08:00"), "startsAt")
                .expect("valid window")
                .as_deref(),
            Some("2026-11-10T09:12:00.000Z")
        );
        assert_eq!(
            canonical_window(Some("   "), "startsAt").expect("blank window"),
            None
        );
        assert!(
            canonical_window(Some("2026/11/10 09:12"), "startsAt").is_err(),
            "unparseable windows must be rejected instead of stored as text"
        );
    }

    #[test]
    fn extension_validation_rejects_past_and_unparseable_targets() {
        // Shape: an unparseable `expiresAt` is a client error, never stored.
        let error = required_instant("not-a-time", "expiresAt").expect_err("unparseable");
        assert_eq!("validation", error.code());
        // Both accepted promotion shapes normalize to the canonical RFC 3339
        // shape, so a later comparison is an instant comparison.
        assert_eq!(
            required_instant("2026-11-10 17:12:00", "expiresAt").expect("server form"),
            "2026-11-10T17:12:00.000Z"
        );
        assert_eq!(
            required_instant("2026-11-10T17:12:00+08:00", "expiresAt").expect("rfc 3339"),
            "2026-11-10T09:12:00.000Z"
        );

        // Future: the database evaluated `new_expires_at > CURRENT_TIMESTAMP`.
        // A past target is rejected by the SQL predicate, and this is the pure
        // mapping of that authoritative answer.
        let error = extension_future_decision(false).expect_err("past target");
        assert_eq!("validation", error.code());
        assert!(error.message().contains("future"), "{error:?}");
        assert!(extension_future_decision(true).is_ok());
    }

    #[test]
    fn text_ordering_across_shapes_is_not_chronological() {
        // The regression this module exists to prevent.
        assert!(("2026-11-10T09:12:00.000Z" <= "2026-11-10 14:09:44") == false);
        assert!(
            epoch_seconds("2026-11-10T09:12:00.000Z").expect("iso")
                < epoch_seconds("2026-11-10 14:09:44").expect("canonical")
        );
    }

    #[test]
    fn instant_expression_is_an_instant_comparison_not_a_text_comparison() {
        let sql = window_started("pc.starts_at");
        assert!(sql.contains("::timestamptz"), "{sql}");
        assert!(sql.contains("AT TIME ZONE 'UTC'"), "{sql}");
        assert!(sql.contains("<= NOW()"), "{sql}");
    }

    #[test]
    fn report_window_normalization_is_shared_with_the_service_layer() {
        // The repository re-exports the service parser, so a report bound that
        // the service accepts is exactly the shape [`canonical_window`] stores.
        let (from, to) = sdkwork_commerce_promotion_service::timestamps::report_window(
            Some("2026-11-10 09:12:00"),
            Some("2026-11-10T18:00:00+08:00"),
            "2026-11-11 02:44:37",
        )
        .expect("window");
        assert_eq!(from, "2026-11-10T09:12:00.000Z");
        assert_eq!(to, "2026-11-10T10:00:00.000Z");
        // Both canonical instants are exactly what `canonical_window` produces,
        // so the bound stored on a row and the bound compared by a report use
        // one shape.
        assert_eq!(
            canonical_window(Some(from.as_str()), "from").expect("canonical from"),
            Some(from)
        );
        assert_eq!(
            canonical_window(Some(to.as_str()), "to").expect("canonical to"),
            Some(to)
        );
    }
}
