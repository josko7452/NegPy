//! Zone placement: solve the print controls that put pinned tones on target zones.
//!
//! Mirrors `negpy/features/exposure/placement.py`. The achromatic forward curve is
//! re-solved from published metrics, then bisected: density for one pin, density and grade
//! for two, plus one knee control for the middle tone of three.

use crate::domain::color::WORKING_GAMMA;
use crate::domain::models::ExposureConfig;
use crate::features::exposure::analysis::zone_of_encoded;
use crate::features::exposure::auto_metrics::AutoSliderMetrics;
use crate::features::exposure::constants as c;
use crate::features::exposure::curve::{
    self, curve_params_from_metrics, effective_midtone_gamma, grade_coupled_shape,
    shadow_hold_offset, split_grade_deltas,
};
use crate::features::exposure::papers::effective_constants;
use crate::features::exposure::papers::effective_paper_profile;

/// The Print Density slider's range.
pub const DENSITY_RANGE: (f64, f64) = (0.0, 2.0);
const DENSITY_RESOLUTION: f64 = 0.005;
const GRADE_RESOLUTION: f64 = 0.5;
const CLAMP_TOL: f64 = 1e-6;
const DEGENERATE_VAL_GAP: f64 = 1e-3;
const KNEE_PROBE: f64 = 0.2;
const KNEE_SENSITIVITY_FLOOR: f64 = 0.05;
const KNEE_PASSES: usize = 3;
const KNEE_SETTLED: f64 = 0.02;
const ON_TARGET_TOL: f64 = 1.0 / 6.0;

/// One control a third pin can be solved on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KneeCandidate {
    pub field: &'static str,
    pub lo: f64,
    pub hi: f64,
    pub resolution: f64,
    pub digits: u32,
}

/// The knee controls in the order the solver tries them.
pub const KNEE_CANDIDATES: [KneeCandidate; 3] = [
    KneeCandidate {
        field: "shadow_grade",
        lo: -50.0,
        hi: 50.0,
        resolution: GRADE_RESOLUTION,
        digits: 1,
    },
    KneeCandidate {
        field: "highlight_grade",
        lo: -50.0,
        hi: 50.0,
        resolution: GRADE_RESOLUTION,
        digits: 1,
    },
    KneeCandidate {
        field: "midtone_gamma",
        lo: -0.5,
        hi: 0.5,
        resolution: DENSITY_RESOLUTION,
        digits: 2,
    },
];

/// One probed spot and the zone it should print on.
#[derive(Clone, Debug, PartialEq)]
pub struct ZonePin {
    pub nx: f64,
    pub ny: f64,
    pub val_rgb: [f64; 3],
    pub val_luma: f64,
    pub target_zone: f64,
    pub label: String,
    pub retargeted: bool,
}

/// The controls a placement sets and the zones the pins land on.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementSolution {
    pub density: f64,
    pub grade: f64,
    pub auto_exposure: bool,
    pub auto_normalize_contrast: bool,
    pub knee: Option<(&'static str, f64)>,
    pub achieved: Vec<f64>,
    pub clamped: bool,
}

impl PlacementSolution {
    /// Apply the solved fields over a base config.
    #[must_use]
    pub fn apply(&self, exposure: &ExposureConfig) -> ExposureConfig {
        let mut placed = ExposureConfig {
            density: self.density,
            grade: self.grade,
            auto_exposure: self.auto_exposure,
            auto_normalize_contrast: self.auto_normalize_contrast,
            ..exposure.clone()
        };
        if let Some((field, value)) = self.knee {
            match field {
                "shadow_grade" => placed.shadow_grade = value,
                "highlight_grade" => placed.highlight_grade = value,
                "midtone_gamma" => placed.midtone_gamma = value,
                _ => {}
            }
        }
        placed
    }
}

fn fast_sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let z = x.exp();
        z / (1.0 + z)
    }
}

fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

