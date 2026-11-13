//! Ports the table-level rules of `tests/test_fade_crossover.py` and `test_fade_neutrals.py`.

use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::features::process::fade::{
    CROSSOVER_LIMIT, CROSSOVER_WEIGHTS, apply_crossover, cast_name, crossover_amount,
    crossover_bands, crossover_casts, crossover_offsets, crossover_reference, crossover_strength,
    crossover_uniform, crossover_zero, measure_crossover, neutral_spread, resolve_crossover,
    shape_cubic,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn table() -> Vec<f64> {
    vec![
        0.5, 0.1, 0.20, 0.04, 0.00, -0.04, 0.4, 0.30, 0.02, 0.01, -0.03, 0.7, 0.40, -0.02, 0.02,
        0.00, 1.0, 0.20, -0.10, -0.10, 0.20,
    ]
}

#[test]
fn the_bands_are_five_values_each_and_sum_to_zero_under_the_weights() {
    let rows = crossover_bands(&table());
    assert_eq!(rows.len(), 4);
    for row in &rows {
        let sum: f64 = row[2..]
            .iter()
            .zip(CROSSOVER_WEIGHTS)
            .map(|(value, weight)| value * weight)
            .sum();
        assert!(sum.abs() < 1e-9);
    }
    assert_eq!(crossover_zero(&table()), 0.5);
}

#[test]
fn the_reference_is_interpolated_at_the_zero() {
    let reference = crossover_reference(&table());
    assert!((reference[0] - 0.01).abs() < 0.01);
    assert!(crossover_amount(Some(&table())) > 0.0);
    assert!(crossover_amount(None) == 0.0);
}

#[test]
fn an_identity_table_has_no_crossover_and_no_cast() {
    let flat = vec![
        0.5, 0.1, 0.1, 0.01, 0.01, 0.01, 0.4, 0.3, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0,
    ];
    assert_eq!(crossover_amount(Some(&flat)), 0.0);
    let (shadow, highlight) = crossover_casts(Some(&flat));
    assert!(shadow.is_some_and(|lean| lean.iter().all(|value| value.abs() < 0.01)));
    assert!(highlight.is_some_and(|lean| lean.iter().all(|value| value.abs() < 0.01)));
    assert!(neutral_spread(&flat).abs() < 1e-9);
}

#[test]
fn the_shadow_and_highlight_casts_keep_their_hue() {
    let (shadow, highlight) = crossover_casts(Some(&table()));
    assert!(shadow.is_some() && highlight.is_some());
    assert!(cast_name(shadow).is_some());
    assert!(cast_name(highlight).is_some());
    assert_eq!(cast_name(None), None);
    assert_eq!(cast_name(Some([0.01, 0.01, 0.01])), None);
}

#[test]
fn a_monotone_cubic_is_flat_beyond_its_ends() {
    let xs = [0.0, 1.0, 2.0];
    let ys = [0.0, 1.0, 2.0];
    assert_eq!(shape_cubic(&xs, &ys, -1.0), 0.0);
    assert_eq!(shape_cubic(&xs, &ys, 3.0), 2.0);
    assert!((shape_cubic(&xs, &ys, 1.0) - 1.0).abs() < 1e-9);
}

#[test]
fn a_flat_table_has_a_zero_spread() {
    let flat = vec![0.0, 0.1, 0.0, 0.01, 0.01, 0.01, 1.0, 0.0, 0.01, 0.01, 0.01];
    assert_eq!(neutral_spread(&flat), 0.0);
}

#[test]
fn the_offsets_cancel_under_the_weights_and_stay_under_the_limit() {
    for strength in [1.0, 0.5] {
        let offsets = crossover_offsets(Some(&table()), strength).unwrap();
        assert_eq!(offsets.bins.len(), 64);
        for row in &offsets.bins {
            let sum: f64 = row
                .iter()
                .zip(CROSSOVER_WEIGHTS)
                .map(|(value, weight)| f64::from(*value) * weight)
                .sum();
            assert!(sum.abs() < 1e-6);
            assert!(
                row.iter()
                    .all(|value| f64::from(value.abs()) <= CROSSOVER_LIMIT + 1e-6)
            );
        }
    }
}

#[test]
fn an_identity_or_zero_strength_table_has_no_offsets() {
    let flat = vec![
        0.5, 0.1, 0.1, 0.01, 0.01, 0.01, 0.4, 0.3, 0.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0,
    ];
    assert!(crossover_offsets(Some(&flat), 1.0).is_none());
    assert!(crossover_offsets(Some(&table()), 0.0).is_none());
    assert!(crossover_offsets(None, 1.0).is_none());
}

#[test]
fn a_single_value_table_has_a_reference_at_its_only_band() {
    let single = vec![0.5, 0.2, 0.1, 0.02, 0.0, -0.02];
    assert_eq!(crossover_reference(&single), [0.02, 0.0, -0.02]);
    assert!(crossover_offsets(Some(&single), 1.0).is_none());
}

#[test]
fn the_offsets_apply_without_moving_the_mean_density() {
    let offsets = crossover_offsets(Some(&table()), 1.0).unwrap();
    let mut pixels = Vec::new();
    for index in 0..32 * 32 {
        let value = -3.0 + 3.0 * index as f32 / (32.0 * 32.0);
        pixels.extend([value, value + 0.02, value - 0.01]);
    }
    let image = ImageBuffer::from_rgb(32, 32, pixels);
    let out = apply_crossover(&image, Some(&offsets));
    let mean = |pixels: &[f32]| {
        pixels
            .chunks_exact(3)
            .map(|p| (p[0] + p[1] + p[2]) / 3.0)
            .sum::<f32>()
            / (pixels.len() / 3) as f32
    };
    assert!((mean(&out.pixels) - mean(&image.pixels)).abs() < 1e-4);
    assert!(
        out.pixels
            .iter()
            .zip(&image.pixels)
            .any(|(a, b)| (a - b).abs() > 0.01)
    );
    assert_eq!(apply_crossover(&image, None).pixels, image.pixels);
}

#[test]
fn the_uniform_is_the_python_layout() {
    assert_eq!(crossover_uniform(None).len(), 16 * 65);
    let offsets = crossover_offsets(Some(&table()), 1.0).unwrap();
    let bytes = crossover_uniform(Some(&offsets));
    assert_eq!(bytes.len(), 16 * 65);
    let value = f32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    assert_eq!(value, offsets.lo as f32);
    let on = f32::from_ne_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(on, 1.0);
}

#[test]
fn strength_needs_a_slide_with_fade() {
    let mut process = ProcessConfig::default();
    process.fade_ratio_r = 0.8;
    assert_eq!(crossover_strength(&process), 0.0);
    process.process_mode = ProcessMode::E6;
    assert_eq!(crossover_strength(&process), 0.0);
    process.fade_crossover = 0.5;
    assert_eq!(crossover_strength(&process), 0.5);
}

#[test]
fn a_stored_table_resolves_under_any_tag_and_zero_is_no_table() {
    let mut process = ProcessConfig::default();
    process.process_mode = ProcessMode::E6;
    process.fade_ratio_r = 0.8;
    process.fade_crossover = 1.0;
    process.fade_crossover_table = Some(table());
    let (resolved, measured) = resolve_crossover(&process, "tag", None, true);
    assert_eq!(resolved, Some(table()));
    assert!(measured.is_none());
    process.fade_crossover = 0.0;
    assert_eq!(resolve_crossover(&process, "tag", None, true), (None, None));
}

#[test]
fn a_measurable_grid_yields_a_rounded_table() {
    let mut pixels = Vec::new();
    for index in 0..64 * 64 {
        let level = -2.5 + 2.2 * index as f32 / (64.0 * 64.0);
        pixels.extend([level, level, level]);
    }
    let grid = ImageBuffer::from_rgb(64, 64, pixels);
    let table = measure_crossover(&grid).unwrap();
    assert_eq!((table.len() - 1) % 5, 0);
    assert!(table.len() >= 6);
}
