//! Mirrors `negpy/features/hdr/models.py`: bracketed exposures merged into one source.

use serde::{Deserialize, Serialize};

use crate::domain::tokens::composite_token;

/// Sentinel for "the render exposure was not set as a value". `output_scale` clamps the scale
/// to 1.0 or less, so any value at or above this renders identically to 0 and is unreachable
/// as a setting; that leaves 0 and below free to mean what they say.
pub const ANCHOR_EV_UNSET: f64 = 1.0;

/// Bracketed exposures of one frame, merged into a single linear source.
///
/// Every field is `hdr_`-prefixed: `WorkspaceConfig` flattens all sub-configs into one
/// namespace and `RgbScanConfig` already owns the bare `enabled` and `align`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HdrConfig {
    #[serde(default)]
    pub hdr_enabled: bool,
    /// Non-reference exposures.
    #[serde(default)]
    pub hdr_paths: Vec<String>,
    /// Per frame including the reference, which is 1.0.
    #[serde(default)]
    pub hdr_ratios: Vec<f64>,
    #[serde(default = "default_true")]
    pub hdr_align: bool,
    /// Path of the frame the merge renders at. Empty falls back to the bracket's middle
    /// exposure.
    #[serde(default)]
    pub hdr_anchor: String,
    /// Render exposure in stops below the reference, when set as a value rather than by
    /// naming a frame. Takes precedence over `hdr_anchor`.
    #[serde(default = "default_anchor_ev")]
    pub hdr_anchor_ev: f64,
}

impl Default for HdrConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the hdr defaults deserialize")
    }
}

/// The predicate the decode paths use to decide to merge a bracket.
#[must_use]
pub fn hdr_active(config: &HdrConfig) -> bool {
    config.hdr_enabled && !config.hdr_paths.is_empty()
}

/// Identity of the merge itself, excluding the exposure it renders at. Keyed apart so
/// dragging the exposure re-uses the merged buffer.
#[must_use]
pub fn hdr_merge_token(config: &HdrConfig) -> String {
    if !hdr_active(config) {
        return String::new();
    }
    let mut neutral = config.clone();
    neutral.hdr_anchor = String::new();
    neutral.hdr_anchor_ev = ANCHOR_EV_UNSET;
    let paths: Vec<&str> = config.hdr_paths.iter().map(String::as_str).collect();
    composite_token("hdrmerge", &neutral, &paths)
}

/// Identity of the active bracket, folded into the render source hash.
#[must_use]
pub fn hdr_token(config: &HdrConfig) -> String {
    if !hdr_active(config) {
        return String::new();
    }
    let paths: Vec<&str> = config.hdr_paths.iter().map(String::as_str).collect();
    composite_token("hdr", config, &paths)
}

/// The bracket's naming stem: its alphabetically first frame. Ordered by the lowercase
/// basename, so naming is stable and independent of which exposure is the reference.
#[must_use]
pub fn hdr_stem(frame_paths: &[String]) -> String {
    let first = frame_paths.iter().min_by_key(|path| {
        std::path::Path::new(path)
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().to_lowercase())
    });
    first
        .and_then(|path| std::path::Path::new(path).file_stem())
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

/// Display name of a merge: the naming stem plus the extra frame count.
#[must_use]
pub fn hdr_name(frame_paths: &[String]) -> String {
    if frame_paths.is_empty() {
        return "(HDR)".to_string();
    }
    format!("{} +{} (HDR)", hdr_stem(frame_paths), frame_paths.len() - 1)
}

const fn default_true() -> bool {
    true
}

const fn default_anchor_ev() -> f64 {
    1.0
}
