//! Indexed identity of a promotion code at rest.
//!
//! `promotion_code` still stores the redeemable value in plaintext because the
//! audited admin reveal/export operations read it. Every *lookup* and every
//! *uniqueness* decision, however, is made on a derived identity:
//!
//! * `code_hash` — lowercase SHA-256 hex of the UTF-8 bytes of
//!   `UPPER(TRIM(code))`, 64 characters. Indexed by `uk_promotion_code_hash`
//!   `(tenant_id, code_hash)`, so it is also the case-insensitive uniqueness
//!   guarantee.
//! * `code_last4` — the last four characters of the trimmed code, the only
//!   non-secret fragment the admin list needs beyond the existing mask.
//!
//! The PostgreSQL side derives the very same hash with the expression pinned by
//! `SQL_CODE_HASH_EXPRESSION`; `sha256(bytea)` is a PostgreSQL 18 builtin, so no
//! extension (and no pgcrypto) is involved. [`code_identity`] is the only Rust
//! producer of the pair, so the write path
//! (`promotion_admin_management::create_code_batch`) and the redemption lookup
//! (`postgres_promotion::load_promotion_for_redeem`) cannot drift apart.
//!
//! Promotion codes are restricted to ASCII alphanumerics plus `-`/`_`, which is
//! why Rust's ASCII case folding and PostgreSQL's locale-aware `upper` agree on
//! the normalized form. For the same reason `trim()` (ASCII spaces in practice)
//! and `btrim()` agree; no code may contain exotic Unicode whitespace.

/// The exact PostgreSQL expression that produces `promotion_code.code_hash`.
///
/// It is kept as a literal here, next to [`code_identity`], so a change to
/// either side of the hash contract is caught by the tests below:
///
/// ```sql
/// encode(sha256(convert_to(upper(btrim(promotion_code)), 'UTF8')), 'hex')
/// ```
///
/// `sha256(bytea)` is builtin in PostgreSQL 18; pgcrypto is neither installed
/// nor required. The Rust side feeds `UPPER(TRIM(code)).as_bytes()` to
/// `sdkwork_utils_rust::sha256_hash`, which returns the same lowercase hex.
#[cfg(test)]
pub(crate) const SQL_CODE_HASH_EXPRESSION: &str =
    "encode(sha256(convert_to(upper(btrim(promotion_code)), 'UTF8')), 'hex')";

/// The exact PostgreSQL expression that produces `promotion_code.code_last4`.
///
/// `right(..., 4)` counts characters, matching [`code_last4`]'s `chars()`
/// iteration rather than a byte slice.
#[cfg(test)]
pub(crate) const SQL_CODE_LAST4_EXPRESSION: &str = "right(btrim(promotion_code), 4)";

/// Lowercase SHA-256 hex of `UPPER(TRIM("WELCOME2026"))`.
///
/// Pinned once here and reused by the SQL/Rust agreement test below. A change to
/// the normalization, the hash algorithm, or the encoding breaks it loudly.
#[cfg(test)]
pub(crate) const PINNED_WELCOME2026_HASH: &str =
    "5edf7240eae6a7f88a96900821c406054d692926b364a393c529ba5f6cf457d7";

/// The stored identity of one promotion code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeIdentity {
    /// `promotion_code.code_hash`: lowercase SHA-256 hex of `UPPER(TRIM(code))`.
    pub(crate) hash: String,
    /// `promotion_code.code_last4`: last four characters of `TRIM(code)`.
    pub(crate) last4: String,
}

/// Derives the persisted identity of `code`.
///
/// Uniqueness, lookup, and the redemption-attempt evidence all go through this
/// one function, so there is exactly one Rust definition of "what a code is"
/// for storage purposes.
pub(crate) fn code_identity(code: &str) -> CodeIdentity {
    CodeIdentity {
        hash: sdkwork_utils_rust::sha256_hash(code.trim().to_ascii_uppercase().as_bytes()),
        last4: code_last4(code),
    }
}

