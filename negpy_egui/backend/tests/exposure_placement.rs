//! Ports the solver cases in tests/test_zone_placement.py.

use negpy_backend::domain::models::ExposureConfig;
use negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics;
use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::placement::{
    DENSITY_RANGE, KNEE_CANDIDATES, ZonePin, predicted_zone, solve_placement,
};

fn metrics() -> AutoSliderMetrics {
    AutoSliderMetrics {
        norm_density_range: Some(1.4),
        ..AutoSliderMetrics::default()
    }
}

fn pin(value: f64, target: f64) -> ZonePin {
    ZonePin {
        nx: 0.5,
        ny: 0.5,
        val_rgb: [value; 3],
        val_luma: value,
        target_zone: target,
        label: String::new(),
        retargeted: false,
    }
}

#[test]
fn one_pin_lands_on_target() {
    let exposure = ExposureConfig::default();
    let solution = solve_placement(&exposure, &metrics(), &[pin(0.46, 5.0)]).unwrap();
    assert!(!solution.clamped);
    assert!((DENSITY_RANGE.0..=DENSITY_RANGE.1).contains(&solution.density));
    assert!((solution.achieved[0] - 5.0).abs() < 0.05);
    assert!(!solution.auto_exposure);
    assert_eq!(solution.grade, exposure.grade);
    assert!(solution.knee.is_none());
}

#[test]
fn two_pins_land_on_both_targets() {
    let exposure = ExposureConfig::default();
    let pins = [pin(0.65, 3.0), pin(0.30, 8.0)];
    let solution = solve_placement(&exposure, &metrics(), &pins).unwrap();
    assert!(!solution.auto_exposure && !solution.auto_normalize_contrast);
    assert_eq!(solution.grade, solution.grade.round());
    assert!((c::ISO_R_MIN..=c::ISO_R_MAX).contains(&solution.grade));
    assert!((solution.achieved[0] - 3.0).abs() < 0.15);
    assert!((solution.achieved[1] - 8.0).abs() < 0.15);
}

#[test]
fn an_unreachable_target_clamps() {
    let solution =
        solve_placement(&ExposureConfig::default(), &metrics(), &[pin(0.2, 0.0)]).unwrap();
    assert!(solution.clamped);
    assert_eq!(solution.density, DENSITY_RANGE.1);
    assert!(solution.achieved[0] > 0.3);
}

#[test]
fn a_split_grade_still_converges() {
    let exposure = ExposureConfig {
        shadow_grade: 20.0,
        highlight_grade: -15.0,
        ..ExposureConfig::default()
    };
    let solution =
        solve_placement(&exposure, &metrics(), &[pin(0.65, 3.0), pin(0.30, 8.0)]).unwrap();
    assert!((solution.achieved[0] - 3.0).abs() < 0.15);
    assert!((solution.achieved[1] - 8.0).abs() < 0.15);
}

#[test]
fn degenerate_two_pins_return_none() {
    assert!(
        solve_placement(
            &ExposureConfig::default(),
            &metrics(),
            &[pin(0.5, 3.0), pin(0.5, 8.0)],
        )
        .is_none()
    );
}

#[test]
fn achieved_zones_are_recomputed_at_the_rounded_values() {
    let exposure = ExposureConfig::default();
    let pins = [pin(0.65, 3.0), pin(0.30, 8.0)];
    let solution = solve_placement(&exposure, &metrics(), &pins).unwrap();
    assert_eq!(solution.density, (solution.density * 100.0).round() / 100.0);
    let applied = solution.apply(&exposure);
    for (achieved, pin) in solution.achieved.iter().zip(&pins) {
        assert!((achieved - predicted_zone(&applied, &metrics(), pin.val_luma)).abs() < 1e-6);
    }
}

#[test]
fn three_pins_solve_one_knee_control() {
    let exposure = ExposureConfig::default();
    let extremes = [pin(0.65, 3.0), pin(0.30, 8.0)];
    let two = solve_placement(&exposure, &metrics(), &extremes).unwrap();
    let applied = two.apply(&exposure);
    let start = predicted_zone(&applied, &metrics(), 0.48);
    let middle = pin(0.48, start + 0.4);
    let solution = solve_placement(
        &exposure,
        &metrics(),
        &[pin(0.65, 3.0), pin(0.30, 8.0), middle],
    )
    .unwrap();
    let knee = solution.knee.expect("a knee control is chosen");
    assert!(
        KNEE_CANDIDATES
            .iter()
            .any(|candidate| candidate.field == knee.0)
    );
    assert!((solution.achieved[0] - 3.0).abs() < 0.2);
    assert!((solution.achieved[1] - 8.0).abs() < 0.2);
}
