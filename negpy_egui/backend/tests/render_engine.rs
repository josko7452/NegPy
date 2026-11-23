//! Ports the checkpoint contract of tests/test_engine_stage_keys.py and the stage order of
//! tests/test_engine_order.py: a cached stage reuses its result, and an exposure edit leaves
//! the base alone.

use negpy_backend::domain::models::{LocalMask, MaskKey, MaskShape, ProcessMode, WorkspaceConfig};
use negpy_backend::features::exposure::auto_metrics::AutoSliderMetrics;
use negpy_backend::features::exposure::placement::{key_edges, predicted_zone};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::render_engine::{DarkroomEngine, PipelineContext};

fn negative(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let scene = (0.05 + 0.8 * (y as f32 / height as f32)).clamp(1e-4, 1.0);
            let texture = 1.0 + 0.1 * (x as f32 / 4.0).sin();
            let value = (0.05 + 0.8 * (1.0 - (scene * texture).powf(0.4))).clamp(1e-4, 1.0);
            pixels.extend_from_slice(&[value, value * 0.9, value * 0.8]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn settings() -> WorkspaceConfig {
    let mut config = WorkspaceConfig::default();
    config.process.process_mode = ProcessMode::C41;
    config.exposure.auto_exposure = false;
    config
}

#[test]
fn one_render_fills_every_checkpoint() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    let config = settings();
    engine.process(&image, &config, "source-a", &mut context);
    assert!(engine.cache.base.is_some());
    assert!(engine.cache.exposure.is_some());
    assert!(engine.cache.clahe.is_some());
    assert!(engine.cache.lab.is_some());
}

#[test]
fn an_exposure_edit_keeps_the_base_checkpoint() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    let mut config = settings();
    engine.process(&image, &config, "source-a", &mut context);
    let base_hash = engine.cache.base.as_ref().unwrap().config_hash.clone();
    config.exposure.grade += 0.25;
    let mut context = PipelineContext::new();
    engine.process(&image, &config, "source-a", &mut context);
    assert_eq!(engine.cache.base.as_ref().unwrap().config_hash, base_hash);
}

#[test]
fn the_source_hash_clears_every_checkpoint() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    let config = settings();
    engine.process(&image, &config, "source-a", &mut context);
    let mut context = PipelineContext::new();
    engine.process(&image, &config, "source-b", &mut context);
    assert_eq!(engine.cache.source_hash, "source-b");
}

#[test]
fn a_flat_master_ends_before_the_tail_stages() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    let mut config = settings();
    config.exposure.render_intent = negpy_backend::domain::models::RenderIntent::Flat;
    let out = engine.process(&image, &config, "source-a", &mut context);
    assert!(engine.cache.clahe.is_none());
    assert_eq!(out.width, image.width);
    assert_eq!(out.height, image.height);
}

#[test]
fn an_uncached_render_leaves_no_checkpoint() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    context.cache_stages = false;
    let config = settings();
    engine.process(&image, &config, "source-a", &mut context);
    assert!(engine.cache.base.is_none());
}

#[test]
fn an_uncached_render_reports_its_metrics_and_crop() {
    let image = negative(80, 60);
    let mut engine = DarkroomEngine::new();
    let mut context = PipelineContext::new();
    context.cache_stages = false;
    let mut config = settings();
    config.geometry.crop_rect = Some(vec![0.25, 0.25, 0.75, 0.75]);
    let out = engine.process(&image, &config, "source-a", &mut context);
    assert!(context.metrics.contains_key("final_bounds"));
    assert!(context.active_roi.is_some());
    assert!(out.width < image.width);
    assert!(out.height < image.height);
}

