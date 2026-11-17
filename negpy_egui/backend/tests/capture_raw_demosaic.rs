//! Ports the linearity rules of `tests/test_capture_raw_demosaic.py`: the sensor decode
//! scales by the camera's calibrated white level and never by the frame's own maximum.

use negpy_backend::infrastructure::capture::raw_demosaic::{
    as_shot_wb, linear_demosaic, linearity_limit,
};

#[test]
fn a_body_with_a_calibrated_table_uses_its_lowest_channel() {
    let levels = [16383, 15311, 15311, 16383];
    assert_eq!(
        linearity_limit(Some(&levels), None, Some(&[0, 1, 3])),
        Some(15311)
    );
}

#[test]
fn the_format_ceiling_caps_a_bad_calibration() {
    let levels = [16383, 20000];
    assert_eq!(
        linearity_limit(Some(&levels), None, Some(&[1])),
        Some(16383)
    );
}

#[test]
fn a_black_level_is_subtracted_before_the_scale() {
    assert_eq!(
        linearity_limit(Some(&[16383]), Some(&[64.0, 128.0]), None),
        Some(16319)
    );
}

#[test]
fn the_as_shot_white_balance_is_green_normalized() {
    let wb = as_shot_wb(Some([1.8, 1.0, 1.4, 1.0])).unwrap();
    assert!((wb[0] - 1.8).abs() < 1e-6);
    assert!((wb[1] - 1.0).abs() < 1e-6);
    assert!((wb[2] - 1.4).abs() < 1e-6);
    assert_eq!(as_shot_wb(Some([f32::NAN, 1.0, 1.0, 1.0])), None);
}

#[test]
fn a_non_raw_file_has_no_sensor_decode() {
    assert!(linear_demosaic(std::path::Path::new("/missing/frame.dng"), false).is_none());
}
