//! Display masking for promotion coupon codes.
//!
//! A code list must never be a distribution channel for redeemable codes: the
//! console renders [`mask_promotion_code`] and reveals the full value only
//! through the audited reveal/export operations. Masking therefore has one
//! invariant that holds for every input, including admin-defined short codes:
//! the masked form is never equal to the value it masks.

/// Codes at least this long keep their first and last four characters.
const MASK_VISIBLE_LENGTH: usize = 9;

/// Masks a redeemable promotion code for display.
///
/// * 9 characters or more: first 4 + `****` + last 4 (`3C76A1B2C3D44E21` ->
///   `3C76****4E21`).
/// * shorter than 9 characters: everything except the last 2 characters is
///   masked (`AB12` -> `****12`), because keeping 8 of 8 characters visible
///   would defeat the mask.
///
/// The result is guaranteed not to equal `value`. That matters for the short
/// branch: a 1- or 2-character value leaves nothing to mask, and returning it
/// unchanged would turn a masked field into the full code.
pub(crate) fn mask_promotion_code(value: &str) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    let masked = if characters.len() >= MASK_VISIBLE_LENGTH {
        format!("{}****{}", take(&characters, 4), take_last(&characters, 4))
    } else {
        // Every character except the last two is masked. The literal mask is
        // kept visible even when the value is shorter than that, so the result
        // can never be mistaken for an unmasked value.
        format!("****{}", take_last(&characters, 2))
    };
    if masked == value {
        // Code values are validated to alphanumerics/`-`/`_`, so a value that
        // literally spells its own masked form is not expected. The helper stays
        // total and keeps the invariant even for such a value.
        return "****".to_owned();
    }
    masked
}

/// The last four characters of a code (fewer when the code is shorter).
///
/// `codeLast4` is the stable, non-redeemable fragment support staff quote to a
/// customer, so an empty code yields an empty string rather than a mask.
pub(crate) fn promotion_code_last4(value: &str) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    take_last(&characters, 4)
}

fn take(characters: &[char], count: usize) -> String {
    characters.iter().take(count).collect()
}

fn take_last(characters: &[char], count: usize) -> String {
    let start = characters.len().saturating_sub(count);
    characters[start..].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{mask_promotion_code, promotion_code_last4, MASK_VISIBLE_LENGTH};

    /// The masked form must never reveal the whole code, whatever its length.
    fn assert_masked(value: &str, expected: &str) {
        let masked = mask_promotion_code(value);
        assert_eq!(masked, expected, "mask of {value:?}");
        assert_ne!(masked, value, "masked form must differ from the code");
        assert!(
            !masked.is_empty(),
            "an empty code still needs a non-empty mask"
        );
    }

    #[test]
    fn sixteen_character_code_keeps_first_and_last_four() {
        assert_eq!(MASK_VISIBLE_LENGTH, 9);
        // The task's example shape: a 16-character generated code renders as
        // first 4 + **** + last 4.
        assert_masked("3C76A1B2C3D44E21", "3C76****4E21");
    }

    #[test]
    fn eight_character_code_masks_all_but_the_last_two() {
        // Shorter than 9: keeping the first four would expose half the code, so
        // everything except the last two is masked.
        assert_masked("AB12CD34", "****34");
    }

    #[test]
    fn four_character_code_masks_all_but_the_last_two() {
        assert_masked("WXYZ", "****YZ");
    }

    #[test]
    fn one_character_code_is_never_returned_unchanged() {
        // The short branch leaves nothing to mask, which is exactly why the
        // fixed mask prefix is kept: `A` must not come back as `A`.
        assert_masked("A", "****A");
    }

    #[test]
    fn empty_code_still_has_a_mask() {
        assert_masked("", "****");
    }

    #[test]
    fn nine_character_code_uses_the_long_branch() {
        assert_masked("ABCDEFGHI", "ABCD****FGHI");
    }

    #[test]
    fn a_code_that_spells_its_own_mask_is_still_masked() {
        // "3C76****4E21" is 12 characters and would render identically to its
        // mask; the invariant fallback keeps the value from leaking.
        assert_eq!(mask_promotion_code("3C76****4E21"), "****");
    }

    #[test]
    fn code_last4_returns_the_tail_without_masking() {
        assert_eq!(promotion_code_last4("3C76A1B2C3D44E21"), "4E21");
        assert_eq!(promotion_code_last4("AB12CD34"), "CD34");
        assert_eq!(promotion_code_last4("AB"), "AB");
        assert_eq!(promotion_code_last4(""), "");
        // Multi-byte characters are counted by character, not byte.
        assert_eq!(promotion_code_last4("优惠券码"), "优惠券码");
    }
}
