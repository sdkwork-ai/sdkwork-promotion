//! Pure promotion timestamp normalization.
//!
//! Promotion rows persist time values as `TEXT` in two historical shapes:
//!
//! * the canonical server form `YYYY-MM-DD HH:MM:SS` (UTC, produced by
//!   `current_timestamp_string` and by `created_at` / `updated_at`), and
//! * the RFC 3339 form the admin console sends, e.g.
//!   `2026-11-10T09:12:00.000Z`.
//!
//! Text comparison between those shapes is not chronological: `'T'` (0x54) sorts
//! after `' '` (0x20), so `starts_at <= '2026-11-10 14:09:44'` is FALSE for a
//! window that already started. Every ordering decision therefore parses to an
//! absolute instant first.
//!
//! This module is the single parser/formatter boundary. It is pure (no clock
//! read unless the caller asks for [`now_iso`], no I/O) so the service can
//! normalize and order a reporting window *before* the repository opens a
//! transaction, and the repository's SQL instant builders can share exactly the
//! same acceptance rules. The SQL-side expression builders live in
//! `sdkwork-commerce-promotion-repository-sqlx::timestamps`, which re-exports
//! these helpers.

use sdkwork_contract_service::CommerceServiceError;

/// Upper bound accepted for a promotion time window (year 9999).
const MAX_EPOCH_SECONDS: i64 = 253_402_300_799;

/// Canonical RFC 3339 UTC shape used for every promotion time-window value.
pub const CANONICAL_SHAPE_HINT: &str = "YYYY-MM-DDTHH:MM:SS.sssZ";

/// Default reporting window: the last 30 days ending at the current instant.
pub const REPORT_DEFAULT_WINDOW_SECONDS: i64 = 30 * 86_400;

/// Normalizes a caller-supplied reporting window to canonical RFC 3339 UTC.
///
/// Both bounds are optional; an absent or blank bound falls back to the default
/// window, which is the last 30 days ending at `now`:
///
/// * `to` defaults to `now`,
/// * `from` defaults to `to - 30 days`.
///
/// The returned pair is `(from, to)`, both in the canonical
/// `YYYY-MM-DDTHH:MM:SS.sssZ` shape. `from == to` is valid (a one-instant,
/// inclusive window); only `from > to` is rejected, and it is rejected by
/// comparing parsed instants, never the raw text of the two values.
///
/// `now` is the authoritative current instant supplied by the caller (the
/// service reads the clock once), so this helper stays pure and unit-testable.
#[must_use = "the normalized window is the value the report must filter on"]
pub fn report_window(
    from: Option<&str>,
    to: Option<&str>,
    now: &str,
) -> Result<(String, String), CommerceServiceError> {
    let now_epoch = epoch_seconds(now).ok_or_else(|| {
        CommerceServiceError::storage("promotion report clock is not an RFC 3339 instant")
    })?;
    let to_epoch = match window_bound("to", to)? {
        Some(epoch) => epoch,
        None => now_epoch,
    };
    let from_epoch = match window_bound("from", from)? {
        Some(epoch) => epoch,
        // Anchored on `to`, not on `now`: an explicit `to` with no `from` still
        // asks for the 30 days that end at `to`.
        None => to_epoch.saturating_sub(REPORT_DEFAULT_WINDOW_SECONDS),
    };
    if from_epoch > to_epoch {
        return Err(CommerceServiceError::validation(
            "from must not be after to",
        ));
    }
    Ok((format_iso_epoch(from_epoch), format_iso_epoch(to_epoch)))
}

/// Parses one optional reporting bound, treating blank input as absent.
fn window_bound(field: &str, value: Option<&str>) -> Result<Option<i64>, CommerceServiceError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    epoch_seconds(value).map(Some).ok_or_else(|| {
        CommerceServiceError::validation(format!(
            "{field} must be an RFC 3339 timestamp such as {CANONICAL_SHAPE_HINT}"
        ))
    })
}

