//! Ports the pure cases of tests/test_auto_sliders.py.

use negpy_backend::domain::models::{ExposureConfig, ProcessMode};
use negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics;
use negpy_backend::features::exposure::auto_sliders::{
    grade_at_softest, print_shown_values, print_stored_value,
};

fn metrics() -> AutoSliderMetrics {
    AutoSliderMetrics {
        metered_anchor: Some(0.52),
        textural_range: Some(0.8),
        norm_density_range: Some(1.3),
        shadow_point: Some(0.93),
        highlight_point: Some(0.04),
        ..AutoSliderMetrics::default()
    }
}

fn edited() -> ExposureConfig {
    ExposureConfig {
        density: 1.15,
        grade: 105.0,
        shadow_density: -0.04,
        highlight_density: 0.08,
        ..ExposureConfig::default()
    }
}

fn shown(metrics: &AutoSliderMetrics, field: &str) -> f64 {
    print_shown_values(&edited(), Some(ProcessMode::C41), metrics)
        .into_iter()
        .find_map(|(name, value)| (name == field).then_some(value))
        .unwrap()
}

#[test]
fn an_auto_that_is_off_or_unmetered_shows_nothing() {
    let mut exposure = ExposureConfig::default();
    exposure.auto_exposure = false;
    exposure.auto_normalize_contrast = false;
    assert!(print_shown_values(&exposure, Some(ProcessMode::C41), &metrics()).is_empty());
    assert!(
        print_shown_values(
            &ExposureConfig::default(),
            Some(ProcessMode::C41),
            &AutoSliderMetrics::default()
        )
        .is_empty()
    );
}

#[test]
fn a_partial_meter_shows_only_the_density_tick() {
    let partial = AutoSliderMetrics {
        metered_anchor: Some(0.52),
        ..AutoSliderMetrics::default()
    };
    let fields: Vec<String> =
        print_shown_values(&ExposureConfig::default(), Some(ProcessMode::C41), &partial)
            .into_iter()
            .map(|(field, _)| field)
            .collect();
    assert_eq!(fields, ["density"]);
}

#[test]
fn the_density_tick_follows_the_metered_anchor() {
    let dense = AutoSliderMetrics {
        metered_anchor: Some(0.56),
        ..metrics()
    };
    let thin = AutoSliderMetrics {
        metered_anchor: Some(0.40),
        ..metrics()
    };
    assert_ne!(shown(&dense, "density"), shown(&thin, "density"));
}

#[test]
fn every_shown_value_inverts_to_the_stored_value() {
    let metrics = metrics();
    let exposure = edited();
    for field in ["density", "grade", "shadow_density", "highlight_density"] {
        let shown = shown(&metrics, field);
        let stored = print_stored_value(&exposure, Some(ProcessMode::C41), &metrics, field, shown);
        let original = match field {
            "density" => exposure.density,
            "grade" => exposure.grade,
            "shadow_density" => exposure.shadow_density,
            "highlight_density" => exposure.highlight_density,
            _ => unreachable!(),
        };
        assert!(
            (stored - original).abs() < 1e-6,
            "{field}: stored {stored}, original {original}"
        );
    }
}

#[test]
fn a_flat_negative_reaches_the_softest_grade() {
    let flat = AutoSliderMetrics {
        metered_anchor: Some(0.507),
        textural_range: Some(0.221),
        norm_density_range: Some(0.509),
        shadow_point: Some(1.062),
        highlight_point: Some(0.15),
        ..AutoSliderMetrics::default()
    };
    let exposure = ExposureConfig {
        paper_profile: "pakon".to_string(),
        grade: 170.0,
        ..ExposureConfig::default()
    };
    assert!(grade_at_softest(&exposure, Some(ProcessMode::C41), &flat));
}

#[test]
fn a_slider_at_its_soft_end_is_no_warning() {
    let flat = AutoSliderMetrics {
        metered_anchor: Some(0.507),
        textural_range: Some(0.221),
        norm_density_range: Some(0.509),
        shadow_point: Some(1.062),
        highlight_point: Some(0.15),
        ..AutoSliderMetrics::default()
    };
    let exposure = ExposureConfig {
        grade: 180.0,
        auto_normalize_contrast: false,
        ..ExposureConfig::default()
    };
    assert!(!grade_at_softest(&exposure, Some(ProcessMode::C41), &flat));
}

#[test]
fn an_unmetered_frame_is_no_warning() {
    assert!(!grade_at_softest(
        &ExposureConfig::default(),
        Some(ProcessMode::C41),
        &AutoSliderMetrics::default()
    ));
}
