//! Wording helpers for text the user reads.
//!
//! Mirrors `negpy/kernel/system/text.py`. A count is nearly always shown with its noun,
//! so `count_of` returns the whole phrase and no caller can forget the agreement.

use super::i18n;

/// The right noun for `count`, without the number. `plural_form` covers nouns an `-s`
/// does not ("copies", "indices").
pub fn plural(count: i64, singular: &str, plural_form: &str) -> String {
    let language = i18n::language();
    // These languages use one noun form for all counts, without an English plural suffix.
    if matches!(language.as_str(), "zh_CN" | "ja" | "ko" | "vi") {
        return i18n::tr(singular, &[]);
    }
    if matches!(language.as_str(), "de" | "nb") {
        // The catalog value of a noun holds both forms as "Singular|Plural".
        let translated = i18n::tr(singular, &[]);
        let mut parts = translated.splitn(2, '|');
        let one = parts.next().unwrap_or("");
        let many = parts.next();
        if count.abs() == 1 {
            return one.to_string();
        }
        return if plural_form.is_empty() {
            many.unwrap_or(one).to_string()
        } else {
            i18n::tr(plural_form, &[])
        };
    }
    if count.abs() == 1 {
        singular.to_string()
    } else if plural_form.is_empty() {
        format!("{singular}s")
    } else {
        plural_form.to_string()
    }
}

/// `count` and its noun together: "1 file", "3 files".
pub fn count_of(count: i64, singular: &str, plural_form: &str) -> String {
    format!("{count} {}", plural(count, singular, plural_form))
}

/// A byte count in the largest unit that keeps it under 1024.
pub fn human_bytes(n: f64) -> String {
    let mut size = n;
    for unit in ["B", "KB", "MB", "GB"] {
        // Compare the number as it will be shown: a value displayed as "1024.0" is not
        // under 1024, so the rounded value decides the unit, not the raw one.
        let shown = if unit == "B" {
            size.round()
        } else {
            round_half_even(size, 1)
        };
        if shown < 1024.0 || unit == "GB" {
            return if unit == "B" {
                format!("{size:.0} {unit}")
            } else {
                format!("{size:.1} {unit}")
            };
        }
        size /= 1024.0;
    }
    format!("{size:.1} GB")
}

/// Round to `places` decimals with banker's rounding, as Python's `round` does.
fn round_half_even(value: f64, places: u32) -> f64 {
    let factor = 10f64.powi(places as i32);
    let scaled = value * factor;
    let rounded = scaled.round_ties_even();
    rounded / factor
}
