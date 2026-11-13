//! Bracket statistics, exposure ordering and render-exposure choices.

use negpy_core::warp::{self, Border, Image, Interp};

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
/// Weight rolls off linearly from here to saturation.
pub const ROLLOFF_START: f64 = 0.90;
const EST_WIDTH: usize = 1024;

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
    let median = if positive.len().is_multiple_of(2) {
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

/// A decoded HxWx3 frame with float RGB samples.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<f32>,
}

impl Frame {
    #[must_use]
    pub fn new(width: usize, height: usize, pixels: Vec<f32>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }
}

/// A uint16 decode as float32 in 0..1, RGB only.
#[must_use]
pub fn to_float(width: usize, height: usize, samples: &[u16]) -> Frame {
    let pixels = samples
        .as_chunks::<3>()
        .0
        .iter()
        .take(width * height)
        .flat_map(|pixel| pixel.iter().map(|value| f32::from(*value) / 65535.0))
        .collect();
    Frame::new(width, height, pixels)
}

/// Fraction of pixels saturated in any channel, over the whole frame.
#[must_use]
pub fn clipped_fraction(frame: &Frame) -> f64 {
    let mut rows = vec![0i64; frame.height];
    negpy_core::hdr::count_clipped_rows(&frame.pixels, (frame.width, 3), SATURATION, &mut rows);
    rows.iter().sum::<i64>() as f64 / (frame.width * frame.height).max(1) as f64
}

/// `(level, clipped fraction)` for one decoded frame.
#[must_use]
pub fn measure_exposure(frame: &Frame) -> (f64, f64) {
    let step = measure_step(frame.height, frame.width);
    let level =
        negpy_core::hdr::exposure_level(&frame.pixels, (frame.height, frame.width, 3), step, 99.0)
            .unwrap_or(0.0);
    (f64::from(level), clipped_fraction(frame))
}

/// Median of long / short over samples above the noise floor and below saturation.
#[must_use]
pub fn pair_ratio(short: &Frame, long: &Frame) -> Option<f64> {
    if short.width != long.width || short.height != long.height {
        return None;
    }
    let step = measure_step(short.height, short.width);
    negpy_core::hdr::pair_ratio(
        &short.pixels,
        &long.pixels,
        (short.height, short.width, 3),
        step,
        (RATIO_FLOOR as f32, SATURATION as f32),
    )
}

