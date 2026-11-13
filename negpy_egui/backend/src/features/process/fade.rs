//! Fade Restoration helpers: the restoration matrix, its density anchor and white-space tag.

use sha1::{Digest, Sha1};

use crate::domain::models::ProcessConfig;
use crate::features::exposure::metering::effective_crosstalk_matrix;
use crate::features::process::capture_color::camera_matrix_parts;
use crate::features::process::path::{RenderPath, render_path};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The neutral log level Remove Stain maps the cleanest white to.
pub const FADE_WHITE_HEADROOM: f64 = 0.1;

/// Values per band in a stored crossover table: mean log, share, then the lean of each channel.
pub const CROSSOVER_BAND_SIZE: usize = 5;
/// Below this spread a band's lean has no crossover to fix.
pub const CROSSOVER_FLOOR: f64 = 0.04;
/// The three channel weights a lean sums to zero under.
pub const CROSSOVER_WEIGHTS: [f64; 3] = [1.0 / 3.0; 3];
/// Bins of the per-pixel offset table between the outer band anchors.
pub const CROSSOVER_BINS: usize = 64;
/// The largest per-channel log offset the table applies.
pub const CROSSOVER_LIMIT: f64 = 0.3;
pub const CROSSOVER_MIN_SHARE: f64 = 0.004;
pub const CROSSOVER_MIN_WEIGHT: f64 = 8.0;
pub const CROSSOVER_MIN_SPAN: f64 = 0.05;
pub const CROSSOVER_MEAN_SHIFT_STEPS: usize = 3;
pub const CROSSOVER_MIN_CELLS: usize = 64;
const COLOR_EXTREME_PCT: f64 = 0.00001;
const ROBUST_EXTREME_PCT: f64 = 0.1;
const MIN_CONFIDENCE: f64 = 0.2;
const ROBUST_MIN_CONFIDENCE: f64 = 0.4;
const CROSSOVER_BAND_EDGES: [f64; 7] = [0.06, 0.15, 0.3, 0.45, 0.6, 0.75, 0.9];
const CROSSOVER_RANGE_PCT: (f64, f64) = (0.5, 99.5);
const CROSSOVER_NEUTRAL_SPREAD: f64 = 0.25;
const CROSSOVER_CLUSTER_RADIUS: f64 = 0.12;
const CLIPPED_LOG: f64 = -5.0;

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

/// The per-pixel offset table: `bins` rows of log offsets between mean logs `lo` and `hi`.
#[derive(Clone, Debug, PartialEq)]
pub struct CrossoverOffsets {
    pub bins: Vec<[f32; 3]>,
    pub lo: f64,
    pub hi: f64,
}

/// The offsets a render adds in log, `-(lean - reference) * crossover`.
///
/// Each row sums to zero under `CROSSOVER_WEIGHTS` and is scaled down as a whole to
/// `CROSSOVER_LIMIT`, so the correction keeps the mean density and its hue.
#[must_use]
pub fn crossover_offsets(table: Option<&[f64]>, crossover: f64) -> Option<CrossoverOffsets> {
    let table = table.filter(|table| !table.is_empty())?;
    if crossover == 0.0 || crossover_amount(Some(table)) < CROSSOVER_FLOOR {
        return None;
    }
    let rows = crossover_bands(table);
    let reference = crossover_reference(table);
    let xs: Vec<f64> = rows.iter().map(|row| row[0]).collect();
    let lo = *xs.first()?;
    let hi = *xs.last()?;
    let x: Vec<f64> = if hi > lo {
        (0..CROSSOVER_BINS)
            .map(|index| lo + (hi - lo) * index as f64 / (CROSSOVER_BINS - 1) as f64)
            .collect()
    } else {
        vec![lo; CROSSOVER_BINS]
    };
    let mut bins: Vec<[f64; 3]> = x
        .iter()
        .map(|&value| {
            std::array::from_fn(|channel| {
                let ys: Vec<f64> = rows.iter().map(|row| row[2 + channel]).collect();
                -(shape_cubic(&xs, &ys, value) - reference[channel]) * crossover
            })
        })
        .collect();
    for row in &mut bins {
        let mean = row
            .iter()
            .zip(CROSSOVER_WEIGHTS)
            .map(|(value, weight)| value * weight)
            .sum::<f64>();
        for value in row.iter_mut() {
            *value -= mean;
        }
        let peak = row.iter().map(|value| value.abs()).fold(0.0, f64::max);
        let scale = (CROSSOVER_LIMIT / peak.max(1e-12)).min(1.0);
        for value in row.iter_mut() {
            *value *= scale;
        }
    }
    if bins
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value.abs())
        .fold(0.0, f64::max)
        <= 1e-4
    {
        return None;
    }
    Some(CrossoverOffsets {
        bins: bins
            .into_iter()
            .map(|row| row.map(|value| value as f32))
            .collect(),
        lo,
        hi,
    })
}

