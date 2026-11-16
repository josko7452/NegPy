//! Scanner transport parameters and the film-type rules.
//!
//! Mirrors `negpy/infrastructure/scanners/params.py`.

use serde::{Deserialize, Serialize};

use crate::string_enum;

string_enum! {
    /// What the scanner reports as its source.
    pub enum ScanMode {
        Negative => "Negative",
        Positive => "Positive",
        Transparency => "Transparency",
    }
    default Negative
}

string_enum! {
    /// Whether a short+long exposure pair is merged.
    pub enum MultiExposureMode {
        Off => "off",
        Adaptive => "adaptive",
        Fixed => "fixed",
    }
    default Off
}

string_enum! {
    /// What is on the film, as the transports spell it.
    pub enum FilmType {
        Negative => "negative",
        Mono => "mono",
        Positive => "positive",
        Kodachrome => "kodachrome",
    }
    default Negative
}

pub const MIN_N_PASSES: i64 = 1;
pub const MAX_N_PASSES: i64 = 9;
pub const DEFAULT_N_PASSES: i64 = 3;
pub const ANALOG_GAIN_EV_RANGE: (f64, f64) = (-2.0, 2.0);
pub const MIN_FRAME_EXTENT_MM: f64 = 1.0;
pub const CANONICAL_DPI_STOPS: [i64; 11] =
    [75, 150, 300, 600, 1200, 2400, 3600, 4800, 6400, 7200, 9600];

/// A normalized `(x1, y1, x2, y2)` scan window.
pub type ScanArea = [f64; 4];

/// One scan request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScanParams {
    pub dpi: i64,
    pub depth: i64,
    #[serde(default)]
    pub capture_ir: bool,
    #[serde(default)]
    pub multi_exposure_mode: MultiExposureMode,
    #[serde(default = "default_n_passes")]
    pub n_passes: i64,
    #[serde(default)]
    pub window: Option<ScanArea>,
    #[serde(default)]
    pub frame_offset_mm: f64,
    #[serde(default = "default_true")]
    pub autofocus: bool,
    #[serde(default)]
    pub frame: Option<i64>,
    #[serde(default)]
    pub auto_exposure: bool,
    #[serde(default)]
    pub exposure_time_us: Option<i64>,
    #[serde(default)]
    pub clean: bool,
    #[serde(default = "default_one")]
    pub samples: i64,
    #[serde(default)]
    pub superfine: bool,
    #[serde(default)]
    pub film_format: Option<String>,
    #[serde(default)]
    pub film_type: FilmType,
    #[serde(default)]
    pub exposures: Option<std::collections::BTreeMap<String, i64>>,
    #[serde(default)]
    pub analog_gain_ev: f64,
    #[serde(default)]
    pub channel_gain_ev: [f64; 3],
}

impl Default for ScanParams {
    fn default() -> Self {
        Self {
            dpi: 0,
            depth: 16,
            capture_ir: false,
            multi_exposure_mode: MultiExposureMode::Off,
            n_passes: MIN_N_PASSES,
            window: None,
            frame_offset_mm: 0.0,
            autofocus: true,
            frame: None,
            auto_exposure: false,
            exposure_time_us: None,
            clean: false,
            samples: 1,
            superfine: false,
            film_format: None,
            film_type: FilmType::Negative,
            exposures: None,
            analog_gain_ev: 0.0,
            channel_gain_ev: [0.0; 3],
        }
    }
}

impl ScanParams {
    /// Whether any Analog Gain control asks for a change.
    #[must_use]
    pub fn has_analog_gain(&self) -> bool {
        self.analog_gain_ev != 0.0 || self.channel_gain_ev.iter().any(|value| *value != 0.0)
    }
}

const fn default_one() -> i64 {
    1
}

const fn default_true() -> bool {
    true
}

const fn default_n_passes() -> i64 {
    MIN_N_PASSES
}

/// Whether infrared can see dust through this film.
#[must_use]
pub fn film_passes_infrared(film_type: &str) -> bool {
    !matches!(film_type, "mono" | "kodachrome")
}

/// Reversal stock develops its unexposed film to maximum density.
#[must_use]
pub fn film_reads_positive(film_type: &str) -> bool {
    matches!(film_type, "positive" | "kodachrome")
}

