//! Ports the classic-TIFF half of `attach_exif_ifd` from
//! `tests/test_flat_export_capture_date.py`.

use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::writer::{ExifData, attach_exif_ifd, dump_exif};
use negpy_backend::infrastructure::loaders::exif_block::parse_exif_block;
use negpy_backend::services::export::encoders::{TiffCompression, encode_tiff};

fn tiff() -> Vec<u8> {
    let pixels = vec![10u8; 8 * 6 * 3];
    encode_tiff(&pixels, 8, 6, 3, 8, TiffCompression::None, None, None).expect("a baseline TIFF")
}

#[test]
fn a_capture_date_reaches_the_exif_ifd() {
    let mut data = ExifData::default();
    data.zeroth.insert(271, ExifValue::Text("Canon".into()));
    data.exif
        .insert(36867, ExifValue::Text("2024:05:12 14:30:00".into()));
    let attached = attach_exif_ifd(&tiff(), &dump_exif(&data));
    let raw = parse_exif_block(&attached).expect("a readable TIFF");
    assert_eq!(
        raw.exif.get(&36867),
        Some(&ExifValue::Text("2024:05:12 14:30:00".into()))
    );
}

#[test]
fn the_attached_ifd_keeps_the_image_data_readable() {
    let data = ExifData::default();
    let attached = attach_exif_ifd(&tiff(), &dump_exif(&data));
    assert!(attached.starts_with(b"II*\0"));
    let ifd = u32::from_le_bytes(attached[4..8].try_into().unwrap()) as usize;
    assert!(ifd >= 8);
    assert!(ifd + 2 <= attached.len());
}

#[test]
fn a_big_endian_tiff_keeps_its_byte_order() {
    let mut source = Vec::new();
    source.extend_from_slice(b"MM");
    source.extend_from_slice(&42u16.to_be_bytes());
    source.extend_from_slice(&8u32.to_be_bytes());
    source.extend_from_slice(&1u16.to_be_bytes());
    source.extend_from_slice(&256u16.to_be_bytes());
    source.extend_from_slice(&3u16.to_be_bytes());
    source.extend_from_slice(&1u32.to_be_bytes());
    source.extend_from_slice(&8u32.to_be_bytes());
    source.extend_from_slice(&0u32.to_be_bytes());

    let mut data = ExifData::default();
    data.exif.insert(33437, ExifValue::Rational(28, 10));
    let attached = attach_exif_ifd(&source, &dump_exif(&data));
    assert!(attached.starts_with(b"MM"));
    let raw = parse_exif_block(&attached).expect("a readable big-endian TIFF");
    assert_eq!(raw.exif.get(&33437), Some(&ExifValue::Rational(28, 10)));
}

#[test]
fn a_non_tiff_input_is_returned_unchanged() {
    let jpeg = vec![0xff, 0xd8, 0xff, 0xd9];
    assert_eq!(attach_exif_ifd(&jpeg, b"exif"), jpeg);
}

#[test]
fn an_empty_exif_block_adds_nothing() {
    let source = tiff();
    assert_eq!(
        attach_exif_ifd(&source, &dump_exif(&ExifData::default())),
        source
    );
}
