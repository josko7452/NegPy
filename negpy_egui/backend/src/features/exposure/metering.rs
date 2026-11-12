//! The exposure meters of `negpy/features/exposure/normalization.py`: analysis-region
//! resolution, crosstalk unmix, clip fractions and the textured anchor and tail meters.

use negpy_core::analysis::{
    block_median_grid, clip_fractions, luma_median, to_log_density, unmix_log,
};
use negpy_core::meter::{self, Activity, ChannelNorm, NeutralAxis, NeutralConsts};

use crate::domain::models::process::DEFAULT_CROSSTALK_MATRIX;
use crate::domain::models::{ProcessConfig, ProcessMode};
use crate::domain::types::{LUMA_B, LUMA_G, LUMA_R};
use crate::features::exposure::constants as c;
use crate::features::exposure::normalization::LogNegativeBounds;

/// A pixel window as `(y1, y2, x1, x2)`.
pub type Roi = (usize, usize, usize, usize);

const NEUTRAL_CONSTS: NeutralConsts = NeutralConsts {
    quantile: c::NEUTRAL_AXIS_CHROMA_QUANTILE,
    cap: c::NEUTRAL_AXIS_CHROMA_CAP,
    first_pass_cap: c::NEUTRAL_AXIS_FIRST_PASS_CAP,
    min_pixels: c::NEUTRAL_AXIS_MIN_PIXELS,
    highlight: c::NEUTRAL_AXIS_HIGHLIGHT_BAND,
    mid: c::NEUTRAL_AXIS_MID_BAND,
    shadow: c::NEUTRAL_AXIS_SHADOW_BAND,
    subjects: [c::NEUTRAL_AXIS_FOLIAGE, c::NEUTRAL_AXIS_SKY],
    n0: c::NEUTRAL_AXIS_CONFIDENCE_N0,
    agreement: c::NEUTRAL_AXIS_AGREEMENT,
    width: c::NEUTRAL_AXIS_WIDTH,
};

fn activity() -> Activity {
    Activity {
        block: c::ACTIVITY_BLOCK,
        gate: c::ACTIVITY_GATE_DENSITY,
        min_fraction: c::ACTIVITY_MIN_FRACTION,
    }
}

/// A normalized (x1, y1, x2, y2) rect as a pixel window; None for a degenerate one.
#[must_use]
pub fn rect_roi(width: usize, height: usize, rect: [f64; 4]) -> Option<Roi> {
    let [x1, y1, x2, y2] = rect;
    let roi = (
        (y1.min(y2) * height as f64) as usize,
        (y1.max(y2) * height as f64) as usize,
        (x1.min(x2) * width as f64) as usize,
        (x1.max(x2) * width as f64) as usize,
    );
    (roi.1.saturating_sub(roi.0) >= 2 && roi.3.saturating_sub(roi.2) >= 2).then_some(roi)
}

/// The window and border buffer the meters read. A freehand analysis rect wins over the
/// crop ROI, and it disables the symmetric buffer.
#[must_use]
pub fn resolve_analysis_region(
    width: usize,
    height: usize,
    active_roi: Option<Roi>,
    analysis_buffer: f64,
    analysis_rect: Option<[f64; 4]>,
    meter_rect: Option<[f64; 4]>,
) -> (Option<Roi>, f64) {
    if let Some(roi) = analysis_rect.and_then(|rect| rect_roi(width, height, rect)) {
        return (Some(roi), 0.0);
    }
    if active_roi.is_none() {
        if let Some(rect) = meter_rect {
            return (rect_roi(width, height, rect), analysis_buffer);
        }
    }
    (active_roi, analysis_buffer)
}

/// The configured dye unmix, only when its profile was derived for `mode`.
#[must_use]
pub fn effective_crosstalk_matrix(
    process: &ProcessConfig,
    mode: Option<ProcessMode>,
) -> Option<[[f32; 3]; 3]> {
    if let Some(mode) = mode {
        let profile = ProcessMode::coerce(&process.crosstalk_process);
        if profile != mode {
            return None;
        }
    }
    resolve_crosstalk_matrix(
        process.crosstalk_strength,
        process.crosstalk_matrix.as_deref(),
    )
}

