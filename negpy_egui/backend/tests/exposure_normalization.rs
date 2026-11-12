//! Ports of the bounds and normalization assertions in `tests/test_color_luma_split.py`
//! and `tests/test_batch_norm.py`.

use negpy_backend::features::exposure::normalization::{
    LogNegativeBounds, analysis_crop, analyze_bounds_from_image, analyze_log_exposure_bounds,
    analyze_log_exposure_bounds_with_roi, block_median_filter, luminance_density_range,
    mix_luma_color_bounds, normalize_bounds, prefilter_log_grid, to_log_density_image,
};

fn offset_image(width: usize, height: usize) -> Vec<f32> {
    let count = width * height;
    (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.98 * index as f32 / (count - 1) as f32;
            [value, value * 0.7, value * 0.85]
        })
        .collect()
}

fn mono_image(width: usize, height: usize) -> Vec<f32> {
    let count = width * height;
    (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.98 * index as f32 / (count - 1) as f32;
            [value, value, value]
        })
        .collect()
}

fn mean(values: [f64; 3]) -> f64 {
    values.iter().sum::<f64>() / 3.0
}

#[test]
fn luma_drives_the_mean_centre_and_span() {
    let image = offset_image(100, 100);
    let (grid, _, _) = prefilter_log_grid(&image, 100, 100, None, 0.0);
    let a = analyze_log_exposure_bounds(&grid, 0.6, 5.0);
    let b = analyze_log_exposure_bounds(&grid, 0.6, 0.5);
    assert!((mean(a.floors) - mean(b.floors)).abs() < 1e-5);
    assert!((mean(a.ceils) - mean(b.ceils)).abs() < 1e-5);
    assert!(((mean(a.ceils) - mean(a.floors)) - (mean(b.ceils) - mean(b.floors))).abs() < 1e-5);
}

#[test]
fn the_color_clip_preserves_the_luma_span() {
    let image = offset_image(100, 100);
    let (grid, _, _) = prefilter_log_grid(&image, 100, 100, None, 0.0);
    let a = analyze_log_exposure_bounds(&grid, 0.0, 10.0);
    let b = analyze_log_exposure_bounds(&grid, 0.0, 0.01);
    let span = |bounds: LogNegativeBounds| (mean(bounds.ceils) - mean(bounds.floors)).abs();
    assert!((span(a) - span(b)).abs() < 1e-5);
}

#[test]
fn the_color_pass_carries_the_per_channel_cast() {
    let image = offset_image(100, 100);
    let bounds = analyze_bounds_from_image(&image, 100, 100, 0.0, 5.0);
    assert!(bounds.floors[1] < bounds.floors[2]);
    assert!(bounds.floors[2] < bounds.floors[0]);
}

#[test]
fn a_mono_image_has_no_cast() {
    let mono = mono_image(100, 100);
    let (grid, _, _) = prefilter_log_grid(&mono, 100, 100, None, 0.0);
    let a = analyze_log_exposure_bounds(&grid, 0.3, 10.0);
    let b = analyze_log_exposure_bounds(&grid, 0.3, 0.01);
    for ch in 0..3 {
        assert!((a.floors[ch] - b.floors[ch]).abs() < 1e-6);
        assert!((a.ceils[ch] - b.ceils[ch]).abs() < 1e-6);
        assert!((a.floors[ch] - a.floors[0]).abs() < 1e-6);
    }
}

#[test]
fn mixing_a_bounds_with_itself_is_the_identity() {
    let asymmetric = LogNegativeBounds::new([-1.5, -1.5, -1.35], [-0.5, -0.5, -0.2]);
    assert_eq!(mix_luma_color_bounds(asymmetric, asymmetric), asymmetric);
}

#[test]
fn swapping_the_color_source_keeps_the_luma_range_and_centre() {
    let locked = LogNegativeBounds::new([-2.0, -2.2, -2.1], [-0.2, -0.3, -0.1]);
    let local = LogNegativeBounds::new([-1.0, -1.1, -0.9], [-0.6, -0.5, -0.7]);
    let centre = |b: LogNegativeBounds| {
        (0..3)
            .map(|ch| [0.2974, 0.6273, 0.0753][ch] * (b.floors[ch] + b.ceils[ch]) / 2.0)
            .sum::<f64>()
    };
    for (luma_src, color_src) in [(local, locked), (locked, local)] {
        let mixed = mix_luma_color_bounds(luma_src, color_src);
        assert!((luminance_density_range(mixed) - luminance_density_range(luma_src)).abs() < 1e-6);
        assert!((centre(mixed) - centre(luma_src)).abs() < 1e-6);
    }
    assert!((mix_luma_color_bounds(local, locked).floors[0] - local.floors[0]).abs() > 1e-4);
}

#[test]
fn roi_excludes_border_pixels() {
    let (h, w) = (200, 200);
    let mut image = vec![1e-5; h * w * 3];
    let roi_value = 10f32.powf(-0.5);
    for y in 75..125 {
        for x in 75..125 {
            let at = (y * w + x) * 3;
            image[at..at + 3].fill(roi_value);
        }
    }
    let no_roi = analyze_bounds_from_image(&image, w, h, 0.5, 0.0);
    let with_roi =
        analyze_log_exposure_bounds_with_roi(&image, w, h, Some((75, 125, 75, 125)), 0.0, 0.5, 0.0);
    for ch in 0..3 {
        assert!(no_roi.floors[ch] < -3.0);
        assert!(with_roi.floors[ch] > -1.0);
    }
}

#[test]
fn normalizing_uses_the_float32_window() {
    let bounds = LogNegativeBounds::new([-2.0, -2.0, -2.0], [-0.2, -0.2, -0.2]);
    let log = to_log_density_image(&[1.0, 0.63, 0.01]);
    let normalized = normalize_bounds(&log, bounds);
    assert!((normalized[0] - 2.0 / 1.8).abs() < 1e-6);
    let expected = (log[1] as f64 + 2.0) / 1.8;
    assert!((normalized[1] as f64 - expected).abs() < 1e-5);
}

#[test]
fn a_centered_buffer_crops_each_border() {
    let image: Vec<f32> = (0..10 * 10 * 3).map(|v| v as f32).collect();
    let (cropped, w, h) = analysis_crop(&image, 10, 10, 0.1);
    assert_eq!((w, h), (8, 8));
    assert_eq!(cropped.len(), 8 * 8 * 3);
}

#[test]
fn a_small_grid_is_already_the_prefilter() {
    let image = mono_image(32, 32);
    let (filtered, w, h) = block_median_filter(&image, 32, 32);
    assert_eq!((w, h), (32, 32));
    assert_eq!(filtered, image);
}