/// Per-frame exposure relative to `frames[reference]`.
#[must_use]
pub fn solve_ratios(frames: &[Frame], reference: usize) -> Vec<f64> {
    let measured: Vec<(f64, f64)> = frames.iter().map(measure_exposure).collect();
    let levels: Vec<f64> = measured.iter().map(|(level, _)| *level).collect();
    let mut order: Vec<usize> = (0..frames.len()).collect();
    order.sort_by(|&left, &right| {
        exposure_order_key(measured[left].0, measured[left].1)
            .partial_cmp(&exposure_order_key(measured[right].0, measured[right].1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let Some(position) = order.iter().position(|index| *index == reference) else {
        return vec![1.0; frames.len()];
    };
    let mut ratios = vec![1.0; frames.len()];
    let step = |short: usize, long: usize| {
        pair_ratio(&frames[short], &frames[long])
            .unwrap_or_else(|| levels[long] / levels[short].max(1e-9))
    };
    for index in position + 1..order.len() {
        ratios[order[index]] = ratios[order[index - 1]] * step(order[index - 1], order[index]);
    }
    for index in (0..position).rev() {
        ratios[order[index]] = ratios[order[index + 1]] / step(order[index], order[index + 1]);
    }
    ratios
}

/// Add one frame to the weighted sums in reference units.
pub fn accumulate(num: &mut [f32], den: &mut [f32], values: &[f32], ratio: f64) {
    negpy_core::hdr::accumulate(num, den, values, ratio, (SATURATION, ROLLOFF_START));
}

fn mean_channels(frame: &Frame) -> Vec<f32> {
    let mut gray = vec![0.0; frame.width * frame.height];
    negpy_core::convert::mean_channels(&frame.pixels, &mut gray);
    gray
}

fn estimate_shift(reference: &[f32], moving: &[f32], height: usize, width: usize) -> (f64, f64) {
    if width <= EST_WIDTH {
        let (dx, dy, _) = negpy_core::phase::phase_correlate(reference, moving, height, width);
        return (dx, dy);
    }
    let scale = width as f64 / EST_WIDTH as f64;
    let target_height = ((height as f64 / scale).round() as usize).max(1);
    let Some(reference) =
        negpy_core::resize::resize_area(reference, height, width, 1, target_height, EST_WIDTH)
    else {
        return (0.0, 0.0);
    };
    let Some(moving) =
        negpy_core::resize::resize_area(moving, height, width, 1, target_height, EST_WIDTH)
    else {
        return (0.0, 0.0);
    };
    let (dx, dy, _) =
        negpy_core::phase::phase_correlate(&reference, &moving, target_height, EST_WIDTH);
    (dx * scale, dy * scale)
}

/// Register a bracket frame to the reference by sub-pixel translation.
#[must_use]
pub fn align_frame(reference: &Frame, moving: &Frame, max_shift: f64) -> Frame {
    let reference_gray = mean_channels(reference);
    let moving_gray = mean_channels(moving);
    let (dx, dy) = estimate_shift(&reference_gray, &moving_gray, moving.height, moving.width);
    if dx.abs().max(dy.abs()) > max_shift {
        return moving.clone();
    }
    let source = Image {
        data: &moving.pixels,
        h: moving.height,
        w: moving.width,
        cn: 3,
    };
    let matrix = [1.0, 0.0, -dx, 0.0, 1.0, -dy];
    let mut out = vec![0.0; moving.pixels.len()];
    warp::warp_affine(
        &source,
        (Interp::Cubic, Border::Replicate),
        matrix,
        false,
        moving.width,
        &mut out,
    );
    Frame::new(moving.width, moving.height, out)
}

/// Merge a bracket into one float32 frame in the reference units, unscaled.
#[must_use]
pub fn merge_frames(
    frames: &[Frame],
    ratios: &[f64],
    reference: usize,
    align: bool,
) -> Option<Frame> {
    if frames.len() != ratios.len() || frames.is_empty() {
        return None;
    }
    let reference_frame = frames.get(reference)?;
    let max_shift = (0.02 * reference_frame.width as f64).max(16.0);
    let mut num = vec![0.0; reference_frame.pixels.len()];
    let mut den = vec![0.0; reference_frame.pixels.len()];
    for (index, (frame, ratio)) in frames.iter().zip(ratios).enumerate() {
        if frame.width != reference_frame.width || frame.height != reference_frame.height {
            return None;
        }
        let aligned = if align && index != reference {
            align_frame(reference_frame, frame, max_shift)
        } else {
            frame.clone()
        };
        accumulate(&mut num, &mut den, &aligned.pixels, *ratio);
    }
    let mut merged = vec![0.0; reference_frame.pixels.len()];
    negpy_core::hdr::finish_merge(&num, &den, &reference_frame.pixels, &mut merged);
    Some(Frame::new(
        reference_frame.width,
        reference_frame.height,
        merged,
    ))
}

/// Scale an unscaled merge to the exposure it renders at, clipped to 0..1.
#[must_use]
pub fn apply_render_exposure(merged: &Frame, ratios: &[f64], anchor: Option<f64>) -> Frame {
    let scale = output_scale(ratios, anchor) as f32;
    let mut out = vec![0.0; merged.pixels.len()];
    negpy_core::hdr::render_exposure(&merged.pixels, scale, &mut out);
    Frame::new(merged.width, merged.height, out)
}
