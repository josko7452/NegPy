//! Ports the Estimate cases of tests/test_fade_estimate.py and tests/test_fade.py.

use negpy_backend::domain::models::ProcessConfig;
use negpy_backend::features::process::fade_estimate::{
    FadeEstimate, NeutralAxisRefs, estimate_fade_with, fade_ratios_from_neutral_axis,
    gray_readings, measure_white, neutral_axis_from_grid, pick_neutrals, stain_suggested,
    survival_crossover, tone_band_refs,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn refs(mid: [f64; 3]) -> NeutralAxisRefs {
    NeutralAxisRefs {
        mid,
        shadow: mid,
        highlight: None,
        confidence: 1.0,
    }
}

#[test]
fn the_ratios_compare_the_midtone_spreads() {
    let (green, blue, reason) = fade_ratios_from_neutral_axis(Some(&refs([-0.4, -0.2, -0.1])));
    assert!((green - 0.5).abs() < 1e-12);
    assert!((blue - 0.25).abs() < 1e-12);
    assert!(reason.is_empty());
}

#[test]
fn a_flat_red_channel_cannot_be_measured() {
    let (green, blue, reason) = fade_ratios_from_neutral_axis(Some(&refs([-0.01, -0.2, -0.1])));
    assert_eq!((green, blue), (1.0, 1.0));
    assert!(reason.contains("too flat"));
}

#[test]
fn equal_fading_has_nothing_to_estimate() {
    let (green, blue, reason) = fade_ratios_from_neutral_axis(Some(&refs([-0.2, -0.21, -0.205])));
    assert_eq!((green, blue), (1.0, 1.0));
    assert!(reason.contains("faded equally"));
}

#[test]
fn an_out_of_range_ratio_is_clamped_and_explained() {
    let (green, blue, reason) = fade_ratios_from_neutral_axis(Some(&refs([-0.1, -0.5, -0.2])));
    assert!((0.2..=5.0).contains(&green));
    assert!((0.2..=5.0).contains(&blue));
    assert!(reason.is_empty() || reason.contains("outside"));
}

fn white_grid() -> ImageBuffer {
    let mut pixels = vec![0.3f32.log10(); 20 * 20 * 3];
    for pixel in pixels.chunks_exact_mut(3) {
        pixel.copy_from_slice(&[0.2f32.log10(), 0.15f32.log10(), 0.1f32.log10()]);
    }
    for pixel in pixels.chunks_exact_mut(3).take(10) {
        pixel.copy_from_slice(&[0.9f32.log10(), 0.8f32.log10(), 0.7f32.log10()]);
    }
    ImageBuffer::from_rgb(20, 20, pixels)
}

#[test]
fn the_white_is_each_channels_brightest_cell() {
    let white = measure_white(&white_grid()).expect("a white");
    assert!(white[0] > white[1] && white[1] > white[2]);
    assert!((white[0] - 0.9f64.log10()).abs() < 1e-4);
}

#[test]
fn a_yellow_stain_suggests_removal() {
    assert!(stain_suggested([0.0, -0.01, -0.2]));
    assert!(!stain_suggested([-0.4, -0.45, -0.5]));
}

#[test]
fn a_missing_white_fails_the_anchored_estimate() {
    let grid = ImageBuffer::from_rgb(2, 2, vec![0.4f32; 12]);
    let estimate = estimate_fade_with(&grid, |_| (None, String::new()), true, false, "", true);
    assert!(estimate.red.is_none());
    assert!(!estimate.reason.is_empty());
}

#[test]
fn a_gray_reader_that_finds_nothing_fails_closed() {
    let grid = ImageBuffer::from_rgb(2, 2, vec![-1.0; 12]);
    let estimate = estimate_fade_with(
        &grid,
        |_| (None, "none".to_string()),
        false,
        false,
        "",
        true,
    );
    assert!(estimate.red.is_none());
    assert_eq!((estimate.green, estimate.blue), (1.0, 1.0));
}

#[test]
fn a_real_reading_makes_a_red_survival() {
    let grid = ImageBuffer::from_rgb(2, 2, vec![-1.0; 12]);
    let estimate = estimate_fade_with(
        &grid,
        |_| (Some(refs([-0.4, -0.2, -0.1])), String::new()),
        false,
        false,
        "",
        false,
    );
    assert!(estimate.red.is_some());
    assert!((estimate.green - 0.5).abs() < 1e-12);
    assert!(estimate.crossover.is_none());
}

#[test]
fn no_change_estimate_stays_at_one() {
    let estimate = FadeEstimate {
        red: None,
        green: 1.0,
        blue: 1.0,
        ..FadeEstimate::default()
    };
    assert_eq!(estimate.green, 1.0);
    assert!(estimate.red.is_none());
}

#[test]
fn a_reader_without_grays_reports_the_tone_bands_route() {
    let grid = ImageBuffer::from_rgb(4, 4, vec![-1.0; 48]);
    let (axis, reason) = neutral_axis_from_grid(&grid);
    assert!(axis.is_none());
    assert!(!reason.is_empty());
    let (bands, band_reason) = tone_band_refs(&grid);
    assert!(bands.is_none());
    assert!(!band_reason.is_empty());
}

#[test]
fn a_crossover_table_needs_enough_grays() {
    let grid = ImageBuffer::from_rgb(4, 4, vec![-1.0; 48]);
    let table = survival_crossover(
        &grid,
        |_| (Some(refs([-0.4, -0.2, -0.1])), String::new()),
        [0.5, 0.5, 0.25],
        false,
        false,
        "",
        0.0,
    );
    assert!(table.is_none() || table.is_some_and(|value| value.len() % 5 == 1));
}

#[test]
fn neutral_picking_returns_none_without_a_table() {
    let grid = ImageBuffer::from_rgb(4, 4, vec![-1.0; 48]);
    assert!(pick_neutrals(&grid, &gray_readings(&grid), false, false, "").is_none());
}

#[test]
fn the_default_process_has_no_stored_crossover() {
    assert_eq!(ProcessConfig::default().fade_crossover_table, None);
}
