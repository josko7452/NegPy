//! Mirrors `negpy/features/retouch/models.py`: dust, scratch and clone repairs.

use serde::{Deserialize, Serialize};

/// Heal `size` is a diameter at this reference long edge (px). Pinned, so stored heals keep
/// their footprint when `preview_render_size` changes.
pub const HEAL_SIZE_REF: f64 = 1600.0;

/// Brush diameter bounds at `HEAL_SIZE_REF`, shared by the heal, scratch and exclusion
/// brushes.
pub const HEAL_SIZE_MIN: f64 = 2.0;
pub const HEAL_SIZE_MAX: f64 = 64.0;

/// Dust, scratch and clone repairs.
///
/// The stroke lists hold nested tuples of floats, so they stay as opaque JSON: the render
/// reads them positionally and shape-validates at use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RetouchConfig {
    #[serde(default)]
    pub dust_remove: bool,
    #[serde(default = "default_dust_threshold")]
    pub dust_threshold: f64,
    #[serde(default = "default_dust_threshold")]
    pub dust_hair_threshold: f64,
    #[serde(default = "default_dust_size")]
    pub dust_size: i64,
    #[serde(default)]
    pub manual_dust_spots: Vec<serde_json::Value>,
    #[serde(default)]
    pub manual_heal_strokes: Vec<serde_json::Value>,
    #[serde(default)]
    pub scratch_lines: Vec<serde_json::Value>,
    #[serde(default = "default_scratch_threshold")]
    pub scratch_threshold: f64,
    #[serde(default = "default_manual_dust_size")]
    pub manual_dust_size: i64,
    #[serde(default)]
    pub dust_exclusion_strokes: Vec<serde_json::Value>,
    #[serde(default)]
    pub clone_strokes: Vec<serde_json::Value>,
    #[serde(default = "default_one")]
    pub clone_strength: f64,
    #[serde(default = "default_clone_feather")]
    pub clone_feather: f64,
    #[serde(default = "default_true")]
    pub clone_match_tone: bool,
    #[serde(default)]
    pub ir_dust_remove: bool,
    /// The IR-division tier: recover the image under semi-transparent dust. Tracks
    /// `ir_dust_remove` from the single "IR Removal" control.
    #[serde(default = "default_true")]
    pub ir_attenuation: bool,
    #[serde(default = "default_ir_threshold")]
    pub ir_threshold: f64,
}

impl Default for RetouchConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the retouch defaults deserialize")
    }
}

const fn default_dust_threshold() -> f64 {
    0.66
}

const fn default_dust_size() -> i64 {
    4
}

const fn default_scratch_threshold() -> f64 {
    0.5
}

const fn default_manual_dust_size() -> i64 {
    6
}

const fn default_one() -> f64 {
    1.0
}

const fn default_clone_feather() -> f64 {
    0.5
}

const fn default_true() -> bool {
    true
}

const fn default_ir_threshold() -> f64 {
    0.66
}
