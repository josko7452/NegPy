//! Source EXIF merged with the export's capture metadata.

use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::features::metadata::exif_read::{ExifTags, ExifValue, SourceExif};
use negpy_backend::features::metadata::payload::build_metadata_payload;
use negpy_backend::features::metadata::writer::{
    ExifData, TAG_DATE_TIME_DIGITIZED, TAG_DATE_TIME_ORIGINAL, TAG_EXPOSURE_TIME, TAG_F_NUMBER,
    TAG_GPS_LATITUDE, TAG_ISO, TAG_MAKE, TAG_MODEL, TAG_ORIENTATION, merge_exif,
};

fn source() -> SourceExif {
    let mut zeroth = ExifTags::new();
    zeroth.insert(TAG_MAKE, ExifValue::Text("Plustek".into()));
    zeroth.insert(TAG_MODEL, ExifValue::Text("OpticFilm".into()));
    let mut exif = ExifTags::new();
    exif.insert(TAG_EXPOSURE_TIME, ExifValue::Rational(1, 60));
    exif.insert(TAG_F_NUMBER, ExifValue::Rational(28, 10));
    exif.insert(TAG_ISO, ExifValue::UInt(100));
    exif.insert(
        TAG_DATE_TIME_ORIGINAL,
        ExifValue::Text("2025:01:01 10:00:00".into()),
    );
    let mut gps = ExifTags::new();
    gps.insert(
        TAG_GPS_LATITUDE,
        ExifValue::Rationals(vec![(1, 1), (2, 1), (3, 100)]),
    );
    SourceExif { zeroth, exif, gps }
}

#[test]
fn a_scan_without_capture_gear_keeps_its_rig_fields() {
    let source = source();
    let payload = build_metadata_payload(&MetadataConfig::default(), None, Some(&source));
    let merged = merge_exif(Some(&ExifData::from(&source)), &payload);
    assert_eq!(merged.zeroth[&TAG_MAKE], ExifValue::Text("Plustek".into()));
    assert!(merged.exif.contains_key(&TAG_EXPOSURE_TIME));
    assert_eq!(merged.zeroth[&TAG_ORIENTATION], ExifValue::UInt(1));
}

#[test]
fn capture_gear_replaces_the_scan_rig_fields() {
    let source = source();
    let mut config = MetadataConfig::default();
    config.camera_make = "Olympus".into();
    config.camera_model = "PEN F".into();
    config.exposure_override = "1/125s f/8 ISO 400".into();
    let payload = build_metadata_payload(&config, None, Some(&source));
    let merged = merge_exif(Some(&ExifData::from(&source)), &payload);
    assert_eq!(
        merged.zeroth[&TAG_MAKE],
        ExifValue::Bytes(b"Olympus".to_vec())
    );
    assert_eq!(merged.exif[&TAG_EXPOSURE_TIME], ExifValue::Rational(1, 125));
    assert_eq!(merged.exif[&TAG_F_NUMBER], ExifValue::Rational(8, 1));
    assert_eq!(merged.exif[&TAG_ISO], ExifValue::UInt(400));
}

#[test]
fn a_picked_location_replaces_the_scan_gps_block() {
    let source = source();
    let mut config = MetadataConfig::default();
    config.gps_latitude = Some(48.5);
    config.gps_longitude = Some(17.25);
    let payload = build_metadata_payload(&config, None, Some(&source));
    let merged = merge_exif(Some(&ExifData::from(&source)), &payload);
    assert!(merged.gps.contains_key(&1));
    assert_eq!(
        merged.gps[&2],
        ExifValue::Rationals(vec![(48, 1), (30, 1), (0, 100)])
    );
}

#[test]
fn a_capture_date_becomes_the_original_and_the_scan_date_the_digitized() {
    let source = source();
    let mut config = MetadataConfig::default();
    config.capture_date = "1998-07-14 16:30".into();
    let payload = build_metadata_payload(&config, None, Some(&source));
    let merged = merge_exif(Some(&ExifData::from(&source)), &payload);
    assert_eq!(
        merged.exif[&TAG_DATE_TIME_ORIGINAL],
        ExifValue::Bytes(b"1998:07:14 16:30:00".to_vec())
    );
    assert_eq!(
        merged.exif[&TAG_DATE_TIME_DIGITIZED],
        ExifValue::Text("2025:01:01 10:00:00".into())
    );
}
