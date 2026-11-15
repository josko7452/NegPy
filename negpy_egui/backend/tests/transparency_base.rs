//! Ports the slide base rules in `tests/test_transparency_base.py` and
//! `tests/test_engine.py`, plus the crossover tag that names its meter area.

use negpy_backend::domain::models::{ProcessConfig, WorkspaceConfig};
use negpy_backend::features::process::fade::{crossover_space, fade_density};
use negpy_backend::features::transparency::base::{
    TransferBaseSettings, process, transfer_assumed_anchor,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn gradient(width: usize, height: usize) -> ImageBuffer {
    let count = width * height;
    let pixels = (0..count)
        .flat_map(|index| {
            let value = 0.02 + 0.9 * index as f32 / count as f32;
            [value, value * 0.92, value * 0.84]
        })
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_slide_base_keeps_the_frame_size_and_normalizes() {
    let image = gradient(64, 48);
    let config = WorkspaceConfig::default();
    let result = process(
        &image,
        &config.process,
        negpy_backend::domain::models::ProcessMode::E6,
        TransferBaseSettings::default(),
    );
    assert_eq!((result.image.width, result.image.height), (64, 48));
    assert_eq!(result.image.pixels.len(), 64 * 48 * 3);
    assert!(result.image.pixels.iter().all(|value| value.is_finite()));
    assert!(result.metrics.norm_density_range > 2.9 && result.metrics.norm_density_range < 3.1);
}

#[test]
fn the_fixed_window_does_not_follow_the_frame_brightness() {
    let dark = ImageBuffer::from_rgb(8, 8, vec![0.1; 8 * 8 * 3]);
    let bright = ImageBuffer::from_rgb(8, 8, vec![0.9; 8 * 8 * 3]);
    let config = WorkspaceConfig::default();
    let mode = negpy_backend::domain::models::ProcessMode::E6;
    let a = process(
        &dark,
        &config.process,
        mode,
        TransferBaseSettings::default(),
    );
    let b = process(
        &bright,
        &config.process,
        mode,
        TransferBaseSettings::default(),
    );
    assert_eq!(a.metrics.bounds, b.metrics.bounds);
    // The brighter capture prints the denser step of the fixed window, as Python does.
    assert!((a.image.pixels[0] - 0.333_333_34).abs() < 1e-5);
    assert!((b.image.pixels[0] - 0.015_252_501).abs() < 1e-5);
}

#[test]
fn the_slide_anchor_is_the_pivot_fraction() {
    assert!((transfer_assumed_anchor() - 0.25).abs() < 1e-9);
}

#[test]
fn the_crossover_tag_names_the_meter_area() {
    let config = ProcessConfig::default();
    let space = String::new();
    let density = fade_density(&config, &space);
    let tag = crossover_space(&density, &space, &config);
    // The Python app's own tag for the shipped defaults.
    assert_eq!(tag, "66cb8af6efef");

    let mut other = config.clone();
    other.analysis_buffer += 0.05;
    let changed = crossover_space(&density, &space, &other);
    assert_ne!(tag, changed);

    let mut rect = config.clone();
    rect.analysis_rect = Some(vec![0.1, 0.1, 0.5, 0.5]);
    assert_ne!(tag, crossover_space(&density, &space, &rect));
}
