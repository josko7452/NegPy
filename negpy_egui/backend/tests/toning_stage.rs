//! Ports of the observable rules in `tests/test_toning_logic.py`.

use negpy_backend::domain::models::{AltProcess, ToningConfig};
use negpy_backend::features::toning::logic::{
    apply_chemical_toning, apply_split_toning, toning_constants,
};
use negpy_backend::features::toning::processor::process;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn gray(value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(4, 4, vec![value; 4 * 4 * 3])
}

fn ramp() -> ImageBuffer {
    let pixels = (0..128)
        .flat_map(|i| {
            let value = 0.001 + 0.999 * i as f32 / 127.0;
            [value; 3]
        })
        .collect();
    ImageBuffer::from_rgb(128, 1, pixels)
}

fn warmth(img: &ImageBuffer) -> f32 {
    img.pixels.chunks_exact(3).map(|p| p[0] - p[2]).sum::<f32>() / (img.pixels.len() / 3) as f32
}

#[test]
fn zero_strength_keeps_the_chemical_baths_inert() {
    let img = ramp();
    let out = apply_chemical_toning(&img, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, false);
    assert_eq!(out.pixels, img.pixels);
}

#[test]
fn selenium_deepens_shadows_and_sepia_warms_highlights() {
    let dark = apply_chemical_toning(&gray(0.05), 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, false);
    assert!(dark.pixels[0] < 0.05);
    assert!(dark.pixels[1] < dark.pixels[0] && dark.pixels[1] < dark.pixels[2]);

    let light = apply_chemical_toning(&gray(0.6), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, false);
    assert!(light.pixels[0] > 0.6);
    assert!(light.pixels[2] < 0.6);
    assert!(warmth(&light) > 0.01);
}

#[test]
fn gold_over_sepia_pushes_the_warm_hue_further() {
    let sepia = apply_chemical_toning(&gray(0.6), 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, false);
    let both = apply_chemical_toning(&gray(0.6), 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, false);
    assert!(warmth(&both) > warmth(&sepia));
}

#[test]
fn a_full_sepia_leaves_no_silver_for_blue_in_the_highlights() {
    let sepia = apply_chemical_toning(&gray(0.7), 0.0, 1.5, 0.0, 0.0, 0.0, 0.0, false);
    let both = apply_chemical_toning(&gray(0.7), 0.0, 1.5, 0.0, 1.0, 0.0, 0.0, false);
    let max = sepia
        .pixels
        .iter()
        .zip(&both.pixels)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(max < 0.005);
}

#[test]
fn every_bath_saturates_without_a_density_reversal() {
    let out = apply_chemical_toning(&ramp(), 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, false);
    let luma: Vec<f32> = out
        .pixels
        .chunks_exact(3)
        .map(|p| (p[0] + p[1] + p[2]) / 3.0)
        .collect();
    assert!(luma.windows(2).all(|pair| pair[1] >= pair[0] - 1e-4));
}

#[test]
fn lith_overrides_only_change_selenium_and_gold() {
    let plain = toning_constants();
    assert_eq!(plain.selenium.d_ref, 2.0);
    assert!(!plain.gold_flat);
    let image = ramp();
    for kwargs in [
        [0.0, 0.8, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.8, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 0.8, 0.0],
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.8],
    ] {
        let normal = apply_chemical_toning(
            &image, kwargs[0], kwargs[1], kwargs[2], kwargs[3], kwargs[4], kwargs[5], false,
        );
        let lith = apply_chemical_toning(
            &image, kwargs[0], kwargs[1], kwargs[2], kwargs[3], kwargs[4], kwargs[5], true,
        );
        assert_eq!(normal.pixels, lith.pixels);
    }
}

#[test]
fn split_toning_is_inert_at_zero_strength() {
    let img = ramp();
    assert_eq!(
        apply_split_toning(&img, 195.0, 0.0, 30.0, 0.0).pixels,
        img.pixels
    );
}

#[test]
fn the_cyanotype_processor_skips_chemical_toners() {
    let config = ToningConfig {
        selenium_strength: 1.0,
        ..ToningConfig::default()
    };
    let plain = process(
        &ramp(),
        &ToningConfig::default(),
        AltProcess::Cyanotype,
        true,
    );
    let toned = process(&ramp(), &config, AltProcess::Cyanotype, true);
    assert_eq!(plain.pixels, toned.pixels);
    let silver = process(&ramp(), &config, AltProcess::None, true);
    assert_ne!(silver.pixels, plain.pixels);
}

#[test]
fn split_toning_still_applies_to_a_cyanotype() {
    let config = ToningConfig {
        shadow_tint_hue: 0.5,
        shadow_tint_strength: 1.0,
        ..ToningConfig::default()
    };
    let plain = process(
        &ramp(),
        &ToningConfig::default(),
        AltProcess::Cyanotype,
        true,
    );
    let split = process(&ramp(), &config, AltProcess::Cyanotype, true);
    assert_ne!(plain.pixels, split.pixels);
}
