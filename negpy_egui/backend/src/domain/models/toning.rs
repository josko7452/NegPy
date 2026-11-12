//! Mirrors `negpy/features/toning/models.py`: the toning stages.

use serde::{Deserialize, Serialize};

/// Selenium, sepia, gold, blue, copper and vanadium toners, plus the split-tint bands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ToningConfig {
    #[serde(default)]
    pub selenium_strength: f64,
    #[serde(default)]
    pub sepia_strength: f64,
    #[serde(default)]
    pub gold_strength: f64,
    #[serde(default)]
    pub blue_strength: f64,
    #[serde(default)]
    pub copper_strength: f64,
    #[serde(default)]
    pub vanadium_strength: f64,
    #[serde(default)]
    pub shadow_tint_hue: f64,
    #[serde(default)]
    pub shadow_tint_strength: f64,
    #[serde(default)]
    pub highlight_tint_hue: f64,
    #[serde(default)]
    pub highlight_tint_strength: f64,
}
