//! Mirrors `negpy/features/geometry/models.py`: framing, crop and lens corrections.

use serde::{Deserialize, Serialize};

use crate::string_enum;

string_enum! {
    /// The crop and export aspect ratios. The reciprocals stay real members: the export
    /// "Paper ratio" picker does not auto-orient.
    pub enum AspectRatio {
        Free => "Free",
        Original => "Original",
        R1x1 => "1:1",
        R3x2 => "3:2",
        R4x3 => "4:3",
        R5x4 => "5:4",
        R6x7 => "6:7",
        R7x5 => "7:5",
        R65x24 => "65:24",
        R16x9 => "16:9",
        R16x10 => "16:10",
        R8p5x11 => "8.5:11",
        IsoA => "1:1.4142",
        R2x3 => "2:3",
        R3x4 => "3:4",
        R4x5 => "4:5",
        R7x6 => "7:6",
        R5x7 => "5:7",
        R24x65 => "24:65",
        R9x16 => "9:16",
        R10x16 => "10:16",
        R11x8p5 => "11:8.5",
        IsoALandscape => "1.4142:1",
    }
    default Free
}

string_enum! {
    /// What Auto Crop fits to.
    pub enum AutocropMode {
        Image => "image",
        Film => "film",
    }
    default Image
}

/// Framing, crop and lens corrections.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryConfig {
    #[serde(default)]
    pub lens_distortion_from_metadata: bool,
    #[serde(default)]
    pub lens_ca_from_metadata: bool,
    #[serde(default)]
    pub rotation: i64,
    #[serde(default)]
    pub fine_rotation: f64,
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
    #[serde(default)]
    pub converge_v: f64,
    #[serde(default)]
    pub converge_h: f64,
    #[serde(default)]
    pub distortion_k1: f64,
    #[serde(default)]
    pub crop_to_valid: bool,
    #[serde(default)]
    pub autocrop_offset: i64,
    #[serde(default)]
    pub autocrop_ratio: AspectRatio,
    #[serde(default)]
    pub autocrop_mode: AutocropMode,
    #[serde(default = "default_one")]
    pub autocrop_rebate_trim: f64,
    #[serde(default)]
    pub autocrop_square: bool,
    #[serde(default)]
    pub autocrop_squared: bool,
    /// One normalized rect in transformed-image space; `None` = no crop. Kept as a list so
    /// a partial stored rect round-trips.
    #[serde(default)]
    pub crop_rect: Option<Vec<f64>>,
    #[serde(default)]
    pub crop_from_auto: bool,
    #[serde(default)]
    pub crop_detect_key: String,
}

impl GeometryConfig {
    /// Apply the Python `__post_init__` conversions a programmatic build skips.
    #[must_use]
    pub fn normalize(mut self) -> Self {
        if self.lens_distortion_from_metadata {
            self.distortion_k1 = 0.0;
        }
        if self.crop_rect.as_ref().is_some_and(|rect| rect.len() != 4) {
            self.crop_rect = None;
        }
        self
    }
}

impl Default for GeometryConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the geometry defaults deserialize")
    }
}

/// A frame with Auto Crop armed or off.
///
/// Every path that switches Auto Crop goes through here: a rect kept across the switch
/// would read as the other kind.
#[must_use]
pub fn with_auto_crop(
    mut config: crate::domain::models::workspace::WorkspaceConfig,
    on: bool,
) -> crate::domain::models::workspace::WorkspaceConfig {
    config.geometry.crop_rect = None;
    config.geometry.crop_from_auto = on;
    config.geometry.crop_detect_key.clear();
    config.geometry.autocrop_squared = false;
    let (invalidate, floors, ceils) =
        crate::domain::models::process::invalidate_local_bounds(&config.process);
    if invalidate {
        config.process.local_floors = floors;
        config.process.local_ceils = ceils;
    }
    config
}

const fn default_one() -> f64 {
    1.0
}
