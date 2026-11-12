//! Ports of the grade, pivot and hold assertions in
//! `tests/test_auto_exposure_contrast.py`, `tests/test_exposure_pivot.py` and
//! `tests/test_paper_profiles.py`.

use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::curve::{
    compute_pivot, default_grade_range, effective_grade_range, effective_midtone_gamma,
    grade_to_slope, highlight_hold_offset, reference_linear_value, shadow_hold_offset,
    shadow_reach_slope, slope_to_grade,
};
use negpy_backend::features::exposure::papers::resolve_paper;

fn density_at(x: f64, slope: f64, pivot: f64, d_min: f64) -> f64 {
    let v = slope * (x - pivot);
    let reference = reference_linear_value(d_min, None, None);
    let mg = effective_midtone_gamma(None, 0.0);
    let width = c::PAPER_GAMMA_WIDTH;
    let v = v + mg * width * ((v - reference) / width).tanh();
    let d_max = c::D_MAX;
    let a_hl = c::SHOULDER_SHARPNESS_BASE;
    let a_sh = c::TOE_SHARPNESS_BASE;
    let v1 = d_min + (a_hl * (v - d_min)).exp().ln_1p() / a_hl;
    d_max - (a_sh * (d_max - v1)).exp().ln_1p() / a_sh
}

#[test]
fn the_fallback_range_scales_a_normal_negative() {
    assert!(
        (default_grade_range()
            - c::AUTO_GRADE_TARGET * c::AUTO_GRADE_NOMINAL_RATIO * c::AUTO_GRADE_NOMINAL_RANGE)
            .abs()
            < 1e-12
    );
}

#[test]
fn grade_to_slope_uses_the_iso_r_ladder() {
    let slope = grade_to_slope(115.0, Some(1.3), 1.0);
    let expected = (c::GRADE_CONTRAST_SCALE * 1.3 / 1.15).clamp(c::SLOPE_MIN, c::SLOPE_MAX);
    assert!((slope - expected).abs() < 1e-12);
    assert!((slope_to_grade(slope, Some(1.3), 1.0) - 115.0).abs() < 1e-9);
    assert!((slope_to_grade(slope, Some(1.3), 1.2) - 115.0).abs() > 1e-3);
    assert_eq!(slope_to_grade(0.0, Some(1.3), 1.0), c::ISO_R_MAX);
}

#[test]
fn auto_grade_off_tracks_the_measured_range() {
    assert_eq!(
        effective_grade_range(false, Some(1.7), Some(0.9)),
        Some(1.7)
    );
    assert_eq!(effective_grade_range(false, None, Some(0.9)), None);
}

#[test]
fn auto_grade_blends_toward_the_population_norm() {
    let expected = c::AUTO_GRADE_TARGET
        * 1.6
        * ((1.0 - c::AUTO_GRADE_STRENGTH)
            + c::AUTO_GRADE_STRENGTH * c::AUTO_GRADE_NOMINAL_RANGE / 1.2);
    let got = effective_grade_range(true, Some(1.6), Some(1.2)).unwrap();
    assert!((got - expected).abs() < 1e-9);
    let nominal =
        effective_grade_range(true, Some(1.8), Some(c::AUTO_GRADE_NOMINAL_RANGE)).unwrap();
    assert!((nominal - c::AUTO_GRADE_TARGET * 1.8).abs() < 1e-9);
}

#[test]
fn a_wider_textural_range_prints_softer() {
    let normal = effective_grade_range(true, Some(1.6), Some(0.9)).unwrap();
    assert!(effective_grade_range(true, Some(1.6), Some(1.3)).unwrap() < normal);
    assert!(effective_grade_range(true, Some(1.6), Some(0.5)).unwrap() > normal);
}

#[test]
fn the_overfill_cap_limits_a_narrow_range() {
    let range = effective_grade_range(true, Some(2.4), Some(2.0)).unwrap();
    let q = c::AUTO_GRADE_NOMINAL_RANGE / 2.0;
    let expected = c::AUTO_GRADE_TARGET * 2.4 * (q * c::AUTO_GRADE_MAX_OVERFILL);
    assert!((range - expected).abs() < 1e-9);
    assert!(effective_grade_range(true, Some(1.6), Some(0.0)).unwrap() <= 3.5 + 1e-6);
    assert!(
        (effective_grade_range(true, Some(2.4), None).unwrap() - default_grade_range()).abs()
            < 1e-12
    );
}

#[test]
fn the_reference_tone_prints_at_the_target() {
    let slope = grade_to_slope(110.0, Some(1.3), 1.0);
    let pivot = compute_pivot(slope, 1.0, 0.0, None, None, None);
    let density = density_at(c::ASSUMED_ANCHOR, slope, pivot, 0.0);
    assert!((density - c::REFERENCE_PRINT_DENSITY).abs() < 2e-3);
}

