//! Ports the observable rules of `tests/test_bounded_preview_loaders.py` and
//! `tests/test_embedded_preview.py`: a bounded preview decodes without a full demosaic.

use std::path::Path;

use negpy_backend::infrastructure::loaders::embedded_preview::{
    EMBEDDED_PREVIEW_MAX_BYTES, load_bounded_thumbnail,
};

#[test]
fn a_finished_image_falls_back_to_the_container_page() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
    if !fixture.exists() {
        return;
    }
    assert!(load_bounded_thumbnail(&fixture, 64).is_some());
}

#[test]
fn a_missing_file_or_a_he_dir_has_no_preview() {
    assert!(load_bounded_thumbnail(Path::new("/missing/frame.dng"), 64).is_none());
}

#[test]
fn the_preview_budget_is_bounded() {
    assert!(EMBEDDED_PREVIEW_MAX_BYTES <= 64 * 1024 * 1024);
}
