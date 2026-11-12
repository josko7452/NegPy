//! Mirrors `negpy/features/finish/models.py`: post-crop finishing effects.

use serde::{Deserialize, Serialize};

/// Vignette, carrier rebate and border.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FinishConfig {
    #[serde(default)]
    pub vignette_stops: f64,
    #[serde(default = "default_vignette_size")]
    pub vignette_size: f64,
    #[serde(default)]
    pub vignette_roundness: f64,
    #[serde(default)]
    pub carrier_width: f64,
    #[serde(default = "default_carrier_rough")]
    pub carrier_rough: f64,
    #[serde(default = "default_carrier_corner")]
    pub carrier_corner: f64,
    #[serde(default = "default_carrier_flare")]
    pub carrier_flare: f64,
    #[serde(default)]
    pub border_size: f64,
    #[serde(default = "default_border_color")]
    pub border_color: String,
    #[serde(default = "default_one")]
    pub border_bottom_weight: f64,
    #[serde(default)]
    pub border_match_paper: bool,
}

impl Default for FinishConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the finish defaults deserialize")
    }
}

const fn default_vignette_size() -> f64 {
    0.5
}

const fn default_carrier_rough() -> f64 {
    0.25
}

const fn default_carrier_corner() -> f64 {
    0.25
}

const fn default_carrier_flare() -> f64 {
    0.5
}

fn default_border_color() -> String {
    "#ffffff".to_string()
}

const fn default_one() -> f64 {
    1.0
}
