//! The transfer curve of `negpy/features/transparency/logic.py`.
//!
//! A slide renders from its capture: the fixed window's inverse, deviated by the controls
//! the user moved, then the standard display rendering. `negpy_core::transfer` holds the
//! per-pixel kernel.

use std::f64::consts::LOG10_2;

use crate::domain::models::{ExposureConfig, ProcessConfig};
use crate::features::exposure::analysis::working_oetf_encode;
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{effective_grade_range, grade_to_slope, slope_to_grade};
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::print::{
    per_channel_dye_separation, per_channel_toe_shoulder, per_channel_widths,
};
use crate::features::exposure::processor::NeutralAxisRefs;

/// Log-density window the fixed-bounds normalization maps to [0, 1].
pub const TRANSFER_DENSITY_RANGE: f64 = 3.0;
pub const SEPARATION_CAP_LEVEL: f64 = 2.5;
pub const SEPARATION_CAP_SPREAD: f64 = 1.5;
pub const SEPARATION_CAP_SOFTNESS: f64 = 0.15;
const ZONE_BLACK_TAPER: f32 = 1.0;
const STAIN_PIVOT_MIN_ARM: f64 = 0.5;
const CAST_AFFINE_GAIN_LIMIT: f64 = 2.0;

pub const TRANSFER_GRADE_REF: f64 = 115.0;
pub const TRANSFER_DENSITY_STOPS: f64 = 2.0;
pub const TRANSFER_CONTRAST_PIVOT: f64 = 0.75;
pub const TRANSFER_TOE_KNEE: f64 = 1.6;
pub const TRANSFER_SHOULDER_KNEE: f64 = 0.35;
pub const TRANSFER_BASELINE_EV: f64 = 0.7;
pub const TRANSFER_KNEE_WIDTH: f64 = 0.45;
pub const TRANSFER_WIDTH_REF: f64 = 2.5;

/// The fixed log window a slide normalizes with.
#[must_use]
pub fn transfer_bounds(density_range: f64) -> LogNegativeBounds {
    LogNegativeBounds::new([0.0; 3], [-density_range; 3])
}

/// White and Black Point offsets on the fixed window, negated.
#[must_use]
pub fn transfer_point_offsets(process: &ProcessConfig) -> ([f64; 3], [f64; 3]) {
    let (white, black) = per_channel_point_offsets(process);
    (white.map(|value| -value), black.map(|value| -value))
}