fn textured_negative(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let key = (-3.2 * (y as f32 / height as f32)).exp();
            let texture = 1.0 + 0.16 * (x as f32 / 3.0).sin() * (y as f32 / 2.5).sin();
            let scene = (key * texture).max(1e-5);
            let value = (0.03 + 0.85 * (1.0 - scene.powf(0.35))).clamp(1e-4, 1.0);
            pixels.extend_from_slice(&[value, value, value]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn the_contrast_mask_reaches_the_print() {
    let image = textured_negative(120, 80);
    let base = settings();
    let mut masked = base.clone();
    masked.exposure.contrast_mask_strength = 0.8;
    let plain =
        DarkroomEngine::new().process(&image, &base, "source-a", &mut PipelineContext::new());
    let with_mask =
        DarkroomEngine::new().process(&image, &masked, "source-a", &mut PipelineContext::new());
    assert_ne!(plain.pixels, with_mask.pixels);
}

#[test]
fn diffusion_reaches_the_print() {
    let image = textured_negative(120, 80);
    let mut diffused = settings();
    diffused.exposure.diffusion = 0.7;
    let plain =
        DarkroomEngine::new().process(&image, &settings(), "source-a", &mut PipelineContext::new());
    let with_diffusion =
        DarkroomEngine::new().process(&image, &diffused, "source-a", &mut PipelineContext::new());
    assert_ne!(plain.pixels, with_diffusion.pixels);
}

#[test]
fn a_stored_local_mask_reaches_the_print() {
    let image = textured_negative(120, 80);
    let plain =
        DarkroomEngine::new().process(&image, &settings(), "source-a", &mut PipelineContext::new());
    let mut masked = settings();
    masked.local.masks.push(LocalMask {
        vertices: vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]],
        stops: 0.8,
        feather: 0.0,
        shape: MaskShape::Polygon,
        ..LocalMask::default()
    });
    let with_mask =
        DarkroomEngine::new().process(&image, &masked, "source-a", &mut PipelineContext::new());
    assert_ne!(plain.pixels, with_mask.pixels);
}

#[test]
fn a_local_grade_changes_the_print() {
    let image = textured_negative(120, 80);
    let plain =
        DarkroomEngine::new().process(&image, &settings(), "source-a", &mut PipelineContext::new());
    let mut graded = settings();
    graded.local.masks.push(LocalMask {
        vertices: vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]],
        grade: -30.0,
        feather: 0.0,
        shape: MaskShape::Polygon,
        ..LocalMask::default()
    });
    let with_grade =
        DarkroomEngine::new().process(&image, &graded, "source-a", &mut PipelineContext::new());
    assert_ne!(plain.pixels, with_grade.pixels);
}

#[test]
fn a_tone_limited_mask_burns_only_the_tones_it_selects() {
    let image = textured_negative(120, 80);
    let plain =
        DarkroomEngine::new().process(&image, &settings(), "source-a", &mut PipelineContext::new());
    let mut keyed = settings();
    keyed.local.masks.push(LocalMask {
        vertices: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        stops: 0.8,
        feather: 0.0,
        shape: MaskShape::Polygon,
        key: MaskKey::Shadows,
        key_zone: 4.0,
        key_softness: 1.0,
        ..LocalMask::default()
    });
    let with_key =
        DarkroomEngine::new().process(&image, &keyed, "source-a", &mut PipelineContext::new());
    assert_ne!(plain.pixels, with_key.pixels);
    let darker = with_key
        .pixels
        .chunks_exact(3)
        .zip(plain.pixels.chunks_exact(3))
        .filter(|(keyed, plain)| keyed[1] < plain[1] - 1e-4)
        .count();
    let untouched = with_key
        .pixels
        .chunks_exact(3)
        .zip(plain.pixels.chunks_exact(3))
        .filter(|(keyed, plain)| keyed[1] == plain[1])
        .count();
    assert!(darker > 0);
    assert!(untouched > 0);
}

#[test]
fn the_key_edges_land_on_the_zones_either_side() {
    let metrics = AutoSliderMetrics::default();
    let exposure = negpy_backend::domain::models::ExposureConfig {
        auto_exposure: false,
        auto_normalize_contrast: false,
        ..Default::default()
    };
    let mask = LocalMask {
        key: MaskKey::Highlights,
        key_zone: 6.0,
        key_softness: 1.0,
        ..LocalMask::default()
    };
    let (darker, lighter) = key_edges(&mask, &exposure, None, &metrics);
    assert!((predicted_zone(&exposure, &metrics, darker) - 5.5).abs() < 0.02);
    assert!((predicted_zone(&exposure, &metrics, lighter) - 6.5).abs() < 0.02);
}

#[test]
fn a_shadow_key_swaps_the_highlight_edges() {
    let metrics = AutoSliderMetrics::default();
    let exposure = negpy_backend::domain::models::ExposureConfig {
        auto_exposure: false,
        auto_normalize_contrast: false,
        ..Default::default()
    };
    let highlight = LocalMask {
        key: MaskKey::Highlights,
        key_zone: 4.0,
        key_softness: 2.0,
        ..LocalMask::default()
    };
    let shadow = LocalMask {
        key: MaskKey::Shadows,
        ..highlight.clone()
    };
    let (hi0, hi1) = key_edges(&highlight, &exposure, None, &metrics);
    let (sh0, sh1) = key_edges(&shadow, &exposure, None, &metrics);
    assert!((hi0 - sh1).abs() < 1e-9);
    assert!((hi1 - sh0).abs() < 1e-9);
}