/// Last four characters of the trimmed code (all of them when shorter).
///
/// Never the whole value for a realistic code: the admin list and support rows
/// may show this fragment without making the code redeemable.
pub(crate) fn code_last4(code: &str) -> String {
    let trimmed = code.trim();
    let count = trimmed.chars().count();
    trimmed.chars().skip(count.saturating_sub(4)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_identity_normalizes_case_and_whitespace_into_one_identity() {
        let canonical = code_identity("WELCOME2026");
        // The pinned production vector: sha256 hex of "WELCOME2026".
        assert_eq!(canonical.hash, PINNED_WELCOME2026_HASH);
        assert_eq!(
            canonical.hash,
            "5edf7240eae6a7f88a96900821c406054d692926b364a393c529ba5f6cf457d7"
        );
        assert_eq!(canonical.last4, "2026");
        assert_eq!(canonical.hash.len(), 64);
        assert!(canonical
            .hash
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase()));

        // Lower-case input, and a padded tab/space paste, fold into the same
        // identity the admin console printed.
        assert_eq!(code_identity("welcome2026"), canonical);
        assert_eq!(code_identity("  Welcome2026\t"), canonical);

        // A short code degrades to the whole trimmed value instead of panicking
        // on a missing four-character tail, and the identity is still stable.
        let short = code_identity(" ab ");
        assert_eq!(short.last4, "ab");
        assert_eq!(short, code_identity("ab"));
        // Empty input is legal at this layer (the admin validator rejects it
        // earlier) and must not panic.
        assert_eq!(code_identity("").last4, "");
        assert_eq!(code_identity("   ").last4, "");

        // The last four characters are the *trimmed* tail, never the raw tail.
        assert_eq!(code_identity("WELCOME2026  ").last4, "2026");
    }

    #[test]
    fn sql_and_rust_agree_on_the_code_hash_and_last4_contract() {
        // The SQL expression documented next to the Rust helper must literally
        // be the one the migration and the folded baseline execute. If either
        // side changes the expression (or the normalization) this breaks.
        let migration = include_str!("../../../database/migrations/postgres/0007_code_hash.up.sql");
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");
        for sql in [migration, baseline] {
            // The backfill lives inside a `DO $$ … EXECUTE format(…)` block so
            // the same migration runs on PostgreSQL 18 (builtin sha256) and on
            // older servers that only ship pgcrypto's digest(). String literals
            // inside that block double their single quotes, so compare against a
            // normalized copy instead of loosening the expression itself.
            let normalized = sql.replace("''", "'");
            assert!(
                normalized.contains(SQL_CODE_HASH_EXPRESSION),
                "the code hash SQL expression drifted from {SQL_CODE_HASH_EXPRESSION}"
            );
            assert!(
                normalized.contains(SQL_CODE_LAST4_EXPRESSION),
                "the code last4 SQL expression drifted from {SQL_CODE_LAST4_EXPRESSION}"
            );
            // Both fallbacks must stay present: builtin, pgcrypto, explicit error.
            assert!(sql.contains("proname = 'sha256'"), "builtin branch missing");
            assert!(sql.contains("digest("), "pgcrypto branch missing");
            assert!(
                sql.contains("RAISE EXCEPTION"),
                "an unsupported server must fail with an explicit message"
            );
            assert!(
                sql.contains("EXECUTE format("),
                "the backfill must resolve the hash function at run time"
            );
        }

        // One pinned vector proves the two implementations agree today. The
        // SQL side hashes `convert_to(upper(btrim('WELCOME2026')), 'UTF8')`;
        // the Rust side hashes `UPPER(TRIM("WELCOME2026")).as_bytes()`.
        assert_eq!(code_identity("WELCOME2026").hash, PINNED_WELCOME2026_HASH);
        // The redemption attempt evidence uses the same normalization: its
        // helper must return the identical hash and suffix.
        assert_eq!(
            crate::redemption_attempt::submitted_code_hash("WELCOME2026"),
            PINNED_WELCOME2026_HASH
        );
        assert_eq!(
            crate::redemption_attempt::submitted_code_suffix("WELCOME2026"),
            "2026"
        );
        // `right(btrim(code), 4)` keeps the raw case of the tail; so does the
        // Rust helper (it trims, it does not upper-case).
        assert_eq!(code_identity("  welcome2026 ").last4, "2026");
        assert_eq!(code_identity("AB12CD34").last4, "CD34");
    }

    #[test]
    fn code_hash_migration_and_baseline_agree_on_columns_and_constraint() {
        let migration = include_str!("../../../database/migrations/postgres/0007_code_hash.up.sql");
        let baseline =
            include_str!("../../../database/ddl/baseline/postgres/0001_promotion_baseline.sql");
        let superseded = include_str!(
            "../../../database/migrations/postgres/0003_coupon_redemption_evidence.up.sql"
        );

        for sql in [migration, baseline] {
            assert!(sql.contains("ADD COLUMN IF NOT EXISTS code_hash VARCHAR(64)"));
            assert!(sql.contains("ADD COLUMN IF NOT EXISTS code_last4 VARCHAR(4)"));
            assert!(sql.contains("ALTER COLUMN code_hash SET NOT NULL"));
            assert!(sql.contains("ALTER COLUMN code_last4 SET NOT NULL"));
            assert!(
                sql.contains("ADD CONSTRAINT uk_promotion_code_hash UNIQUE (tenant_id, code_hash)")
            );
            assert!(sql.contains("DROP INDEX IF EXISTS idx_promotion_code_value_normalized"));
            // The backfill fills both columns in one pass and is guarded so a
            // re-run does not touch already-hashed rows.
            assert!(sql.contains("WHERE code_hash IS NULL"));
        }

        // The superseded non-unique expression index is no longer created: 0003
        // keeps only the explanatory comment and the baseline mirrors that, so
        // a fresh install never pays for an index the unique hash replaces.
        let create_normalized = [
            "CREATE INDEX IF NOT EXISTS idx_promotion_code_",
            "value_normalized",
        ]
        .concat();
        for sql in [superseded, baseline] {
            assert!(
                !sql.contains(create_normalized.as_str()),
                "the superseded normalized expression index is still created"
            );
        }
        assert!(superseded.contains("Superseded by `0007_code_hash`"));
        // Raw-value uniqueness stays: the hash adds case-insensitive uniqueness,
        // it does not replace the plaintext uniqueness constraint.
        assert!(baseline
            .contains("CONSTRAINT uk_promotion_code_value UNIQUE (tenant_id, promotion_code)"));
    }
}
