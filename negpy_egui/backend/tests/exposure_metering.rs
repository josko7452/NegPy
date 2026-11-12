//! Ports of the meter assertions in `tests/test_exposure_bounds.py` and
//! `tests/test_exposure_neutral_axis.py`.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::exposure::constants as c;
use negpy_backend::features::exposure::metering::{
    analysis_grid, density_above_base, effective_crosstalk_matrix, measure_anchor,
    measure_clip_fractions, measure_highlight_point, measure_neutral_axis, measure_shadow_point,
    measure_textural_range, rect_roi, resolve_analysis_region, resolve_crosstalk_matrix,
    unmix_log_image,
};
use negpy_backend::features::exposure::normalization::LogNegativeBounds;

fn gradient(width: usize, height: usize) -> Vec<f32> {
    let count = width * height;
    (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.98 * index as f32 / (count - 1) as f32;
            [value, value, value]
        })
        .collect()
}

fn bounds() -> LogNegativeBounds {
    LogNegativeBounds::new([-1.6, -1.6, -1.6], [-0.05, -0.05, -0.05])
}

#[test]
fn a_degenerate_rect_resolves_to_none() {
    assert_eq!(rect_roi(100, 80, [0.5, 0.5, 0.5, 0.5]), None);
    assert_eq!(rect_roi(100, 80, [0.0, 0.0, 0.01, 0.01]), None);
}

#[test]
fn a_normalized_rect_maps_to_pixels() {
    assert_eq!(
        rect_roi(100, 80, [0.1, 0.2, 0.6, 0.7]),
        Some((16, 56, 10, 60))
    );
}

#[test]
fn a_freehand_rect_wins_over_the_crop_roi() {
    let (roi, buffer) = resolve_analysis_region(
        100,
        80,
        Some((10, 20, 10, 20)),
        0.12,
        Some([0.25, 0.25, 0.75, 0.75]),
        None,
    );
    assert_eq!(roi, Some((20, 60, 25, 75)));
    assert_eq!(buffer, 0.0);
}

#[test]
fn the_meter_rect_stands_in_for_a_missing_crop() {
    let (roi, buffer) =
        resolve_analysis_region(100, 80, None, 0.12, None, Some([0.1, 0.1, 0.9, 0.9]));
    assert_eq!(roi, Some((8, 72, 10, 90)));
    assert_eq!(buffer, 0.12);
}

#[test]
fn a_zero_strength_crosstalk_matrix_is_off() {
    assert_eq!(resolve_crosstalk_matrix(0.0, None), None);
}

#[test]
fn the_crosstalk_rows_stay_normalized() {
    let matrix = resolve_crosstalk_matrix(0.5, None).unwrap();
    for row in matrix {
        let sum: f32 = row.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }
}

#[test]
fn the_crosstalk_profile_must_match_the_mode() {
    let mut process = ProcessConfig::default();
    process.crosstalk_strength = 0.5;
    process.crosstalk_process = ProcessMode::E6.as_str().to_string();
    assert!(effective_crosstalk_matrix(&process, Some(ProcessMode::C41)).is_none());
    assert!(effective_crosstalk_matrix(&process, Some(ProcessMode::E6)).is_some());
}

#[test]
fn an_unmixed_image_keeps_its_shape() {
    let image = vec![0.2; 4 * 4 * 3];
    let matrix = resolve_crosstalk_matrix(0.5, None);
    assert_eq!(
        unmix_log_image(&image, matrix),
        unmix_log_image(&image, matrix)
    );
    assert_eq!(unmix_log_image(&image, None), image);
}

#[test]
fn clip_fractions_count_only_the_bright_window() {
    let (w, h) = (32, 32);
    let mut image = vec![0.4; w * h * 3];
    for y in 0..8 {
        for x in 0..8 {
            let at = (y * w + x) * 3;
            image[at..at + 3].fill(1.0);
        }
    }
    let dark = measure_clip_fractions(&image, w, h, Some((16, 32, 16, 32)), 0.0);
    let bright = measure_clip_fractions(&image, w, h, Some((0, 8, 0, 8)), 0.0);
    assert_eq!(dark, [0.0; 3]);
    assert_eq!(bright, [1.0; 3]);
}

#[test]
fn the_analysis_grid_crops_to_the_roi() {
    let (w, h) = (64, 64);
    let mut image = vec![0.001; w * h * 3];
    for y in 24..40 {
        for x in 24..40 {
            let at = (y * w + x) * 3;
            image[at..at + 3].fill(0.5);
        }
    }
    let (whole, _, _) = analysis_grid(&image, w, h, None, 0.0);
    let (center, _, _) = analysis_grid(&image, w, h, Some((24, 40, 24, 40)), 0.0);
    assert!(whole.iter().any(|value| *value < -2.5));
    assert!(center.iter().all(|value| *value > -1.0));
}

#[test]
fn the_textured_tails_bracket_the_anchor() {
    let (w, h) = (128, 128);
    let image = gradient(w, h);
    let (grid, gw, gh) = analysis_grid(&image, w, h, None, 0.0);
    let b = bounds();
    let anchor = measure_anchor(&grid, gw, gh, b);
    let shadow = measure_shadow_point(&grid, gw, gh, b);
    let highlight = measure_highlight_point(&grid, gw, gh, b);
    assert!(anchor >= c::ASSUMED_ANCHOR - c::ANCHOR_METER_BAND - 1e-9);
    assert!(anchor <= c::ASSUMED_ANCHOR + c::ANCHOR_METER_BAND + 1e-9);
    assert!(shadow > anchor && anchor > highlight);
}

#[test]
fn a_textured_gradient_has_a_positive_range() {
    let (w, h) = (128, 128);
    let image = gradient(w, h);
    let (grid, gw, gh) = analysis_grid(&image, w, h, None, 0.0);
    assert!(measure_textural_range(&grid, gw, gh) > 0.5);
}

#[test]
fn a_neutral_gradient_meters_a_confident_axis() {
    let (w, h) = (128, 128);
    let image = gradient(w, h);
    let (grid, gw, gh) = analysis_grid(&image, w, h, None, 0.0);
    let axis = measure_neutral_axis(&grid, gw, gh, bounds(), true).unwrap();
    assert!(axis.confidence > 0.5);
    for ch in 0..3 {
        assert!((axis.mid[ch] - axis.mid[1]).abs() < 1e-5);
    }
}

#[test]
fn density_above_base_reads_the_thin_end() {
    let grid = vec![0.0; 3 * 3 * 3];
    let b = LogNegativeBounds::new([-1.0; 3], [0.0; 3]);
    let density = density_above_base(Some([0.0; 3]), b, &grid, None).unwrap();
    assert!((density[0] - 0.0).abs() < 1e-12);
    assert!(density[1].abs() < 1e-12);
    assert!((density[2] - 1.0).abs() < 1e-6);
}
