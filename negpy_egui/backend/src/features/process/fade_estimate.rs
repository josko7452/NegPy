//! Fade Restoration's Estimate path: grays, survival ratios and the crossover table.
//!
//! Mirrors `negpy/features/process/fade.py` where it turns one Estimate grid into a
//! [`FadeEstimate`].

use crate::domain::models::{ProcessConfig, ProcessMode};
use crate::features::exposure::metering::Roi;
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::process::fade::{
    CrossoverOffsets, FadeDensity, apply_crossover, crossover_offsets, fade_density,
    measure_crossover_at, survival_density, through_density,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_core::meter::NeutralAxis;

const SPREAD_FLOOR: f64 = 0.02;
const AGREEMENT_TOLERANCE: f64 = 0.05;
const RATIO_BOUNDS: (f64, f64) = (0.2, 5.0);
const RED_SURVIVAL_BOUNDS: (f64, f64) = (0.05, 1.0);
const COLOR_EXTREME_PCT: f64 = 0.00001;
const MIN_CONFIDENCE: f64 = 0.2;
const ROBUST_EXTREME_PCT: f64 = 0.1;
const ROBUST_MIN_CONFIDENCE: f64 = 0.4;
const CLIPPED_LOG: f64 = -5.0;
const WHITE_PCT: f64 = 99.9;
const STAIN_FULL_SCALE_TOLERANCE: f64 = 0.05;
const STAIN_BLUE_MIN: f64 = 0.12;
const CROSSOVER_BAND_EDGES: [f64; 9] = [0.0, 0.06, 0.15, 0.3, 0.45, 0.6, 0.75, 0.9, 1.0];
const CROSSOVER_WEIGHTS: [f64; 3] = [1.0 / 3.0; 3];
const CROSSOVER_NEUTRAL_SPREAD: f64 = 0.25;
const CROSSOVER_CLUSTER_RADIUS: f64 = 0.12;
const CROSSOVER_RANGE_PCT: (f64, f64) = (0.5, 99.5);
const CROSSOVER_MIN_SPAN: f64 = 0.05;
const CROSSOVER_MEAN_SHIFT_STEPS: usize = 3;
const CROSSOVER_MIN_WEIGHT: f64 = 8.0;
const CROSSOVER_MIN_CELLS: usize = 64;

/// The estimate a frame gives: survivals, the measured white and its crossover table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FadeEstimate {
    pub red: Option<f64>,
    pub green: f64,
    pub blue: f64,
    pub reason: String,
    pub white: Option<[f64; 3]>,
    pub crossover: Option<Vec<f64>>,
}

impl FadeEstimate {
    fn none(reason: String) -> Self {
        Self {
            red: None,
            green: 1.0,
            blue: 1.0,
            reason,
            ..Self::default()
        }
    }
}

/// The neutral-axis references, midtone and shadow required.
#[derive(Clone, Debug, PartialEq)]
pub struct NeutralAxisRefs {
    pub mid: [f64; 3],
    pub shadow: [f64; 3],
    pub highlight: Option<[f64; 3]>,
    pub confidence: f64,
}

impl From<NeutralAxis> for NeutralAxisRefs {
    fn from(axis: NeutralAxis) -> Self {
        Self {
            mid: axis.mid,
            shadow: axis.shadow,
            highlight: axis.highlight,
            confidence: axis.confidence,
        }
    }
}