/// Parses any accepted promotion timestamp shape into epoch seconds (UTC).
pub fn epoch_seconds(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let split = value.find(['T', ' '])?;
    let (date, rest) = value.split_at(split);
    let rest = &rest[1..];

    let mut date_parts = date.split('-');
    let year = parse_fixed(date_parts.next()?, 4)?;
    let month = parse_fixed(date_parts.next()?, 2)?;
    let day = parse_fixed(date_parts.next()?, 2)?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if civil_from_days(days) != (year, month, day) {
        // Rejects calendar-impossible dates such as 2026-02-31.
        return None;
    }

    let (time, offset_seconds) = split_offset(rest)?;
    let mut time_parts = time.split(':');
    let hour = parse_fixed(time_parts.next()?, 2)?;
    let minute = parse_fixed(time_parts.next()?, 2)?;
    let second = match time_parts.next() {
        Some(value) => parse_seconds(value)?,
        None => 0,
    };
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))?
        .checked_sub(i64::from(offset_seconds))?;
    if !(-MAX_EPOCH_SECONDS..=MAX_EPOCH_SECONDS).contains(&seconds) {
        return None;
    }
    Some(seconds)
}

/// Formats epoch seconds as the canonical RFC 3339 UTC shape.
pub fn format_iso_epoch(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

/// Current instant in the canonical RFC 3339 UTC shape.
pub fn now_iso() -> String {
    format_iso_epoch(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0),
    )
}

/// Converts the canonical server form (`YYYY-MM-DD HH:MM:SS`, UTC) to RFC 3339.
pub fn server_form_to_iso(value: &str) -> Option<String> {
    epoch_seconds(value).map(format_iso_epoch)
}

fn parse_fixed(value: &str, width: usize) -> Option<i64> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_seconds(value: &str) -> Option<i64> {
    let (whole, fraction) = match value.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (value, None),
    };
    if let Some(fraction) = fraction {
        if fraction.is_empty()
            || fraction.len() > 9
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
    }
    parse_fixed(whole, 2)
}

/// Splits a trailing numeric offset / `Z` from the time part.
///
/// A value without an offset is the canonical server form and is UTC.
fn split_offset(value: &str) -> Option<(&str, i32)> {
    if let Some(time) = value.strip_suffix(['Z', 'z']) {
        return Some((time, 0));
    }
    // The time part itself contains no '-' or '+', so the trailing sign marks
    // the offset.
    let Some(sign_index) = value.rfind(['+', '-']) else {
        return Some((value, 0));
    };
    let (time, offset) = value.split_at(sign_index);
    if time.is_empty() {
        return None;
    }
    let sign = if value.as_bytes()[sign_index] == b'-' {
        -1
    } else {
        1
    };
    let digits = &offset[1..];
    let (hours, minutes) = match digits.len() {
        2 => (parse_fixed(digits, 2)?, 0),
        4 => (parse_fixed(&digits[..2], 2)?, parse_fixed(&digits[2..], 2)?),
        5 if digits.as_bytes()[2] == b':' => {
            (parse_fixed(&digits[..2], 2)?, parse_fixed(&digits[3..], 2)?)
        }
        _ => return None,
    };
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some((time, sign * (hours as i32 * 3_600 + minutes as i32 * 60)))
}

