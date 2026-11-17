//! The reduced-page splash cases of `tests/test_preview_manager.py`.

use negpy_backend::services::rendering::splash::try_splash_preview;
use tiff::encoder::{TiffEncoder, colortype::RGB8};
use tiff::tags::Tag;

fn write_tiff(path: &std::path::Path, width: u32, height: u32, value: u8) {
    let mut encoder = TiffEncoder::new(std::fs::File::create(path).unwrap()).unwrap();
    encoder
        .write_image::<RGB8>(width, height, &vec![value; (width * height * 3) as usize])
        .unwrap();
}

#[test]
fn a_single_page_tiff_has_no_reduced_preview() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    write_tiff(&path, 32, 24, 120);
    assert!(try_splash_preview(&path, 1600).is_none());
}

#[test]
fn a_reduced_later_page_is_the_splash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    encoder
        .write_image::<RGB8>(64, 48, &vec![60u8; 64 * 48 * 3])
        .unwrap();
    let mut image = encoder.new_image::<RGB8>(32, 24).unwrap();
    image.encoder().write_tag(Tag::Unknown(254), 1u32).unwrap();
    image.write_data(&vec![200u8; 32 * 24 * 3]).unwrap();
    let splash = try_splash_preview(&path, 1600).expect("splash");
    assert_eq!((splash.image.width, splash.image.height), (32, 24));
    assert_eq!(splash.image.pixels.len(), 32 * 24 * 3);
}

#[test]
fn a_splash_is_capped_to_the_preview_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let file = std::fs::File::create(&path).unwrap();
    let mut encoder = TiffEncoder::new(file).unwrap();
    encoder
        .write_image::<RGB8>(4096, 2048, &vec![90u8; 4096 * 2048 * 3])
        .unwrap();
    let mut image = encoder.new_image::<RGB8>(1280, 640).unwrap();
    image.encoder().write_tag(Tag::Unknown(254), 1u32).unwrap();
    image.write_data(&vec![90u8; 1280 * 640 * 3]).unwrap();
    let splash = try_splash_preview(&path, 640).expect("splash");
    assert_eq!(splash.image.width.max(splash.image.height), 640);
    assert_eq!((splash.image.width, splash.image.height), (640, 320));
}
