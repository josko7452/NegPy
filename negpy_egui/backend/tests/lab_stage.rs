//! Ports of the observable rules in `tests/test_lab_logic.py` and `test_lab_row_blocks.py`.

use negpy_backend::domain::models::{LabConfig, SharpenMethod};
use negpy_backend::features::lab::logic::{
    apply_chroma_denoise, apply_clahe, apply_glow_and_halation, apply_saturation,
    apply_usm_sharpening, auto_masking, effective_masking, gaussian_kernel_1d, rl_iterations,
};
use negpy_backend::features::lab::processor::{LabContext, process};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn gradient(width: usize, height: usize) -> ImageBuffer {
    let pixels = (0..width * height)
        .flat_map(|index| {
            let value = 0.02 + 0.9 * index as f32 / (width * height) as f32;
            [value, value * 0.8, value * 0.6]
        })
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}

fn flat(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

fn max_abs(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

#[test]
fn the_gaussian_taps_are_symmetric_and_normalized() {
    for (sigma, radius) in [(0.5, 2), (1.0, 3), (3.75, 10), (45.0, 113)] {
        let taps = gaussian_kernel_1d(sigma);
        assert_eq!((taps.len() - 1) / 2, radius);
        assert!((taps.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        for i in 0..taps.len() / 2 {
            assert_eq!(taps[i], taps[taps.len() - 1 - i]);
        }
    }
    assert_eq!(gaussian_kernel_1d(1000.0).len(), 511);
}

#[test]
fn rl_iterations_stay_in_the_shared_bounds() {
    assert_eq!(rl_iterations(0.0), 5);
    assert_eq!(rl_iterations(1.0), 10);
    assert_eq!(rl_iterations(5.0), 20);
}

#[test]
fn zero_strength_stages_are_exact_passthroughs() {
    let img = gradient(31, 17);
    assert_eq!(apply_clahe(&img, 0.0).pixels, img.pixels);
    assert_eq!(apply_chroma_denoise(&img, 2.5, 0.0).pixels, img.pixels);
    assert_eq!(apply_saturation(&img, 1.0, 0.0).pixels, img.pixels);
    assert_eq!(apply_usm_sharpening(&img, 0.0, 1.0, 0.0).pixels, img.pixels);
    assert_eq!(
        apply_glow_and_halation(&img, 0.0, 0.0, false).pixels,
        img.pixels
    );
}

#[test]
fn clahe_keeps_a_flat_frame_close_to_itself() {
    let img = flat(256, 256, 0.35);
    let out = apply_clahe(&img, 1.0);
    assert!(max_abs(&out.pixels, &img.pixels) < 0.02);
}

#[test]
fn usm_increases_variance_at_a_step() {
    let (w, h) = (40, 40);
    let mut pixels = vec![0.0; w * h * 3];
    for y in 0..h {
        for x in 20..w {
            let i = (y * w + x) * 3;
            pixels[i..i + 3].fill(0.8);
        }
    }
    let out = apply_usm_sharpening(&ImageBuffer::from_rgb(w, h, pixels.clone()), 1.0, 1.0, 0.0);
    let mean = |values: &[f32]| values.iter().sum::<f32>() / values.len() as f32;
    let variance = |values: &[f32]| {
        let m = mean(values);
        values.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / values.len() as f32
    };
    assert!(variance(&out.pixels) > variance(&pixels));
}

#[test]
fn saturation_keeps_a_gray_gray() {
    let img = flat(16, 16, 0.4);
    let out = apply_saturation(&img, 1.6, 0.5);
    assert!(max_abs(&out.pixels, &img.pixels) < 1e-4);
}

#[test]
fn auto_masking_is_an_offset_on_the_measured_grain() {
    let mut config = LabConfig::default();
    assert_eq!(auto_masking(&flat(8, 8, 0.5), &config), None);
    config.sharpen_auto_masking = true;
    let auto = auto_masking(&gradient(64, 64), &config).expect("a measured grain");
    assert!((0.0..=1.0).contains(&auto));
    assert_eq!(effective_masking(&config, None), 0.1);
    assert!((effective_masking(&config, Some(0.2)) - 0.2).abs() < 1e-6);
}

#[test]
fn glow_and_halation_keep_a_flat_area_flat() {
    let img = flat(64, 64, 0.5);
    let out = apply_glow_and_halation(&img, 0.4, 0.3, false);
    assert!(max_abs(&out.pixels, &img.pixels) < 1e-5);
}

#[test]
fn the_processor_runs_the_python_stage_order() {
    let config = LabConfig {
        sharpen: 0.5,
        sharpen_method: SharpenMethod::Usm,
        glow_amount: 0.2,
        ..LabConfig::default()
    };
    let result = process(
        &gradient(96, 64),
        &config,
        false,
        LabContext { scale_factor: 1.0 },
    );
    assert!(result.auto_masking.is_none());
    assert_eq!(result.image.pixels.len(), 96 * 64 * 3);
    assert!(result.image.pixels.iter().all(|v| (0.0..=1.0).contains(v)));
}
