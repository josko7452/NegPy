//! Mirrors the bounds core of `negpy/features/exposure/normalization.py`: log-density
//! conversion, the meter prefilter, percentile bounds and the luma/color mix.

use negpy_core::analysis::{block_median_grid, normalize_log_image, to_log_density};
use negpy_core::bounds::{ColorPass, channel_percentiles, same_pixel_color_floor_refs};

use crate::domain::types::{LUMA_B, LUMA_G, LUMA_R};

/// Bump in any change that alters the bounds a frame measures for unchanged settings.
pub const BOUNDS_VERSION: u32 = 5;

/// Side length of the block-median prefilter grid for exposure analysis.
pub const ANALYSIS_GRID: usize = 1024;

/// Base percentile clip on the luma-range histogram analysis.
pub const BASE_LUMA_CLIP: f64 = 0.01;

/// Neutral per-tail clip for per-channel balance, independent of the luma range.
pub const BASE_COLOR_CLIP: f64 = 1.0;

/// Width of the luma-extreme band the same-pixel color floor refs read.
const COLOR_BOUNDS_BAND_WIDTH: f64 = 4.0;

/// The lowest-chroma fraction of a band kept as the near-neutral set.
const NEUTRAL_AXIS_CHROMA_QUANTILE: f64 = 0.30;

/// Above this median corrected chroma the set is not trustworthy.
const NEUTRAL_AXIS_CHROMA_CAP: f64 = 0.29;

/// Pass-1 chroma ceiling: admits strong correctable casts, rejects saturated content.
const NEUTRAL_AXIS_FIRST_PASS_CAP: f64 = 0.55;

/// Minimum pixels in a near-neutral set.
const NEUTRAL_AXIS_MIN_PIXELS: usize = 64;

/// D-min / D-max bounds: per-channel log-density floors and ceils.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogNegativeBounds {
    pub floors: [f64; 3],
    pub ceils: [f64; 3],
}

impl LogNegativeBounds {
    #[must_use]
    pub const fn new(floors: [f64; 3], ceils: [f64; 3]) -> Self {
        Self { floors, ceils }
    }
}

/// Linear scan -> log10 density clamped to 1e-6..1, NaN to 1e-6.
#[must_use]
pub fn to_log_density_image(image: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; image.len()];
    to_log_density(image, &mut out);
    out
}

/// The center crop that excludes `buffer_ratio` of each border, clamped to 0.3.
#[must_use]
pub fn analysis_crop(
    image: &[f32],
    width: usize,
    height: usize,
    buffer: f32,
) -> (Vec<f32>, usize, usize) {
    if buffer <= 0.0 {
        return (image.to_vec(), width, height);
    }
    let safe = buffer.clamp(0.0, 0.3);
    let cut_x = (width as f32 * safe) as usize;
    let cut_y = (height as f32 * safe) as usize;
    let (w, h) = (width - 2 * cut_x, height - 2 * cut_y);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in cut_y..height - cut_y {
        let start = (y * width + cut_x) * 3;
        out.extend_from_slice(&image[start..start + w * 3]);
    }
    (out, w, h)
}

/// Block-median prefilter to a fixed target grid. Isolated extremes vanish inside their
/// block's median, and stats become nearly resolution-invariant.
#[must_use]
pub fn block_median_filter(image: &[f32], width: usize, height: usize) -> (Vec<f32>, usize, usize) {
    let block = (width.max(height).div_ceil(ANALYSIS_GRID)).max(1);
    if block <= 1 || height < block || width < block {
        return (image.to_vec(), width, height);
    }
    let (hb, wb) = ((height / block) * block, (width / block) * block);
    let mut cropped = Vec::with_capacity(hb * wb * 3);
    for y in 0..hb {
        let start = y * width * 3;
        cropped.extend_from_slice(&image[start..start + wb * 3]);
    }
    let grid = block_median_grid(&cropped, (hb, wb, 3), block);
    (grid, wb / block, hb / block)
}