fn per_channel_point_offsets(process: &ProcessConfig) -> ([f64; 3], [f64; 3]) {
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

/// Per-channel knee softness from the Toe and Shoulder Width sliders.
#[must_use]
pub fn transfer_widths(exposure: &ExposureConfig) -> ([f64; 3], [f64; 3]) {
    let (toe, shoulder) = per_channel_widths(
        exposure.toe_width,
        exposure.shoulder_width,
        [
            exposure.toe_width_trim_red,
            exposure.toe_width_trim_green,
            exposure.toe_width_trim_blue,
        ],
        [
            exposure.shoulder_width_trim_red,
            exposure.shoulder_width_trim_green,
            exposure.shoulder_width_trim_blue,
        ],
    );
    let scale = TRANSFER_KNEE_WIDTH / TRANSFER_WIDTH_REF;
    (
        toe.map(|value| value * scale),
        shoulder.map(|value| value * scale),
    )
}

/// The exposure offset and contrast the manual sliders ask for.
#[must_use]
pub fn transfer_curve_params(exposure: &ExposureConfig) -> (f64, f64) {
    let offset = (1.0 - exposure.density) * TRANSFER_DENSITY_STOPS * LOG10_2;
    let contrast = if exposure.grade > 1e-6 {
        TRANSFER_GRADE_REF / exposure.grade
    } else {
        1.0
    };
    (offset, contrast)
}

/// The transfer curve's toe and shoulder, in the kernel's normalized density.
#[must_use]
pub fn transfer_shape(exposure: &ExposureConfig) -> ([f64; 3], [f64; 3]) {
    per_channel_toe_shoulder(
        exposure.toe,
        exposure.shoulder,
        [
            exposure.toe_trim_red,
            exposure.toe_trim_green,
            exposure.toe_trim_blue,
        ],
        [
            exposure.shoulder_trim_red,
            exposure.shoulder_trim_green,
            exposure.shoulder_trim_blue,
        ],
    )
}

fn zone_geometry() -> (f64, f64, f64) {
    let span = c::D_MAX - c::D_MIN;
    let anchor = c::REFERENCE_PRINT_DENSITY;
    (
        (anchor + c::ZONE_DENSITY_SHADOW_OFFSET - c::D_MIN) / span * TRANSFER_DENSITY_RANGE,
        (anchor + c::ZONE_DENSITY_HIGHLIGHT_OFFSET - c::D_MIN) / span * TRANSFER_DENSITY_RANGE,
        c::ZONE_DENSITY_SHARPNESS * span / TRANSFER_DENSITY_RANGE,
    )
}

fn wb_split_geometry() -> (f64, f64) {
    let span = c::D_MAX - c::D_MIN;
    (
        (c::REFERENCE_PRINT_DENSITY - c::D_MIN) / span * TRANSFER_DENSITY_RANGE,
        3.0 * span / TRANSFER_DENSITY_RANGE,
    )
}

fn transfer_shadow_reach_density() -> f64 {
    (c::SHADOW_REACH_DENSITY - c::D_MIN) / (c::D_MAX - c::D_MIN) * TRANSFER_DENSITY_RANGE
}

fn transfer_highlight_hold_density() -> f64 {
    (c::HIGHLIGHT_HOLD_DENSITY - c::D_MIN) / (c::D_MAX - c::D_MIN) * TRANSFER_DENSITY_RANGE
}

fn transfer_shadow_hold_start() -> f64 {
    transfer_shadow_reach_density()
        + c::SHADOW_HOLD_OVERSHOOT / (c::D_MAX - c::D_MIN) * TRANSFER_DENSITY_RANGE
}

fn fast_sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// The autos folded onto the manual values, as the CPU and GPU uniforms read them.
#[must_use]
pub fn transfer_auto_terms(
    exposure: &ExposureConfig,
    manual_offset: f64,
    manual_contrast: f64,
    textural_range: Option<f64>,
    anchor: Option<f64>,
    shadow_point: Option<f64>,
    highlight_point: Option<f64>,
) -> (f64, f64, f64, f64) {
    let pivot = TRANSFER_CONTRAST_PIVOT;
    let range = TRANSFER_DENSITY_RANGE;
    let mut offset = manual_offset;
    if exposure.auto_exposure
        && let Some(anchor) = anchor
    {
        offset += anchor * range - pivot;
    }
    let mut contrast = manual_contrast;
    if exposure.auto_normalize_contrast
        && let Some(textural) = textural_range
    {
        let effective = effective_grade_range(true, Some(range), Some(textural)).unwrap_or(range);
        let reference = grade_to_slope(TRANSFER_GRADE_REF, Some(range), 1.0);
        if reference > 1e-9 {
            contrast = grade_to_slope(exposure.grade, Some(effective), 1.0) / reference;
        }
    }
    if exposure.auto_normalize_contrast
        && let (Some(anchor), Some(shadow_point)) = (anchor, shadow_point)
    {
        let span = shadow_point - anchor;
        if span > 1e-6 {
            let denom = shadow_point * range - offset - pivot;
            if denom > 1e-6 {
                let needed = (transfer_shadow_reach_density() - pivot) / denom;
                contrast = contrast.max(needed).min(c::SLOPE_MAX);
            }
        }
    }
    let mut highlight_auto = 0.0;
    if exposure.auto_normalize_contrast
        && let Some(highlight_point) = highlight_point
    {
        let target = transfer_highlight_hold_density();
        let d_highlight = pivot + (highlight_point * range - offset - pivot) * contrast;
        if d_highlight < target {
            let (_, hi_c, k_zone) = zone_geometry();
            let weight = 1.0 - fast_sigmoid(k_zone * (d_highlight - hi_c));
            highlight_auto = ((target - d_highlight) / weight.max(1e-6)).min(c::HIGHLIGHT_HOLD_MAX);
        }
    }
    let mut shadow_auto = 0.0;
    if exposure.auto_normalize_contrast
        && let Some(shadow_point) = shadow_point
    {
        let strength = c::SHADOW_HOLD_STRENGTH;
        if strength > 0.0 {
            let d_shadow = pivot + (shadow_point * range - offset - pivot) * contrast;
            let over = d_shadow - transfer_shadow_hold_start();
            if over > 0.0 {
                shadow_auto = -(strength * over).min(c::SHADOW_HOLD_MAX);
            }
        }
    }
    (offset, contrast, highlight_auto, shadow_auto)
}

/// Filtration divided by the curve's contrast, so a Grade change keeps the same gain.
#[must_use]
pub fn contrast_free_cmy(cmy: [f64; 3], contrast: f64) -> [f64; 3] {
    let k = contrast.max(1e-6);
    cmy.map(|value| value / k)
}

fn normalize_refs(refs: [f64; 3], bounds: LogNegativeBounds) -> [f64; 3] {
    std::array::from_fn(|ch| {
        let mut denom = bounds.ceils[ch] - bounds.floors[ch];
        if denom.abs() < 1e-6 {
            denom = if denom >= 0.0 { 1e-6 } else { -1e-6 };
        }
        (refs[ch] - bounds.floors[ch]) / denom
    })
}

fn neutral_axis_affine(
    axis: Option<([f64; 3], [f64; 3], Option<[f64; 3]>)>,
    strength: f64,
) -> ([f64; 3], [f64; 3]) {
    let Some((mid, shadow, _)) = axis.filter(|_| strength > 0.0) else {
        return ([1.0; 3], [0.0; 3]);
    };
    let limit = c::MIDTONE_CAST_MAX_OFFSET;
    let clamp_dev =
        |green: f64, value: f64| green + (strength * (value - green)).clamp(-limit, limit);
    let (m_g, s_g) = (mid[1], shadow[1]);
    let mut gains = [1.0; 3];
    let mut offsets = [0.0; 3];
    for ch in [0, 2] {
        let u_m = clamp_dev(m_g, mid[ch]);
        let u_s = clamp_dev(s_g, shadow[ch]);
        let du = u_m - u_s;
        if du.abs() < 1e-6 {
            continue;
        }
        let gain = ((m_g - s_g) / du)
            .max(1.0 / CAST_AFFINE_GAIN_LIMIT)
            .min(CAST_AFFINE_GAIN_LIMIT);
        gains[ch] = gain;
        offsets[ch] = m_g - gain * u_m;
    }
    (gains, offsets)
}

/// Cast Removal's per-channel (gain, offset) on normalized density.
#[must_use]
pub fn transfer_cast_affine(
    bounds: LogNegativeBounds,
    refs: Option<NeutralAxisRefs>,
    slider_strength: f64,
    white_target: Option<f64>,
) -> ([f64; 3], [f64; 3]) {
    let Some(refs) = refs else {
        return ([1.0; 3], [0.0; 3]);
    };
    let strength = refs.confidence * slider_strength;
    let mut mid = normalize_refs(refs.mid, bounds);
    if let Some(target) = white_target
        && target - refs.mid[1] >= STAIN_PIVOT_MIN_ARM
    {
        mid = normalize_refs([target; 3], bounds);
    }
    let shadow = normalize_refs(refs.shadow, bounds);
    let highlight = refs.highlight.map(|value| normalize_refs(value, bounds));
    neutral_axis_affine(Some((mid, shadow, highlight)), strength)
}

/// The per-pixel parameters of one transfer render.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferSettings {
    pub density_range: f32,
    pub cast_gain: [f32; 3],
    pub cast_offset: [f32; 3],
    pub exposure_offset: f32,
    pub cmy: [f32; 3],
    pub contrast: f32,
    pub shadow_density: f32,
    pub highlight_density: f32,
    pub toe: [f32; 3],
    pub shoulder: [f32; 3],
    pub toe_width: [f32; 3],
    pub shoulder_width: [f32; 3],
    pub shadow_cmy: [f32; 3],
    pub highlight_cmy: [f32; 3],
    pub separation: f64,
    pub separation_trims: [f64; 3],
    pub damping: f64,
    pub positive_source: bool,
}

