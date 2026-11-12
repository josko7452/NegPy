//! Ports of the loader-selection rows in tests/test_source_encoding.py.

use negpy_backend::domain::models::SourceEncoding;
use negpy_backend::infrastructure::loaders::factory::{LoadOptions, is_camera_raw, load};
use tiff::encoder::TiffEncoder;
use tiff::encoder::colortype::RGB16;

#[test]
fn a_tiff_loads_through_the_factory() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let mut encoder = TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
    encoder
        .write_image::<RGB16>(4, 2, &vec![32768; 4 * 2 * 3])
        .unwrap();
    let loaded = load(&path, LoadOptions::new()).unwrap();
    let image = loaded.image.unwrap();
    assert_eq!((image.width, image.height), (4, 2));
    assert_eq!(image.source_u16.unwrap()[0], 32768);
}

#[test]
fn a_positive_tiff_uses_the_requested_encoding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let mut encoder = TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
    encoder
        .write_image::<RGB16>(1, 1, &[16384, 16384, 16384])
        .unwrap();
    let options = LoadOptions {
        positive_decode: Some(SourceEncoding::Gamma22),
        ..LoadOptions::new()
    };
    let loaded = load(&path, options).unwrap();
    assert_eq!(loaded.source_encoding.as_deref(), Some("gamma22"));
}

#[test]
fn a_missing_file_reports_its_path() {
    let error = load(
        std::path::Path::new("/missing/scan.tif"),
        LoadOptions::new(),
    )
    .unwrap_err();
    assert!(error.contains("scan.tif"), "{error}");
    assert!(!is_camera_raw(std::path::Path::new("scan.tif")));
}