/// `apply_crossover`: the image plus the offset row its own mean log selects.
#[must_use]
pub fn apply_crossover(image: &ImageBuffer, offsets: Option<&CrossoverOffsets>) -> ImageBuffer {
    let Some(offsets) = offsets else {
        return image.clone();
    };
    let mut out = vec![0.0; image.pixels.len()];
    negpy_core::analysis::apply_crossover(
        &image.pixels,
        &offsets.bins,
        offsets.lo as f32,
        (offsets.hi - offsets.lo) as f32,
        &mut out,
    );
    ImageBuffer::from_rgb(image.width, image.height, out)
}

/// The shader's table: `(lo, hi, on, 0)` then one vec4 per bin, zeros when off.
#[must_use]
pub fn crossover_uniform(offsets: Option<&CrossoverOffsets>) -> Vec<u8> {
    let Some(offsets) = offsets else {
        return vec![0; 16 * (CROSSOVER_BINS + 1)];
    };
    let mut bytes = Vec::with_capacity(16 * (CROSSOVER_BINS + 1));
    for value in [offsets.lo as f32, offsets.hi as f32, 1.0, 0.0] {
        bytes.extend_from_slice(&value.to_ne_bytes());
    }
    for row in &offsets.bins {
        for value in [row[0], row[1], row[2], 0.0] {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

/// The table's strength in a render: Crossover on a slide whose survivals act, else 0.
#[must_use]
pub fn crossover_strength(process: &ProcessConfig) -> f64 {
    if fade_matrix(process).is_none() {
        return 0.0;
    }
    process.fade_crossover
}

/// A table a render measured for itself: `(table, fade_crossover_space)`.
pub type MeasuredCrossover = (Option<Vec<f64>>, String);

/// The table a render applies, and the one it measured from `grid` when the frame has none.
#[must_use]
pub fn resolve_crossover(
    process: &ProcessConfig,
    space: &str,
    grid: Option<&ImageBuffer>,
    measure: bool,
) -> (Option<Vec<f64>>, Option<MeasuredCrossover>) {
    if crossover_strength(process) == 0.0 {
        return (None, None);
    }
    if let Some(table) = process.fade_crossover_table.clone() {
        return (Some(table), None);
    }
    if !measure {
        return (None, None);
    }
    let Some(grid) = grid else {
        return (None, None);
    };
    let table = measure_crossover(grid);
    (
        table.clone(),
        table.map(|table| (Some(table), space.to_string())),
    )
}

/// Each band's neutral centre, its mean log and its share, dense end first.
#[derive(Clone, Debug, PartialEq)]
pub struct NeutralBands {
    pub centre: Vec<[f64; 3]>,
    pub level: Vec<f64>,
    pub share: Vec<f64>,
}

fn mean_log(cell: [f64; 3]) -> f64 {
    cell[0] * CROSSOVER_WEIGHTS[0] + cell[1] * CROSSOVER_WEIGHTS[1] + cell[2] * CROSSOVER_WEIGHTS[2]
}

/// The neutral centre of each density band of a log grid.
#[must_use]
pub fn neutral_bands(grid: &ImageBuffer) -> Option<NeutralBands> {
    let cells: Vec<[f32; 3]> = grid
        .pixels
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    let params = negpy_core::fade::BandParams {
        inner_edges: CROSSOVER_BAND_EDGES.to_vec(),
        weights: CROSSOVER_WEIGHTS,
        range_pct: CROSSOVER_RANGE_PCT,
        min_span: CROSSOVER_MIN_SPAN,
        neutral_spread: CROSSOVER_NEUTRAL_SPREAD,
        cluster_radius: CROSSOVER_CLUSTER_RADIUS,
        steps: CROSSOVER_MEAN_SHIFT_STEPS,
        min_weight: CROSSOVER_MIN_WEIGHT,
        clipped_log: CLIPPED_LOG,
        min_cells: CROSSOVER_MIN_CELLS,
    };
    let found = negpy_core::fade::neutral_bands(&cells, &params)?;
    let mut centre = Vec::new();
    let mut level = Vec::new();
    let mut share = Vec::new();
    for (band, weight) in found.centre.iter().zip(&found.weight) {
        if band[0].is_nan() {
            continue;
        }
        let share_value = weight / found.total;
        if share_value < CROSSOVER_MIN_SHARE || *weight < CROSSOVER_MIN_WEIGHT {
            continue;
        }
        centre.push(*band);
        level.push(mean_log(*band));
        share.push(share_value);
    }
    (!centre.is_empty()).then_some(NeutralBands {
        centre,
        level,
        share,
    })
}

/// The crossover table of a log grid, as a flat tuple.
#[must_use]
pub fn measure_crossover(grid: &ImageBuffer) -> Option<Vec<f64>> {
    measure_crossover_at(grid, None)
}

/// `measure_crossover` with a known table zero, such as the Estimate's midtone.
#[must_use]
pub fn measure_crossover_at(grid: &ImageBuffer, zero: Option<f64>) -> Option<Vec<f64>> {
    let bands = neutral_bands(grid)?;
    let zero = zero.unwrap_or_else(|| match neutral_axis_midtone(grid) {
        Some(midtone) => midtone,
        None => {
            let total: f64 = bands.share.iter().sum();
            if total == 0.0 {
                0.0
            } else {
                bands
                    .share
                    .iter()
                    .zip(&bands.level)
                    .map(|(share, level)| share * level)
                    .sum::<f64>()
                    / total
            }
        }
    });
    let mut out = vec![(zero * 100_000.0).round() / 100_000.0];
    for ((centre, level), share) in bands.centre.iter().zip(&bands.level).zip(&bands.share) {
        out.push((level * 100_000.0).round() / 100_000.0);
        out.push((share * 100_000.0).round() / 100_000.0);
        for channel in 0..3 {
            out.push(((centre[channel] - level) * 100_000.0).round() / 100_000.0);
        }
    }
    Some(out)
}

fn slide_bounds(grid: &ImageBuffer, extreme_pct: f64) -> Option<[f64; 6]> {
    let base = crate::features::exposure::normalization::BASE_LUMA_CLIP;
    let cells: Vec<[f32; 3]> = grid
        .pixels
        .chunks_exact(3)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let extremes = negpy_core::fade::slide_extremes(
        &cells,
        CLIPPED_LOG,
        &[100.0 - base, base, 100.0 - extreme_pct, extreme_pct],
    )?;
    let luma_f = extremes[0];
    let luma_c = extremes[1];
    let color_f = extremes[2];
    let color_c = extremes[3];
    let mean_f = luma_f.iter().sum::<f32>() as f64 / 3.0;
    let mean_c = luma_c.iter().sum::<f32>() as f64 / 3.0;
    let mut sorted_f = color_f;
    sorted_f.sort_by(f32::total_cmp);
    let mut sorted_c = color_c;
    sorted_c.sort_by(f32::total_cmp);
    let mid_f = sorted_f[1] as f64;
    let mid_c = sorted_c[1] as f64;
    let mut out = [0.0; 6];
    for channel in 0..3 {
        out[channel] = mean_f + (f64::from(color_f[channel]) - mid_f);
        out[3 + channel] = mean_c + (f64::from(color_c[channel]) - mid_c);
    }
    Some(out)
}

/// The midtone neutral axis of a slide grid, or `None` without a trustworthy set.
#[must_use]
pub fn neutral_axis_midtone(grid: &ImageBuffer) -> Option<f64> {
    let reading = |bounds: [f64; 6], exclude_subjects: bool| {
        crate::features::exposure::metering::measure_neutral_axis(
            &grid.pixels,
            grid.width,
            grid.height,
            crate::features::exposure::normalization::LogNegativeBounds::new(
                [bounds[0], bounds[1], bounds[2]],
                [bounds[3], bounds[4], bounds[5]],
            ),
            exclude_subjects,
        )
    };
    if let Some(bounds) = slide_bounds(grid, COLOR_EXTREME_PCT) {
        if let Some(axis) = reading(bounds, true) {
            if axis.confidence >= MIN_CONFIDENCE {
                return Some(midtone_level(axis.mid));
            }
        }
    }
    if let Some(bounds) = slide_bounds(grid, ROBUST_EXTREME_PCT) {
        for exclude_subjects in [true, false] {
            if let Some(axis) = reading(bounds, exclude_subjects) {
                if axis.confidence >= ROBUST_MIN_CONFIDENCE {
                    return Some(midtone_level(axis.mid));
                }
            }
        }
    }
    None
}

fn midtone_level(mid: [f64; 3]) -> f64 {
    mid[0] * CROSSOVER_WEIGHTS[0] + mid[1] * CROSSOVER_WEIGHTS[1] + mid[2] * CROSSOVER_WEIGHTS[2]
}

/// The log-density matrix and anchor `fade_density` applies after the crosstalk unmix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FadeDensity {
    pub matrix: Option<[[f64; 3]; 3]>,
    pub offset: [f64; 3],
    pub anchor: Option<[f64; 3]>,
}

fn inverse(matrix: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let a = matrix;
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    Some([
        [
            (a[1][1] * a[2][2] - a[1][2] * a[2][1]) * inv_det,
            (a[0][2] * a[2][1] - a[0][1] * a[2][2]) * inv_det,
            (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * inv_det,
        ],
        [
            (a[1][2] * a[2][0] - a[1][0] * a[2][2]) * inv_det,
            (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * inv_det,
            (a[0][2] * a[1][0] - a[0][0] * a[1][2]) * inv_det,
        ],
        [
            (a[1][0] * a[2][1] - a[1][1] * a[2][0]) * inv_det,
            (a[0][1] * a[2][0] - a[0][0] * a[2][1]) * inv_det,
            (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * inv_det,
        ],
    ])
}

fn multiply(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|col| (0..3).map(|k| left[row][k] * right[k][col]).sum())
    })
}

fn apply(matrix: [[f64; 3]; 3], point: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| (0..3).map(|col| matrix[row][col] * point[col]).sum())
}

fn white_terms(
    process: &ProcessConfig,
    white_space: &str,
    fade_acts: bool,
) -> Option<([f64; 3], [f64; 3])> {
    let white = fade_white(process, white_space)?;
    if process.fade_white_anchor {
        let target = white_target(white);
        return Some((white, [target, target, target]));
    }
    if !white_space.is_empty() && fade_acts {
        let level = density_anchor(white, false);
        return Some((level, level));
    }
    None
}

fn base_density(
    process: &ProcessConfig,
    white_space: &str,
) -> (Option<[[f64; 3]; 3]>, Option<[f64; 3]>, [f64; 3]) {
    let unmix = effective_crosstalk_matrix(process, Some(process.process_mode))
        .map(|matrix| matrix.map(|row| row.map(f64::from)));
    let fade = fade_matrix(process);
    let matrix = match (fade, unmix) {
        (None, unmix) => unmix,
        (Some(fade), None) => Some(fade),
        (Some(fade), Some(unmix)) => Some(multiply(fade, unmix)),
    };
    let Some((white, target)) = white_terms(process, white_space, fade.is_some()) else {
        return (matrix, None, [0.0; 3]);
    };
    let anchor = unmix
        .and_then(inverse)
        .map_or(white, |inverse| apply(inverse, white));
    let linear = matrix.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let mapped = apply(linear, anchor);
    let offset = std::array::from_fn(|channel| target[channel] - mapped[channel]);
    let anchor = (!white_space.is_empty()).then_some(anchor);
    (Some(linear), anchor, offset)
}

/// The log-density matrix and anchor a render applies after the crosstalk unmix.
#[must_use]
pub fn fade_density(process: &ProcessConfig, white_space: &str) -> FadeDensity {
    let (matrix, anchor, mut offset) = base_density(process, white_space);
    if process.fade_density_shift != 0.0 && render_path(process) != RenderPath::Print {
        offset.iter_mut()
            .for_each(|value| *value -= process.fade_density_shift);
    }
    FadeDensity {
        matrix,
        offset,
        anchor,
    }
}

/// `grid` through a fade affine, with each channel capped at the anchor first.
#[must_use]
pub fn through_density(grid: &[[f32; 3]], density: &FadeDensity) -> Vec<[f32; 3]> {
    let linear = density.matrix.unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let anchor = density.anchor.unwrap_or([f32::INFINITY as f64; 3]);
    grid.iter()
        .map(|pixel| {
            let capped = [
                f64::from(pixel[0]).min(anchor[0]),
                f64::from(pixel[1]).min(anchor[1]),
                f64::from(pixel[2]).min(anchor[2]),
            ];
            std::array::from_fn(|channel| {
                let sum: f64 = (0..3).map(|col| linear[channel][col] * capped[col]).sum();
                (sum + density.offset[channel]) as f32
            })
        })
        .collect()
}

/// The fade a fresh slide render takes with `survivals`, its measured white and a shift.
#[must_use]
pub fn survival_density(
    survivals: [f64; 3],
    white: Option<[f64; 3]>,
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
    shift: f64,
) -> FadeDensity {
    let mut process = ProcessConfig {
        process_mode: crate::domain::models::ProcessMode::E6,
        fade_ratio_r: survivals[0],
        fade_ratio_g: survivals[1],
        fade_ratio_b: survivals[2],
        fade_white_anchor: white_anchor,
        fade_density_shift: shift,
        ..ProcessConfig::default()
    };
    let space = if white_space.is_empty() && camera_raw {
        "estimate"
    } else {
        white_space
    };
    let white = white.unwrap_or([0.0; 3]);
    process.fade_white_r = white[0];
    process.fade_white_g = white[1];
    process.fade_white_b = white[2];
    process.fade_white_space = space.to_string();
    fade_density(&process, space)
}
