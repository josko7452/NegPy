//! Mirrors `negpy/features/lab/models.py`: scanner emulation (sharpening, CLAHE).

use serde::{Deserialize, Serialize};

use crate::string_enum;

string_enum! {
    /// The sharpening kernel.
    pub enum SharpenMethod {
        Usm => "usm",
        Rl => "rl",
    }
    default Usm
}

/// Scanner emulation (Sharpening, CLAHE).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabConfig {
    #[serde(default = "default_one")]
    pub saturation: f64,
    #[serde(default = "default_skin_protection")]
    pub skin_protection: f64,
    #[serde(default)]
    pub clahe_strength: f64,
    #[serde(default = "default_sharpen")]
    pub sharpen: f64,
    #[serde(default)]
    pub sharpen_method: SharpenMethod,
    #[serde(default = "default_one")]
    pub sharpen_radius: f64,
    #[serde(default = "default_sharpen_masking")]
    pub sharpen_masking: f64,
    #[serde(default)]
    pub sharpen_auto_masking: bool,
    #[serde(default = "default_chroma_denoise")]
    pub chroma_denoise: f64,
    #[serde(default)]
    pub glow_amount: f64,
    #[serde(default)]
    pub halation_strength: f64,
}

impl Default for LabConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the lab defaults deserialize")
    }
}

const fn default_one() -> f64 {
    1.0
}

const fn default_skin_protection() -> f64 {
    0.5
}

const fn default_sharpen() -> f64 {
    0.25
}

const fn default_sharpen_masking() -> f64 {
    0.1
}

const fn default_chroma_denoise() -> f64 {
    2.5
}
