//! Mirrors `negpy/features/altprocess/models.py`: the mutually exclusive lith, cyanotype
//! and Sabattier stages.

use serde::{Deserialize, Serialize};

use crate::string_enum;

string_enum! {
    /// The alternative process the config selects.
    pub enum AltProcess {
        None => "none",
        Lith => "lith",
        Cyanotype => "cyanotype",
        Sabattier => "sabattier",
    }
    default None
}

string_enum! {
    /// Cyanotype sensitizer. Classic = Herschel's ammonium ferric citrate, New = Ware's
    /// ammonium ferric oxalate.
    pub enum Sensitizer {
        Classic => "classic",
        New => "new",
    }
    default Classic
}

/// The shared config of the mutually exclusive lith, cyanotype and Sabattier stages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AltProcessConfig {
    #[serde(default)]
    pub alt_process: AltProcess,
    #[serde(default = "default_lith_exposure")]
    pub lith_exposure: f64,
    #[serde(default = "default_lith_snatch")]
    pub lith_snatch: f64,
    #[serde(default = "default_lith_abruptness")]
    pub lith_abruptness: f64,
    #[serde(default)]
    pub cyano_sensitizer: Sensitizer,
    #[serde(default)]
    pub cyano_exposure: f64,
    #[serde(default = "default_cyano_scale")]
    pub cyano_scale: f64,
    #[serde(default)]
    pub cyano_bleach: f64,
    #[serde(default)]
    pub cyano_tannin: f64,
    #[serde(default = "default_sabattier_strength")]
    pub sabattier_strength: f64,
    #[serde(default = "default_sabattier_reexposure")]
    pub sabattier_reexposure: f64,
    #[serde(default = "default_sabattier_agitation")]
    pub sabattier_agitation: f64,
}

impl Default for AltProcessConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the altprocess defaults deserialize")
    }
}

const fn default_lith_exposure() -> f64 {
    2.0
}

const fn default_lith_snatch() -> f64 {
    0.55
}

const fn default_lith_abruptness() -> f64 {
    0.6
}

const fn default_cyano_scale() -> f64 {
    1.4
}

const fn default_sabattier_strength() -> f64 {
    1.3
}

const fn default_sabattier_reexposure() -> f64 {
    0.45
}

const fn default_sabattier_agitation() -> f64 {
    0.7
}
