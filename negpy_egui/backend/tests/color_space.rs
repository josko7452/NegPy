//! The shared working-space constants match the Python float32 values.

use negpy_backend::domain::color::{
    WORKING_GAMMA, WORKING_INV_GAMMA, WORKING_TO_XYZ, WORKING_WHITE, XYZ_TO_WORKING, clip01,
    working_lab,
};

#[test]
fn the_transfer_curve_is_the_adobe_rgb_power() {
    assert_eq!(WORKING_GAMMA, 563.0 / 256.0);
    assert_eq!(WORKING_INV_GAMMA, (1.0f64 / WORKING_GAMMA) as f32);
}

#[test]
fn the_matrices_keep_their_python_precision() {
    assert_eq!(WORKING_TO_XYZ[1], [0.2973769, 0.6273491, 0.0752741]);
    assert_eq!(XYZ_TO_WORKING[1], [-0.9692660, 1.8760108, 0.0415560]);
    assert_eq!(WORKING_WHITE, [0.95047, 1.00000, 1.08883]);
}

#[test]
fn a_neutral_in_is_a_neutral_out() {
    let (to_lab, to_rgb) = working_lab();
    let (mut lab, mut rgb) = ([0f32; 3], [0f32; 3]);
    negpy_core::color::rgb_to_lab(&[0.5, 0.5, 0.5], &mut lab, to_lab);
    assert!(lab[1].abs() < 1e-4 && lab[2].abs() < 1e-4);
    negpy_core::color::lab_to_rgb(&lab, &mut rgb, to_rgb);
    assert!((rgb[0] - 0.5).abs() < 1e-3);
    assert!((rgb[1] - 0.5).abs() < 1e-3);
    assert!((rgb[2] - 0.5).abs() < 1e-3);
}

#[test]
fn clip01_passes_a_nan_and_clamps_the_ends() {
    assert_eq!(clip01(-1.0), 0.0);
    assert_eq!(clip01(2.0), 1.0);
    assert_eq!(clip01(0.25), 0.25);
    assert!(clip01(f32::NAN).is_nan());
}