/// The achromatic print curve's density, as `CharacteristicCurve`.
fn curve_density(exposure: &ExposureConfig, metrics: &AutoSliderMetrics, value: f64) -> f64 {
    let (slopes, pivots, curvatures) = curve_params_from_metrics(exposure, None, metrics);
    let profile = effective_paper_profile(&exposure.paper_profile, None);
    let constants = effective_constants(Some(profile));
    let d_min = if exposure.paper_dmin {
        profile.tonal.d_min
    } else {
        0.0
    };
    let midtone_gamma = effective_midtone_gamma(Some(profile), exposure.midtone_gamma);
    let (slope, pivot, curvature) = (slopes[1], pivots[1], curvatures[1]);
    let mut v = if curvature == 0.0 {
        slope * (value - pivot)
    } else {
        let vertex = -slope / (2.0 * curvature);
        let clipped = if curvature > 0.0 {
            value.max(vertex)
        } else {
            value.min(vertex)
        };
        slope * (clipped - pivot) + curvature * clipped * clipped
    };
    if exposure.preflash > 0.0 {
        let (v_th, gamma) =
            curve::preflash_params(exposure.grade, d_min, Some(profile), Some(midtone_gamma));
        v = curve::preflash_value(v, v_th, exposure.preflash, gamma);
    }
    if midtone_gamma != 0.0 {
        let v_star = curve::reference_linear_value(d_min, Some(profile), None);
        v += midtone_gamma
            * constants.paper_gamma_width
            * ((v - v_star) / constants.paper_gamma_width).tanh();
    }
    let (shadow, highlight) = split_grade_deltas(
        exposure.grade,
        exposure.shadow_grade,
        exposure.highlight_grade,
        [
            exposure.shadow_grade_trim_red,
            exposure.shadow_grade_trim_green,
            exposure.shadow_grade_trim_blue,
        ],
        [
            exposure.highlight_grade_trim_red,
            exposure.highlight_grade_trim_green,
            exposure.highlight_grade_trim_blue,
        ],
    );
    let zone_sh = c::REFERENCE_PRINT_DENSITY + c::ZONE_DENSITY_SHADOW_OFFSET;
    let zone_hi = c::REFERENCE_PRINT_DENSITY + c::ZONE_DENSITY_HIGHLIGHT_OFFSET;
    if shadow[1] != 0.0 || highlight[1] != 0.0 {
        let w_sh = fast_sigmoid(c::ZONE_DENSITY_SHARPNESS * (v - zone_sh));
        let w_hi = 1.0 - fast_sigmoid(c::ZONE_DENSITY_SHARPNESS * (v - zone_hi));
        v += shadow[1] * w_sh * (v - zone_sh) + highlight[1] * w_hi * (v - zone_hi);
    }
    let shadow_density = exposure.shadow_density
        + if exposure.auto_normalize_contrast {
            metrics.shadow_point.map_or(0.0, |point| {
                shadow_hold_offset(
                    slopes[1],
                    pivots[1],
                    point,
                    d_min,
                    Some(profile),
                    Some(midtone_gamma),
                )
            })
        } else {
            0.0
        };
    let highlight_density = exposure.highlight_density
        + if exposure.auto_normalize_contrast {
            metrics.highlight_point.map_or(0.0, |point| {
                curve::highlight_hold_offset(
                    slopes[1],
                    pivots[1],
                    point,
                    d_min,
                    Some(profile),
                    exposure.preflash,
                    exposure.grade,
                    Some(midtone_gamma),
                )
            })
        } else {
            0.0
        };
    if shadow_density != 0.0 || highlight_density != 0.0 {
        let mut w_sh = fast_sigmoid(c::ZONE_DENSITY_SHARPNESS * (v - zone_sh));
        if exposure.hold_black && shadow_density < 0.0 {
            let toe_eff = grade_coupled_shape(slope, exposure.toe, exposure.shoulder).0
                * c::TOE_SHOULDER_STRENGTH;
            let mut d_max_eff = constants.d_max;
            if toe_eff >= 0.0 {
                d_max_eff -= toe_eff * constants.toe_height;
            }
            let d_min_eff = (d_min
                + grade_coupled_shape(slope, exposure.toe, exposure.shoulder).1
                    * c::TOE_SHOULDER_STRENGTH
                    * constants.shoulder_height)
                .max(0.0);
            d_max_eff = d_max_eff.max(d_min_eff + 0.1);
            let t = ((d_max_eff - v) / c::ZONE_BLACK_HOLD_TAPER).clamp(0.0, 1.0);
            w_sh *= t * t * (3.0 - 2.0 * t);
        }
        let w_hi = 1.0 - fast_sigmoid(c::ZONE_DENSITY_SHARPNESS * (v - zone_hi));
        v += shadow_density * w_sh + highlight_density * w_hi;
    }
    let (toe, shoulder) = grade_coupled_shape(slope, exposure.toe, exposure.shoulder);
    let strength = c::TOE_SHOULDER_STRENGTH;
    let toe_eff = toe * strength;
    let shoulder_eff = shoulder * strength;
    let d_min_eff = (d_min + shoulder_eff * constants.shoulder_height).max(0.0);
    let a_hl = constants.shoulder_sharpness_base * c::TOESHOULDER_WIDTH_REF
        / exposure.shoulder_width.max(1e-6);
    let a_sh_base =
        constants.toe_sharpness_base * c::TOESHOULDER_WIDTH_REF / exposure.toe_width.max(1e-6);
    let (d_max_eff, a_sh) = if toe_eff >= 0.0 {
        (constants.d_max - toe_eff * constants.toe_height, a_sh_base)
    } else {
        (constants.d_max, a_sh_base * (1.0 - toe_eff * 4.0))
    };
    let d_max_eff = d_max_eff.max(d_min_eff + 0.1);
    let v1 = d_min_eff + softplus(a_hl * (v - d_min_eff)) / a_hl;
    let density = d_max_eff - softplus(a_sh * (d_max_eff - v1)) / a_sh;
    if exposure.paper_black {
        return density;
    }
    let bpc_black = 10f64.powf(-constants.d_max);
    let transmittance = 10f64.powf(-density);
    let mapped = (transmittance - bpc_black) / (1.0 - bpc_black);
    -mapped.max(1e-12).log10()
}

