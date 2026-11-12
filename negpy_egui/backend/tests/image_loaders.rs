//! Ports of tests/test_source_encoding.py's TIFF and JPEG decode rows.

use std::fs::File;

use negpy_backend::domain::models::SourceEncoding;
use negpy_backend::infrastructure::loaders::jpeg_loader;
use negpy_backend::infrastructure::loaders::tiff_loader;
use tiff::encoder::colortype::{RGB16, RGBA8};
use tiff::encoder::{TiffEncoder, TiffKindStandard};
use tiff::tags::{ExtraSamples, Tag};

fn open(path: &std::path::Path) -> TiffEncoder<File, TiffKindStandard> {
    TiffEncoder::new(File::create(path).unwrap()).unwrap()
}

fn adobe_rgb_icc() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../icc/AdobeCompat-v4.icc");
    std::fs::read(path).unwrap()
}

#[test]
fn an_untagged_16_bit_tiff_keeps_its_samples() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let mut encoder = open(&path);
    encoder
        .write_image::<RGB16>(4, 2, &vec![32768; 4 * 2 * 3])
        .unwrap();
    let loaded = tiff_loader::load(&path, false, None, true).unwrap();
    let image = loaded.image.unwrap();
    assert_eq!((image.width, image.height), (4, 2));
    assert_eq!(image.source_u16.as_ref().unwrap()[0], 32768);
    assert!(image.pixels.is_empty());
}

#[test]
fn an_adobe_tiff_decodes_by_its_profile_as_positive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let mut encoder = open(&path);
    let mut image = encoder.new_image::<RGB16>(2, 1).unwrap();
    image
        .encoder()
        .write_tag(Tag::IccProfile, adobe_rgb_icc().as_slice())
        .unwrap();
    image
        .write_data(&[32768, 32768, 32768, 32768, 32768, 32768])
        .unwrap();
    let loaded = tiff_loader::load(&path, true, Some(SourceEncoding::Auto), true).unwrap();
    assert_eq!(loaded.source_encoding.as_deref(), Some("profile"));
    assert_eq!(loaded.color_space.as_deref(), Some("Adobe RGB"));
    let image = loaded.image.unwrap();
    let expected = (32768.0f64 / 65535.0).powf(563.0 / 256.0);
    assert!((image.pixels[0] as f64 - expected).abs() < 1e-3);
}

#[test]
fn a_fourth_sample_is_read_as_ir() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tif");
    let mut encoder = open(&path);
    let mut image = encoder.new_image::<RGBA8>(2, 1).unwrap();
    image.extra_samples(&[ExtraSamples::Unspecified]).unwrap();
    image
        .write_data(&[10, 20, 30, 64, 0, 40, 50, 60, 192, 0])
        .unwrap();
    let loaded = tiff_loader::load(&path, false, None, true).unwrap();
    assert_eq!(loaded.image.unwrap().width, 2);
    assert_eq!(loaded.ir.unwrap().len(), 2);
}

#[test]
fn a_jpeg_decodes_to_srgb_linear() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.jpg");
    let image = image::RgbImage::from_pixel(4, 2, image::Rgb([128, 128, 128]));
    image.save(&path).unwrap();
    let loaded = jpeg_loader::load(&path, None).unwrap();
    assert_eq!(loaded.color_space.as_deref(), Some("sRGB"));
    let image = loaded.image.unwrap();
    assert!(image.pixels[0] < 128.0 / 255.0);
}

#[test]
fn a_positive_jpeg_always_decodes_as_auto() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.jpg");
    let image = image::RgbImage::from_pixel(4, 2, image::Rgb([128, 128, 128]));
    image.save(&path).unwrap();
    let loaded = jpeg_loader::load(&path, Some(SourceEncoding::Linear)).unwrap();
    assert_eq!(loaded.source_encoding.as_deref(), Some("srgb"));
}
