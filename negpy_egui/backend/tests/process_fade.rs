//! Ports the pure helpers of tests/test_fade* and test_process_logic.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::process::fade::{
    density_anchor, fade_matrix, fade_white, fade_white_space, fade_white_stale, fade_white_target,
    render_white_space, resolve_fade_matrix, white_target, with_fade_white,
};
use negpy_backend::features::process::logic::{
    effective_highlight_reconstruction, highlight_reconstruction_bakes_wb,
    highlight_reconstruction_bakes_wb_token, highlight_reconstruction_bright_gain,
    highlight_reconstruction_token,
};

fn slide() -> ProcessConfig {
    ProcessConfig {
        process_mode: ProcessMode::E6,
        ..ProcessConfig::default()
    }
}

#[test]
fn a_fade_matrix_is_the_inverse_dye_survival() {
    let matrix = resolve_fade_matrix(0.8, 0.9, 0.7);
    assert!((matrix[0][0] - 1.25).abs() < 1e-12);
    assert!((matrix[1][1] - 1.0 / (0.8 * 0.9)).abs() < 1e-12);
    assert!((matrix[2][2] - 1.0 / (0.8 * 0.7)).abs() < 1e-12);
}

#[test]
fn a_negative_or_unit_survival_has_no_fade_matrix() {
    let mut negative = ProcessConfig::default();
    negative.fade_ratio_r = 0.5;
    assert!(fade_matrix(&negative).is_none());
    assert!(fade_matrix(&slide()).is_none());
}

#[test]
fn a_slide_takes_its_fade_matrix() {
    let mut process = slide();
    process.fade_ratio_r = 0.8;
    assert!(fade_matrix(&process).is_some());
}

#[test]
fn a_stored_white_only_applies_in_its_own_space() {
    let mut process = slide();
    process.fade_white_r = -0.1;
    process.fade_white_g = -0.2;
    process.fade_white_b = -0.3;
    process.fade_white_space = "abc".into();
    process.fade_white_anchor = true;
    assert_eq!(fade_white(&process, "abc"), Some([-0.1, -0.2, -0.3]));
    assert_eq!(fade_white(&process, "other"), None);
    assert_eq!(white_target([-0.1, -0.2, -0.3]), -0.1);
    assert_eq!(white_target([-2.0, -0.2, -0.3]), -0.2);
}

#[test]
fn remove_stain_anchors_each_channel_on_its_own_white() {
    assert_eq!(density_anchor([-0.1, -0.2, -0.3], true), [-0.1, -0.2, -0.3]);
    assert_eq!(
        density_anchor([-0.1, -0.2, -0.3], false),
        [-0.1, -0.1, -0.1]
    );
}

#[test]
fn a_stale_white_is_detected_for_a_fade_or_remove_stain() {
    let mut process = slide();
    process.fade_white_space = "old".into();
    assert!(!fade_white_stale(&process, "new"));
    process.fade_white_anchor = true;
    assert!(fade_white_stale(&process, "new"));
    assert!(!fade_white_stale(&process, ""));
}

#[test]
fn a_measured_white_is_stored_on_the_process() {
    let process = slide();
    let updated = with_fade_white(&process, Some((-0.1, -0.2, -0.3, "space".into())));
    let mut anchored = updated.clone();
    anchored.fade_white_anchor = true;
    assert_eq!(
        (
            updated.fade_white_r,
            updated.fade_white_g,
            updated.fade_white_b
        ),
        (-0.1, -0.2, -0.3)
    );
    assert_eq!(updated.fade_white_space, "space");
    assert_eq!(with_fade_white(&process, None), process);
    assert_eq!(fade_white_target(&anchored, "space"), Some(-0.1));
}

#[test]
fn a_source_with_no_camera_matrix_has_no_white_space() {
    assert!(fade_white_space(None, None, None, 0).is_empty());
    assert!(render_white_space(&ProcessConfig::default(), None, None).is_empty());
}

#[test]
fn highlight_reconstruction_is_slide_only_and_validated() {
    let mut process = ProcessConfig::default();
    process.highlight_reconstruction = 3;
    assert_eq!(effective_highlight_reconstruction(&process), 0);
    process.process_mode = ProcessMode::E6;
    assert_eq!(effective_highlight_reconstruction(&process), 3);
    process.highlight_reconstruction = 1;
    assert_eq!(effective_highlight_reconstruction(&process), 0);
    process.highlight_reconstruction = 3;
    assert_eq!(highlight_reconstruction_token(&process), "|hr:3");
}

#[test]
fn reconstruction_bakes_white_balance_only_on_a_slide_decode() {
    let mut process = slide();
    assert!(!highlight_reconstruction_bakes_wb(&process));
    process.highlight_reconstruction = 3;
    assert!(highlight_reconstruction_bakes_wb(&process));
    process.linear_raw = true;
    assert!(!highlight_reconstruction_bakes_wb(&process));
    process.linear_raw = false;
    process.positive_source = true;
    assert!(!highlight_reconstruction_bakes_wb(&process));
    process.positive_source = false;
    assert_eq!(highlight_reconstruction_bakes_wb_token(&process), "|hrwb:1");
}

#[test]
fn the_bright_gain_offsets_libraw_scaling() {
    assert_eq!(highlight_reconstruction_bright_gain(None, 3), 1.0);
    assert_eq!(
        highlight_reconstruction_bright_gain(Some(&[1.0, 2.0, 4.0]), 0),
        1.0
    );
    assert_eq!(
        highlight_reconstruction_bright_gain(Some(&[1.0, 2.0, 4.0]), 3),
        4.0
    );
    assert_eq!(
        highlight_reconstruction_bright_gain(Some(&[1.0, 0.0, 4.0]), 3),
        1.0
    );
}
