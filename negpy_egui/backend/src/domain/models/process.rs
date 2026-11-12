//! Mirrors `negpy/features/process/models.py`: the film/sensor capture parameters.

use serde::{Deserialize, Serialize};

use crate::domain::models::zero_triple;
use crate::string_enum;

string_enum! {
    /// Film chemistry. Legacy codes and anything stale read as a color negative, the way
    /// every pipeline branch's `else` does.
    pub enum ProcessMode {
        C41 => "Color Negative",
        BW => "B&W Negative",
        E6 => "Transparency",
    }
    aliases { "C41" => C41, "B&W" => BW, "E-6" => E6 }
    default C41
}

string_enum! {
    /// How the sensor matrix is applied to the linear capture.
    pub enum SensorUnmix {
        Linear => "linear",
        TwoScale => "two_scale",
        Density => "density",
    }
    default TwoScale
}

string_enum! {
    /// CFA interpolation; `Auto` keeps NegPy's own choice per path.
    pub enum DemosaicMode {
        Auto => "Auto",
        Linear => "Linear",
        Vng => "VNG",
        Ppg => "PPG",
        Ahd => "AHD",
        Dcb => "DCB",
        Dht => "DHT",
        Aahd => "AAHD",
    }
    default Auto
}

string_enum! {
    /// How a Positive TIFF's samples encode light.
    pub enum SourceEncoding {
        Auto => "auto",
        Linear => "linear",
        Gamma22 => "gamma22",
    }
    default Auto
}

/// Built-in fallback crosstalk matrix (row-major 3x3) used when no profile is baked.
pub const DEFAULT_CROSSTALK_MATRIX: [f64; 9] =
    [1.0, -0.05, -0.02, -0.04, 1.0, -0.08, -0.01, -0.1, 1.0];