/// Shared meter prefilter: log10 -> optional ROI -> optional center crop -> block median.
#[must_use]
pub fn prefilter_log_grid(
    image: &[f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
    analysis_buffer: f32,
) -> (Vec<f32>, usize, usize) {
    let log = to_log_density_image(image);
    let (mut current, mut w, mut h) = (log, width, height);
    if let Some((y1, y2, x1, x2)) = roi {
        let mut cropped = Vec::with_capacity((y2 - y1) * (x2 - x1) * 3);
        for y in y1..y2 {
            let start = (y * width + x1) * 3;
            cropped.extend_from_slice(&current[start..start + (x2 - x1) * 3]);
        }
        current = cropped;
        (w, h) = (x2 - x1, y2 - y1);
    }
    if analysis_buffer > 0.0 {
        (current, w, h) = analysis_crop(&current, w, h, analysis_buffer);
    }
    block_median_filter(&current, w, h)
}

/// Per-channel (floors, ceils) at one clip level. A negative clip expands outward by a
/// log-density margin from the same robust basis.
#[must_use]
pub fn sample_bounds(
    grid: &[f32],
    percentile_clip: f64,
    base: f64,
    sorted_grid: Option<&[f32]>,
) -> ([f64; 3], [f64; 3]) {
    let (clip, margin) = if percentile_clip >= 0.0 {
        ((percentile_clip + base).clamp(0.00001, 50.0), 0.0)
    } else {
        (base, -percentile_clip)
    };
    let (p_low, p_high) = (clip, 100.0 - clip);
    let mut floors = [0.0; 3];
    let mut ceils = [0.0; 3];
    if let Some(sorted) = sorted_grid {
        let qs = [p_low, p_high];
        let mut scratch = sorted.to_vec();
        let values =
            crate::features::exposure::normalization::percentiles_from_sorted(&mut scratch, &qs);
        for (ch, values) in values.into_iter().enumerate() {
            floors[ch] = values[0];
            ceils[ch] = values[1];
        }
    } else {
        let per_channel = channel_percentiles(grid, &[p_low, p_high]);
        for (ch, values) in per_channel.into_iter().enumerate() {
            floors[ch] = values[0];
            ceils[ch] = values[1];
        }
    }
    if margin > 0.0 {
        for ch in 0..3 {
            if ceils[ch] >= floors[ch] {
                floors[ch] -= margin;
                ceils[ch] += margin;
            } else {
                floors[ch] += margin;
                ceils[ch] -= margin;
            }
        }
    }
    (floors, ceils)
}

/// `np.percentile(grid[:, :, ch], qs)` over a sorted channel grid, the fast path for a
/// caller that already holds one. The grid holds three channels row-major.
#[must_use]
pub fn percentiles_from_sorted(sorted: &mut [f32], qs: &[f64]) -> [[f64; 2]; 3] {
    std::array::from_fn(|ch| {
        let plane: Vec<f32> = sorted.iter().skip(ch).step_by(3).copied().collect();
        let values = negpy_core::stats::np_percentiles_f64(&plane, qs);
        [values[0], values[1]]
    })
}

/// Per-channel percentile bounds of an HxWx3 log grid at one clip level.
#[must_use]
pub fn analyze_log_exposure_bounds(
    grid: &[f32],
    percentile_clip: f64,
    color_clip: f64,
) -> LogNegativeBounds {
    let (floors, ceils) = sample_bounds(grid, percentile_clip, BASE_LUMA_CLIP, None);
    let (c_floors, c_ceils) = sample_bounds(grid, color_clip, 0.0, None);
    let pass = ColorPass {
        chroma_quantile: NEUTRAL_AXIS_CHROMA_QUANTILE,
        chroma_cap: NEUTRAL_AXIS_CHROMA_CAP,
        first_pass_cap: NEUTRAL_AXIS_FIRST_PASS_CAP,
        min_pixels: NEUTRAL_AXIS_MIN_PIXELS,
        band_width: COLOR_BOUNDS_BAND_WIDTH,
    };
    let same_pixel = (color_clip >= 0.0)
        .then(|| same_pixel_color_floor_refs(grid, floors, ceils, c_ceils, color_clip, pass))
        .flatten();
    let c_floors = same_pixel.unwrap_or(c_floors);
    let spans: [f64; 3] = std::array::from_fn(|ch| c_floors[ch] - c_ceils[ch]);
    let total: f64 = spans.iter().sum();
    if spans.iter().all(|&s| s * total > 0.0) {
        let reach_f: f64 = (0..3).map(|ch| floors[ch] - c_floors[ch]).sum::<f64>() / total;
        let reach_c: f64 = (0..3).map(|ch| ceils[ch] - c_ceils[ch]).sum::<f64>() / total;
        let mut sorted_ceils = c_ceils;
        sorted_ceils.sort_by(f64::total_cmp);
        let shift = (c_ceils.iter().sum::<f64>() - 3.0 * sorted_ceils[1]) / total;
        LogNegativeBounds::new(
            std::array::from_fn(|ch| c_floors[ch] + (reach_f + shift) * spans[ch]),
            std::array::from_fn(|ch| c_ceils[ch] + (reach_c + shift) * spans[ch]),
        )
    } else {
        let mean_lf = floors.iter().sum::<f64>() / 3.0;
        let mean_lc = ceils.iter().sum::<f64>() / 3.0;
        let mut sorted_floors = c_floors;
        let mut sorted_ceils = c_ceils;
        sorted_floors.sort_by(f64::total_cmp);
        sorted_ceils.sort_by(f64::total_cmp);
        let (mean_cf, mean_cc) = (sorted_floors[1], sorted_ceils[1]);
        LogNegativeBounds::new(
            std::array::from_fn(|ch| mean_lf + (c_floors[ch] - mean_cf)),
            std::array::from_fn(|ch| mean_lc + (c_ceils[ch] - mean_cc)),
        )
    }
}

/// Linear-image wrapper around [`analyze_log_exposure_bounds`].
#[must_use]
pub fn analyze_bounds_from_image(
    image: &[f32],
    width: usize,
    height: usize,
    percentile_clip: f64,
    color_clip: f64,
) -> LogNegativeBounds {
    analyze_log_exposure_bounds_with_roi(
        image,
        width,
        height,
        None,
        0.0,
        percentile_clip,
        color_clip,
    )
}

/// Linear-image bounds over an optional ROI and border buffer.
#[must_use]
pub fn analyze_log_exposure_bounds_with_roi(
    image: &[f32],
    width: usize,
    height: usize,
    roi: Option<(usize, usize, usize, usize)>,
    analysis_buffer: f32,
    percentile_clip: f64,
    color_clip: f64,
) -> LogNegativeBounds {
    let (grid, _, _) = prefilter_log_grid(image, width, height, roi, analysis_buffer);
    analyze_log_exposure_bounds(&grid, percentile_clip, color_clip)
}

/// Log -> ~0.0-1.0 linear stretch, unclamped: out-of-bounds densities are rolled off by
/// the downstream characteristic curve.
#[must_use]
pub fn normalize_bounds(img_log: &[f32], bounds: LogNegativeBounds) -> Vec<f32> {
    let floors = bounds.floors.map(|value| value as f32);
    let ceils = bounds.ceils.map(|value| value as f32);
    let mut out = vec![0.0; img_log.len()];
    normalize_log_image(img_log, floors, ceils, &mut out);
    out
}

/// Single global density range as a luminance weighting of the per-channel ranges.
#[must_use]
pub fn luminance_density_range(bounds: LogNegativeBounds) -> f64 {
    let rr = (bounds.ceils[0] - bounds.floors[0]).abs();
    let rg = (bounds.ceils[1] - bounds.floors[1]).abs();
    let rb = (bounds.ceils[2] - bounds.floors[2]).abs();
    let (r, g, b) = (LUMA_R as f64, LUMA_G as f64, LUMA_B as f64);
    r * rr + g * rg + b * rb
}

/// Luma-weighted centre and range from `luma_src`, per-channel color cast from
/// `color_src`. Color-average then moves only the cast, never contrast or brightness.
#[must_use]
pub fn mix_luma_color_bounds(
    luma_src: LogNegativeBounds,
    color_src: LogNegativeBounds,
) -> LogNegativeBounds {
    if luma_src == color_src {
        return luma_src;
    }
    let w = [LUMA_R as f64, LUMA_G as f64, LUMA_B as f64];
    let centre = |b: LogNegativeBounds| {
        (0..3)
            .map(|ch| w[ch] * (b.floors[ch] + b.ceils[ch]) / 2.0)
            .sum::<f64>()
    };
    let range = |b: LogNegativeBounds| {
        (0..3)
            .map(|ch| w[ch] * (b.ceils[ch] - b.floors[ch]))
            .sum::<f64>()
    };
    let dc = centre(luma_src) - centre(color_src);
    let dr = range(luma_src) - range(color_src);
    let (df, d_ceil) = (dc - dr / 2.0, dc + dr / 2.0);
    LogNegativeBounds::new(
        color_src.floors.map(|value| value + df),
        color_src.ceils.map(|value| value + d_ceil),
    )
}