fn cells(grid: &ImageBuffer) -> Vec<[f32; 3]> {
    grid.pixels
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn mean_log(cells: &[[f64; 3]]) -> Vec<f64> {
    cells
        .iter()
        .map(|cell| (0..3).map(|ch| cell[ch] * CROSSOVER_WEIGHTS[ch]).sum())
        .collect()
}

fn measured_slide_bounds(grid: &ImageBuffer, extreme_pct: f64) -> LogNegativeBounds {
    let qs = [100.0 - 0.01, 0.01, 100.0 - extreme_pct, extreme_pct];
    let Some(extremes) = negpy_core::fade::slide_extremes(&cells(grid), CLIPPED_LOG, &qs) else {
        return LogNegativeBounds::new([0.0; 3], [0.0; 3]);
    };
    let rows: Vec<[f64; 3]> = extremes.into_iter().map(|row| row.map(f64::from)).collect();
    let [luma_f, luma_c, color_f, color_c] = [rows[0], rows[1], rows[2], rows[3]];
    let mean_f = luma_f.iter().sum::<f64>() / 3.0;
    let mean_c = luma_c.iter().sum::<f64>() / 3.0;
    let mut sorted_f = color_f;
    let mut sorted_c = color_c;
    sorted_f.sort_by(f64::total_cmp);
    sorted_c.sort_by(f64::total_cmp);
    let mid_f = sorted_f[1];
    let mid_c = sorted_c[1];
    LogNegativeBounds::new(
        std::array::from_fn(|ch| mean_f + (color_f[ch] - mid_f)),
        std::array::from_fn(|ch| mean_c + (color_c[ch] - mid_c)),
    )
}

/// `neutral_axis_from_grid`: the gray finder on the transfer meter grid.
#[must_use]
pub fn neutral_axis_from_grid(grid: &ImageBuffer) -> (Option<NeutralAxisRefs>, String) {
    let bounds = measured_slide_bounds(grid, COLOR_EXTREME_PCT);
    if let Some(axis) = meter_axis(grid, bounds, true) {
        if axis.confidence >= MIN_CONFIDENCE {
            return (Some(axis), String::new());
        }
    }
    let bounds = measured_slide_bounds(grid, ROBUST_EXTREME_PCT);
    for exclude in [true, false] {
        if let Some(axis) = meter_axis(grid, bounds, exclude) {
            if axis.confidence >= ROBUST_MIN_CONFIDENCE {
                return (Some(axis), String::new());
            }
        }
    }
    (None, "No reliable grays found on this frame.".to_string())
}

fn meter_axis(
    grid: &ImageBuffer,
    bounds: LogNegativeBounds,
    exclude_subjects: bool,
) -> Option<NeutralAxisRefs> {
    meter_axis_with_roi(grid, bounds, exclude_subjects, None)
}

fn meter_axis_with_roi(
    grid: &ImageBuffer,
    bounds: LogNegativeBounds,
    exclude_subjects: bool,
    roi: Option<Roi>,
) -> Option<NeutralAxisRefs> {
    let floors = bounds.floors.map(|value| value as f32);
    let ceils = bounds.ceils.map(|value| value as f32);
    negpy_core::meter::neutral_axis(
        &grid.pixels,
        (floors, ceils),
        (bounds.floors, bounds.ceils),
        exclude_subjects,
        &neutral_consts(),
    )
    .map(NeutralAxisRefs::from)
    .map(|refs| {
        let _ = roi;
        refs
    })
}

fn neutral_consts() -> negpy_core::meter::NeutralConsts {
    use crate::features::exposure::constants as c;
    negpy_core::meter::NeutralConsts {
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
    }
}

/// `tone_band_refs`: the neutral centre of the shadow, midtone and highlight bands.
#[must_use]
pub fn tone_band_refs(grid: &ImageBuffer) -> (Option<NeutralAxisRefs>, String) {
    use negpy_core::fade::{BandParams, neutral_bands};
    let params = BandParams {
        inner_edges: CROSSOVER_BAND_EDGES[1..CROSSOVER_BAND_EDGES.len() - 1].to_vec(),
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
    let Some(bands) = neutral_bands(&cells(grid), &params) else {
        return (None, "No reliable grays found on this frame.".to_string());
    };
    let share: Vec<f64> = bands
        .weight
        .iter()
        .map(|weight| weight / bands.total)
        .collect();
    let keep: Vec<usize> = (0..bands.centre.len())
        .filter(|&index| {
            bands.centre[index][0].is_finite()
                && share[index] >= 0.004
                && bands.weight[index] >= CROSSOVER_MIN_WEIGHT
        })
        .collect();
    if keep.is_empty() {
        return (None, "No reliable grays found on this frame.".to_string());
    }
    let group = |indices: &[usize]| -> Option<[f64; 3]> {
        let selected: Vec<usize> = keep
            .iter()
            .copied()
            .filter(|index| indices.contains(index))
            .collect();
        if selected.is_empty() {
            return None;
        }
        let total: f64 = selected.iter().map(|index| share[*index]).sum();
        Some(std::array::from_fn(|ch| {
            selected
                .iter()
                .map(|index| share[*index] * bands.centre[*index][ch])
                .sum::<f64>()
                / total
        }))
    };
    let mid = group(&[3, 4]);
    let shadow = group(&[1, 2]);
    let highlight = group(&[5, 6]);
    match (mid, shadow) {
        (Some(mid), Some(shadow)) => (
            Some(NeutralAxisRefs {
                mid,
                shadow,
                highlight,
                confidence: 1.0,
            }),
            String::new(),
        ),
        _ => (None, "No reliable grays found on this frame.".to_string()),
    }
}

/// `fade_ratios_from_neutral_axis`: the green and blue survivals relative to red.
#[must_use]
pub fn fade_ratios_from_neutral_axis(refs: Option<&NeutralAxisRefs>) -> (f64, f64, String) {
    let Some(refs) = refs else {
        return (1.0, 1.0, "No grays to measure.".to_string());
    };
    let spreads = [-(refs.mid[0]), -(refs.mid[1]), -(refs.mid[2])];
    let abs_spreads = spreads.map(f64::abs);
    let min = abs_spreads.iter().copied().fold(f64::INFINITY, f64::min);
    let max = abs_spreads.iter().copied().fold(0.0, f64::max);
    if min < SPREAD_FLOOR {
        return (
            1.0,
            1.0,
            "The grays in one channel are too flat to measure.".to_string(),
        );
    }
    if max - min <= AGREEMENT_TOLERANCE * max {
        return (
            1.0,
            1.0,
            "The three layers faded equally, so there is nothing to estimate.".to_string(),
        );
    }
    let (r, g, b) = (spreads[0], spreads[1], spreads[2]);
    let (ratio_g, ratio_b) = (g / r, b / r);
    let (lo, hi) = RATIO_BOUNDS;
    if !(lo..=hi).contains(&ratio_g) || !(lo..=hi).contains(&ratio_b) {
        let clamped_g = ratio_g.clamp(lo, hi);
        let clamped_b = ratio_b.clamp(lo, hi);
        let mut reason = String::new();
        if ratio_g != clamped_g {
            reason.push_str(&format!(
                "Green read {ratio_g:.1}, outside {lo} to {hi}, so it is set to {clamped_g:.1}."
            ));
        }
        if ratio_b != clamped_b {
            if !reason.is_empty() {
                reason.push(' ');
            }
            reason.push_str(&format!(
                "Blue read {ratio_b:.1}, outside {lo} to {hi}, so it is set to {clamped_b:.1}."
            ));
        }
        if ratio_g > hi || ratio_b > hi {
            reason.push_str(" Red faded more than this range covers. Lower Red Survival by eye.");
        }
        return (clamped_g, clamped_b, reason);
    }
    (ratio_g, ratio_b, String::new())
}

/// Estimate's red survival for the green and blue ratios: the best-kept layer at 1.
#[must_use]
pub fn estimate_red(green: f64, blue: f64) -> f64 {
    let (lo, hi) = RED_SURVIVAL_BOUNDS;
    (1.0 / green.max(blue).max(1.0)).clamp(lo, hi)
}

fn blend_refs(a: &NeutralAxisRefs, b: &NeutralAxisRefs, mix: f64) -> NeutralAxisRefs {
    let lerp =
        |x: [f64; 3], y: [f64; 3]| std::array::from_fn(|ch| (1.0 - mix) * x[ch] + mix * y[ch]);
    let highlight = match (a.highlight, b.highlight) {
        (Some(a), Some(b)) => Some(lerp(a, b)),
        (a, b) => {
            if mix < 0.5 {
                a.or(b)
            } else {
                b.or(a)
            }
        }
    };
    NeutralAxisRefs {
        mid: lerp(a.mid, b.mid),
        shadow: lerp(a.shadow, b.shadow),
        highlight,
        confidence: (1.0 - mix) * a.confidence + mix * b.confidence,
    }
}

/// Both readers' grays on `grid`.
#[must_use]
pub fn gray_readings(
    grid: &ImageBuffer,
) -> (
    (Option<NeutralAxisRefs>, String),
    (Option<NeutralAxisRefs>, String),
) {
    (neutral_axis_from_grid(grid), tone_band_refs(grid))
}

/// `blend_gray_readings`: each ref moved toward the tone bands by `mix`.
#[must_use]
pub fn blend_gray_readings(
    readings: &(
        (Option<NeutralAxisRefs>, String),
        (Option<NeutralAxisRefs>, String),
    ),
    mix: f64,
) -> (Option<NeutralAxisRefs>, String) {
    let ((ours, why), (theirs, why_nc)) = readings;
    let mix = mix.clamp(0.0, 1.0);
    if ours.is_none() && theirs.is_some() {
        return (theirs.clone(), why_nc.clone());
    }
    if mix <= 0.0 {
        return (ours.clone(), why.clone());
    }
    if mix >= 1.0 {
        return (theirs.clone(), why_nc.clone());
    }
    match (ours, theirs) {
        (Some(a), Some(b)) => (Some(blend_refs(a, b, mix)), String::new()),
        _ => (ours.clone(), why.clone()),
    }
}

/// Whether only the tone bands find grays in `readings`.
#[must_use]
pub fn tone_bands_only(
    readings: &(
        (Option<NeutralAxisRefs>, String),
        (Option<NeutralAxisRefs>, String),
    ),
) -> bool {
    readings.0.0.is_none() && readings.1.0.is_some()
}

/// `measure_white`: each channel's brightest-cell log transmittance.
#[must_use]
pub fn measure_white(grid: &ImageBuffer) -> Option<[f64; 3]> {
    negpy_core::fade::white(&cells(grid), CLIPPED_LOG, WHITE_PCT).map(|white| white.map(f64::from))
}

/// `white_anchored_grid`: density above each channel's white.
#[must_use]
pub fn white_anchored_grid(grid: &ImageBuffer, white: [f64; 3]) -> ImageBuffer {
    let pixels = grid
        .pixels
        .chunks_exact(3)
        .flat_map(|pixel| {
            std::array::from_fn::<f32, 3, _>(|ch| (pixel[ch] - white[ch] as f32).min(0.0))
        })
        .collect();
    ImageBuffer::from_rgb(grid.width, grid.height, pixels)
}

/// `stain_suggested`: the white reads as a yellow stain on clear film.
#[must_use]
pub fn stain_suggested(white: [f64; 3]) -> bool {
    white.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= -STAIN_FULL_SCALE_TOLERANCE
        && white[0].min(white[1]) - white[2] >= STAIN_BLUE_MIN
}

/// `density_anchor`: the log point a camera RAW's fade scales density about.
#[must_use]
pub fn density_anchor(white: [f64; 3], remove_stain: bool) -> [f64; 3] {
    if remove_stain {
        white
    } else {
        let level = white.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        [level; 3]
    }
}

fn gray_grid(
    grid: &ImageBuffer,
    white: Option<[f64; 3]>,
    white_anchor: bool,
    camera_raw: bool,
) -> ImageBuffer {
    match white {
        Some(white) if white_anchor || camera_raw => {
            white_anchored_grid(grid, density_anchor(white, white_anchor))
        }
        _ => grid.clone(),
    }
}

fn table_after(
    grid: &ImageBuffer,
    refs: &NeutralAxisRefs,
    white: Option<[f64; 3]>,
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
    survivals: [f64; 3],
    shift: f64,
) -> Option<Vec<f64>> {
    let density = survival_density(
        survivals,
        white,
        white_anchor,
        camera_raw,
        white_space,
        shift,
    );
    let mid = if white.is_some() && (white_anchor || camera_raw) {
        std::array::from_fn(|ch| {
            refs.mid[ch] as f32 + density_anchor(white.unwrap(), white_anchor)[ch] as f32
        })
    } else {
        refs.mid.map(|value| value as f32)
    };
    let zero = mean_log(
        &through_density(&[mid], &density)
            .iter()
            .map(|pixel| pixel.map(f64::from))
            .collect::<Vec<_>>(),
    )[0];
    let faded = through_density(&cells(grid), &density);
    let faded = ImageBuffer::from_rgb(
        grid.width,
        grid.height,
        faded.iter().flatten().copied().collect(),
    );
    measure_crossover_at(&faded, Some(zero))
}

/// `estimate_fade_with`: the survivals a gray reader finds on `grid`.
#[must_use]
pub fn estimate_fade_with(
    grid: &ImageBuffer,
    reader: impl Fn(&ImageBuffer) -> (Option<NeutralAxisRefs>, String),
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
    crossover: bool,
) -> FadeEstimate {
    let white = measure_white(grid);
    if white_anchor && white.is_none() {
        return FadeEstimate::none("No clear area to measure the stain on this frame.".to_string());
    }
    let camera_raw = camera_raw || !white_space.is_empty();
    let anchored = gray_grid(grid, white, white_anchor, camera_raw);
    let (refs, reason) = reader(&anchored);
    let Some(refs) = refs else {
        return FadeEstimate {
            white,
            reason,
            ..FadeEstimate::none(String::new())
        };
    };
    let (green, blue, why) = fade_ratios_from_neutral_axis(Some(&refs));
    let red = (!(why.is_empty() && green == 1.0 && blue == 1.0)).then(|| estimate_red(green, blue));
    let table = red.and_then(|_| {
        crossover.then(|| {
            table_after(
                grid,
                &refs,
                white,
                white_anchor,
                camera_raw,
                white_space,
                [red.unwrap(), green, blue],
                0.0,
            )
        })?
    });
    FadeEstimate {
        red,
        green,
        blue,
        reason: why,
        white,
        crossover: table,
    }
}

/// `survival_crossover`: the table a hand-set survival and shift store.
#[must_use]
pub fn survival_crossover(
    grid: &ImageBuffer,
    reader: impl Fn(&ImageBuffer) -> (Option<NeutralAxisRefs>, String),
    survivals: [f64; 3],
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
    shift: f64,
) -> Option<Vec<f64>> {
    let white = measure_white(grid);
    if white_anchor && white.is_none() {
        return None;
    }
    let camera_raw = camera_raw || !white_space.is_empty();
    let (refs, _) = reader(&gray_grid(grid, white, white_anchor, camera_raw));
    refs.and_then(|refs| {
        table_after(
            grid,
            &refs,
            white,
            white_anchor,
            camera_raw,
            white_space,
            survivals,
            shift,
        )
    })
}

/// `pick_neutrals`: the mix whose estimate leaves the least neutral spread.
#[must_use]
pub fn pick_neutrals(
    grid: &ImageBuffer,
    readings: &(
        (Option<NeutralAxisRefs>, String),
        (Option<NeutralAxisRefs>, String),
    ),
    white_anchor: bool,
    camera_raw: bool,
    white_space: &str,
) -> Option<(f64, FadeEstimate)> {
    const CANDIDATES: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    let mut best: Option<(f64, f64, FadeEstimate)> = None;
    for mix in CANDIDATES {
        let estimate = estimate_fade_with(
            grid,
            |_| blend_gray_readings(readings, mix),
            white_anchor,
            camera_raw,
            white_space,
            true,
        );
        let (Some(_red), Some(table)) = (estimate.red, estimate.crossover.as_ref()) else {
            continue;
        };
        let score = crate::features::process::fade::neutral_spread(table);
        if best
            .as_ref()
            .is_none_or(|(best_score, _, _)| score < *best_score - 1e-6)
        {
            best = Some((score, mix, estimate));
        }
    }
    best.map(|(_, mix, estimate)| (mix, estimate))
}

/// `toward_no_change`: survivals raised toward 1 by `mix`.
#[must_use]
pub fn toward_no_change(mut estimate: FadeEstimate, mix: f64) -> FadeEstimate {
    let Some(_) = estimate.red else {
        return estimate;
    };
    let mix = mix.clamp(0.0, 1.0);
    if mix >= 1.0 {
        return estimate;
    }
    let green = estimate.green.powf(mix);
    let blue = estimate.blue.powf(mix);
    estimate.red = Some(estimate_red(green, blue));
    estimate.green = green;
    estimate.blue = blue;
    estimate.crossover = None;
    estimate
}

/// `survival_density` with the estimate's own white.
#[must_use]
pub fn estimate_density(
    estimate: &FadeEstimate,
    camera_raw: bool,
    white_space: &str,
) -> FadeDensity {
    survival_density(
        [estimate.red.unwrap_or(1.0), estimate.green, estimate.blue],
        estimate.white,
        false,
        camera_raw,
        white_space,
        0.0,
    )
}

/// `with_fade_crossover`: the table the estimate stores.
#[must_use]
pub fn with_fade_crossover(process: &ProcessConfig, measured: Option<&[f64]>) -> ProcessConfig {
    let mut out = process.clone();
    out.fade_crossover_table = measured.map(<[f64]>::to_vec);
    out
}

/// The crossover offsets a render applies from a process config.
#[must_use]
pub fn render_crossover(
    process: &ProcessConfig,
    table: Option<&[f64]>,
    crossover: f64,
) -> Option<CrossoverOffsets> {
    let table = table.or(process.fade_crossover_table.as_deref());
    crossover_offsets(table, crossover)
}

/// Whether the current fade matrix and shift produce a crossover table.
#[must_use]
pub fn crossover_applies(process: &ProcessConfig, white_space: &str) -> bool {
    let density = fade_density(process, white_space);
    density.matrix.is_some() || density.offset.iter().any(|value| *value != 0.0)
}

/// Apply a table to a log image, as a render does after the density stage.
#[must_use]
pub fn apply_render_crossover(
    image: &ImageBuffer,
    table: Option<&[f64]>,
    crossover: f64,
) -> ImageBuffer {
    match crossover_offsets(table, crossover) {
        Some(offsets) => apply_crossover(image, Some(&offsets)),
        None => image.clone(),
    }
}

/// The mode Restore Range reads on.
#[must_use]
pub fn applies(mode: ProcessMode) -> bool {
    mode == ProcessMode::E6
}
