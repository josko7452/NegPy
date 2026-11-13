//! Scalar exposure math of `negpy/features/exposure/logic.py`: grade conversion, auto-grade
//! range, pivot placement and the shadow/highlight holds.

use negpy_core::mathf::{exp10_f64, log10_f64, tanh_f64};

use crate::features::exposure::constants as c;
use crate::features::exposure::normalization::LogNegativeBounds;
use crate::features::exposure::papers::{PaperProfile, effective_constants, slope_gain};

/// Per-layer ISO-R trim as a slope ratio: `R0 / clamp(R0 + trim)`.
#[must_use]
pub fn grade_trim_mult(grade: f64, trim: f64) -> f64 {
    let r0 = grade.clamp(c::ISO_R_MIN, c::ISO_R_MAX);
    let r1 = (r0 + trim).clamp(c::ISO_R_MIN, c::ISO_R_MAX);
    r0 / r1
}

/// Split-grade ISO-R trims as per-layer local contrast gains (multiplier - 1).
#[must_use]
pub fn split_grade_deltas(
    grade: f64,
    shadow_grade: f64,
    highlight_grade: f64,
    shadow_trims: [f64; 3],
    highlight_trims: [f64; 3],
) -> ([f64; 3], [f64; 3]) {
    (
        std::array::from_fn(|ch| grade_trim_mult(grade, shadow_grade + shadow_trims[ch]) - 1.0),
        std::array::from_fn(|ch| {
            grade_trim_mult(grade, highlight_grade + highlight_trims[ch]) - 1.0
        }),
    )
}

/// Grade-coupled baseline toe and shoulder: hard grades get snappier knees.
#[must_use]
pub fn grade_coupled_shape(slope_g: f64, toe: f64, shoulder: f64) -> (f64, f64) {
    let slope_norm = ((slope_g - c::SLOPE_MIN) / (c::SLOPE_MAX - c::SLOPE_MIN)).clamp(0.0, 1.0);
    (
        toe + c::TOE_GRADE_STRENGTH * slope_norm,
        shoulder + c::SHOULDER_GRADE_STRENGTH * slope_norm,
    )
}

