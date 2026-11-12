//! Mirrors `negpy/features/rgbscan/models.py`: trichromatic (narrowband RGB) capture.

use serde::{Deserialize, Serialize};

use crate::domain::tokens::composite_token;

/// One frame assembled from three exposures: the red exposure is the primary source file,
/// the green and blue exposures ride along.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RgbScanConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub green_path: String,
    #[serde(default)]
    pub blue_path: String,
    #[serde(default = "default_true")]
    pub align: bool,
}

impl Default for RgbScanConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the rgbscan defaults deserialize")
    }
}

/// The predicate the decode paths use to decide to merge a triplet.
#[must_use]
pub fn is_rgb_triplet(config: &RgbScanConfig) -> bool {
    config.enabled && !config.green_path.is_empty() && !config.blue_path.is_empty()
}

/// Identity of the active triplet, folded into the render source hash.
#[must_use]
pub fn rgbscan_token(config: &RgbScanConfig) -> String {
    if !is_rgb_triplet(config) {
        return String::new();
    }
    composite_token("rgb", config, &[&config.green_path, &config.blue_path])
}

const fn default_true() -> bool {
    true
}