/// Identity-to-calibration blend by strength, row-normalized so a neutral gray stays neutral.
#[must_use]
pub fn resolve_crosstalk_matrix(strength: f64, matrix: Option<&[f64]>) -> Option<[[f32; 3]; 3]> {
    if strength <= 0.0 {
        return None;
    }
    let values = matrix.unwrap_or(&DEFAULT_CROSSTALK_MATRIX);
    if values.len() < 9 {
        return None;
    }
    let applied: [[f64; 3]; 3] = std::array::from_fn(|r| {
        std::array::from_fn(|ch| {
            let identity = if r == ch { 1.0 } else { 0.0 };
            identity * (1.0 - strength) + values[r * 3 + ch] * strength
        })
    });
    Some(std::array::from_fn(|r| {
        let sum = applied[r].iter().sum::<f64>().max(1e-6);
        applied[r].map(|value| (value / sum) as f32)
    }))
}

/// Apply the unmix to an HxWx3 log grid; a copy when off.
#[must_use]
pub fn unmix_log_image(image: &[f32], matrix: Option<[[f32; 3]; 3]>) -> Vec<f32> {
    let Some(matrix) = matrix else {
        return image.to_vec();
    };
    let mut out = vec![0.0; image.len()];
    unmix_log(image, matrix, &mut out);
    out
}

/// Per-channel share of the scan at or above the sensor-white clip level.
#[must_use]
pub fn measure_clip_fractions(
    image: &[f32],
    width: usize,
    height: usize,
    roi: Option<Roi>,
    analysis_buffer: f64,
) -> [f64; 3] {
    let (y1, y2, x1, x2) = roi.unwrap_or((0, height, 0, width));
    let (y2, x2) = (y2.min(height), x2.min(width));
    let (y1, x1) = (y1.min(y2), x1.min(x2));
    let (mut y1, mut y2, mut x1, mut x2) = (y1, y2, x1, x2);
    if analysis_buffer > 0.0 {
        let b = analysis_buffer.clamp(0.0, 0.3);
        let (cut_h, cut_w) = (
            ((y2 - y1) as f64 * b) as usize,
            ((x2 - x1) as f64 * b) as usize,
        );
        (y1, y2, x1, x2) = (y1 + cut_h, y2 - cut_h, x1 + cut_w, x2 - cut_w);
    }
    clip_fractions(image, width, (y1, y2, x1, x2), c::SCAN_CLIP_LEVEL)
}

/// The shared analysis grid the meters read: log10 density, optional ROI and buffer, then the
/// block-median prefilter.
#[must_use]
pub fn analysis_grid(
    image: &[f32],
    width: usize,
    height: usize,
    roi: Option<Roi>,
    analysis_buffer: f64,
) -> (Vec<f32>, usize, usize) {
    let mut log = vec![0.0; image.len()];
    to_log_density(image, &mut log);
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
        let b = analysis_buffer.clamp(0.0, 0.3);
        let (cut_x, cut_y) = ((w as f64 * b) as usize, (h as f64 * b) as usize);
        let (cropped_w, cropped_h) = (w - 2 * cut_x, h - 2 * cut_y);
        let mut cropped = Vec::with_capacity(cropped_w * cropped_h * 3);
        for y in cut_y..cut_y + cropped_h {
            let start = (y * w + cut_x) * 3;
            cropped.extend_from_slice(&current[start..start + cropped_w * 3]);
        }
        current = cropped;
        (w, h) = (cropped_w, cropped_h);
    }
    let block = (w.max(h).div_ceil(c::ANALYSIS_GRID)).max(1);
    if block <= 1 || h < block || w < block {
        return (current, w, h);
    }
    let (hb, wb) = ((h / block) * block, (w / block) * block);
    let mut cropped = Vec::with_capacity(hb * wb * 3);
    for y in 0..hb {
        let start = y * w * 3;
        cropped.extend_from_slice(&current[start..start + wb * 3]);
    }
    let grid = block_median_grid(&cropped, (hb, wb, 3), block);
    (grid, wb / block, hb / block)
}