/// The straight-line slope before the per-layer gammas: Grade over the effective range, raised
/// to Auto Grade's Shadow Reach floor.
#[must_use]
pub fn auto_base_slope(
    grade: f64,
    auto_normalize_contrast: bool,
    lum_range: Option<f64>,
    textural_range: Option<f64>,
    shadow_point: Option<f64>,
    anchor: Option<f64>,
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> f64 {
    let range = effective_grade_range(auto_normalize_contrast, lum_range, textural_range);
    let slope = grade_to_slope(grade, range, slope_gain(paper));
    match (auto_normalize_contrast, shadow_point) {
        (true, Some(point)) => {
            let reference = anchor.unwrap_or(c::ASSUMED_ANCHOR);
            shadow_reach_slope(
                slope,
                reference,
                point,
                d_min,
                paper,
                midtone_gamma,
                anchor.is_some(),
            )
        }
        _ => slope,
    }
}

/// Per-channel (slope, pivot, curvature). Cast Removal fits R/B to green's neutral axis.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn per_channel_curve_params(
    grade: f64,
    density: f64,
    auto_normalize_contrast: bool,
    strength: f64,
    lum_range: Option<f64>,
    textural_range: Option<f64>,
    d_min: f64,
    anchor: Option<f64>,
    paper: Option<&PaperProfile>,
    neutral_axis: Option<NeutralAxisNorm>,
    grade_trims: [f64; 3],
    shadow_point: Option<f64>,
    midtone_gamma: Option<f64>,
) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let mut channel_gamma = paper.map_or([1.0; 3], |profile| profile.channel_gamma);
    if grade_trims != [0.0; 3] {
        channel_gamma =
            std::array::from_fn(|ch| channel_gamma[ch] * grade_trim_mult(grade, grade_trims[ch]));
    }
    let base_slope = auto_base_slope(
        grade,
        auto_normalize_contrast,
        lum_range,
        textural_range,
        shadow_point,
        anchor,
        d_min,
        paper,
        midtone_gamma,
    );
    if strength > 0.0 {
        if let Some((mid, shadow, highlight)) = neutral_axis {
            let (m_g, s_g) = (mid[1], shadow[1]);
            let slope_g = (base_slope * channel_gamma[1]).clamp(c::SLOPE_MIN, c::SLOPE_MAX);
            let pivot_g = compute_pivot(slope_g, density, d_min, anchor, paper, midtone_gamma);
            let target = |g: f64| slope_g * (g - pivot_g);
            let (t_m, t_s) = (target(m_g), target(s_g));
            let limit = c::MIDTONE_CAST_MAX_OFFSET;
            let curv_lim = c::NEUTRAL_AXIS_CURV_MAX_RATIO;
            let clamp_dev = |g: f64, v: f64| g + (strength * (v - g)).clamp(-limit, limit);
            let mut slopes = [slope_g; 3];
            let mut pivots = [pivot_g; 3];
            let mut curvs = [0.0; 3];
            for ch in [0, 2] {
                let u_m = clamp_dev(m_g, mid[ch]);
                let u_s = clamp_dev(s_g, shadow[ch]);
                let mut curv = 0.0;
                if let Some(hl) = highlight {
                    let h_g = hl[1];
                    let u_h = clamp_dev(h_g, hl[ch]);
                    curv = quadratic_leading(u_h, u_m, u_s, target(h_g), t_m, t_s);
                    curv = curv.clamp(-curv_lim * slope_g, curv_lim * slope_g);
                }
                let du = u_m - u_s;
                let raw = if du.abs() < 1e-6 {
                    slope_g
                } else {
                    ((t_m - t_s) - curv * (u_m * u_m - u_s * u_s)) / du
                };
                let slope_ch = (raw * channel_gamma[ch]).clamp(c::SLOPE_MIN, c::SLOPE_MAX);
                let curv_ch = curv * channel_gamma[ch];
                let pivot_ch = if slope_ch.abs() > 1e-6 {
                    u_m - (t_m - curv_ch * u_m * u_m) / slope_ch
                } else {
                    pivot_g
                };
                slopes[ch] = slope_ch;
                pivots[ch] = pivot_ch;
                curvs[ch] = curv_ch;
            }
            return (slopes, pivots, curvs);
        }
    }
    let slopes = std::array::from_fn(|ch| {
        (base_slope * channel_gamma[ch]).clamp(c::SLOPE_MIN, c::SLOPE_MAX)
    });
    let pivots = std::array::from_fn(|ch| {
        compute_pivot(slopes[ch], density, d_min, anchor, paper, midtone_gamma)
    });
    (slopes, pivots, [0.0; 3])
}

/// The per-channel curve re-derived from the metrics a render published.
#[must_use]
pub fn curve_params_from_metrics(
    exposure: &crate::domain::models::ExposureConfig,
    mode: Option<crate::domain::models::ProcessMode>,
    metrics: &crate::features::exposure::auto_metrics::AutoSliderMetrics,
) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let profile =
        crate::features::exposure::papers::effective_paper_profile(&exposure.paper_profile, mode);
    let d_min = if exposure.paper_dmin {
        profile.tonal.d_min
    } else {
        0.0
    };
    let anchor = exposure
        .auto_exposure
        .then_some(metrics.metered_anchor)
        .flatten();
    let (strength, neutral_axis) = cast_solve_inputs(
        metrics
            .bounds
            .unwrap_or(LogNegativeBounds::new([-1.0; 3], [0.0; 3])),
        metrics.neutral_axis,
        exposure.cast_removal_strength,
    );
    per_channel_curve_params(
        exposure.grade,
        exposure.density,
        exposure.auto_normalize_contrast,
        strength,
        metrics.norm_density_range,
        metrics.textural_range,
        d_min,
        anchor,
        Some(profile),
        if strength > 0.0 { neutral_axis } else { None },
        [
            exposure.grade_trim_red,
            exposure.grade_trim_green,
            exposure.grade_trim_blue,
        ],
        metrics.shadow_point,
        Some(effective_midtone_gamma(
            Some(profile),
            exposure.midtone_gamma,
        )),
    )
}

