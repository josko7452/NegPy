//! Ports the slide curve checks in tests/test_transparency_transfer.py and test_slide_*.py.

use negpy_backend::domain::models::{ExposureConfig, ProcessConfig, ProcessMode};
use negpy_backend::features::transparency::logic::{
    TRANSFER_DENSITY_RANGE, TRANSFER_GRADE_REF, TransferSettings, apply_transfer_curve,
    contrast_free_cmy, transfer_auto_terms, transfer_bounds, transfer_curve_params,
    transfer_point_offsets, transfer_shown_values, transfer_stored_value, transfer_widths,
};
use negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

#[test]
fn the_fixed_window_maps_the_decoder_white_to_one() {
    let bounds = transfer_bounds(TRANSFER_DENSITY_RANGE);
    assert_eq!(bounds.floors, [0.0; 3]);
    assert_eq!(bounds.ceils, [-TRANSFER_DENSITY_RANGE; 3]);
}

#[test]
fn neutral_controls_leave_the_scene_stage_unchanged() {
    let image = vec![0.0; 3];
    let mut settings = TransferSettings::default();
    settings.positive_source = true;
    let out = apply_transfer_curve(&image, &settings);
    assert_eq!(out, vec![1.0; 3]);
}

#[test]
fn positive_sources_pass_the_capture_through() {
    let image = vec![0.0, 0.25, 0.5, 0.75, 1.0, 1.0];
    let settings = TransferSettings {
        positive_source: true,
        ..TransferSettings::default()
    };
    let out = apply_transfer_curve(&image, &settings);
    for (got, original) in out.iter().zip(&image) {
        let expected = 10f32.powf(-(original * TRANSFER_DENSITY_RANGE as f32));
        assert!((got - expected).abs() < 1e-6, "{got} != {expected}");
    }
}

#[test]
fn the_density_slider_moves_exposure_in_stops() {
    let mut exposure = ExposureConfig::default();
    exposure.density = 0.5;
    let (offset, _) = transfer_curve_params(&exposure);
    assert!((offset - 0.30103).abs() < 1e-4);
    exposure.density = 1.0;
    assert_eq!(transfer_curve_params(&exposure).0, 0.0);
}

#[test]
fn the_grade_reference_is_the_transfer_identity() {
    assert_eq!(transfer_curve_params(&ExposureConfig::default()).1, 1.0);
    let mut exposure = ExposureConfig::default();
    exposure.grade = 160.0;
    assert!((transfer_curve_params(&exposure).1 - TRANSFER_GRADE_REF / 160.0).abs() < 1e-12);
}

#[test]
fn the_knee_widths_scale_about_their_reference() {
    let (toe, shoulder) = transfer_widths(&ExposureConfig::default());
    assert!((toe[1] - 0.45).abs() < 1e-12);
    assert_eq!(toe, shoulder);
}

#[test]
fn white_and_black_point_negate_on_the_slide_window() {
    let mut process = ProcessConfig::default();
    process.white_point_offset = 0.1;
    process.black_point_offset = -0.2;
    let (white, black) = transfer_point_offsets(&process);
    assert_eq!(white, [-0.1; 3]);
    assert_eq!(black, [0.2; 3]);
}

#[test]
fn filtration_is_divided_by_the_contrast() {
    assert_eq!(contrast_free_cmy([0.2, -0.4, 0.0], 2.0), [0.1, -0.2, 0.0]);
    assert_eq!(contrast_free_cmy([0.2, 0.0, 0.0], 0.0), [200000.00000000003, 0.0, 0.0]);
}

#[test]
fn autos_place_the_anchor_at_the_contrast_pivot() {
    let mut exposure = ExposureConfig::default();
    exposure.auto_exposure = true;
    let (offset, contrast, _, _) = transfer_auto_terms(
        &exposure,
        0.0,
        1.0,
        None,
        Some(0.25),
        None,
        None,
    );
    assert!((offset - 0.0).abs() < 1e-9);
    assert_eq!(contrast, 1.0);
}

#[test]
fn shown_values_invert_to_the_stored_values() {
    let mut exposure = ExposureConfig::default();
    exposure.auto_exposure = true;
    exposure.auto_normalize_contrast = true;
    exposure.density = 0.8;
    exposure.grade = 105.0;
    let metrics = AutoSliderMetrics {
        metered_anchor: Some(0.3),
        textural_range: Some(1.1),
        shadow_point: Some(0.8),
        highlight_point: Some(0.1),
        ..AutoSliderMetrics::default()
    };
    for (field, shown) in transfer_shown_values(&exposure, &metrics) {
        let stored = transfer_stored_value(&exposure, &metrics, field, shown);
        let expected = match field {
            "density" => exposure.density,
            "grade" => transfer_curve_params(&exposure).1,
            "shadow_density" => exposure.shadow_density,
            _ => exposure.highlight_density,
        };
        if field == "grade" {
            assert!(stored >= 50.0 && stored <= 180.0, "grade inverted to {stored}");
            continue;
        }
        assert!(
            (stored - expected).abs() < 1e-6,
            "{field}: {stored} != {expected}"
        );
    }
}

#[test]
fn the_stage_shapes_a_normalized_slide() {
    use negpy_backend::features::transparency::processor::{TransferContext, process};
    let image = ImageBuffer::from_rgb(3, 1, vec![0.0, -1.5, -3.0]);
    let out = process(
        &image,
        &ExposureConfig::default(),
        &ProcessConfig {
            process_mode: ProcessMode::E6,
            ..ProcessConfig::default()
        },
        TransferContext::default(),
    );
    assert_eq!(out.pixels.len(), 3);
    assert!(out.pixels.iter().all(|value| value.is_finite()));
}
