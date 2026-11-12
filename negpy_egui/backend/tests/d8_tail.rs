//! The D8 tail stage order and its flat-intent bypass.

use negpy_backend::domain::models::{RenderIntent, WorkspaceConfig};
use negpy_backend::exposure_pipeline::{Frame, is_mono, render_tail};
use negpy_backend::features::geometry::processor::GeometryContext;

fn gradient(width: usize, height: usize) -> Frame {
    let count = width * height;
    let pixels = (0..count)
        .flat_map(|i| {
            let value = 0.02 + 0.9 * i as f32 / count as f32;
            [value, value * 0.92, value * 0.84]
        })
        .collect();
    Frame {
        pixels,
        width,
        height,
    }
}

#[test]
fn a_bw_negative_is_mono_and_a_slide_obeys_its_toggle() {
    let mut config = WorkspaceConfig::default();
    assert!(!is_mono(&config));
    config.process.process_mode = negpy_backend::domain::models::ProcessMode::BW;
    assert!(is_mono(&config));
    config.process.process_mode = negpy_backend::domain::models::ProcessMode::E6;
    assert!(!is_mono(&config));
    config.process.slide_bw = true;
    assert!(is_mono(&config));
}

#[test]
fn the_tail_returns_a_frame_of_the_same_size_without_a_crop() {
    let config = WorkspaceConfig::default();
    let out = render_tail(
        &gradient(64, 48),
        &config,
        GeometryContext { scale_factor: 1.0 },
        1600.0,
    );
    assert_eq!((out.width, out.height), (64, 48));
    assert_eq!(out.pixels.len(), 64 * 48 * 3);
}

#[test]
fn the_tail_crops_to_a_manual_rect() {
    let mut config = WorkspaceConfig::default();
    config.geometry.crop_rect = Some(vec![0.25, 0.25, 0.75, 0.75]);
    let out = render_tail(
        &gradient(64, 48),
        &config,
        GeometryContext { scale_factor: 1.0 },
        1600.0,
    );
    assert_eq!((out.width, out.height), (32, 24));
}

#[test]
fn a_flat_intent_bypasses_the_creative_stages() {
    let mut config = WorkspaceConfig::default();
    config.exposure.render_intent = RenderIntent::Flat;
    config.lab.glow_amount = 0.5;
    config.toning.sepia_strength = 1.0;
    let source = gradient(64, 48);
    let out = render_tail(
        &source,
        &config,
        GeometryContext { scale_factor: 1.0 },
        1600.0,
    );
    assert_eq!(out.pixels, source.pixels);
}
