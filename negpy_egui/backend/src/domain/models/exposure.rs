//! Mirrors `negpy/features/exposure/models.py`: the print parameters.

use serde::{Deserialize, Serialize};

use crate::domain::models::bool_or_float;
use crate::string_enum;

string_enum! {
    /// How the Print stage renders the positive.
    pub enum RenderIntent {
        Print => "print",
        Flat => "flat",
    }
    default Print
}

string_enum! {
    /// A Wratten contrast filter on the camera lens, for a B&W print from a color
    /// negative.
    pub enum BwFilter {
        None => "none",
        Yellow8 => "yellow_8",
        DeepYellow15 => "deep_yellow_15",
        Orange21 => "orange_21",
        Red25 => "red_25",
        YellowGreen11 => "yellow_green_11",
        Green58 => "green_58",
    }
    default None
}

/// Print parameters (Density, Grade, Color).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExposureConfig {
    #[serde(default = "default_density")]
    pub density: f64,
    /// The ISO-R grade the print and E-6 transfer curves are an identity at.
    #[serde(default = "default_grade", deserialize_with = "deserialize_grade")]
    pub grade: f64,
    #[serde(default)]
    pub grade_trim_red: f64,
    #[serde(default)]
    pub grade_trim_green: f64,
    #[serde(default)]
    pub grade_trim_blue: f64,
    #[serde(default)]
    pub wb_cyan: f64,
    #[serde(default)]
    pub wb_magenta: f64,
    #[serde(default)]
    pub wb_yellow: f64,
    #[serde(default)]
    pub shadow_cyan: f64,
    #[serde(default)]
    pub shadow_magenta: f64,
    #[serde(default)]
    pub shadow_yellow: f64,
    #[serde(default)]
    pub highlight_cyan: f64,
    #[serde(default)]
    pub highlight_magenta: f64,
    #[serde(default)]
    pub highlight_yellow: f64,
    #[serde(default)]
    pub shadow_density: f64,
    #[serde(default)]
    pub highlight_density: f64,
    #[serde(default)]
    pub hold_black: bool,
    #[serde(default)]
    pub contrast_mask_strength: f64,
    #[serde(default)]
    pub diffusion: f64,
    #[serde(default)]
    pub shadow_grade: f64,
    #[serde(default)]
    pub highlight_grade: f64,
    #[serde(default)]
    pub shadow_grade_trim_red: f64,
    #[serde(default)]
    pub shadow_grade_trim_green: f64,
    #[serde(default)]
    pub shadow_grade_trim_blue: f64,
    #[serde(default)]
    pub highlight_grade_trim_red: f64,
    #[serde(default)]
    pub highlight_grade_trim_green: f64,
    #[serde(default)]
    pub highlight_grade_trim_blue: f64,
    #[serde(default)]
    pub toe: f64,
    #[serde(default = "default_toe_width")]
    pub toe_width: f64,
    #[serde(default)]
    pub shoulder: f64,
    #[serde(default = "default_shoulder_width")]
    pub shoulder_width: f64,
    #[serde(default)]
    pub toe_trim_red: f64,
    #[serde(default)]
    pub toe_trim_green: f64,
    #[serde(default)]
    pub toe_trim_blue: f64,
    #[serde(default)]
    pub shoulder_trim_red: f64,
    #[serde(default)]
    pub shoulder_trim_green: f64,
    #[serde(default)]
    pub shoulder_trim_blue: f64,
    #[serde(default)]
    pub toe_width_trim_red: f64,
    #[serde(default)]
    pub toe_width_trim_green: f64,
    #[serde(default)]
    pub toe_width_trim_blue: f64,
    #[serde(default)]
    pub shoulder_width_trim_red: f64,
    #[serde(default)]
    pub shoulder_width_trim_green: f64,
    #[serde(default)]
    pub shoulder_width_trim_blue: f64,
    #[serde(default)]
    pub paper_dmin: bool,
    #[serde(default)]
    pub paper_black: bool,
    #[serde(default)]
    pub midtone_gamma: f64,
    #[serde(default)]
    pub midtone_gamma_trim_red: f64,
    #[serde(default)]
    pub midtone_gamma_trim_green: f64,
    #[serde(default)]
    pub midtone_gamma_trim_blue: f64,
    #[serde(default)]
    pub preflash: f64,
    /// Cast Removal strength. A legacy bool toggle coerces to `1.0`/`0.0`.
    #[serde(default = "default_cast_removal", deserialize_with = "bool_or_float")]
    pub cast_removal_strength: f64,
    #[serde(default = "default_true")]
    pub auto_exposure: bool,
    #[serde(default = "default_true")]
    pub auto_normalize_contrast: bool,
    #[serde(default)]
    pub render_intent: RenderIntent,
    #[serde(default = "default_paper_profile")]
    pub paper_profile: String,
    #[serde(default = "default_one")]
    pub dye_separation: f64,
    #[serde(default)]
    pub dye_separation_trim_red: f64,
    #[serde(default)]
    pub dye_separation_trim_green: f64,
    #[serde(default)]
    pub dye_separation_trim_blue: f64,
    #[serde(default)]
    pub separation_damping: f64,
    #[serde(default)]
    pub bw_filter: BwFilter,
}

impl ExposureConfig {
    /// Apply the legacy conversions the Python `__post_init__` runs on every build.
    ///
    /// A programmatic Rust build skips `serde`, so it calls this after it sets the fields;
    /// loading goes through `Deserialize`, which already normalized. The grade ladder is
    /// idempotent: a converted value is above the legacy ceiling.
    #[must_use]
    pub fn normalize(mut self) -> Self {
        if self.grade <= 5.0 {
            self.grade = 150.0 - 20.0 * self.grade;
        }
        self
    }
}

impl Default for ExposureConfig {
    fn default() -> Self {
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the exposure defaults deserialize")
    }
}

/// Legacy 0-5 paper grade converted with `R = 150 - 20*G`; ISO R starts at 50, so a stored
/// value at or below 5 is the old ladder.
fn deserialize_grade<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    Ok(if value <= 5.0 {
        150.0 - 20.0 * value
    } else {
        value
    })
}

const fn default_density() -> f64 {
    1.0
}

const fn default_grade() -> f64 {
    115.0
}

const fn default_toe_width() -> f64 {
    2.5
}

const fn default_shoulder_width() -> f64 {
    2.5
}

const fn default_cast_removal() -> f64 {
    0.5
}

const fn default_true() -> bool {
    true
}

const fn default_one() -> f64 {
    1.0
}

fn default_paper_profile() -> String {
    "neutral".to_string()
}