/// Core film/sensor processing parameters.
///
/// A JSON-loaded list is a sequence of floats, so the per-channel triples and pick
/// positions are `Vec<f64>`; the fixed triples keep their three slots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcessConfig {
    #[serde(default)]
    pub process_mode: ProcessMode,
    /// Measured once on load, stored: whether the scan is of color film. `None` = not
    /// measured yet.
    #[serde(default)]
    pub source_color: Option<bool>,
    #[serde(default)]
    pub linear_raw: bool,
    /// The source is a finished positive rather than a raw capture. Slide only.
    #[serde(default)]
    pub positive_source: bool,
    #[serde(default)]
    pub source_encoding: SourceEncoding,
    /// Render the slide in black and white. Slide only.
    #[serde(default)]
    pub slide_bw: bool,
    #[serde(default)]
    pub demosaic_preview: DemosaicMode,
    #[serde(default)]
    pub demosaic_export: DemosaicMode,
    /// libraw HighlightMode: 0 = clip, 2 = blend, 3-9 = reconstruct(level).
    #[serde(default)]
    pub highlight_reconstruction: i64,
    #[serde(default = "default_analysis_buffer")]
    pub analysis_buffer: f64,
    /// Optional freehand analysis region, normalized in the transformed image.
    #[serde(default)]
    pub analysis_rect: Option<Vec<f64>>,
    /// The picture box the meters read on a frame with no crop. `None` meters the frame.
    #[serde(default)]
    pub meter_rect: Option<Vec<f64>>,
    #[serde(default)]
    pub meter_key: String,
    /// The clear film base picked on this frame with no roll open, as per-channel raw
    /// log10 transmittance.
    #[serde(default)]
    pub film_base: Option<Vec<f64>>,
    /// Manual Pick Points: per-channel log10 in the bounds (unmixed) space.
    #[serde(default)]
    pub manual_dmin: Option<Vec<f64>>,
    #[serde(default)]
    pub manual_dmax: Option<Vec<f64>>,
    /// Where each end was picked, content-normalized in the uncropped pick-tool view.
    #[serde(default)]
    pub manual_dmin_at: Option<Vec<f64>>,
    #[serde(default)]
    pub manual_dmax_at: Option<Vec<f64>>,
    #[serde(default)]
    pub luma_range_clip: f64,
    #[serde(default = "default_color_range_clip")]
    pub color_range_clip: f64,
    #[serde(default)]
    pub use_luma_average: bool,
    #[serde(default)]
    pub use_color_average: bool,
    #[serde(default = "zero_triple")]
    pub locked_floors: [f64; 3],
    #[serde(default = "zero_triple")]
    pub locked_ceils: [f64; 3],
    #[serde(default = "zero_triple")]
    pub local_floors: [f64; 3],
    #[serde(default = "zero_triple")]
    pub local_ceils: [f64; 3],
    #[serde(default)]
    pub use_cast_average: bool,
    /// The pooled neutral axis in raw log: `(midtone, shadow, highlight, scalars...)`.
    /// Kept opaque so a nested pooled axis round-trips unchanged.
    #[serde(default)]
    pub locked_neutral_axis: Option<serde_json::Value>,
    #[serde(default)]
    pub white_point_offset: f64,
    #[serde(default)]
    pub black_point_offset: f64,
    #[serde(default)]
    pub white_point_trim_red: f64,
    #[serde(default)]
    pub white_point_trim_green: f64,
    #[serde(default)]
    pub white_point_trim_blue: f64,
    #[serde(default)]
    pub black_point_trim_red: f64,
    #[serde(default)]
    pub black_point_trim_green: f64,
    #[serde(default)]
    pub black_point_trim_blue: f64,
    #[serde(default)]
    pub crosstalk_strength: f64,
    #[serde(default)]
    pub crosstalk_matrix: Option<Vec<f64>>,
    #[serde(default = "default_crosstalk_profile")]
    pub crosstalk_profile: String,
    #[serde(default = "default_crosstalk_process")]
    pub crosstalk_process: String,
    #[serde(default = "default_one")]
    pub fade_ratio_r: f64,
    #[serde(default)]
    pub fade_density_shift: f64,
    #[serde(default = "default_one")]
    pub fade_ratio_g: f64,
    #[serde(default = "default_one")]
    pub fade_ratio_b: f64,
    #[serde(default)]
    pub fade_white_anchor: bool,
    #[serde(default)]
    pub fade_white_r: f64,
    #[serde(default)]
    pub fade_white_g: f64,
    #[serde(default)]
    pub fade_white_b: f64,
    #[serde(default)]
    pub fade_white_space: String,
    #[serde(default)]
    pub fade_crossover: f64,
    #[serde(default)]
    pub fade_crossover_table: Option<Vec<f64>>,
    #[serde(default)]
    pub fade_crossover_space: String,
    #[serde(default)]
    pub fade_auto_red: bool,
    #[serde(default)]
    pub fade_neutrals_mix: f64,
    /// Sensor (CFA) crosstalk unmix on the linear capture. 9 floats row-major; `None` = off.
    #[serde(default)]
    pub sensor_matrix: Option<Vec<f64>>,
    #[serde(default = "default_sensor_profile")]
    pub sensor_profile: String,
    #[serde(default)]
    pub sensor_unmix: SensorUnmix,
    /// Light-source hue rotation in degrees in CIELAB a*b*; 0.0 = off.
    #[serde(default)]
    pub hue_trim: f64,
    #[serde(default)]
    pub lock_bounds: bool,
    #[serde(default)]
    pub roll_name: Option<String>,
    /// Where `locked_floors`/`locked_ceils` came from.
    #[serde(default)]
    pub baseline_source: String,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        // Deserializing `{}` applies every field default, so the two paths cannot drift.
        serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))
            .expect("the process defaults deserialize")
    }
}

const fn default_analysis_buffer() -> f64 {
    0.05
}

const fn default_color_range_clip() -> f64 {
    1.0
}

const fn default_one() -> f64 {
    1.0
}

fn default_crosstalk_profile() -> String {
    "Generic C41".to_string()
}

fn default_crosstalk_process() -> String {
    ProcessMode::C41.as_str().to_string()
}

fn default_sensor_profile() -> String {
    "None".to_string()
}