/// (midtone, shadow, highlight) references in normalized density.
pub type NeutralAxisNorm = ([f64; 3], [f64; 3], Option<[f64; 3]>);

/// `cast_solve_inputs`: the confidence-scaled strength and the refs normalized to the window.
#[must_use]
pub fn cast_solve_inputs(
    bounds: LogNegativeBounds,
    refs: Option<crate::features::exposure::processor::NeutralAxisRefs>,
    slider_strength: f64,
) -> (f64, Option<NeutralAxisNorm>) {
    let Some(refs) = refs else {
        return (slider_strength, None);
    };
    let norm = |rgb: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|ch| {
            let mut denom = bounds.ceils[ch] - bounds.floors[ch];
            if denom.abs() < 1e-6 {
                denom = if denom >= 0.0 { 1e-6 } else { -1e-6 };
            }
            (rgb[ch] - bounds.floors[ch]) / denom
        })
    };
    (
        refs.confidence * slider_strength,
        Some((norm(refs.mid), norm(refs.shadow), refs.highlight.map(norm))),
    )
}

/// The leading coefficient of the quadratic through three points, or 0 when they are collinear.
fn quadratic_leading(u_h: f64, u_m: f64, u_s: f64, t_h: f64, t_m: f64, t_s: f64) -> f64 {
    let det = (u_h - u_m) * (u_m - u_s) * (u_s - u_h);
    if det.abs() < 1e-12 {
        return 0.0;
    }
    (u_s * (t_h - t_m) + u_m * (t_s - t_h) + u_h * (t_m - t_s)) / det
}

/// White-balance CMY as normalized-space offsets over each channel's stretch range.
#[must_use]
pub fn filtration_offsets(wb_cmy: [f64; 3], bounds: Option<LogNegativeBounds>) -> [f64; 3] {
    std::array::from_fn(|ch| {
        let mut density = wb_cmy[ch] * c::CMY_MAX_DENSITY;
        if let Some(bounds) = bounds {
            density /= (bounds.ceils[ch] - bounds.floors[ch]).abs().max(1e-6);
        }
        density
    })
}

/// Each channel's stretch range in log10 units; 1 without bounds.
#[must_use]
pub fn channel_density_ranges(bounds: Option<LogNegativeBounds>) -> [f64; 3] {
    bounds.map_or([1.0; 3], |bounds| {
        std::array::from_fn(|ch| (bounds.ceils[ch] - bounds.floors[ch]).abs().max(1e-6))
    })
}

/// Fallback density range when none is measured: a normal negative's, scaled by K.
#[must_use]
pub fn default_grade_range() -> f64 {
    c::AUTO_GRADE_TARGET * c::AUTO_GRADE_NOMINAL_RATIO * c::AUTO_GRADE_NOMINAL_RANGE
}

/// Straight-line slope from an ISO-R grade and a measured density range.
#[must_use]
pub fn grade_to_slope(grade: f64, density_range: Option<f64>, gain: f64) -> f64 {
    let range = density_range.unwrap_or_else(default_grade_range);
    let er = grade.clamp(c::ISO_R_MIN, c::ISO_R_MAX) / 100.0;
    let rng = range.abs().clamp(0.3, 3.5);
    let k = c::GRADE_CONTRAST_SCALE * rng / er * gain;
    k.clamp(c::SLOPE_MIN, c::SLOPE_MAX)
}

/// Inverse of [`grade_to_slope`]: the ISO-R grade equivalent to an effective slope.
#[must_use]
pub fn slope_to_grade(slope: f64, density_range: Option<f64>, gain: f64) -> f64 {
    let range = density_range.unwrap_or_else(default_grade_range);
    let rng = range.abs().clamp(0.3, 3.5);
    if slope <= 0.0 {
        return c::ISO_R_MAX;
    }
    let er = c::GRADE_CONTRAST_SCALE * rng * gain / slope;
    (er * 100.0).clamp(c::ISO_R_MIN, c::ISO_R_MAX)
}

/// Paper's variable midtone gamma plus the user's additive trim.
#[must_use]
pub fn effective_midtone_gamma(paper: Option<&PaperProfile>, trim: f64) -> f64 {
    effective_constants(paper).paper_midtone_gamma + trim
}