/// Days since the Unix epoch for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
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
    fn epoch_applies_numeric_offsets() {
        // 17:12+08:00 is 09:12Z
        assert_eq!(
            epoch_seconds("2026-11-10T17:12:00+08:00"),
            epoch_seconds("2026-11-10T09:12:00Z")
        );
        assert_eq!(
            epoch_seconds("2026-11-10T17:12:00+0800"),
            epoch_seconds("2026-11-10T09:12:00Z")
        );
    }

    #[test]
    fn epoch_rejects_malformed_and_impossible_values() {
        for value in [
            "",
            "not-a-time",
            "2026-11-10",
            "2026-02-31T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-11-10T24:00:00Z",
            "2026-11-10T09:12:00+99:00",
            "09:12:00",
        ] {
            assert!(epoch_seconds(value).is_none(), "{value} must be rejected");
        }
    }

    #[test]
    fn report_window_defaults_to_the_last_30_days_ending_now() {
        // `now` is the canonical server form at 2026-11-11T02:44:37Z.
        let (from, to) = report_window(None, None, "2026-11-11 02:44:37").expect("default window");
        assert_eq!(to, "2026-11-11T02:44:37.000Z");
        // 2026-11-11 minus 30 days is 2026-10-12.
        assert_eq!(from, "2026-10-12T02:44:37.000Z");
        assert_eq!(
            epoch_seconds(&to).expect("to") - epoch_seconds(&from).expect("from"),
            REPORT_DEFAULT_WINDOW_SECONDS
        );

        // Blank values are absent values, never an empty-string bound.
        let (blank_from, blank_to) =
            report_window(Some("  "), Some(""), "2026-11-11 02:44:37").expect("blank window");
        assert_eq!(blank_from, from);
        assert_eq!(blank_to, to);
    }

    #[test]
    fn report_window_defaults_from_against_an_explicit_to() {
        // An explicit `to` with no `from` still asks for the 30 days ending at `to`.
        let (from, to) = report_window(None, Some("2026-11-10T09:12:00Z"), "2026-11-11 02:44:37")
            .expect("explicit to");
        assert_eq!(to, "2026-11-10T09:12:00.000Z");
        assert_eq!(from, "2026-10-11T09:12:00.000Z");
    }

    #[test]
    fn report_window_bounds_are_inclusive_and_normalized() {
        // `from == to` is a valid one-instant window: the SQL predicate is
        // `>= from AND <= to`, so equality must not be rejected here.
        let (from, to) = report_window(
            Some("2026-11-10T09:12:00.000Z"),
            Some("2026-11-10T09:12:00.000Z"),
            "2026-11-11 02:44:37",
        )
        .expect("equal bounds");
        assert_eq!(from, "2026-11-10T09:12:00.000Z");
        assert_eq!(to, from);

        // Every accepted shape normalizes to the canonical RFC 3339 UTC form.
        // This pair is the historical defect: the RFC 3339 bound is *earlier*
        // than the server-form bound, but `'T'` sorts after `' '`, so comparing
        // the raw text would order it the other way around.
        let (rfc_from, server_to) = report_window(
            Some("2026-11-10T09:12:00.000Z"),
            Some("2026-11-10 14:09:44"),
            "2026-11-11 02:44:37",
        )
        .expect("an earlier RFC 3339 from is a valid window");
        assert_eq!(rfc_from, "2026-11-10T09:12:00.000Z");
        assert_eq!(server_to, "2026-11-10T14:09:44.000Z");
        assert!(
            "2026-11-10T09:12:00.000Z" > "2026-11-10 14:09:44",
            "text order of the raw shapes is deliberately not chronological"
        );
        assert!(
            epoch_seconds(&rfc_from).expect("from") < epoch_seconds(&server_to).expect("to"),
            "the parsed instants are chronological"
        );
    }

    #[test]
    fn report_window_rejects_from_after_to() {
        // Both are the canonical shape here, but the check must still be an
        // instant comparison: `2026-11-11T00:00:00Z` is after
        // `2026-11-10T23:00:00Z`, one hour apart.
        let error = report_window(
            Some("2026-11-11T00:00:00Z"),
            Some("2026-11-10T23:00:00Z"),
            "2026-11-11 02:44:37",
        )
        .expect_err("00:00Z on the 11th is after 23:00Z on the 10th");
        assert_eq!("validation", error.code(), "{error:?}");
        assert!(
            error.message().contains("from must not be after to"),
            "{error:?}"
        );

        // The same instants written in the two accepted shapes are comparable.
        let error = report_window(
            Some("2026-11-11 00:00:00"),
            Some("2026-11-10T23:00:00Z"),
            "2026-11-11 02:44:37",
        )
        .expect_err("server form 00:00 is after 23:00Z");
        assert_eq!("validation", error.code(), "{error:?}");
    }

    #[test]
    fn report_window_rejects_unparseable_bounds() {
        for (from, to) in [
            (Some("2026/11/10"), None),
            (Some("not-a-time"), Some("2026-11-10T09:12:00Z")),
            (None, Some("2026-11-10")),
            (None, Some("2026-13-01T00:00:00Z")),
        ] {
            let error = report_window(from, to, "2026-11-11 02:44:37")
                .expect_err("unparseable bounds must be rejected");
            assert_eq!("validation", error.code(), "{from:?}..{to:?}");
        }

        // An unparseable value is rejected with the field name it came from.
        let error =
            report_window(Some("2026/11/10"), None, "2026-11-11 02:44:37").expect_err("bad from");
        assert!(error.message().starts_with("from must be"), "{error:?}");
        let error =
            report_window(None, Some("2026/11/10"), "2026-11-11 02:44:37").expect_err("bad to");
        assert!(error.message().starts_with("to must be"), "{error:?}");
    }

    #[test]
    fn report_window_requires_an_authoritative_clock() {
        // `now` is supplied by the caller, never read here; an unusable value is
        // an internal fault, not a client error.
        let error = report_window(None, None, "not-a-time").expect_err("bad clock");
        assert_eq!("storage", error.code(), "{error:?}");
    }
}
