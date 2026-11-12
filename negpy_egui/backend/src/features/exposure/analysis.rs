//! Analysis-panel histogram and zone math over the shared `negpy_core` kernels.

use negpy_core::zones;

use crate::domain::color::{WORKING_GAMMA, WORKING_INV_GAMMA};

pub const DENSITY_HIST_BINS: usize = negpy_core::histogram::DENSITY_HIST_BINS;
pub const OUTPUT_HIST_BINS: usize = negpy_core::histogram::OUTPUT_HIST_BINS;
pub const COLOR_HIST_BINS: usize = negpy_core::histogram::COLOR_HIST_BINS;
pub const SCAN_DENSITY_RANGE: (f64, f64) = (0.0, 3.0);
pub const WAVEFORM_COLS: usize = negpy_core::histogram::WAVEFORM_COLS;
pub const WAVEFORM_BINS: usize = negpy_core::histogram::WAVEFORM_BINS;
pub const SCOPE_HUE_BINS: usize = negpy_core::histogram::SCOPE_HUE_BINS;
pub const SCOPE_CHROMA_BINS: usize = negpy_core::histogram::SCOPE_CHROMA_BINS;
pub const SCOPE_MAX_CHROMA: f64 = 60.0;
pub const SCOPE_LOG_KNEE: f64 = 4.0;
pub const ZONE_COUNT: u8 = zones::ZONE_COUNT;
pub const ZONE_MID_REFLECTANCE: f64 = 0.18;
pub const ZONE_EMPTY: f64 = 0.005;
pub const ZONE_LOADED: f64 = 0.02;
pub const MASK_MAX_EDGE: usize = 1024;
pub const ZONE_GRID_CELLS: usize = 24;
pub const ZONE_FIELD_MAX_EDGE: usize = 768;
pub const WEDGE_STEPS: usize = 21;
pub const WEDGE_STEP_DENSITY: f64 = 0.15;
pub const WEDGE_SEPARATION: f64 = 0.004;

const MAX_HIST_SAMPLES: usize = 2_000_000;

/// The stride that keeps a whole frame near `max_samples` samples.
#[must_use]
pub fn frame_step(height: usize, width: usize, max_samples: usize) -> usize {
    if height == 0 || width == 0 || max_samples == 0 {
        return 1;
    }
    ((height as f64 * width as f64 / max_samples as f64)
        .sqrt()
        .round_ties_even() as usize)
        .max(1)
}

/// The working-space OETF encode of linear values.
#[must_use]
pub fn working_oetf_encode(values: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    negpy_core::color::oetf_encode(values, &mut out, WORKING_INV_GAMMA);
    out
}

/// The working-space OETF decode of encoded values.
#[must_use]
pub fn working_oetf_decode(values: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    negpy_core::color::oetf_decode(values, &mut out, WORKING_GAMMA as f32);
    out
}

/// `_mid_gray_encoded`: 18 % gray on the print zone ruler.
#[must_use]
pub fn mid_gray_encoded() -> f32 {
    working_oetf_encode(&[ZONE_MID_REFLECTANCE as f32])[0]
}

/// Print zone (0..10) of a display-encoded lightness.
#[must_use]
pub fn zone_of_encoded(encoded: f32) -> f32 {
    let mid = mid_gray_encoded();
    let encoded = encoded.clamp(0.0, 1.0);
    if encoded <= mid {
        5.0 * encoded / mid
    } else {
        5.0 + 5.0 * (encoded - mid) / (1.0 - mid)
    }
}

/// Display-encoded lightness of a print zone.
#[must_use]
pub fn encoded_of_zone(zone: f32) -> f32 {
    let mid = mid_gray_encoded();
    let zone = zone.clamp(0.0, ZONE_COUNT as f32);
    if zone <= 5.0 {
        mid * zone / 5.0
    } else {
        mid + (1.0 - mid) * (zone - 5.0) / 5.0
    }
}

/// Fold display-luma histogram bins into the share of each zone.
#[must_use]
pub fn zone_occupancy(bins: &[f64]) -> Vec<f64> {
    zones::zone_occupancy(bins, mid_gray_encoded() as f64)
}

/// (shadow, highlight) zone warnings.
#[must_use]
pub fn zone_warnings(occupancy: &[f64]) -> (bool, bool) {
    let at = |index: usize| occupancy.get(index).copied().unwrap_or(0.0);
    let shadow = at(2) + at(3) < ZONE_EMPTY && at(0) + at(1) > ZONE_LOADED;
    let highlight = at(7) + at(8) < ZONE_EMPTY && at(9) + at(10) > ZONE_LOADED;
    (shadow, highlight)
}

