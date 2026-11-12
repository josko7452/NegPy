//! Scalar exposure math of `negpy/features/exposure/logic.py`: grade conversion, auto-grade
//! range, pivot placement and the shadow/highlight holds.

use negpy_core::mathf::{log10_f64, tanh_f64};

use crate::features::exposure::constants as c;
use crate::features::exposure::papers::{PaperProfile, effective_constants};

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
    midtone_gamma: Option<f64>,
) -> f64 {
    let target = c::HIGHLIGHT_HOLD_DENSITY;
    if target <= 0.0 {
        return 0.0;
    }
    let v = through_midtone(
        slope * (highlight_point - pivot),
        d_min,
        paper,
        midtone_gamma,
    );
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
