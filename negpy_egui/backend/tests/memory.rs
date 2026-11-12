//! Ports of the preview memory estimate rows behind
//! tests/test_thumbnail_refresh_worker.py.

use std::path::Path;

use negpy_backend::infrastructure::loaders::memory::estimate_preview_memory;

fn write_png(path: &Path, width: u32, height: u32) {
    let image = image::RgbImage::new(width, height);
    image.save(path).unwrap();
}

#[test]
fn a_display_preview_uses_the_header_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.png");
    write_png(&path, 800, 400);

    let estimate = estimate_preview_memory(&path, 200, "display");

    assert_eq!(estimate.source_dimensions, Some((800, 400)));
    assert_eq!(estimate.cached_bytes, 200 * 100 * 3 * 4 * 2);
    assert_eq!(estimate.temporary_bytes, 64 * 1024 * 1024);
}

#[test]
fn a_scan_preview_keeps_a_source_and_ir_plane() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.png");
    write_png(&path, 800, 400);

    let estimate = estimate_preview_memory(&path, 200, "scan");

    assert_eq!(estimate.source_dimensions, Some((800, 400)));
    assert_eq!(
        estimate.cached_bytes,
        200 * 100 * 3 * 4 * 2 + 200 * 100 * 4 + 800 * 400 * 4
    );
    assert_eq!(estimate.temporary_bytes, 256 * 1024 * 1024);
}

#[test]
fn a_file_without_dimensions_uses_the_profile_floor() {
    let estimate = estimate_preview_memory(Path::new("/missing.dng"), 100, "raw");

    assert_eq!(estimate.source_dimensions, None);
    assert_eq!(
        estimate.cached_bytes,
        100 * 100 * 3 * 4 * 2 + 100 * 100 * 4 + 512 * 1024 * 1024
    );
    assert_eq!(estimate.temporary_bytes, 512 * 1024 * 1024);
}