/// Integer Adams zone (0..10) per cell of a fixed grid over a display-encoded frame.
#[must_use]
pub fn zone_grid(
    pixels: &[f32],
    width: usize,
    height: usize,
    cells: usize,
) -> Option<(Vec<i32>, usize, usize)> {
    zones::zone_grid(pixels, (height, width, 3), cells, mid_gray_encoded() as f64)
}

/// Integer Adams zone (0..10) per pixel, at most `max_edge` on the long side.
#[must_use]
pub fn zone_field(
    pixels: &[f32],
    width: usize,
    height: usize,
    max_edge: usize,
) -> Option<(Vec<u8>, usize, usize)> {
    zones::zone_field(
        pixels,
        (height, width, 3),
        max_edge,
        mid_gray_encoded() as f64,
    )
}

/// Fraction of the frame in each zone 0..10.
#[must_use]
pub fn zone_shares(zones: &[u8]) -> Vec<f64> {
    zones::zone_shares(zones)
}

/// (x, y, zone, inset) per region holding at least `min_share` of the frame.
#[must_use]
pub fn zone_anchors(
    zones: &[u8],
    width: usize,
    height: usize,
    min_share: f64,
) -> Vec<(usize, usize, u8, f32)> {
    zones::zone_anchors(zones, (height, width), min_share)
}

/// One label anchor per contiguous same-zone region.
#[must_use]
pub fn zone_region_labels(zones: &[i32], width: usize, height: usize) -> Vec<(usize, usize, i32)> {
    zones::zone_region_labels(zones, (height, width))
}

/// (shadow, highlight) masks of pixels in the output histogram's edge bins.
#[must_use]
pub fn clip_masks(
    pixels: &[f32],
    width: usize,
    height: usize,
    max_edge: usize,
) -> Option<(Vec<bool>, Vec<bool>)> {
    if width == 0 || height == 0 || pixels.len() < width * height * 3 {
        return None;
    }
    let step = width.max(height).div_ceil(max_edge).max(1);
    let mut low = Vec::new();
    let mut high = Vec::new();
    let edge = 1.0 / OUTPUT_HIST_BINS as f32;
    for y in (0..height).step_by(step) {
        for x in (0..width).step_by(step) {
            let at = (y * width + x) * 3;
            let pixel = &pixels[at..at + 3];
            low.push(pixel.iter().any(|value| *value < edge));
            high.push(pixel.iter().any(|value| *value >= 1.0 - edge));
        }
    }
    Some((low, high))
}

/// Worst-channel share of the black and white histogram bins.
#[must_use]
pub fn output_clip_fractions(bins: &[f64], bins_per_channel: usize) -> (f64, f64) {
    let width = bins_per_channel;
    let mut low = 0.0_f64;
    let mut high = 0.0_f64;
    for channel in 0..3 {
        let row = &bins[channel * width..(channel + 1) * width];
        let total = row.iter().sum::<f64>().max(1.0);
        low = low.max(row.first().copied().unwrap_or(0.0) / total);
        high = high.max(row.last().copied().unwrap_or(0.0) / total);
    }
    (low, high)
}

/// Share of a joint histogram the output profile cannot print.
#[must_use]
pub fn gamut_fraction(histogram: &[f64], out_of_gamut: &[bool]) -> Option<f64> {
    if histogram.len() != out_of_gamut.len() {
        return None;
    }
    let total: f64 = histogram.iter().sum();
    if total <= 0.0 {
        return None;
    }
    Some(
        histogram
            .iter()
            .zip(out_of_gamut)
            .filter(|(_, blocked)| **blocked)
            .map(|(count, _)| count)
            .sum::<f64>()
            / total,
    )
}

/// Chroma -> 0..1 of the vectorscope radius.
#[must_use]
pub fn scope_radius(chroma: f64, log: bool) -> f64 {
    if log {
        (chroma / SCOPE_LOG_KNEE).ln_1p() / (SCOPE_MAX_CHROMA / SCOPE_LOG_KNEE).ln_1p()
    } else {
        chroma / SCOPE_MAX_CHROMA
    }
}

/// Exact inverse of [`scope_radius`].
#[must_use]
pub fn scope_chroma(radius: f64, log: bool) -> f64 {
    if log {
        ((radius * (SCOPE_MAX_CHROMA / SCOPE_LOG_KNEE).ln_1p()).exp_m1()) * SCOPE_LOG_KNEE
    } else {
        radius * SCOPE_MAX_CHROMA
    }
}

