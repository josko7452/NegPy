//! Ports of tests/test_linear_output.py's Coolscan NEF and Flextight FFF rows.

use std::fs::File;

use negpy_backend::infrastructure::loaders::fff_loader::{find_full_res_ifd, is_flextight_fff};
use negpy_backend::infrastructure::loaders::nef_loader::{find_rgb_subifd, is_coolscan_nef};
use negpy_backend::infrastructure::loaders::tiff_walker::read_ifds;
use tiff::encoder::colortype::{Gray16, RGB8, RGB16};
use tiff::encoder::{Ifd, TiffEncoder, TiffKindStandard};
use tiff::tags::Tag;

fn open(path: &std::path::Path) -> TiffEncoder<File, TiffKindStandard> {
    TiffEncoder::new(File::create(path).unwrap()).unwrap()
}

fn coolscan_nef(path: &std::path::Path, channels: u16) {
    let mut encoder = open(path);
    let mut sub = encoder.extra_directory().unwrap();
    sub.write_tag(Tag::ImageWidth, 300u32).unwrap();
    sub.write_tag(Tag::ImageLength, 200u32).unwrap();
    sub.write_tag(Tag::BitsPerSample, 16u16).unwrap();
    sub.write_tag(Tag::SamplesPerPixel, channels).unwrap();
    sub.write_tag(Tag::PhotometricInterpretation, 2u16).unwrap();
    sub.write_tag(Tag::Compression, 1u16).unwrap();
    sub.write_tag(Tag::StripOffsets, 0u32).unwrap();
    sub.write_tag(Tag::RowsPerStrip, 200u32).unwrap();
    sub.write_tag(Tag::StripByteCounts, u32::from(channels) * 300 * 200 * 2)
        .unwrap();
    let sub = sub.finish_with_offsets().unwrap();

    let mut parent = encoder.image_directory().unwrap();
    parent.write_tag(Tag::ImageWidth, 75u32).unwrap();
    parent.write_tag(Tag::ImageLength, 50u32).unwrap();
    parent.write_tag(Tag::BitsPerSample, 8u16).unwrap();
    parent.write_tag(Tag::SamplesPerPixel, 3u16).unwrap();
    parent
        .write_tag(Tag::PhotometricInterpretation, 2u16)
        .unwrap();
    parent.write_tag(Tag::Compression, 1u16).unwrap();
    parent.write_tag(Tag::StripOffsets, 0u32).unwrap();
    parent.write_tag(Tag::RowsPerStrip, 50u32).unwrap();
    parent
        .write_tag(Tag::StripByteCounts, 75 * 50 * 3u32)
        .unwrap();
    parent.write_tag(Tag::SubIfd, Ifd(sub.offset)).unwrap();
    parent.finish().unwrap();
}

#[test]
fn a_coolscan_nef_is_detected_by_its_rgb_subifd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("coolscan.nef");
    coolscan_nef(&path, 3);
    let ifds = read_ifds(&path).unwrap();
    assert_eq!(find_rgb_subifd(&ifds).unwrap().width, 300);
    assert!(is_coolscan_nef(&path));
}

#[test]
fn a_camera_nef_or_an_empty_nef_is_not_a_coolscan() {
    let dir = tempfile::tempdir().unwrap();
    let camera = dir.path().join("camera.nef");
    let mut encoder = open(&camera);
    encoder
        .write_image::<Gray16>(300, 200, &vec![0; 300 * 200])
        .unwrap();
    assert!(!is_coolscan_nef(&camera));

    let other = dir.path().join("photo.tiff");
    let mut encoder = open(&other);
    encoder
        .write_image::<RGB16>(10, 10, &vec![0; 10 * 10 * 3])
        .unwrap();
    assert!(!is_coolscan_nef(&other));
}

#[test]
fn a_flextight_fff_is_detected_by_its_largest_16_bit_rgb_ifd() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.fff");
    let mut encoder = open(&path);
    encoder
        .write_image::<RGB16>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    encoder
        .write_image::<RGB8>(75, 50, &vec![0; 75 * 50 * 3])
        .unwrap();
    let ifds = read_ifds(&path).unwrap();
    assert_eq!(find_full_res_ifd(&ifds).unwrap().width, 600);
    assert!(is_flextight_fff(&path));
}

#[test]
fn an_8_bit_or_non_fff_file_is_not_a_flextight() {
    let dir = tempfile::tempdir().unwrap();
    let small = dir.path().join("small.fff");
    let mut encoder = open(&small);
    encoder
        .write_image::<RGB8>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    assert!(!is_flextight_fff(&small));

    let tiff = dir.path().join("photo.tiff");
    let mut encoder = open(&tiff);
    encoder
        .write_image::<RGB16>(600, 400, &vec![0; 600 * 400 * 3])
        .unwrap();
    assert!(!is_flextight_fff(&tiff));
}