#[test]
fn grade_changes_rotate_around_the_reference_tone() {
    for grade in [160.0, 110.0, 60.0] {
        let slope = grade_to_slope(grade, Some(1.3), 1.0);
        let pivot = compute_pivot(slope, 1.0, 0.0, None, None, None);
        let density = density_at(c::ASSUMED_ANCHOR, slope, pivot, 0.0);
        assert!(
            (density - c::REFERENCE_PRINT_DENSITY).abs() < 2e-3,
            "grade {grade} printed {density}"
        );
    }
}

#[test]
fn the_density_slider_shifts_exposure() {
    let slope = grade_to_slope(110.0, Some(1.3), 1.0);
    let light = compute_pivot(slope, 0.5, 0.0, None, None, None);
    let dark = compute_pivot(slope, 1.5, 0.0, None, None, None);
    assert!(light > dark);
    assert_eq!(
        compute_pivot(slope, 1.0, 0.0, None, None, None),
        compute_pivot(slope, 1.0, 0.0, None, None, None)
    );
}

#[test]
fn shadow_reach_never_lowers_the_slope() {
    let anchor = 0.46;
    assert_eq!(
        shadow_reach_slope(6.0, anchor, 0.9, 0.0, None, None, false),
        6.0
    );
    assert_eq!(
        shadow_reach_slope(2.5, anchor, anchor + 0.01, 0.0, None, None, false),
        c::SLOPE_MAX
    );
    assert_eq!(
        shadow_reach_slope(2.5, anchor, anchor, 0.0, None, None, false),
        2.5
    );
    assert_eq!(
        shadow_reach_slope(2.5, anchor, 0.3, 0.0, None, None, false),
        2.5
    );
    assert!(shadow_reach_slope(2.5, anchor, 0.8, 0.0, None, None, false) > 2.5);
}

#[test]
fn highlight_hold_burns_only_a_bright_tail() {
    let slope = 3.0;
    let pivot = compute_pivot(slope, 1.0, 0.0, Some(0.46), None, None);
    assert!(highlight_hold_offset(slope, pivot, 0.05, 0.0, None, None) > 0.0);
    assert_eq!(
        highlight_hold_offset(slope, pivot, 0.40, 0.0, None, None),
        0.0
    );
    assert_eq!(
        highlight_hold_offset(slope, pivot, -1.0, 0.0, None, None),
        c::HIGHLIGHT_HOLD_MAX
    );
}

#[test]
fn shadow_hold_is_strength_times_the_overshoot() {
    let slope = 3.0;
    let pivot = compute_pivot(slope, 1.0, 0.0, Some(0.46), None, None);
    let start =
        reference_linear_value(0.0, None, Some(c::SHADOW_REACH_DENSITY)) + c::SHADOW_HOLD_OVERSHOOT;
    let point = pivot + (start + 0.4) / slope;
    let expected = -c::SHADOW_HOLD_STRENGTH * 0.4;
    assert!(
        (shadow_hold_offset(slope, pivot, point, 0.0, None, Some(0.0)) - expected).abs() < 1e-6
    );
    let short = pivot + (start - 0.1) / slope;
    assert_eq!(
        shadow_hold_offset(slope, pivot, short, 0.0, None, None),
        0.0
    );
}

#[test]
fn the_frontier_profile_scales_the_slope() {
    let paper = resolve_paper("frontier");
    let base = grade_to_slope(115.0, Some(1.3), 1.0);
    let gained = grade_to_slope(115.0, Some(1.3), paper.slope_gain);
    assert!((gained - base * paper.slope_gain).abs() < 1e-12);
    assert!((slope_to_grade(gained, Some(1.2), 2.4) - 130.0).abs() > 1e-3);
    assert!(
        (slope_to_grade(grade_to_slope(130.0, Some(1.2), 2.4), Some(1.2), 2.4) - 130.0).abs()
            < 1e-6
    );
}

#[test]
fn a_full_flash_is_positive_and_monotone() {
    use negpy_backend::features::exposure::curve::{preflash_params, preflash_value};
    for d_min in [0.0, 0.06] {
        let (v_th, gamma) = preflash_params(115.0, d_min, None, None);
        let bare = -5.0;
        let off = preflash_value(bare, v_th, 0.0, gamma);
        let full = preflash_value(bare, v_th, 1.0, gamma);
        assert!(full > off);
        let near = preflash_value(v_th, v_th, 0.0, gamma);
        let flashed = preflash_value(v_th, v_th, 1.0, gamma);
        assert!((flashed - near) > 0.0);
    }
}

#[test]
fn the_flash_is_a_paper_exposure_not_a_scan_range() {
    use negpy_backend::features::exposure::curve::{preflash_params, preflash_value};
    let (v_th, gamma) = preflash_params(115.0, 0.0, None, None);
    for v in [-1.0, 0.0, 0.5, 1.0] {
        let a = preflash_value(v, v_th, 0.5, gamma);
        let b = preflash_value(v, v_th, 0.5, gamma);
        assert_eq!(a, b);
    }
}
