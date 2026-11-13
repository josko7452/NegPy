//! Restore Range: scale a faded slide's density span through Red Survival.
//!
//! Mirrors `negpy/features/process/fade_auto_red.py`.

use crate::domain::models::{ProcessConfig, ProcessMode};

/// Percentiles of the mean density, light end first.
pub const RANGE_PCT: [f64; 13] = [
    0.5, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 95.0, 99.5,
];
/// Below this density span a frame has no range to read.
pub const RANGE_MIN_SPAN: f64 = 0.05;
/// The lowest Red Survival Restore Range sets.
pub const AUTO_RED_MIN: f64 = 0.4;
/// The density span Restore Range maps the frame's span onto.
pub const AUTO_RED_SPAN: f64 = 1.6;
/// The lowest density the shift leaves the 0.5 % point at.
pub const AUTO_RED_LIGHT: f64 = 0.2;
/// Estimate suggests Restore Range below this share of its own Red Survival.
pub const AUTO_RED_HINT_RATIO: f64 = 0.85;

const RED_SURVIVAL_BOUNDS: (f64, f64) = (0.05, 1.0);
const CLIPPED_LOG: f64 = -5.0;
const TRANSFER_DENSITY_RANGE: f64 = 3.0;

/// The per-cell mean density at [`RANGE_PCT`], light end first; `None` without a range.
#[must_use]
pub fn range_points(density: &[[f32; 3]]) -> Option<Vec<f64>> {
    let points = negpy_core::fade::mean_percentiles(density, &RANGE_PCT, 64)?;
    (points[points.len() - 1] - points[0] >= RANGE_MIN_SPAN).then_some(points)
}

fn point_offsets(process: &ProcessConfig) -> ([f64; 3], [f64; 3]) {
    let white = process.white_point_offset;
    let black = process.black_point_offset;
    (
        [
            white + process.white_point_trim_red,
            white + process.white_point_trim_green,
            white + process.white_point_trim_blue,
        ],
        [
            black + process.black_point_trim_red,
            black + process.black_point_trim_green,
            black + process.black_point_trim_blue,
        ],
    )
}

/// The meter grid through the slide's fixed window, as the transfer curve reads it.
#[must_use]
pub fn fade_range_points(log_grid: &[[f32; 3]], process: &ProcessConfig) -> Option<Vec<f64>> {
    let (wp, bp) = point_offsets(process);
    let floors = std::array::from_fn(|channel| (0.0 + -wp[channel]) as f32);
    let ceils = std::array::from_fn(|channel| (-TRANSFER_DENSITY_RANGE + -bp[channel]) as f32);
    let mut normalized = vec![0.0; log_grid.len() * 3];
    negpy_core::analysis::normalize_log_image(
        log_grid.as_flattened(),
        floors,
        ceils,
        &mut normalized,
    );
    let scaled: Vec<[f32; 3]> = normalized
        .chunks_exact(3)
        .map(|pixel| {
            [
                (f64::from(pixel[0]) * TRANSFER_DENSITY_RANGE) as f32,
                (f64::from(pixel[1]) * TRANSFER_DENSITY_RANGE) as f32,
                (f64::from(pixel[2]) * TRANSFER_DENSITY_RANGE) as f32,
            ]
        })
        .collect();
    range_points(&scaled)
}

/// [`fade_range_points`] of the Estimate grid after the fade and its shift.
#[must_use]
pub fn survival_points(
    grid: &[[f32; 3]],
    process: &ProcessConfig,
    survivals: [f64; 3],
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
    shift: f64,
) -> Option<Vec<f64>> {
    let white = measure_white(grid);
    let density = crate::features::process::fade::survival_density(
        survivals,
        white,
        white_anchor,
        camera_raw || !white_space.is_empty(),
        white_space,
        shift,
    );
    let faded = crate::features::process::fade::through_density(grid, &density);
    fade_range_points(&faded, process)
}

/// Each channel's white, measured on the grid; `None` with no usable cell.
#[must_use]
pub fn measure_white(grid: &[[f32; 3]]) -> Option<[f64; 3]> {
    negpy_core::fade::white(grid, CLIPPED_LOG, 99.9).map(|white| white.map(f64::from))
}

/// The changes Restore Range makes, or a reason it made none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoRed {
    pub red: f64,
    pub shift: f64,
}

/// The Red Survival that maps the frame's density span onto [`AUTO_RED_SPAN`], with the
/// shift that keeps its median where it printed.
#[must_use]
pub fn auto_red(
    points_at: impl Fn(f64) -> Option<Vec<f64>>,
    red: f64,
    shift: f64,
) -> Option<AutoRed> {
    let points = points_at(red)?;
    let span = points[points.len() - 1] - points[0];
    if span < RANGE_MIN_SPAN {
        return None;
    }
    let new = round2(
        (red * span / AUTO_RED_SPAN)
            .max(AUTO_RED_MIN)
            .max(RED_SURVIVAL_BOUNDS.0)
            .min(1.0)
            .min(RED_SURVIVAL_BOUNDS.1),
    );
    let mut gain = 0.0;
    if let Some(after) = points_at(new) {
        let median_after = median(&after);
        let median_before = median(&points);
        gain = (median_after - median_before)
            .min(after[0] - AUTO_RED_LIGHT.min(points[0]));
    }
    Some(AutoRed {
        red: new,
        shift: round4(shift - gain),
    })
}

/// Python `round` on a two-decimal value.
fn round2(value: f64) -> f64 {
    (value * 100.0).round_ties_even() / 100.0
}

fn median(points: &[f64]) -> f64 {
    let mut sorted = points.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round_ties_even() / 10_000.0
}

/// The config a fit applies to.
#[must_use]
pub fn with_auto_red(process: &ProcessConfig, fit: AutoRed) -> ProcessConfig {
    ProcessConfig {
        fade_ratio_r: fit.red,
        fade_density_shift: round4(fit.shift),
        fade_auto_red: true,
        ..process.clone()
    }
}

/// Whether a slide's density span suggests Restore Range at the current survivals.
#[must_use]
pub fn short_range(points: Option<&[f64]>) -> bool {
    let Some(points) = points else {
        return false;
    };
    let span = points[points.len() - 1] - points[0];
    span / AUTO_RED_SPAN <= 0.85 && span >= RANGE_MIN_SPAN
}

/// The mode Restore Range reads on.
#[must_use]
pub fn applies(mode: ProcessMode) -> bool {
    mode == ProcessMode::E6
}
