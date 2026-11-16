//! Source EXIF cleanup before a rewrite.

use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::writer::{
    ExifData, TAG_COLOR_SPACE, TAG_DATE_TIME_DIGITIZED, TAG_DATE_TIME_ORIGINAL,
    TAG_IMAGE_DESCRIPTION, TAG_MAKER_NOTE, TAG_ORIENTATION, TAG_X_RESOLUTION, demote_scan_datetime,
    prepare_jpeg_exif, sanitize_exif,
};

fn tags(values: &[(u16, ExifValue)]) -> ExifData {
    let mut data = ExifData::default();
    for (tag, value) in values {
        data.zeroth.insert(*tag, value.clone());
    }
    data
}

#[test]
fn raw_preview_pointers_and_printim_are_dropped() {
    let mut data = tags(&[
        (254, ExifValue::UInt(0)),
        (330, ExifValue::UInt(12)),
        (0xC4A5, ExifValue::Bytes(b"PrintIM".to_vec())),
        (TAG_IMAGE_DESCRIPTION, ExifValue::Bytes(b"keep".to_vec())),
    ]);
    data.zeroth
        .insert(0x9C9B, ExifValue::Bytes(b"above gps".to_vec()));
    let clean = sanitize_exif(&data);
    assert!(!clean.zeroth.contains_key(&254));
    assert!(!clean.zeroth.contains_key(&330));
    assert!(!clean.zeroth.contains_key(&0xC4A5));
    assert!(!clean.zeroth.contains_key(&0x9C9B));
    assert_eq!(
        clean.zeroth[&TAG_IMAGE_DESCRIPTION],
        ExifValue::Bytes(b"keep".to_vec())
    );
}

#[test]
fn a_raw_capture_tag_and_a_stale_color_space_do_not_survive() {
    let mut data = ExifData::default();
    data.exif.insert(41730, ExifValue::Bytes(vec![1]));
    data.exif.insert(TAG_COLOR_SPACE, ExifValue::UInt(65535));
    data.exif
        .insert(TAG_MAKER_NOTE, ExifValue::Bytes(vec![1, 2]));
    data.zeroth
        .insert(TAG_X_RESOLUTION, ExifValue::Bytes(vec![0, 1]));
    let clean = sanitize_exif(&data);
    assert!(!clean.exif.contains_key(&41730));
    assert!(!clean.exif.contains_key(&TAG_COLOR_SPACE));
    assert!(!clean.exif.contains_key(&TAG_MAKER_NOTE));
    assert!(!clean.zeroth.contains_key(&TAG_X_RESOLUTION));
}

#[test]
fn a_short_overflow_is_removed() {
    let mut data = ExifData::default();
    data.zeroth.insert(TAG_ORIENTATION, ExifValue::UInt(70000));
    data.zeroth.insert(258, ExifValue::UInt(70000));
    let clean = sanitize_exif(&data);
    assert!(!clean.zeroth.contains_key(&TAG_ORIENTATION));
    assert!(!clean.zeroth.contains_key(&258));
}

#[test]
fn a_gps_ifd_without_coordinates_is_dropped() {
    let mut data = ExifData::default();
    data.gps.insert(0, ExifValue::Bytes(vec![2, 3, 0, 0]));
    assert!(sanitize_exif(&data).gps.is_empty());
}

#[test]
fn a_jpeg_drops_its_thumbnail_and_demotes_the_source_date() {
    let mut data = ExifData::default();
    data.first.insert(TAG_ORIENTATION, ExifValue::UInt(1));
    data.exif.insert(
        TAG_DATE_TIME_ORIGINAL,
        ExifValue::Bytes(b"1998:07:14 16:30:00".to_vec()),
    );
    demote_scan_datetime(&mut data);
    let prepared = prepare_jpeg_exif(&data);
    assert!(prepared.first.is_empty());
    assert_eq!(
        prepared.exif[&TAG_DATE_TIME_DIGITIZED],
        ExifValue::Bytes(b"1998:07:14 16:30:00".to_vec())
    );
}