impl Default for TransferSettings {
    fn default() -> Self {
        let width = TRANSFER_KNEE_WIDTH as f32;
        Self {
            density_range: TRANSFER_DENSITY_RANGE as f32,
            cast_gain: [1.0; 3],
            cast_offset: [0.0; 3],
            exposure_offset: 0.0,
            cmy: [0.0; 3],
            contrast: 1.0,
            shadow_density: 0.0,
            highlight_density: 0.0,
            toe: [0.0; 3],
            shoulder: [0.0; 3],
            toe_width: [width; 3],
            shoulder_width: [width; 3],
            shadow_cmy: [0.0; 3],
            highlight_cmy: [0.0; 3],
            separation: 1.0,
            separation_trims: [0.0; 3],
            damping: 0.0,
            positive_source: false,
        }
    }
}

impl TransferSettings {
    /// The core kernel parameters.
    #[must_use]
    pub fn core(&self) -> negpy_core::transfer::Transfer {
        let (wb_c, wb_k) = wb_split_geometry();
        let (sh_c, hi_c, zone_k) = zone_geometry();
        let sep = per_channel_dye_separation(self.separation, self.separation_trims);
        negpy_core::transfer::Transfer {
            density_range: self.density_range,
            cast_gain: self.cast_gain,
            cast_offset: self.cast_offset,
            cast_on: std::array::from_fn(|ch| {
                self.cast_gain[ch] != 1.0 || self.cast_offset[ch] != 0.0
            }),
            exposure_offset: self.exposure_offset,
            cmy: self.cmy,
            exposure_on: std::array::from_fn(|ch| {
                self.exposure_offset != 0.0 || self.cmy[ch] != 0.0
            }),
            contrast: self.contrast,
            contrast_on: self.contrast != 1.0,
            pivot: TRANSFER_CONTRAST_PIVOT as f32,
            shadow_cmy: self.shadow_cmy,
            highlight_cmy: self.highlight_cmy,
            regional_on: std::array::from_fn(|ch| {
                self.shadow_cmy[ch] != 0.0 || self.highlight_cmy[ch] != 0.0
            }),
            wb_c: wb_c as f32,
            wb_k: wb_k as f32,
            shadow_density: self.shadow_density,
            highlight_density: self.highlight_density,
            zone_on: self.shadow_density != 0.0 || self.highlight_density != 0.0,
            zone_sh: sh_c as f32,
            zone_hi: hi_c as f32,
            zone_k: zone_k as f32,
            black_taper: ZONE_BLACK_TAPER,
            toe: self.toe,
            shoulder: self.shoulder,
            toe_width: self.toe_width,
            shoulder_width: self.shoulder_width,
            toe_knee: TRANSFER_TOE_KNEE as f32,
            shoulder_knee: TRANSFER_SHOULDER_KNEE as f32,
            sep_k: sep.map(|value| value as f32),
            sep_on: sep != [1.0; 3],
            cap_level: SEPARATION_CAP_LEVEL as f32,
            cap_offset: (SEPARATION_CAP_SPREAD - SEPARATION_CAP_LEVEL) as f32,
            cap_softness: SEPARATION_CAP_SOFTNESS as f32,
            damping: self.damping as f32,
            damping_on: self.damping > 0.0,
            ref_spread: 0.35,
            positive_source: self.positive_source,
            gain: 2f32.powf(TRANSFER_BASELINE_EV as f32),
        }
    }
}

