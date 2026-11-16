//! Ports the cache-key, half-slice and cancellation cases for the preview manager.

use std::path::Path;

use negpy_backend::domain::models::LensCorrections;
use negpy_backend::domain::types::AppConfig;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::assets::half_frame::HalfSlice;
use negpy_backend::services::rendering::preview_manager::{
    PreviewManager, PreviewRequest, decode_for_detection,
};

fn config() -> AppConfig {
    AppConfig {
        preview_render_size: 64,
        preview_cache_max_entries: 4,
        preview_cache_max_bytes: 4096,
        ..AppConfig::with_defaults()
    }
}

#[test]
fn the_cache_key_folds_every_preview_field() {
    let base = PreviewRequest::new("frame", "Adobe RGB");
    let key = base.cache_key().expect("key");
    assert_eq!(key.file_hash, "frame");
    assert_eq!(key.workspace_color_space, "Adobe RGB");
    assert_eq!(key.split_x, 0.5);
    assert_eq!(key.split_axis, "x");

    let mut changed = base.clone();
    changed.full_resolution = true;
    changed.demosaic = "DCB".to_string();
    changed.half_slice = Some(HalfSlice {
        half: 1,
        split_x: 0.4,
        crop_rect: Some([0.1, 0.1, 0.9, 0.9]),
        gutter_thickness: 0.01,
        split_axis: 'y',
    });
    changed.lens_corrections = LensCorrections {
        distortion: true,
        ca: true,
    };
    let changed_key = changed.cache_key().expect("key");
    assert_ne!(key, changed_key);
    assert_eq!(changed_key.half, 1);
    assert_eq!(changed_key.split_axis, "y");
    assert!(!changed_key.lens_token.is_empty());
}

#[test]
fn a_request_without_a_hash_has_no_cache_key() {
    let mut request = PreviewRequest::new("", "Adobe RGB");
    assert!(request.cache_key().is_none());
    request.file_hash = None;
    assert!(request.cache_key().is_none());
}

#[test]
fn a_cancelled_prefetch_does_not_decode() {
    let manager = PreviewManager::new(config());
    let request = PreviewRequest::new("frame", "Adobe RGB");
    let error = manager
        .prefetch(
            Path::new("/missing.tif"),
            &request,
            &[],
            false,
            Some(&|| true),
        )
        .expect_err("cancelled");
    assert!(error.contains("cancelled"));
}

#[test]
fn a_detection_decode_of_a_missing_file_is_none() {
    assert!(decode_for_detection(Path::new("/missing.tif"), 64).is_none());
}

#[test]
fn the_manager_caches_a_loaded_preview() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.tif");
    let mut encoder =
        tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
    encoder
        .write_image::<tiff::encoder::colortype::RGB16>(3, 2, &vec![16384; 3 * 2 * 3])
        .unwrap();
    let manager = PreviewManager::new(config());
    let request = PreviewRequest::new("frame", "sRGB");
    let loaded = manager
        .load(&path, &request, &[], false, None)
        .expect("load");
    assert_eq!(loaded.1, (2, 3));
    let hit = manager.peek(&request).expect("cache hit");
    assert_eq!(hit.0.width, 3);
    assert_eq!(hit.0.height, 2);
}

#[test]
fn a_half_slice_resizes_both_pixels_and_dimensions() {
    let manager = PreviewManager::new(config());
    let image = ImageBuffer::from_rgb(4, 2, vec![0.5; 24]);
    let (sliced, dims) = negpy_backend::services::rendering::preview_manager::apply_half_for_test(
        &image,
        Some(HalfSlice {
            half: 1,
            split_x: 0.5,
            crop_rect: None,
            gutter_thickness: 0.0,
            split_axis: 'x',
        }),
    );
    assert_eq!(dims, (2, 2));
    assert_eq!((sliced.width, sliced.height), (2, 2));
    assert!(
        manager
            .peek(&PreviewRequest::new("none", "Adobe RGB"))
            .is_none()
    );
}
