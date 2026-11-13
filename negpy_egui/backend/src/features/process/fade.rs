//! Fade Restoration helpers: the restoration matrix, its density anchor and white-space tag.

use sha1::{Digest, Sha1};

use crate::domain::models::ProcessConfig;
use crate::features::exposure::metering::effective_crosstalk_matrix;
use crate::features::process::capture_color::camera_matrix_parts;
use crate::features::process::path::{RenderPath, render_path};

/// The neutral log level Remove Stain maps the cleanest white to.
pub const FADE_WHITE_HEADROOM: f64 = 0.1;

/// A white a render measured for itself: red, green, blue and the space it was read in.
pub type MeasuredWhite = (f64, f64, f64, String);

/// The restoration matrix: the inverse of `diag(ar, ar*ag, ar*ab)`.
#[must_use]
pub fn resolve_fade_matrix(ratio_r: f64, ratio_g: f64, ratio_b: f64) -> [[f64; 3]; 3] {
    [
        [1.0 / ratio_r, 0.0, 0.0],
        [0.0, 1.0 / (ratio_r * ratio_g), 0.0],
        [0.0, 0.0, 1.0 / (ratio_r * ratio_b)],
    ]
}

/// The restoration matrix, or `None` at unit survival or on a negative.
#[must_use]
pub fn fade_matrix(process: &ProcessConfig) -> Option<[[f64; 3]; 3]> {
    let ratios = (
        process.fade_ratio_r,
        process.fade_ratio_g,
        process.fade_ratio_b,
    );
    if render_path(process) == RenderPath::Print
        || ratios.0 == 1.0 && ratios.1 == 1.0 && ratios.2 == 1.0
    {
        return None;
    }
    Some(resolve_fade_matrix(ratios.0, ratios.1, ratios.2))
}

/// The tag for the camera channels a log white is measured in.
///
/// Names the as-shot gains, the crosstalk unmix and the decode highlight mode.
#[must_use]
pub fn fade_white_space(
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
    crosstalk: Option<[[f32; 3]; 3]>,
    highlight_mode: i64,
) -> String {
    let Some(cam_xyz) = cam_xyz else {
        return String::new();
    };
    let (gains, primaries) = camera_matrix_parts(Some(cam_xyz), camera_wb);
    if primaries.is_none() {
        return String::new();
    }
    let mut named = b"camera-gains".to_vec();
    if let Some(gains) = gains {
        for value in gains {
            named.extend_from_slice(&value.round().to_le_bytes());
        }
    }
    if let Some(crosstalk) = crosstalk {
        named.extend_from_slice(b"unmix");
        for row in crosstalk {
            for value in row {
                named.extend_from_slice(&(value as f64).round().to_le_bytes());
            }
        }
    }
    named.extend_from_slice(b"highlight");
    named.push(highlight_mode as u8);
    let digest = Sha1::digest(&named);
    digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The white-space tag of a render, empty on a negative.
#[must_use]
pub fn render_white_space(
    process: &ProcessConfig,
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
) -> String {
    if render_path(process) == RenderPath::Print {
        return String::new();
    }
    let crosstalk = effective_crosstalk_matrix(process, Some(process.process_mode));
    fade_white_space(
        cam_xyz,
        camera_wb,
        crosstalk,
        crate::features::process::logic::effective_highlight_reconstruction(process),
    )
}

/// The stored per-channel white when it applies to this render.
#[must_use]
pub fn fade_white(process: &ProcessConfig, white_space: &str) -> Option<[f64; 3]> {
    if render_path(process) == RenderPath::Print || process.fade_white_space != white_space {
        return None;
    }
    let white = [
        process.fade_white_r,
        process.fade_white_g,
        process.fade_white_b,
    ];
    white.iter().any(|value| *value != 0.0).then_some(white)
}

/// The cleanest channel's white, capped below full scale.
#[must_use]
pub fn white_target(white: [f64; 3]) -> f64 {
    white
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .min(-FADE_WHITE_HEADROOM)
}

/// The neutral log level Remove Stain maps the stored white to.
#[must_use]
pub fn fade_white_target(process: &ProcessConfig, white_space: &str) -> Option<f64> {
    let white = process
        .fade_white_anchor
        .then(|| fade_white(process, white_space))??;
    Some(white_target(white))
}

/// Whether a render needs its white measured again because the space changed.
#[must_use]
pub fn fade_white_stale(process: &ProcessConfig, white_space: &str) -> bool {
    if white_space.is_empty()
        || render_path(process) == RenderPath::Print
        || process.fade_white_space == white_space
    {
        return false;
    }
    process.fade_white_anchor || fade_matrix(process).is_some()
}

/// The log point a camera RAW's fade scales density about.
#[must_use]
pub fn density_anchor(white: [f64; 3], remove_stain: bool) -> [f64; 3] {
    if remove_stain {
        return white;
    }
    let level = white.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    [level, level, level]
}

/// `process` carrying a measured white; unchanged with none.
#[must_use]
pub fn with_fade_white(process: &ProcessConfig, white: Option<MeasuredWhite>) -> ProcessConfig {
    let Some((red, green, blue, space)) = white else {
        return process.clone();
    };
    let mut updated = process.clone();
    updated.fade_white_r = red;
    updated.fade_white_g = green;
    updated.fade_white_b = blue;
    updated.fade_white_space = space;
    updated
}
