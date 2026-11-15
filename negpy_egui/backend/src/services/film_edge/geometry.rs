//! Mirrors `negpy/services/film_edge/geometry.py`: film strip geometry in millimeters.

use crate::string_enum;

/// 135 (ISO 1007, Kodak KS perforation).
pub const FILM_135_WIDTH: f64 = 35.0;
pub const FRAME_135_PITCH: f64 = 38.0;
pub const HALF_FRAME_PITCH: f64 = 19.0;
pub const PERF_ALONG: f64 = 1.98;
pub const PERF_ACROSS: f64 = 2.80;
pub const PERF_RADIUS: f64 = 0.5;
pub const PERF_FROM_EDGE: f64 = 2.0;
pub const PERF_PITCH: f64 = 4.75;
/// Half a pitch puts every cut, at a frame-gap center, between two holes.
pub const PERF_PHASE: f64 = PERF_PITCH / 2.0;
pub const EDGE_BAND_135: f64 = 2.0;

/// 120 (ISO 732). `FRAME_GAP_120` is typical; the camera sets the real gap.
pub const FILM_120_WIDTH: f64 = 61.0;
pub const FRAME_120_ACROSS: f64 = 56.0;
pub const FRAME_GAP_120: f64 = 4.0;
pub const DEFAULT_FRAME_120: &str = "6×6";

/// The 120 frame widths, keyed by the label the UI shows.
pub const FRAME_SIZES_120: [(&str, f64); 7] = [
    ("6×4.5", 41.5),
    ("6×6", 56.0),
    ("6×7", 70.0),
    ("6×8", 77.0),
    ("6×9", 84.0),
    ("6×12", 112.0),
    ("6×17", 168.0),
];

string_enum! {
    /// The film format a sheet lays strips from.
    pub enum SheetFormat {
        FullFrame => "35mm",
        HalfFrame => "35mm Half Frame",
        Medium => "120",
    }
    default FullFrame
}

/// One film format's strip dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct FilmGeometry {
    pub format: SheetFormat,
    pub width: f64,
    pub frame_along: f64,
    pub frame_across: f64,
    pub pitch: f64,
    pub frame_size: String,
}

impl FilmGeometry {
    /// Whether the film carries sprocket holes.
    #[must_use]
    pub fn perforated(&self) -> bool {
        self.format != SheetFormat::Medium
    }

    /// The rebate between a film edge and the perforations (135) or the image (120).
    #[must_use]
    pub fn edge_band(&self) -> f64 {
        if self.perforated() {
            EDGE_BAND_135
        } else {
            (self.width - self.frame_across) / 2.0
        }
    }

    /// The top of the image area inside a 120 strip.
    #[must_use]
    pub fn image_top(&self) -> f64 {
        (self.width - self.frame_across) / 2.0
    }

    /// The frame's width over its height.
    #[must_use]
    pub fn window_aspect(&self) -> f64 {
        self.frame_along / self.frame_across
    }

    /// The center of frame `index` along the strip.
    #[must_use]
    pub fn frame_center(&self, index: usize) -> f64 {
        self.pitch * (index as f64 + 0.5)
    }
}

/// The strip geometry of one format; an unknown 120 size falls back to 6×6.
#[must_use]
pub fn film_geometry(format: SheetFormat, frame_size: &str) -> FilmGeometry {
    match format {
        SheetFormat::HalfFrame => FilmGeometry {
            format,
            width: FILM_135_WIDTH,
            frame_along: 18.0,
            frame_across: 24.0,
            pitch: HALF_FRAME_PITCH,
            frame_size: String::new(),
        },
        SheetFormat::Medium => {
            let (label, along) = FRAME_SIZES_120
                .iter()
                .find(|(label, _)| *label == frame_size)
                .copied()
                .unwrap_or((DEFAULT_FRAME_120, 56.0));
            FilmGeometry {
                format,
                width: FILM_120_WIDTH,
                frame_along: along,
                frame_across: FRAME_120_ACROSS,
                pitch: along + FRAME_GAP_120,
                frame_size: label.to_string(),
            }
        }
        SheetFormat::FullFrame => FilmGeometry {
            format,
            width: FILM_135_WIDTH,
            frame_along: 36.0,
            frame_across: 24.0,
            pitch: FRAME_135_PITCH,
            frame_size: String::new(),
        },
    }
}

/// Perforation centers in `[x0, x1]` along the strip.
#[must_use]
pub fn perforation_centers(x0: f64, x1: f64) -> Vec<f64> {
    let first = ((x0 - PERF_PHASE) / PERF_PITCH).ceil() as i64;
    let last = ((x1 - PERF_PHASE) / PERF_PITCH).floor() as i64;
    if last < first {
        return Vec::new();
    }
    (first..=last)
        .map(|index| PERF_PHASE + index as f64 * PERF_PITCH)
        .collect()
}
