//! Mirrors `negpy/kernel/system/version.py`: the shipped version.

/// The repository whose releases NegPy reads.
pub const GITHUB_REPO: &str = "marcinz606/NegPy";
/// The latest-release API endpoint.
pub const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/marcinz606/NegPy/releases/latest";
/// The human-readable releases page.
pub const RELEASES_PAGE: &str = "https://github.com/marcinz606/NegPy/releases";
/// The User-Agent every update request sends.
pub const USER_AGENT: &str = "NegPy-Updater";

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

/// The GitHub payload for the newest release, or `None` when it cannot be read.
#[must_use]
pub fn fetch_latest_release(
    timeout_seconds: u64,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(timeout_seconds)))
        .build()
        .new_agent();
    let mut response = agent
        .get(LATEST_RELEASE_API)
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let mut body = Vec::new();
    use std::io::Read;
    response
        .body_mut()
        .as_reader()
        .take(4 * 1024 * 1024)
        .read_to_end(&mut body)
        .ok()?;
    match serde_json::from_slice::<serde_json::Value>(&body).ok()? {
        serde_json::Value::Object(map) => Some(map),
        _ => None,
    }
}