/// Range fed to [`grade_to_slope`]. Auto Grade off: the measured range. On: the paper gamma
/// follows the negative's textural density scale, shrunk toward a normal negative's.
#[must_use]
pub fn effective_grade_range(
    auto_normalize_contrast: bool,
    floor_ceil_range: Option<f64>,
    textural_range: Option<f64>,
) -> Option<f64> {
    if !auto_normalize_contrast {
        return floor_ceil_range;
    }
    let (Some(textural), Some(range)) = (textural_range, floor_ceil_range) else {
        return Some(default_grade_range());
    };
    let measured = textural.abs();
    if measured < 1e-6 {
        return Some(3.5);
    }
    let q = c::AUTO_GRADE_NOMINAL_RANGE / measured;
    let factor = ((1.0 - c::AUTO_GRADE_STRENGTH) + c::AUTO_GRADE_STRENGTH * q)
        .min(q * c::AUTO_GRADE_MAX_OVERFILL);
    Some(c::AUTO_GRADE_TARGET * range.abs() * factor)
}

/// Inverse softplus `log(exp(y) - 1)`, stable above 20.
fn inv_softplus(y: f64) -> f64 {
    if y > 20.0 {
        y
    } else {
        (y.max(1e-12).exp_m1()).ln()
    }
}

/// Straight-line density value that the base toe/shoulder bounds map onto `target`.
#[must_use]
pub fn reference_linear_value(
    d_min: f64,
    paper: Option<&PaperProfile>,
    target: Option<f64>,
) -> f64 {
    let constants = effective_constants(paper);
    let t = target.unwrap_or(c::REFERENCE_PRINT_DENSITY);
    let d_max = constants.d_max;
    let a_hl = constants.shoulder_sharpness_base;
    let a_sh = constants.toe_sharpness_base;
    let v1 = d_max - inv_softplus(a_sh * (d_max - t)) / a_sh;
    d_min + inv_softplus(a_hl * (v1 - d_min)) / a_hl
}