/// Canonical stops a `(min, max)` hardware range covers, or the whole ladder when it covers none.
#[must_use]
pub fn dpi_stops_in_range(lo: f64, hi: f64) -> Vec<i64> {
    let stops: Vec<i64> = CANONICAL_DPI_STOPS
        .into_iter()
        .filter(|stop| lo <= *stop as f64 && (*stop as f64) <= hi)
        .collect();
    if stops.is_empty() {
        CANONICAL_DPI_STOPS.to_vec()
    } else {
        stops
    }
}

/// Clamp a normalized rect to `0..1` with a tiny positive size.
#[must_use]
pub fn clamp_scan_area(area: ScanArea) -> ScanArea {
    let [mut x1, mut y1, mut x2, mut y2] = area;
    x1 = x1.clamp(0.0, 1.0);
    y1 = y1.clamp(0.0, 1.0);
    x2 = x2.clamp(0.0, 1.0);
    y2 = y2.clamp(0.0, 1.0);
    if x2 <= x1 {
        x2 = (x1 + 1e-3).min(1.0);
    }
    if y2 <= y1 {
        y2 = (y1 + 1e-3).min(1.0);
    }
    [x1, y1, x2, y2]
}

/// Map Prescan coordinates to the transport area; self-inverse on a fixed mirror.
#[must_use]
pub fn crop_to_scan_window(crop: ScanArea, mirror_x: bool) -> ScanArea {
    let [x1, y1, x2, y2] = clamp_scan_area(crop);
    if mirror_x {
        [1.0 - x2, y1, 1.0 - x1, y2]
    } else {
        [x1, y1, x2, y2]
    }
}

/// The feed-axis offset, floored at 0 and held short of one frame pitch.
#[must_use]
pub fn clamp_frame_offset_mm(offset_mm: f64, pitch_mm: f64) -> f64 {
    let offset = offset_mm.max(0.0);
    if pitch_mm <= 0.0 {
        return offset;
    }
    offset.min((pitch_mm - MIN_FRAME_EXTENT_MM).max(0.0))
}

/// A normalized scan window in approximate millimetres, for the UI readout.
#[must_use]
pub fn scan_window_to_area(rect: Option<ScanArea>, max_area_mm: (f64, f64)) -> Option<ScanArea> {
    let [x1, y1, x2, y2] = rect?;
    let (width, height) = max_area_mm;
    Some([x1 * width, y1 * height, x2 * width, y2 * height])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clamp_enforces_a_positive_extent() {
        assert_eq!(
            clamp_scan_area([0.5, 0.5, 0.5, 0.5]),
            [0.5, 0.5, 0.501, 0.501]
        );
    }

    #[test]
    fn a_mirrored_crop_reflects_x_and_returns() {
        let crop = [0.1, 0.2, 0.4, 0.8];
        let reflected = crop_to_scan_window(crop, true);
        assert_eq!(reflected, [0.6, 0.2, 0.9, 0.8]);
        let back = crop_to_scan_window(reflected, true);
        assert!((back[0] - crop[0]).abs() < 1e-12);
        assert!((back[2] - crop[2]).abs() < 1e-12);
    }

    #[test]
    fn the_offset_never_reaches_a_frame_pitch() {
        assert!((clamp_frame_offset_mm(50.0, 37.83) - 36.83).abs() < 1e-9);
        assert_eq!(clamp_frame_offset_mm(-2.0, 37.83), 0.0);
        assert_eq!(clamp_frame_offset_mm(50.0, 0.0), 50.0);
    }

    #[test]
    fn a_range_with_no_stop_gets_the_whole_ladder() {
        assert_eq!(dpi_stops_in_range(10.0, 20.0), CANONICAL_DPI_STOPS.to_vec());
        assert_eq!(dpi_stops_in_range(300.0, 1200.0), vec![300, 600, 1200]);
    }

    #[test]
    fn silver_and_kodachrome_block_infrared() {
        assert!(film_passes_infrared("negative"));
        assert!(!film_passes_infrared("mono"));
        assert!(!film_passes_infrared("kodachrome"));
        assert!(film_reads_positive("kodachrome"));
    }
}
