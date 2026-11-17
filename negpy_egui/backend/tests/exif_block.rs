//! Ports `read_exif_from_file` observations from `tests/test_flat_export_capture_date.py`
//! and `tests/test_jxl_loader.py`.

use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::writer::{ExifData, dump_exif};
use negpy_backend::infrastructure::loaders::exif_block::{parse_exif_block, read_exif_file};

#[test]
fn a_jpeg_keeps_its_capture_date_and_camera() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("camera.jpg");
    let mut data = ExifData::default();
    data.zeroth.insert(271, ExifValue::Text("Canon".into()));
    data.zeroth.insert(272, ExifValue::Text("EOS R5".into()));
    data.exif
        .insert(36867, ExifValue::Text("2024:05:12 14:30:00".into()));
    let block = dump_exif(&data);
    let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
    let length = (block.len() + 8) as u16;
    jpeg.extend_from_slice(&length.to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&block);
    jpeg.extend_from_slice(&[0xff, 0xd9]);
    std::fs::write(&path, jpeg).unwrap();

    let exif = read_exif_file(&path).expect("a readable JPEG");
    assert_eq!(
        exif.exif.get(&36867),
        Some(&ExifValue::Text("2024:05:12 14:30:00".into()))
    );
    assert_eq!(
        exif.zeroth.get(&271),
        Some(&ExifValue::Text("Canon".into()))
    );
}

#[test]
fn a_bare_tiff_keeps_its_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tiff");
    let mut data = ExifData::default();
    data.zeroth.insert(271, ExifValue::Text("Plustek".into()));
    std::fs::write(&path, dump_exif(&data)).unwrap();
    let exif = read_exif_file(&path).expect("a readable TIFF");
    assert_eq!(
        exif.zeroth.get(&271),
        Some(&ExifValue::Text("Plustek".into()))
    );
}

#[test]
fn the_cache_follows_a_rewritten_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.tiff");
    let mut first = ExifData::default();
    first.zeroth.insert(271, ExifValue::Text("one".into()));
    std::fs::write(&path, dump_exif(&first)).unwrap();
    assert_eq!(
        read_exif_file(&path).unwrap().zeroth.get(&271),
        Some(&ExifValue::Text("one".into()))
    );

    let mut second = ExifData::default();
    second.zeroth.insert(271, ExifValue::Text("two".into()));
    std::fs::write(&path, dump_exif(&second)).unwrap();
    assert_eq!(
        read_exif_file(&path).unwrap().zeroth.get(&271),
        Some(&ExifValue::Text("two".into()))
    );
}

#[test]
fn a_pointer_is_not_left_in_the_zero_directory() {
    let mut data = ExifData::default();
    data.exif.insert(33437, ExifValue::Rational(28, 10));
    let raw = parse_exif_block(&dump_exif(&data)).unwrap();
    assert_eq!(raw.exif.get(&33437), Some(&ExifValue::Rational(28, 10)));
    assert!(!raw.zeroth.contains_key(&34665));
}