fn midtone_terms(
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> (f64, f64, f64) {
    let constants = effective_constants(paper);
    let mg = midtone_gamma.unwrap_or(constants.paper_midtone_gamma);
    (
        mg,
        constants.paper_gamma_width,
        reference_linear_value(d_min, paper, None),
    )
}

/// Straight-line value after the variable midtone gamma.
#[must_use]
pub fn through_midtone(
    v: f64,
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> f64 {
    let (mg, width, v_star) = midtone_terms(d_min, paper, midtone_gamma);
    v + mg * width * tanh_f64((v - v_star) / width)
}

/// Straight-line value that prints `target` through the midtone gamma and the bounds.
#[must_use]
pub fn straight_line_target(
    d_min: f64,
    paper: Option<&PaperProfile>,
    target: Option<f64>,
    midtone_gamma: Option<f64>,
) -> f64 {
    let u = reference_linear_value(d_min, paper, target);
    let (mg, width, v_star) = midtone_terms(d_min, paper, midtone_gamma);
    let (mut lo, mut hi) = (u - mg.abs() * width, u + mg.abs() * width);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if mid + mg * width * tanh_f64((mid - v_star) / width) < u {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Straight-line value the anchor prints from.
#[must_use]
pub fn anchor_linear_value(
    d_min: f64,
    paper: Option<&PaperProfile>,
    metered: bool,
    midtone_gamma: Option<f64>,
) -> f64 {
    if metered {
        straight_line_target(d_min, paper, Some(c::ANCHOR_TARGET_DENSITY), midtone_gamma)
    } else {
        reference_linear_value(d_min, paper, None)
    }
}

/// `(v_th, gamma)` for [`preflash_value`]. `v_th` prints the paper's threshold density,
/// `gamma` is the paper's density per log10 exposure at `grade`.
#[must_use]
pub fn preflash_params(
    grade: f64,
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> (f64, f64) {
    let v_th = straight_line_target(
        d_min,
        paper,
        Some(d_min + c::PREFLASH_THRESHOLD_DENSITY),
        midtone_gamma,
    );
    let er = grade.clamp(c::ISO_R_MIN, c::ISO_R_MAX) / 100.0;
    (v_th, c::GRADE_CONTRAST_SCALE / er * slope_gain(paper))
}

/// Straight-line value after a uniform flash of `flash` times the threshold exposure.
/// Exposures add, so the flash lifts tones near threshold and barely moves tones far above it.
#[must_use]
pub fn preflash_value(v: f64, v_th: f64, flash: f64, gamma: f64) -> f64 {
    v_th + gamma * log10_f64(exp10_f64((v - v_th) / gamma) + flash)
}

/// Pivot so the reference tone prints at its target, plus the density slider offset.
#[must_use]
pub fn compute_pivot(
    slope: f64,
    density: f64,
    d_min: f64,
    anchor: Option<f64>,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> f64 {
    let reference = anchor.unwrap_or(c::ASSUMED_ANCHOR);
    let v = anchor_linear_value(d_min, paper, anchor.is_some(), midtone_gamma);
    reference - v / slope + (1.0 - density) * c::DENSITY_MULTIPLIER
}

/// Auto Grade floor: raise the slope until the textured dark tail prints at the shadow
/// reach density while the anchor stays at its target. Never lowers it.
#[must_use]
pub fn shadow_reach_slope(
    slope: f64,
    anchor: f64,
    shadow_point: f64,
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
    metered: bool,
) -> f64 {
    let span = shadow_point - anchor;
    if span <= 1e-6 {
        return slope;
    }
    let v_black = straight_line_target(d_min, paper, Some(c::SHADOW_REACH_DENSITY), midtone_gamma);
    let needed = (v_black - anchor_linear_value(d_min, paper, metered, midtone_gamma)) / span;
    slope.max(needed).min(c::SLOPE_MAX)
}

/// Highlight hold burn that lands the bright textured tail on the hold density, capped.
#[must_use]
pub fn highlight_hold_offset(
    slope: f64,
    pivot: f64,
    highlight_point: f64,
    d_min: f64,
    paper: Option<&PaperProfile>,
    preflash: f64,
    grade: f64,
    midtone_gamma: Option<f64>,
) -> f64 {
    let target = c::HIGHLIGHT_HOLD_DENSITY;
    if target <= 0.0 {
        return 0.0;
    }
    let mut v = slope * (highlight_point - pivot);
    if preflash > 0.0 {
        let (v_th, gamma) = preflash_params(grade, d_min, paper, midtone_gamma);
        v = preflash_value(v, v_th, preflash, gamma);
    }
    let v = through_midtone(v, d_min, paper, midtone_gamma);
    let v_hold = reference_linear_value(d_min, paper, Some(target));
    if v >= v_hold {
        return 0.0;
    }
    let z_hi = c::REFERENCE_PRINT_DENSITY + c::ZONE_DENSITY_HIGHLIGHT_OFFSET;
    let weight = 1.0 - fast_sigmoid(c::ZONE_DENSITY_SHARPNESS * (v - z_hi));
    ((v_hold - v) / weight.max(1e-6)).min(c::HIGHLIGHT_HOLD_MAX)
}

/// Shadow hold lift on the textured dark tail past Shadow Reach's line, capped.
#[must_use]
pub fn shadow_hold_offset(
    slope: f64,
    pivot: f64,
    shadow_point: f64,
    d_min: f64,
    paper: Option<&PaperProfile>,
    midtone_gamma: Option<f64>,
) -> f64 {
    let strength = c::SHADOW_HOLD_STRENGTH;
    if strength <= 0.0 {
        return 0.0;
    }
    let v = through_midtone(slope * (shadow_point - pivot), d_min, paper, midtone_gamma);
    let v_start = reference_linear_value(d_min, paper, Some(c::SHADOW_REACH_DENSITY))
        + c::SHADOW_HOLD_OVERSHOOT;
    if v <= v_start {
        return 0.0;
    }
    -(strength * (v - v_start)).min(c::SHADOW_HOLD_MAX)
}

fn fast_sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

/// The log10 helper the preflash math needs, kept here so the module has one import.
#[must_use]
pub fn log10(value: f64) -> f64 {
    log10_f64(value)
}
