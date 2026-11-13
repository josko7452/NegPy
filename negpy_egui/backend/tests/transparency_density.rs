//! Ports tests/test_slide_temperature.py and the density-stage cases of test_raw_density_order.py.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::process::capture_color::{
    camera_matrix_parts, camera_to_working_matrix,
};
use negpy_backend::features::transparency::logic::{
    density_channels, density_stage, slide_kelvin_to_wb, slide_pick_wb, slide_wb_to_kelvin,
    transfer_total_contrast,
};

const CAMERA: [[f64; 3]; 3] = [
    [0.6722, -0.0635, -0.0963],
    [-0.4287, 1.246, 0.2046],
    [-0.0908, 0.2268, 0.5762],
];
const WB: [f64; 4] = [2.0, 1.0, 1.5, 1.0];
const LOG_GAINS: [f64; 3] = [0.3010299956639812, 0.0, 0.17609125905568124];

fn slide(positive: bool) -> ProcessConfig {
    ProcessConfig {
        process_mode: ProcessMode::E6,
        positive_source: positive,
        linear_raw: true,
        ..ProcessConfig::default()
    }
}

#[test]
fn slide_temperature_round_trips_at_neutral() {
    assert_eq!(slide_wb_to_kelvin(0.0, 0.0), 5500.0);
    for kelvin in [3200.0, 4500.0, 7000.0, 11000.0] {
        let (cyan, yellow) = slide_kelvin_to_wb(kelvin, 0.0, 0.0);
        assert!((slide_wb_to_kelvin(cyan, yellow) - kelvin).abs() < 1e-6);
    }
}

#[test]
fn warming_lowers_blue_more_than_it_raises_red() {
    let (cyan, yellow) = slide_kelvin_to_wb(4000.0, 0.0, 0.0);
    assert!(cyan < 0.0 && yellow > 0.0);
    assert!((1.8..2.4).contains(&(yellow / -cyan)));
}

#[test]
fn slide_temperature_keeps_the_off_locus_component() {
    let (c0, y0) = (0.1, 0.05);
    let (c1, y1) = slide_kelvin_to_wb(4500.0, c0, y0);
    let (c2, y2) = slide_kelvin_to_wb(slide_wb_to_kelvin(c0, y0), c1, y1);
    assert!((c2 - c0).abs() < 1e-9 && (y2 - y0).abs() < 1e-9);
}

#[test]
fn slide_temperature_stays_inside_the_slider_range() {
    for kelvin in [3000.0, 12000.0] {
        let (cyan, yellow) = slide_kelvin_to_wb(kelvin, 0.0, 0.0);
        assert!(cyan.abs() < 1.0 && yellow.abs() < 1.0);
    }
}

#[test]
fn picking_a_neutral_patch_changes_nothing() {
    assert_eq!(slide_pick_wb([0.3; 3], [4.0; 3], 1.0), (0.0, 0.0, 0.0));
}

#[test]
fn a_density_matrix_splits_at_the_white_balance_fold() {
    let unmix = [[1.0, -0.1, 0.0], [0.0, 1.0, -0.2], [-0.05, 0.0, 1.0]];
    let stage = density_stage(&slide(false), Some(CAMERA), Some(&WB), Some(unmix), None);
    assert_eq!(stage.pre, None);
    let offset = [0, 1, 2].map(|row| {
        (0..3)
            .map(|col| unmix[row][col] * LOG_GAINS[col])
            .sum::<f64>()
    });
    let matrix = stage.unmix.unwrap();
    for row in 0..3 {
        assert_eq!(matrix[row][3], offset[row]);
    }
    assert_eq!(stage.post, camera_matrix_parts(Some(CAMERA), Some(&WB)).1);
}

#[test]
fn no_density_matrix_keeps_the_whole_conversion_before_the_log() {
    let stage = density_stage(&slide(false), Some(CAMERA), Some(&WB), None, None);
    assert_eq!(stage.pre, camera_to_working_matrix(Some(CAMERA), Some(&WB)));
    assert!(stage.unmix.is_none() && stage.post.is_none());
}

#[test]
fn the_gains_add_to_a_stain_offset() {
    let affine = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]];
    let stage = density_stage(&slide(false), Some(CAMERA), Some(&WB), Some(affine), None);
    let matrix = stage.unmix.unwrap();
    for row in 0..3 {
        assert!((matrix[row][3] - (2.0 * LOG_GAINS[row])).abs() < 1e-12);
    }
}

#[test]
fn a_negative_keeps_the_whole_matrix_before_the_log() {
    let config = ProcessConfig {
        process_mode: ProcessMode::C41,
        linear_raw: true,
        ..ProcessConfig::default()
    };
    let stage = density_stage(
        &config,
        Some(CAMERA),
        Some(&WB),
        Some([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]]),
        None,
    );
    assert_eq!(stage.pre, camera_to_working_matrix(Some(CAMERA), Some(&WB)));
    assert_eq!(stage.unmix.unwrap()[0][0], 2.0);
    assert_eq!(stage.post, None);
}

#[test]
fn the_positive_primaries_stay_after_the_density_matrix() {
    let stage = density_channels(&slide(true), Some(CAMERA), Some(&WB));
    assert!(stage.pre.is_none());
    assert_eq!(stage.post, camera_matrix_parts(Some(CAMERA), Some(&WB)).1);
}

#[test]
fn the_total_contrast_is_the_transfer_grade() {
    let exposure = negpy_backend::domain::models::ExposureConfig::default();
    let metrics = negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics::default();
    assert_eq!(transfer_total_contrast(&exposure, &metrics), 1.0);
}
