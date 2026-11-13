//! Bracket statistics, exposure ordering and render-exposure choices.

use crate::domain::models::hdr::{ANCHOR_EV_UNSET, HdrConfig};

/// A sample at or above this is saturated and carries no information.
pub const SATURATION: f64 = 0.995;
/// A frame with more than this fraction of saturated pixels cannot define white.
pub const MAX_REFERENCE_CLIPPING: f64 = 0.001;
/// Samples below this are noise in both frames of a ratio fit.
pub const RATIO_FLOOR: f64 = 0.02;
/// Pixel budget for distribution measurements.
pub const MEASURE_PIXELS: usize = 1_500_000;
/// Shadows Density seeded per stop of recovered shadow reach.
pub const SEED_SHADOW_PER_STOP: f64 = 0.30;
/// Cap on the seeded shadow lift.
pub const SEED_SHADOW_LIMIT: f64 = 0.70;

/// One bracket frame's measured exposure.
#[derive(Clone, Debug, PartialEq)]
pub struct ExposureStats {
    pub path: String,
    /// Robust brightness, monotonic in exposure time.
    pub level: f64,
    /// Fraction of samples at or above [`SATURATION`].
    pub clipped: f64,
}

/// The stride that leaves about [`MEASURE_PIXELS`] pixels.
#[must_use]
pub fn measure_step(height: usize, width: usize) -> usize {
    let pixels = height.saturating_mul(width);
    if pixels <= MEASURE_PIXELS {
        return 1;
    }
    ((pixels as f64 / MEASURE_PIXELS as f64).sqrt() as usize).max(1)
}

/// Sort key that orders a bracket by exposure, shortest first.
#[must_use]
pub fn exposure_order_key(level: f64, clipped: f64) -> (f64, f64) {
    (clipped, level)
}

/// Index of the frame that should define white.
///
/// Returns the longest exposure that does not clip, else the least-clipped one. An empty
/// bracket has no reference.
#[must_use]
pub fn choose_reference(stats: &[ExposureStats]) -> Option<usize> {
    if stats.is_empty() {
        return None;
    }
    let usable: Vec<usize> = stats
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.clipped <= MAX_REFERENCE_CLIPPING).then_some(index))
        .collect();
    if !usable.is_empty() {
        return usable.into_iter().max_by(|&left, &right| {
            exposure_order_key(stats[left].level, stats[left].clipped)
                .partial_cmp(&exposure_order_key(
                    stats[right].level,
                    stats[right].clipped,
                ))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    (0..stats.len()).min_by(|&left, &right| {
        stats[left]
            .clipped
            .partial_cmp(&stats[right].clipped)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Gain that decides which exposure the merged result renders at.
#[must_use]
pub fn output_scale(ratios: &[f64], anchor: Option<f64>) -> f64 {
    if let Some(anchor) = anchor.filter(|value| *value > 0.0) {
        return anchor.min(1.0);
    }
    let mut positive: Vec<f64> = ratios
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .collect();
    if positive.is_empty() {
        return 1.0;
    }
    positive.sort_by(f64::total_cmp);
    let middle = positive.len() / 2;
    let median = if positive.len() % 2 == 0 {
        (positive[middle - 1] * positive[middle]).sqrt()
    } else {
        positive[middle]
    };
    median.min(1.0)
}

/// Ratio of the frame the merge should render at, or `None` for the bracket middle.
#[must_use]
pub fn anchor_ratio(paths: &[String], ratios: &[f64], anchor_path: &str) -> Option<f64> {
    if anchor_path.is_empty() {
        return None;
    }
    paths
        .iter()
        .zip(ratios)
        .find_map(|(path, ratio)| (path == anchor_path && *ratio > 0.0).then_some(*ratio))
}

/// The exposure a merge renders at, as a ratio, however the user asked for it.
#[must_use]
pub fn resolve_anchor(paths: &[String], ratios: &[f64], config: &HdrConfig) -> Option<f64> {
    if config.hdr_anchor_ev < ANCHOR_EV_UNSET {
        return Some(2.0_f64.powf(config.hdr_anchor_ev));
    }
    anchor_ratio(paths, ratios, &config.hdr_anchor)
}

/// (path, stops from the reference) for every frame that can be the render exposure.
#[must_use]
pub fn anchor_choices(paths: &[String], ratios: &[f64]) -> Vec<(String, f64)> {
    let mut usable: Vec<(String, f64)> = paths
        .iter()
        .zip(ratios)
        .filter(|(_, ratio)| **ratio > 0.0 && **ratio <= 1.0)
        .map(|(path, ratio)| (path.clone(), *ratio))
        .collect();
    usable.sort_by(|left, right| right.1.total_cmp(&left.1));
    usable
        .into_iter()
        .map(|(path, ratio)| (path, ratio.log2()))
        .collect()
}

/// Stops of shadow the merge reaches below the exposure it renders at.
#[must_use]
pub fn shadow_reach_stops(ratios: &[f64], anchor: Option<f64>) -> f64 {
    let longest = ratios
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .fold(0.0_f64, f64::max);
    if longest <= 0.0 {
        return 0.0;
    }
    (longest / output_scale(ratios, anchor)).log2().max(0.0)
}

/// Shadows Density a fresh merge opens with.
#[must_use]
pub fn seed_shadow_density(ratios: &[f64], anchor: Option<f64>) -> f64 {
    let stops = shadow_reach_stops(ratios, anchor);
    if stops <= 0.0 {
        return 0.0;
    }
    -(SEED_SHADOW_PER_STOP * stops).min(SEED_SHADOW_LIMIT)
}