/// Per-channel (floor, span) the meters normalize with. Every term is a plain f64, so the
/// normalization runs in f64.
#[must_use]
pub fn meter_norm(bounds: LogNegativeBounds) -> [ChannelNorm; 3] {
    std::array::from_fn(|ch| {
        let floor = bounds.floors[ch];
        let mut span = bounds.ceils[ch] - floor;
        if span.abs() < 1e-6 {
            span = if span >= 0.0 { 1e-6 } else { -1e-6 };
        }
        ChannelNorm {
            floor,
            floor_wide: true,
            span,
            span_wide: true,
        }
    })
}

/// The textured dark tail in normalized luma: Shadow Reach's print point.
#[must_use]
pub fn measure_shadow_point(
    grid: &[f32],
    width: usize,
    height: usize,
    bounds: LogNegativeBounds,
) -> f64 {
    textured_percentile(grid, width, height, bounds, c::SHADOW_REACH_PERCENTILE)
}

/// The textured bright tail in normalized luma: Highlight Hold's print point.
#[must_use]
pub fn measure_highlight_point(
    grid: &[f32],
    width: usize,
    height: usize,
    bounds: LogNegativeBounds,
) -> f64 {
    textured_percentile(grid, width, height, bounds, c::HIGHLIGHT_HOLD_PERCENTILE)
}

fn textured_percentile(
    grid: &[f32],
    width: usize,
    height: usize,
    bounds: LogNegativeBounds,
    q: f64,
) -> f64 {
    let lum = meter::luma(grid, Some(&meter_norm(bounds)));
    let cells = meter::textured_cells(lum, height, width, activity());
    meter::percentile(&cells, q) as f64
}

/// The textured midtone anchor, pulled `ANCHOR_METER_STRENGTH` toward the meter and clamped
/// to the assumed anchor's band.
#[must_use]
pub fn measure_anchor(grid: &[f32], width: usize, height: usize, bounds: LogNegativeBounds) -> f64 {
    let lum = meter::luma(grid, Some(&meter_norm(bounds)));
    let cells = meter::textured_cells(lum, height, width, activity());
    let measured = meter::anchor_measure(&cells, c::ANCHOR_TRIM_CLIP);
    let a = c::ASSUMED_ANCHOR;
    let anchor = a + c::ANCHOR_METER_STRENGTH * (measured - a);
    anchor.clamp(a - c::ANCHOR_METER_BAND, a + c::ANCHOR_METER_BAND)
}

/// The P10-P90 spread of the textured raw log luma.
#[must_use]
pub fn measure_textural_range(grid: &[f32], width: usize, height: usize) -> f64 {
    let lum = meter::luma(grid, None);
    let cells = meter::textured_cells(lum, height, width, activity());
    meter::textural_range(&cells, c::TEXTURAL_RANGE_CLIP)
}

/// The per-channel neutral axis over the analysis grid, or None without a midtone or shadow set.
#[must_use]
pub fn measure_neutral_axis(
    grid: &[f32],
    _width: usize,
    _height: usize,
    bounds: LogNegativeBounds,
    exclude_subjects: bool,
) -> Option<NeutralAxis> {
    let floors = bounds.floors.map(|value| value as f32);
    let ceils = bounds.ceils.map(|value| value as f32);
    meter::neutral_axis(
        grid,
        (floors, ceils),
        (bounds.floors, bounds.ceils),
        exclude_subjects,
        &NEUTRAL_CONSTS,
    )
}

/// Luma-weighted density above the clear base: `(thin, median, dense)`.
#[must_use]
pub fn density_above_base(
    base_log: Option<[f64; 3]>,
    bounds: LogNegativeBounds,
    grid: &[f32],
    unmix: Option<[[f32; 3]; 3]>,
) -> Option<[f64; 3]> {
    let base_log = base_log?;
    let base = unmix_log_image(&base_log.map(|value| value as f32), unmix);
    let w = [LUMA_R as f64, LUMA_G as f64, LUMA_B as f64];
    let dot = |v: [f64; 3]| (0..3).map(|ch| w[ch] * v[ch]).sum::<f64>();
    let base = dot([base[0] as f64, base[1] as f64, base[2] as f64]);
    let thin = dot(bounds.ceils);
    let dense = dot(bounds.floors);
    let median = luma_median(grid);
    Some([base - thin, base - median, base - dense])
}
