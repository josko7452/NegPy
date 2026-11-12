//! Ports of the observable rules in `tests/test_finish_logic.py` and `test_finish_processor.py`.

use negpy_backend::domain::models::FinishConfig;
use negpy_backend::features::finish::logic::{
    CARRIER_SAMPLES, apply_carrier, apply_vignette, carrier_profiles, carrier_tone_exposures,
    linear_carrier_tone,
};
use negpy_backend::features::finish::processor::{FinishContext, carrier_width_px, process};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn flat(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

fn pixel(img: &ImageBuffer, x: usize, y: usize) -> [f32; 3] {
    let i = (y * img.width + x) * 3;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
}

#[test]
fn the_carrier_profiles_are_the_python_table() {
    let profiles = carrier_profiles();
    assert_eq!(profiles.len(), 8 * CARRIER_SAMPLES);
    assert!(profiles.iter().all(|v| (-1.0..=1.0).contains(v)));
}

#[test]
fn the_tone_table_runs_from_bare_paper_to_black() {
    let tone = linear_carrier_tone();
    assert_eq!(tone[0], [1.0; 3]);
    assert_eq!(tone[63], [0.0; 3]);
    assert!(tone.windows(2).all(|pair| pair[1][0] <= pair[0][0]));
    let exposures = carrier_tone_exposures();
    assert_eq!(exposures[0], 0.0);
    assert_eq!(exposures[63], 1.0);
}

#[test]
fn a_vignette_at_zero_stops_is_an_exact_passthrough() {
    let img = flat(32, 24, 0.5);
    assert_eq!(apply_vignette(&img, 0.0, 0.5, 0.0).pixels, img.pixels);
}

#[test]
fn a_burn_darkens_the_corner_more_than_the_center() {
    let img = flat(100, 100, 0.5);
    let out = apply_vignette(&img, 1.0, 0.5, 0.0);
    assert!(pixel(&out, 0, 0)[0] < pixel(&out, 50, 50)[0]);
    assert!((pixel(&out, 0, 0)[0] - 0.25).abs() < 1e-4);
}

#[test]
fn a_dodge_brightens_the_corners() {
    let img = flat(100, 100, 0.5);
    let out = apply_vignette(&img, -1.0, 0.5, 0.0);
    assert!(pixel(&out, 0, 0)[0] > pixel(&out, 50, 50)[0]);
}

#[test]
fn a_zero_width_carrier_is_the_original() {
    let img = flat(40, 30, 0.5);
    let out = apply_carrier(&img, 0.0, 0.25, 0.5, 0.25, [1.0; 3], None);
    assert_eq!(out.pixels, img.pixels);
}

#[test]
fn the_carrier_margin_takes_the_paper_colour() {
    let img = flat(400, 300, 0.5);
    let out = apply_carrier(&img, 16.0, 0.0, 0.0, 0.0, [0.2, 0.4, 0.6], None);
    assert_eq!(pixel(&out, 200, 0), [0.2, 0.4, 0.6]);
    assert_eq!(pixel(&out, 200, 150), [0.5, 0.5, 0.5]);
}

#[test]
fn the_carrier_width_scales_with_the_print_long_edge() {
    assert_eq!(carrier_width_px(1.0, 30.0, 300.0), 1.0);
    assert_eq!(carrier_width_px(1.0, 30.0, 600.0), 2.0);
}

#[test]
fn the_processor_skips_the_carrier_on_a_full_crop_preview() {
    let img = flat(80, 60, 0.5);
    let mut config = FinishConfig::default();
    config.carrier_width = 1.0;
    let full = process(
        &img,
        &config,
        FinishContext {
            print_size_cm: 30.0,
            paper: [1.0; 3],
            crop_preview_full: true,
        },
        None,
    );
    assert_eq!(full.pixels, img.pixels);
    let framed = process(
        &img,
        &config,
        FinishContext {
            print_size_cm: 30.0,
            paper: [1.0; 3],
            crop_preview_full: false,
        },
        None,
    );
    assert_ne!(framed.pixels, img.pixels);
}
