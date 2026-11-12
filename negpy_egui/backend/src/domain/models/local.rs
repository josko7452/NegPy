//! Mirrors `negpy/features/local/models.py`: local-adjustment masks.

use serde::{Deserialize, Serialize};

use crate::string_enum;

string_enum! {
    /// How to read a mask's vertices.
    pub enum MaskShape {
        Polygon => "polygon",
        Oval => "oval",
        Gradient => "gradient",
    }
    default Polygon
}

string_enum! {
    /// Which tones of the unburned print a mask acts on.
    pub enum MaskKey {
        Off => "off",
        Highlights => "highlights",
        Shadows => "shadows",
    }
    default Off
}

/// Each limited mask needs its own shape plane (GPU: one rgba texture). A fifth prints
/// unlimited.
pub const MAX_KEYED_MASKS: i64 = 4;

/// One local-adjustment mask.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalMask {
    /// Vertices in raw-image normalized coords `[0, 1]`, as `[x, y]` pairs.
    #[serde(default)]
    pub vertices: Vec<[f64; 2]>,
    /// Print exposure in stops, darkroom-signed like `vignette_stops`.
    #[serde(default)]
    pub stops: f64,
    #[serde(default = "default_feather")]
    pub feather: f64,
    /// Local grade in ISO-R points off the global grade; negative is harder.
    #[serde(default)]
    pub grade: f64,
    /// Preflash under the mask, a fraction of the threshold exposure. A tone-limited mask
    /// carries no flash.
    #[serde(default)]
    pub flash: f64,
    #[serde(default)]
    pub shape: MaskShape,
    #[serde(default)]
    pub invert: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub key: MaskKey,
    #[serde(default = "default_key_zone")]
    pub key_zone: f64,
    #[serde(default = "default_one")]
    pub key_softness: f64,
}

impl Default for LocalMask {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the local mask defaults deserialize")
    }
}

/// The frame's local masks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalAdjustmentsConfig {
    #[serde(default)]
    pub masks: Vec<LocalMask>,
}

const fn default_feather() -> f64 {
    0.04
}

const fn default_true() -> bool {
    true
}

const fn default_key_zone() -> f64 {
    6.0
}

const fn default_one() -> f64 {
    1.0
}