/// Render a normalized HxWx3 log image through the transfer curve.
#[must_use]
pub fn apply_transfer_curve(image: &[f32], settings: &TransferSettings) -> Vec<f32> {
    let mut out = vec![0.0; image.len()];
    negpy_core::transfer::apply_transfer_curve(image, &settings.core(), &mut out);
    out
}

/// The standard display rendering a raw converter opens with.
#[must_use]
pub fn display_rendering(values: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let x = value.max(0.0);
        let num = x * (2.51 * x + 0.03);
        let den = x * (2.43 * x + 0.59) + 0.14;
        out.push((num / den.max(1e-8)).clamp(0.0, 1.0));
    }
    working_oetf_encode(&out)
}

/// The exposure a slide renders: B&W drops the color terms the UI hides there.
#[must_use]
pub fn slide_exposure(process: &ProcessConfig, exposure: &ExposureConfig) -> ExposureConfig {
    if !process.slide_bw {
        return exposure.clone();
    }
    ExposureConfig {
        wb_cyan: 0.0,
        wb_magenta: 0.0,
        wb_yellow: 0.0,
        shadow_cyan: 0.0,
        shadow_magenta: 0.0,
        shadow_yellow: 0.0,
        highlight_cyan: 0.0,
        highlight_magenta: 0.0,
        highlight_yellow: 0.0,
        dye_separation_trim_red: 0.0,
        dye_separation_trim_green: 0.0,
        dye_separation_trim_blue: 0.0,
        cast_removal_strength: 0.0,
        dye_separation: 1.0,
        separation_damping: 0.0,
        ..exposure.clone()
    }
}

/// The B&W collapse weights a slide uses, or `None` for a color slide.
#[must_use]
pub fn slide_mono_weights(process: &ProcessConfig, exposure: &ExposureConfig) -> Option<[f32; 3]> {
    process
        .slide_bw
        .then(|| crate::features::exposure::bw_filter::collapse_weights(process, exposure))
}

