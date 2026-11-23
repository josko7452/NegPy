//! The bounded RAW preview: half-resolution binning, no full demosaic.

use negpy_backend::infrastructure::loaders::factory::{LoadOptions, load, load_binned_preview};
use std::path::Path;

fn golden() -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from("/home/bruno/WORK/svetovy_vylet/skeny/vv9/P5080635.ORF");
    path.is_file().then_some(path)
}

#[test]
fn a_binned_preview_is_half_the_sensor_size() {
    let Some(path) = golden() else {
        return;
    };
    let binned = load_binned_preview(&path)
        .expect("a binned preview")
        .image
        .expect("pixels");
    let full = load(
        &path,
        LoadOptions {
            linear_raw: false,
            positive_decode: None,
            load_ir: false,
        },
    )
    .expect("a full decode")
    .image
    .expect("pixels");
    assert_eq!(binned.width * 2, full.width);
    assert_eq!(binned.height * 2, full.height);
    assert!(
        binned
            .pixels
            .iter()
            .all(|value| (0.0..=1.0).contains(value))
    );
}

#[test]
fn a_finished_image_has_no_binned_preview() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl_rgb8.jxl");
    assert!(load_binned_preview(&path).is_none());
}
