//! Ports the white-balance rules of `tests/test_preview_manager.py` and
//! `tests/test_batch_norm_wb.py`: a preview decodes on the file's own multipliers only
//! when the request asks for them.

use std::path::Path;

use negpy_backend::domain::types::AppConfig;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::rendering::preview_manager::{PreviewManager, PreviewRequest};

fn fixture() -> Option<std::path::PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
    path.exists().then_some(path)
}

fn config() -> AppConfig {
    let mut app = AppConfig::with_defaults();
    app.preview_render_size = 256;
    app
}

#[test]
fn a_neutral_request_ignores_the_bake_flag() {
    let mut request = PreviewRequest::new("hash", "sRGB");
    request.bake_camera_wb = false;
    assert!(!request.cache_key().unwrap().bake_camera_wb);
}

#[test]
fn a_bake_flag_is_part_of_the_cache_key() {
    let mut request = PreviewRequest::new("hash", "sRGB");
    request.bake_camera_wb = true;
    assert!(request.cache_key().unwrap().bake_camera_wb);
}

#[test]
fn an_override_is_part_of_the_cache_key() {
    let mut request = PreviewRequest::new("hash", "sRGB");
    request.wb_override = Some([1.2, 1.0, 0.8]);
    assert!(request.cache_key().unwrap().bake_camera_wb);
}

#[test]
fn a_splash_and_linear_load_share_one_open() {
    let Some(path) = fixture() else {
        return;
    };
    let manager = PreviewManager::new(config());
    let request = PreviewRequest {
        full_resolution: false,
        ..PreviewRequest::new("hash", "sRGB")
    };
    let (_splash, linear) = manager.load_splash_and_linear(&path, &request, &[], true, None);
    assert!(linear.is_ok());
}

#[test]
fn a_cached_linear_preview_returns_no_splash() {
    let Some(path) = fixture() else {
        return;
    };
    let manager = PreviewManager::new(config());
    let request = PreviewRequest::new("hash", "sRGB");
    let first = manager.load_splash_and_linear(&path, &request, &[], true, None);
    assert!(first.1.is_ok());
    let second = manager.load_splash_and_linear(&path, &request, &[], true, None);
    assert!(second.0.is_none());
    assert!(second.1.is_ok());
}

#[test]
fn a_baked_white_balance_changes_the_cache_slot() {
    let mut first = PreviewRequest::new("hash", "sRGB");
    let mut second = PreviewRequest::new("hash", "sRGB");
    second.wb_override = Some([1.1, 1.0, 0.9]);
    assert_ne!(
        first.cache_key(),
        second.cache_key(),
        "an override decodes with WB"
    );
    first.bake_camera_wb = true;
    second.wb_override = None;
    second.bake_camera_wb = true;
    assert_eq!(
        first.cache_key(),
        second.cache_key(),
        "the override is a render input, not a cache identity"
    );
}

#[test]
fn a_white_balance_buffer_keeps_its_shape() {
    let image = ImageBuffer::from_rgb(3, 2, vec![0.5; 18]);
    let out = negpy_backend::infrastructure::loaders::camera_wb::apply_camera_wb(
        &image,
        Some([1.5, 1.0, 0.75]),
    );
    assert_eq!((out.width, out.height), (3, 2));
}
