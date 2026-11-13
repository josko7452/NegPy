//! Fade Restoration helpers: the restoration matrix, its density anchor and white-space tag.

use sha1::{Digest, Sha1};

use crate::domain::models::ProcessConfig;
use crate::features::exposure::metering::effective_crosstalk_matrix;
use crate::features::process::capture_color::camera_matrix_parts;
use crate::features::process::path::{RenderPath, render_path};

/// The neutral log level Remove Stain maps the cleanest white to.
pub const FADE_WHITE_HEADROOM: f64 = 0.1;

/// Values per band in a stored crossover table: mean log, share, then the lean of each channel.
pub const CROSSOVER_BAND_SIZE: usize = 5;
/// Below this spread a band's lean has no crossover to fix.
pub const CROSSOVER_FLOOR: f64 = 0.04;
/// The three channel weights a lean sums to zero under.
pub const CROSSOVER_WEIGHTS: [f64; 3] = [1.0 / 3.0; 3];

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

/// A stored table's bands as rows of `(mean log, share, lean r, g, b)`, dense end first.
#[must_use]
pub fn crossover_bands(table: &[f64]) -> Vec<[f64; CROSSOVER_BAND_SIZE]> {
    table
        .get(1..)
        .unwrap_or_default()
        .chunks_exact(CROSSOVER_BAND_SIZE)
        .map(|row| std::array::from_fn(|index| row[index]))
        .collect()
}

/// The mean log a stored table is zero at.
#[must_use]
pub fn crossover_zero(table: &[f64]) -> f64 {
    table.first().copied().unwrap_or(0.0)
}

/// Fritsch-Carlson monotone cubic through `(xs, ys)`, flat beyond the ends.
#[must_use]
pub fn shape_cubic(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len().min(ys.len());
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return ys[0];
    }
    let h: Vec<f64> = (0..n - 1).map(|index| xs[index + 1] - xs[index]).collect();
    let delta: Vec<f64> = (0..n - 1)
        .map(|index| (ys[index + 1] - ys[index]) / h[index])
        .collect();
    let mut d = vec![0.0; n];
    d[0] = delta[0];
    d[n - 1] = delta[n - 2];
    for index in 1..n - 1 {
        if delta[index - 1] * delta[index] > 0.0 {
            let w1 = 2.0 * h[index] + h[index - 1];
            let w2 = h[index] + 2.0 * h[index - 1];
            d[index] = (w1 + w2) / (w1 / delta[index - 1] + w2 / delta[index]);
        }
    }
    let xc = x.clamp(xs[0], xs[n - 1]);
    let index = xs[..n]
        .partition_point(|value| *value <= xc)
        .saturating_sub(1)
        .min(n - 2);
    let t = (xc - xs[index]) / h[index];
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * ys[index]
        + (t3 - 2.0 * t2 + t) * h[index] * d[index]
        + (-2.0 * t3 + 3.0 * t2) * ys[index + 1]
        + (t3 - t2) * h[index] * d[index + 1]
}

/// The lean at the table's zero, where the survivals were fit.
#[must_use]
pub fn crossover_reference(table: &[f64]) -> [f64; 3] {
    let rows = crossover_bands(table);
    let xs: Vec<f64> = rows.iter().map(|row| row[0]).collect();
    let zero = crossover_zero(table);
    std::array::from_fn(|channel| {
        let ys: Vec<f64> = rows.iter().map(|row| row[2 + channel]).collect();
        shape_cubic(&xs, &ys, zero)
    })
}

/// The largest channel spread of a band's lean away from the reference, in D.
#[must_use]
pub fn crossover_amount(table: Option<&[f64]>) -> f64 {
    let Some(table) = table.filter(|table| !table.is_empty()) else {
        return 0.0;
    };
    let reference = crossover_reference(table);
    crossover_bands(table)
        .iter()
        .map(|row| {
            let values: [f64; 3] =
                std::array::from_fn(|channel| row[2 + channel] - reference[channel]);
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - values.iter().copied().fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}

/// The share-weighted channel spread of a table's neutral bands.
#[must_use]
pub fn neutral_spread(table: &[f64]) -> f64 {
    let rows = crossover_bands(table);
    let total: f64 = rows.iter().map(|row| row[1]).sum();
    if total == 0.0 {
        return 0.0;
    }
    let weighted: f64 = rows
        .iter()
        .map(|row| {
            let hi = row[2..].iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let lo = row[2..].iter().copied().fold(f64::INFINITY, f64::min);
            row[1] * (hi - lo)
        })
        .sum();
    weighted / total
}

/// The `(shadow, highlight)` share-weighted lean beyond the reference.
#[must_use]
pub fn crossover_casts(table: Option<&[f64]>) -> (Option<[f64; 3]>, Option<[f64; 3]>) {
    let Some(table) = table.filter(|table| !table.is_empty()) else {
        return (None, None);
    };
    let rows = crossover_bands(table);
    let Some(first) = rows.first() else {
        return (None, None);
    };
    let last = rows.last().copied().unwrap_or(*first);
    let span = (last[0] - first[0]).max(1e-12);
    let group = |lo: f64, hi: f64| -> Option<[f64; 3]> {
        let selected: Vec<&[f64; CROSSOVER_BAND_SIZE]> = rows
            .iter()
            .filter(|row| {
                let t = (row[0] - first[0]) / span;
                t >= lo && t <= hi
            })
            .collect();
        if selected.is_empty() {
            return None;
        }
        let total: f64 = selected.iter().map(|row| row[1]).sum();
        if total == 0.0 {
            return None;
        }
        Some(std::array::from_fn(|channel| {
            selected
                .iter()
                .map(|row| row[1] * row[2 + channel])
                .sum::<f64>()
                / total
        }))
    };
    (group(0.0, 0.35), group(0.6, 1.0))
}

const HUE_NAMES: [&str; 6] = ["red", "yellow", "green", "cyan", "blue", "magenta"];

/// The hue a lean tilts toward, or `None` under the crossover floor.
#[must_use]
pub fn cast_name(lean: Option<[f64; 3]>) -> Option<&'static str> {
    let [r, g, b] = lean?;
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = hi - lo;
    if chroma < CROSSOVER_FLOOR {
        return None;
    }
    let hue = if hi == r {
        ((g - b) / chroma).rem_euclid(6.0)
    } else if hi == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    let index = ((hue + 0.5).floor() as i64).rem_euclid(6) as usize;
    Some(HUE_NAMES[index])
}