fn encoded_output(exposure: &ExposureConfig, metrics: &AutoSliderMetrics, value: f64) -> f64 {
    let linear = 10f64.powf(-curve_density(exposure, metrics, value));
    (linear as f32).powf(1.0 / WORKING_GAMMA as f32) as f64
}

/// The zone the achromatic print curve puts `value` on under `exposure`.
#[must_use]
pub fn predicted_zone(exposure: &ExposureConfig, metrics: &AutoSliderMetrics, value: f64) -> f64 {
    f64::from(zone_of_encoded(
        encoded_output(exposure, metrics, value) as f32
    ))
}

fn solve_density(
    candidate: &ExposureConfig,
    metrics: &AutoSliderMetrics,
    pin: &ZonePin,
) -> (f64, bool) {
    bisect_decreasing(
        |density| {
            let mut config = candidate.clone();
            config.density = density;
            predicted_zone(&config, metrics, pin.val_luma) - pin.target_zone
        },
        DENSITY_RANGE.0,
        DENSITY_RANGE.1,
        DENSITY_RESOLUTION,
    )
}

fn solve_grade_and_density(
    candidate: &ExposureConfig,
    metrics: &AutoSliderMetrics,
    dark: &ZonePin,
    light: &ZonePin,
) -> (f64, f64, bool) {
    let light_residual = |grade: f64| {
        let mut graded = candidate.clone();
        graded.grade = grade;
        let (density, _) = solve_density(&graded, metrics, dark);
        graded.density = density;
        predicted_zone(&graded, metrics, light.val_luma) - light.target_zone
    };
    let (grade, grade_clamped) =
        bisect_decreasing(light_residual, c::ISO_R_MIN, c::ISO_R_MAX, GRADE_RESOLUTION);
    let mut graded = candidate.clone();
    graded.grade = grade;
    let (density, density_clamped) = solve_density(&graded, metrics, dark);
    (grade, density, grade_clamped || density_clamped)
}

