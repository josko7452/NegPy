//! Ports the observable rules of `tests/test_flatfield_logic.py`.

use negpy_backend::domain::models::FlatFieldConfig;
use negpy_backend::features::flatfield::logic::{
    GainEntry, apply_flatfield, check_copy, compute_gain, evenness, evenness_view, flatfield_token,
    gain_token, is_clipped, to_sensor_layout, work_copy,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn falloff(height: usize, width: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(height * width * 3);
    for y in 0..height {
        for x in 0..width {
            let yy = (y as f32 - (height - 1) as f32 / 2.0) / ((height - 1) as f32 / 2.0);
            let xx = (x as f32 - (width - 1) as f32 / 2.0) / ((width - 1) as f32 / 2.0);
            let value = 1.0 - 0.4 * (yy * yy + xx * xx).sqrt().clamp(0.0, 1.0);
            pixels.extend([value; 3]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn flat(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

fn entry(gain: ImageBuffer) -> GainEntry {
    let token = gain_token(&gain);
    GainEntry {
        gain,
        token,
        sensor_layout: true,
    }
}

fn config(profile_id: &str) -> FlatFieldConfig {
    FlatFieldConfig {
        apply: true,
        profile_id: profile_id.to_string(),
    }
}

#[test]
fn the_gain_token_is_the_python_blake2b_prefix() {
    let gain = ImageBuffer::from_rgb(1, 1, vec![1.0, 0.5, 2.0]);
    assert_eq!(gain_token(&gain), "2df7f8bcbd387f1b");
}

#[test]
fn a_disabled_empty_or_unknown_profile_is_a_passthrough() {
    let image = flat(16, 16, 0.5);
    let disabled = FlatFieldConfig {
        apply: false,
        profile_id: "rig".into(),
    };
    assert_eq!(
        apply_flatfield(&image, &disabled, None, Some(1), None),
        image
    );
    assert_eq!(
        apply_flatfield(&image, &config(""), None, Some(1), None),
        image
    );
    assert_eq!(
        apply_flatfield(&image, &config("ghost"), None, Some(1), None),
        image
    );
    assert!(flatfield_token(&config("ghost"), None).is_empty());
}

#[test]
fn the_correction_flattens_uneven_illumination() {
    let reference = falloff(128, 192);
    let entry = entry(compute_gain(&reference));
    let corrected = apply_flatfield(&reference, &config("rig"), Some(&entry), Some(1), None);
    let mean = corrected.pixels.iter().sum::<f32>() / corrected.pixels.len() as f32;
    let variance = corrected
        .pixels
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / corrected.pixels.len() as f32;
    assert!(variance.sqrt() < 0.02, "spread {}", variance.sqrt());
}

#[test]
fn the_working_copy_uses_the_gain_and_check_sizes() {
    let reference = falloff(683, 1024);
    assert_eq!(
        work_copy(&reference)
            .width
            .max(work_copy(&reference).height),
        256
    );
    assert_eq!(
        check_copy(&reference)
            .width
            .max(check_copy(&reference).height),
        512
    );
}

#[test]
fn a_gain_resizes_to_a_differently_sized_image() {
    let reference = falloff(64, 64);
    let entry = entry(compute_gain(&reference));
    let image = flat(140, 100, 0.5);
    let out = apply_flatfield(&image, &config("rig"), Some(&entry), Some(1), None);
    assert_eq!((out.width, out.height), (140, 100));
}

#[test]
fn the_sensor_layout_undoes_every_exif_orientation() {
    let sensor = falloff(12, 18);
    for orientation in 1..=8 {
        let turned = to_sensor_layout(&sensor, Some(orientation));
        let back = to_sensor_layout(&turned, Some(orientation));
        assert_eq!(back, sensor, "orientation {orientation}");
    }
}

#[test]
fn a_frame_of_any_orientation_is_corrected_where_the_light_fell() {
    let light = falloff(120, 180);
    let entry = entry(compute_gain(&light));
    for orientation in 1..=8 {
        let mut frame = to_sensor_layout(&light, Some(orientation));
        for value in &mut frame.pixels {
            *value *= 0.5;
        }
        let out = apply_flatfield(
            &frame,
            &config("rig"),
            Some(&entry),
            Some(orientation),
            None,
        );
        let lum: Vec<f32> = out
            .pixels
            .chunks_exact(3)
            .map(|p| (p[0] + p[1] + p[2]) / 3.0)
            .collect();
        let lo = lum.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = lum.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut sorted = lum.clone();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        assert!((hi - lo) / median < 0.06, "orientation {orientation}");
    }
}

#[test]
fn evenness_reports_falloff_and_a_zero_view_on_carrier() {
    let mut image = flat(96, 64, 0.5);
    for y in 0..4 {
        for x in 0..96 {
            let i = (y * 96 + x) * 3;
            image.pixels[i..i + 3].fill(0.0);
        }
    }
    let (view, check) = evenness_view(&image, 0.1);
    assert_eq!(
        view[..(4 * 96 * 3)].iter().copied().fold(0.0f32, f32::max),
        0.0
    );
    assert!(check.spread() < 1e-6);
    assert!(evenness(&flat(128, 192, 0.5)).spread() < 1e-6);
}

#[test]
fn a_clipped_reference_is_reported() {
    let mut image = falloff(128, 192);
    for value in &mut image.pixels {
        *value = (*value * 2.5).min(1.0);
    }
    assert!(is_clipped(&image));
}

#[test]
fn a_token_names_the_profile_and_marks_an_unsorted_gain() {
    let image = falloff(32, 32);
    let mut entry = entry(compute_gain(&image));
    let token = flatfield_token(&config("rig"), Some(&entry));
    assert!(token.starts_with("|ff:rig:"));
    assert!(!token.ends_with(":ref"));
    entry.sensor_layout = false;
    assert!(flatfield_token(&config("rig"), Some(&entry)).ends_with(":ref"));
    assert!(flatfield_token(&FlatFieldConfig::default(), Some(&entry)).is_empty());
}