/// Slider values shown for the active autos, as the transfer curve prints them.
#[must_use]
pub fn transfer_shown_values(
    exposure: &ExposureConfig,
    metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
) -> Vec<(&'static str, f64)> {
    let (_, offset, contrast, highlight, shadow) = transfer_totals(exposure, metrics);
    let mut out = Vec::new();
    let offset_per_density = TRANSFER_DENSITY_STOPS * LOG10_2;
    if exposure.auto_exposure && metrics.metered_anchor.is_some() {
        out.push(("density", 1.0 - offset / offset_per_density));
    }
    if exposure.auto_normalize_contrast && metrics.textural_range.is_some() {
        out.push(("grade", TRANSFER_GRADE_REF / contrast));
    }
    if exposure.auto_normalize_contrast && metrics.highlight_point.is_some() {
        out.push(("highlight_density", exposure.highlight_density + highlight));
    }
    if exposure.auto_normalize_contrast && metrics.shadow_point.is_some() {
        out.push(("shadow_density", exposure.shadow_density + shadow));
    }
    out
}

/// (manual offset, total offset, total contrast, highlight hold, shadow hold).
fn transfer_totals(
    exposure: &ExposureConfig,
    metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
) -> (f64, f64, f64, f64, f64) {
    let (manual_offset, manual_contrast) = transfer_curve_params(exposure);
    let (offset, contrast, highlight, shadow) = transfer_auto_terms(
        exposure,
        manual_offset,
        manual_contrast,
        metrics.textural_range,
        metrics.metered_anchor,
        metrics.shadow_point,
        metrics.highlight_point,
    );
    (manual_offset, offset, contrast, highlight, shadow)
}

/// Inverse of [`transfer_shown_values`] for one field.
#[must_use]
pub fn transfer_stored_value(
    exposure: &ExposureConfig,
    metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
    field: &str,
    shown: f64,
) -> f64 {
    let (manual_offset, offset, _, highlight, shadow) = transfer_totals(exposure, metrics);
    match field {
        "density" => shown + (offset - manual_offset) / (TRANSFER_DENSITY_STOPS * LOG10_2),
        "grade" => {
            let reference = grade_to_slope(TRANSFER_GRADE_REF, Some(TRANSFER_DENSITY_RANGE), 1.0);
            let slope = TRANSFER_GRADE_REF / shown * reference;
            slope_to_grade(
                slope,
                effective_grade_range(true, Some(TRANSFER_DENSITY_RANGE), metrics.textural_range),
                1.0,
            )
        }
        "shadow_density" => shown - shadow,
        _ => shown - highlight,
    }
}

/// Where the source-to-working matrix applies around the log-density matrix of a render.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DensityStage {
    /// Linear 3x3 before the log.
    pub pre: Option<[[f64; 3]; 3]>,
    /// Log-density affine `(linear, offset)`.
    pub unmix: Option<[[f64; 4]; 3]>,
    /// Linear 3x3 after the log-density matrix.
    pub post: Option<[[f64; 3]; 3]>,
    /// The fade's anchor in the stage's input log.
    pub anchor: Option<[f64; 3]>,
}

fn is_identity(matrix: [[f64; 3]; 3]) -> bool {
    matrix.iter().enumerate().all(|(row, values)| {
        values
            .iter()
            .enumerate()
            .all(|(col, value)| (*value - f64::from(u8::from(row == col))).abs() < 1e-6)
    })
}

fn with_log_offset(
    unmix: Option<[[f64; 3]; 3]>,
    offset: Option<[f64; 3]>,
) -> Option<[[f64; 4]; 3]> {
    match (unmix, offset) {
        (None, None) => None,
        (None, Some(offset)) => Some(std::array::from_fn(|row| {
            [
                f64::from(u8::from(row == 0)),
                f64::from(u8::from(row == 1)),
                f64::from(u8::from(row == 2)),
                offset[row],
            ]
        })),
        (Some(matrix), offset) => Some(std::array::from_fn(|row| {
            let offset = offset.unwrap_or([0.0; 3]);
            let t = (0..3)
                .map(|col| matrix[row][col] * offset[col])
                .sum::<f64>();
            [matrix[row][0], matrix[row][1], matrix[row][2], t]
        })),
    }
}

fn whole_matrix(
    process: &ProcessConfig,
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
) -> Option<[[f64; 3]; 3]> {
    let wb = if crate::features::process::logic::should_fold_camera_wb(process) {
        camera_wb
    } else {
        None
    };
    crate::features::process::capture_color::camera_to_working_matrix(cam_xyz, wb)
}