fn set_knee(mut config: ExposureConfig, field: &str, value: f64) -> ExposureConfig {
    match field {
        "shadow_grade" => config.shadow_grade = value,
        "highlight_grade" => config.highlight_grade = value,
        "midtone_gamma" => config.midtone_gamma = value,
        _ => {}
    }
    config
}

fn knee_value(config: &ExposureConfig, field: &str) -> f64 {
    match field {
        "shadow_grade" => config.shadow_grade,
        "highlight_grade" => config.highlight_grade,
        _ => config.midtone_gamma,
    }
}

fn solve_knee_field(
    base: &ExposureConfig,
    metrics: &AutoSliderMetrics,
    pin: &ZonePin,
    knee: KneeCandidate,
) -> (f64, bool) {
    bisect_monotone(
        |value| {
            predicted_zone(
                &set_knee(base.clone(), knee.field, value),
                metrics,
                pin.val_luma,
            ) - pin.target_zone
        },
        knee.lo,
        knee.hi,
        knee.resolution,
    )
}

fn pick_knee(
    placed: &ExposureConfig,
    metrics: &AutoSliderMetrics,
    pin: &ZonePin,
) -> Option<KneeCandidate> {
    let here = predicted_zone(placed, metrics, pin.val_luma);
    let mut best = None;
    let mut best_delta = KNEE_SENSITIVITY_FLOOR;
    for candidate in KNEE_CANDIDATES {
        let current = knee_value(placed, candidate.field);
        let step = KNEE_PROBE * (candidate.hi - candidate.lo);
        let probe = if current + step <= candidate.hi {
            current + step
        } else {
            current - step
        };
        let delta = (predicted_zone(
            &set_knee(placed.clone(), candidate.field, probe),
            metrics,
            pin.val_luma,
        ) - here)
            .abs();
        if delta > best_delta {
            best = Some(candidate);
            best_delta = delta;
        }
    }
    best
}

struct KneeSolved {
    candidate: ExposureConfig,
    grade: f64,
    density: f64,
    knee: Option<KneeCandidate>,
    clamped: bool,
}

fn solve_with_knee(
    candidate: ExposureConfig,
    metrics: &AutoSliderMetrics,
    dark: &ZonePin,
    light: &ZonePin,
    mid: &ZonePin,
) -> KneeSolved {
    let (grade, density, mut clamped) = solve_grade_and_density(&candidate, metrics, dark, light);
    let mut placed = candidate.clone();
    placed.grade = grade;
    placed.density = density;
    let landed = predicted_zone(&placed, metrics, mid.val_luma);
    if (landed - mid.target_zone).abs() <= ON_TARGET_TOL {
        return KneeSolved {
            candidate,
            grade,
            density,
            knee: None,
            clamped,
        };
    }
    let Some(knee) = pick_knee(&placed, metrics, mid) else {
        return KneeSolved {
            candidate,
            grade,
            density,
            knee: None,
            clamped,
        };
    };
    let mut previous: Option<f64> = None;
    let mut candidate = candidate;
    let mut grade = grade;
    let mut density = density;
    for _ in 0..KNEE_PASSES {
        let mut base = candidate.clone();
        base.grade = grade;
        base.density = density;
        let (value, knee_clamped) = solve_knee_field(&base, metrics, mid, knee);
        candidate = set_knee(candidate, knee.field, value);
        let solved = solve_grade_and_density(&candidate, metrics, dark, light);
        grade = solved.0;
        density = solved.1;
        clamped = solved.2;
        let mut placed = candidate.clone();
        placed.grade = grade;
        placed.density = density;
        let landed = predicted_zone(&placed, metrics, mid.val_luma);
        if previous.is_some_and(|last| (landed - last).abs() < KNEE_SETTLED) {
            break;
        }
        previous = Some(landed);
        clamped |= knee_clamped;
    }
    KneeSolved {
        candidate,
        grade,
        density,
        knee: Some(knee),
        clamped,
    }
}

