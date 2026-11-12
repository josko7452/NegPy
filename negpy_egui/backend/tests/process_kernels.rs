//! Ports the lightweight process-kernel cases: sensor gating, capture matrices, hue and chart geometry.

use negpy_backend::domain::color::{WORKING_TO_XYZ, XYZ_TO_WORKING};
use negpy_backend::domain::models::{ProcessConfig, ProcessMode, SensorUnmix};
use negpy_backend::features::process::capture_color::{
    camera_matrix_parts, camera_to_working_matrix, primaries, wb_gains,
};
use negpy_backend::features::process::chart::{patch_centers, sample_radius, COLS, ROWS};
use negpy_backend::features::process::hue::apply_hue_trim;
use negpy_backend::features::process::sensor::{
    build_sensor_matrix, density_unmix_matrix, effective_sensor_matrix, sensor_token,
    sensor_unmix_available, unmix_block_reason,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn process(mode: ProcessMode, linear_raw: bool) -> ProcessConfig {
    ProcessConfig {
        process_mode: mode,
        linear_raw,
        ..ProcessConfig::default()
    }
}

#[test]
fn a_slide_blocks_the_sensor_unmix() {
    assert_eq!(
        unmix_block_reason(&process(ProcessMode::E6, true)),
        "transparency"
    );
}

#[test]
fn a_non_linear_negative_blocks_the_sensor_unmix() {
    let process = process(ProcessMode::C41, false);
    assert_eq!(unmix_block_reason(&process), "linear_raw");
    assert!(!sensor_unmix_available(&process));
}

#[test]
fn a_linear_negative_accepts_the_sensor_unmix() {
    let process = process(ProcessMode::C41, true);
    assert!(sensor_unmix_available(&process));
    assert_eq!(unmix_block_reason(&process), "");
}

#[test]
fn the_sensor_token_names_the_matrix_and_mode() {
    let mut process = process(ProcessMode::C41, true);
    process.sensor_matrix = Some(vec![
        1.0, 0.1, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    process.sensor_unmix = SensorUnmix::Density;
    assert_eq!(
        sensor_token(&process),
        "|sn:1,0.1,0,0,1,0,0,0,1|density"
    );
    process.sensor_unmix = SensorUnmix::Linear;
    assert!(!sensor_token(&process).ends_with("|density"));
}

#[test]
fn a_blocked_basis_has_no_effective_matrix_or_token() {
    let mut process = process(ProcessMode::C41, false);
    process.sensor_matrix = Some(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(effective_sensor_matrix(&process), None);
    assert_eq!(sensor_token(&process), "");
}

#[test]
fn a_clean_three_band_calibration_is_identity() {
    let matrix = build_sensor_matrix([0.8, 0.0, 0.0], [0.0, 0.8, 0.0], [0.0, 0.0, 0.8]).unwrap();
    for (index, value) in matrix.iter().enumerate() {
        let expected = if index % 4 == 0 { 1.0 } else { 0.0 };
        assert!((value - expected).abs() < 1e-9, "{index}: {value}");
    }
}

#[test]
fn a_capture_with_no_own_channel_signal_is_rejected() {
    assert!(build_sensor_matrix([0.0, 0.1, 0.1], [0.1, 0.8, 0.1], [0.1, 0.1, 0.8]).is_err());
}

#[test]
fn identical_bands_are_rejected_as_not_independent() {
    let band = [0.8, 0.2, 0.2];
    assert!(build_sensor_matrix(band, band, band).is_err());
}

#[test]
fn the_density_matrix_rows_sum_to_one() {
    let matrix = [1.0, 0.1, 0.0, 0.0, 1.0, 0.2, 0.1, 0.0, 1.0];
    let base = [0.8, 0.7, 0.6];
    let result = density_unmix_matrix(&matrix, base);
    for row in 0..3 {
        let sum: f64 = result[row * 3..row * 3 + 3].iter().sum();
        assert!((sum - 1.0).abs() < 1e-9, "row {row}: {sum}");
    }
}

#[test]
fn no_camera_matrix_is_available_for_a_profiled_source() {
    assert!(camera_to_working_matrix(None, Some(&[2.0, 1.0, 1.5])).is_none());
    assert_eq!(camera_matrix_parts(None, None), (None, None));
}

#[test]
fn a_neutral_white_balance_keeps_the_matrix() {
    let camera = [
        [0.5, 0.3, 0.2],
        [0.2, 0.6, 0.2],
        [0.1, 0.2, 0.7],
    ];
    let plain = camera_to_working_matrix(Some(camera), None).unwrap();
    let neutral = camera_to_working_matrix(Some(camera), Some(&[1.0, 1.0, 1.0])).unwrap();
    for row in 0..3 {
        for column in 0..3 {
            assert!((plain[row][column] - neutral[row][column]).abs() < 1e-12);
        }
    }
}

#[test]
fn the_working_matrices_invert_each_other() {
    let product: [[f64; 3]; 3] = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| f64::from(WORKING_TO_XYZ[row][index]) * f64::from(XYZ_TO_WORKING[index][column]))
                .sum()
        })
    });
    for (row, values) in product.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            let expected = if row == column { 1.0 } else { 0.0 };
            assert!((value - expected).abs() < 1e-5);
        }
    }
}

#[test]
fn raising_white_balance_to_green_scales_the_matrix_columns() {
    let camera = [
        [0.5, 0.3, 0.2],
        [0.2, 0.6, 0.2],
        [0.1, 0.2, 0.7],
    ];
    let gains = wb_gains(Some(&[2.0, 1.0, 1.5])).unwrap();
    assert_eq!(gains, [2.0, 1.0, 1.5]);
    let plain = camera_to_working_matrix(Some(camera), None).unwrap();
    let balanced = camera_to_working_matrix(Some(camera), Some(&[2.0, 1.0, 1.5])).unwrap();
    for row in 0..3 {
        assert!((balanced[row][0] - plain[row][0] * 2.0).abs() < 1e-9);
        assert!((balanced[row][2] - plain[row][2] * 1.5).abs() < 1e-9);
    }
}

#[test]
fn a_degenerate_primaries_matrix_is_rejected() {
    let singular = [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 1.0]];
    assert!(primaries(Some(singular)).is_none());
}

#[test]
fn hue_trim_at_zero_is_an_identity() {
    let img = ImageBuffer::from_rgb(2, 1, vec![0.2, 0.4, 0.6, 0.1, 0.3, 0.5]);
    assert_eq!(apply_hue_trim(&img, 0.0), img);
}

#[test]
fn chart_patch_centers_run_row_major() {
    let corners = [[0.0, 0.0], [60.0, 0.0], [60.0, 40.0], [0.0, 40.0]];
    let centers = patch_centers(corners);
    assert_eq!(centers.len(), ROWS * COLS);
    assert_eq!(centers[0], [5.0, 5.0]);
    assert_eq!(centers[COLS - 1], [55.0, 5.0]);
}

#[test]
fn the_sample_radius_uses_the_smaller_patch_pitch() {
    let corners = [[0.0, 0.0], [60.0, 0.0], [60.0, 40.0], [0.0, 40.0]];
    assert!((sample_radius(corners) - 2.0).abs() < 1e-9);
}