/// The stage that puts a source in the channels the crosstalk unmix, fade and stain offset act on.
#[must_use]
pub fn density_channels(
    process: &ProcessConfig,
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
) -> DensityStage {
    let wb = if crate::features::process::logic::should_fold_camera_wb(process) {
        camera_wb
    } else {
        None
    };
    let (gains, primaries) =
        crate::features::process::capture_color::camera_matrix_parts(cam_xyz, wb);
    let offset = gains.map(|gains| gains.map(f64::log10));
    let post = primaries.filter(|matrix| !is_identity(*matrix));
    DensityStage {
        pre: None,
        unmix: with_log_offset(None, offset),
        post,
        anchor: None,
    }
}

/// The stage a render applies, with the fade anchor moved into its input log.
#[must_use]
pub fn density_stage(
    process: &ProcessConfig,
    cam_xyz: Option<[[f64; 3]; 3]>,
    camera_wb: Option<&[f64]>,
    unmix: Option<[[f64; 3]; 3]>,
    anchor: Option<[f64; 3]>,
) -> DensityStage {
    if unmix.is_none()
        || crate::features::process::path::render_path(process)
            == crate::features::process::path::RenderPath::Print
    {
        return DensityStage {
            pre: whole_matrix(process, cam_xyz, camera_wb),
            unmix: with_log_offset(unmix, None),
            post: None,
            anchor,
        };
    }
    let channels = density_channels(process, cam_xyz, camera_wb);
    let offset = channels
        .unmix
        .map(|matrix| std::array::from_fn(|row| matrix[row][3]));
    let anchor = match (anchor, offset) {
        (Some(anchor), Some(offset)) => Some(std::array::from_fn(|ch| anchor[ch] - offset[ch])),
        _ => anchor,
    };
    DensityStage {
        pre: channels.pre,
        unmix: with_log_offset(unmix, offset),
        post: channels.post,
        anchor,
    }
}

/// The slide Temperature of a `(cyan, yellow)` pair, in the mired domain.
#[must_use]
pub fn slide_wb_to_kelvin(cyan: f64, yellow: f64) -> f64 {
    const REF_KELVIN: f64 = 5500.0;
    const K_CYAN: f64 = 0.0031;
    const K_YELLOW: f64 = 0.0065;
    let dmu = (-K_CYAN * cyan + K_YELLOW * yellow) / (K_CYAN * K_CYAN + K_YELLOW * K_YELLOW);
    let mu = (1e6 / REF_KELVIN + dmu).clamp(1e6 / 12_000.0, 1e6 / 3_000.0);
    1e6 / mu
}

/// Move `(cyan, yellow)` along the Planckian direction to `kelvin`.
#[must_use]
pub fn slide_kelvin_to_wb(kelvin: f64, cyan: f64, yellow: f64) -> (f64, f64) {
    const REF_KELVIN: f64 = 5500.0;
    const K_CYAN: f64 = 0.0031;
    const K_YELLOW: f64 = 0.0065;
    let kelvin = kelvin.clamp(3_000.0, 12_000.0);
    let dmu_cur = (-K_CYAN * cyan + K_YELLOW * yellow) / (K_CYAN * K_CYAN + K_YELLOW * K_YELLOW);
    let d = (1e6 / kelvin - 1e6 / REF_KELVIN) - dmu_cur;
    (
        (cyan - K_CYAN * d).clamp(-1.0, 1.0),
        (yellow + K_YELLOW * d).clamp(-1.0, 1.0),
    )
}

/// The contrast the slide curve prints with, including a metered auto term.
#[must_use]
pub fn transfer_total_contrast(
    exposure: &ExposureConfig,
    metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
) -> f64 {
    transfer_totals(exposure, metrics).2
}

/// `(cyan, magenta, yellow)` that print a picked patch gray.
#[must_use]
pub fn slide_pick_wb(
    sampled_norm_rgb: [f64; 3],
    channel_ranges: [f64; 3],
    contrast: f64,
) -> (f64, f64, f64) {
    const K_CYAN: f64 = 0.0031;
    const K_YELLOW: f64 = 0.0065;
    let cmy_max = c::CMY_MAX_DENSITY;
    let k = contrast.max(1e-6);
    let offsets = std::array::from_fn::<f64, 3, _>(|ch| {
        -k * sampled_norm_rgb[ch] * channel_ranges[ch] / cmy_max
    });
    let a = (offsets[2] - offsets[0]) / (K_CYAN + K_YELLOW);
    let tint = offsets[1] - offsets[0] - K_CYAN * a;
    (-K_CYAN * a, tint, K_YELLOW * a)
}