/// Solve one, two or three pins against the print curve.
#[must_use]
pub fn solve_placement(
    exposure: &ExposureConfig,
    metrics: &AutoSliderMetrics,
    pins: &[ZonePin],
) -> Option<PlacementSolution> {
    let solved = match pins.len() {
        1 => {
            let candidate = ExposureConfig {
                auto_exposure: false,
                ..exposure.clone()
            };
            let (density, clamped) = solve_density(&candidate, metrics, &pins[0]);
            KneeSolved {
                candidate,
                grade: exposure.grade,
                density,
                knee: None,
                clamped,
            }
        }
        2 | 3 => {
            let mut ordered = pins.to_vec();
            ordered.sort_by(|a, b| b.val_luma.total_cmp(&a.val_luma));
            let dark = &ordered[0];
            let light = &ordered[ordered.len() - 1];
            if (dark.val_luma - light.val_luma).abs() < DEGENERATE_VAL_GAP {
                return None;
            }
            let candidate = ExposureConfig {
                auto_exposure: false,
                auto_normalize_contrast: false,
                ..exposure.clone()
            };
            if pins.len() == 3 {
                solve_with_knee(candidate, metrics, dark, light, &ordered[1])
            } else {
                let (grade, density, clamped) =
                    solve_grade_and_density(&candidate, metrics, dark, light);
                KneeSolved {
                    candidate,
                    grade,
                    density,
                    knee: None,
                    clamped,
                }
            }
        }
        _ => return None,
    };
    let knee = solved.knee.map(|candidate| {
        (
            candidate.field,
            round_digits(
                knee_value(&solved.candidate, candidate.field),
                candidate.digits,
            ),
        )
    });
    let mut solution = PlacementSolution {
        density: round_digits(solved.density, 2),
        grade: solved.grade.round_ties_even(),
        auto_exposure: false,
        auto_normalize_contrast: false,
        knee,
        achieved: Vec::new(),
        clamped: solved.clamped,
    };
    let applied = solution.apply(exposure);
    solution.achieved = pins
        .iter()
        .map(|pin| predicted_zone(&applied, metrics, pin.val_luma))
        .collect();
    if solution.knee.is_some()
        && solution
            .achieved
            .iter()
            .zip(pins)
            .any(|(achieved, pin)| (achieved - pin.target_zone).abs() > ON_TARGET_TOL)
    {
        solution.clamped = true;
    }
    Some(solution)
}

fn round_digits(value: f64, digits: u32) -> f64 {
    let scale = 10f64.powi(digits as i32);
    (value * scale).round_ties_even() / scale
}

fn bisect_monotone(f: impl Fn(f64) -> f64, lo: f64, hi: f64, resolution: f64) -> (f64, bool) {
    if f(lo) >= f(hi) {
        bisect_decreasing(f, lo, hi, resolution)
    } else {
        bisect_decreasing(|value| -f(value), lo, hi, resolution)
    }
}

fn bisect_decreasing(
    f: impl Fn(f64) -> f64,
    mut lo: f64,
    mut hi: f64,
    resolution: f64,
) -> (f64, bool) {
    let f_lo = f(lo);
    if f_lo <= 0.0 {
        return (lo, f_lo < -CLAMP_TOL);
    }
    let f_hi = f(hi);
    if f_hi >= 0.0 {
        return (hi, f_hi > CLAMP_TOL);
    }
    while hi - lo > resolution {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0.5 * (lo + hi), false)
}