/// The 21 wedge steps in val-domain log exposure.
#[must_use]
pub fn wedge_vals() -> Vec<f32> {
    (0..WEDGE_STEPS)
        .map(|index| 1.0 - index as f32 / (WEDGE_STEPS - 1) as f32)
        .collect()
}

/// Density per step on this scan.
#[must_use]
pub fn wedge_step_density(norm_density_range: Option<f64>) -> f64 {
    norm_density_range
        .filter(|range| *range != 0.0)
        .map_or(WEDGE_STEP_DENSITY, |range| {
            range.abs() / (WEDGE_STEPS - 1) as f64
        })
}

/// (first, last) wedge step that still separates from its neighbour.
#[must_use]
pub fn wedge_usable_span(encoded: &[f64]) -> Option<(usize, usize)> {
    let steps: Vec<usize> = encoded
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| (pair[1] - pair[0]).abs() > WEDGE_SEPARATION)
        .map(|(index, _)| index)
        .collect();
    Some((*steps.first()?, steps.last()? + 1))
}

/// Edge energy of a display-encoded patch: RMS Laplacian of its luma, times 100.
#[must_use]
pub fn loupe_acutance(pixels: &[f32], width: usize, height: usize) -> f64 {
    if width < 3 || height < 3 || pixels.len() < width * height * 3 {
        return 0.0;
    }
    zones::loupe_acutance(pixels, (height, width, 3))
}

/// Scan density -> 0..1 up the scan parade.
#[must_use]
pub fn scan_density_axis(density: f64) -> f64 {
    let (low, high) = SCAN_DENSITY_RANGE;
    (high - density) / (high - low)
}

/// Per-channel scan density of a normalized val triplet.
#[must_use]
pub fn scan_density_of_val(val: [f64; 3], floors: [f64; 3], ceils: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| -(floors[index] + val[index] * (ceils[index] - floors[index])))
}

/// A stride-subsampled crop of an RGB frame.
fn samples<'a>(
    pixels: &'a [f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
) -> Option<negpy_core::histogram::Samples<'a>> {
    if width == 0 || height == 0 || pixels.len() < width * height * 3 {
        return None;
    }
    let (y1, y2, x1, x2) = roi.unwrap_or((0, height, 0, width));
    let y1 = y1.min(height);
    let y2 = y2.min(height);
    let x1 = x1.min(width);
    let x2 = x2.min(width);
    if y2 <= y1 || x2 <= x1 {
        return None;
    }
    Some(negpy_core::histogram::Samples {
        img: pixels,
        w: width,
        cn: 3,
        rows: (y1, y2),
        cols: (x1, x2),
        step: frame_step(y2 - y1, x2 - x1, MAX_HIST_SAMPLES),
    })
}

/// (4, 256) output histogram of an encoded frame or a ready histogram.
#[must_use]
pub fn output_histogram(pixels: &[f32], width: usize, height: usize) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, None)?;
    Some(negpy_core::histogram::output_histogram(samples))
}

/// (32, 32, 32) joint RGB histogram of an encoded frame.
#[must_use]
pub fn color_histogram(pixels: &[f32], width: usize, height: usize) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, None)?;
    Some(negpy_core::histogram::color_histogram(samples))
}

/// Waveform counts of a display-encoded frame.
#[must_use]
pub fn waveform(pixels: &[f32], width: usize, height: usize) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, None)?;
    Some(negpy_core::histogram::waveform(
        samples,
        negpy_core::histogram::Waveform::Luma,
    ))
}

/// RGB parade counts of a display-encoded frame.
#[must_use]
pub fn waveform_parade(pixels: &[f32], width: usize, height: usize) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, None)?;
    Some(negpy_core::histogram::waveform(
        samples,
        negpy_core::histogram::Waveform::Parade,
    ))
}

/// Scan-density parade of a log frame.
#[must_use]
pub fn density_parade(
    pixels: &[f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, roi)?;
    Some(negpy_core::histogram::density_parade(samples))
}

/// (4, 120) val-domain density histogram.
#[must_use]
pub fn density_histogram(
    pixels: &[f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, roi)?;
    Some(negpy_core::histogram::density_histogram(samples))
}

/// (3, WAVEFORM_COLS) counts of scan pixels at or above the sensor clip level.
#[must_use]
pub fn scan_full_scale_columns(
    pixels: &[f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
    level: f32,
) -> Option<Vec<f64>> {
    let samples = samples(pixels, width, height, roi)?;
    Some(negpy_core::histogram::scan_full_scale_columns(
        samples, level,
    ))
}
