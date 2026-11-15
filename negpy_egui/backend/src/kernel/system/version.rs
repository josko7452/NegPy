//! Mirrors `negpy/kernel/system/version.py`: the shipped version.

/// The version file that ships beside the executable and the repository root file.
const VERSION: &str = include_str!("../../../../../VERSION");

/// The application version, or `"unknown"` when the file is unreadable.
#[must_use]
pub fn app_version() -> String {
    let value = VERSION.trim();
    if value.is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

/// The numeric parts of a version, for ordered comparison.
#[must_use]
pub fn parse_version(value: &str) -> Vec<i64> {
    value
        .split('.')
        .filter_map(|part| part.parse::<i64>().ok())
        .collect()
}

/// Whether `candidate` is a later release than `current`.
#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let candidate_parts = parse_version(candidate);
    if candidate_parts.is_empty() || current == "unknown" {
        return false;
    }
    candidate_parts > parse_version(current)
}
