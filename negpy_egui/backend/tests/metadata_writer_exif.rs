//! Custom EXIF tags built from the resolved metadata payload.

use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::features::metadata::capture::parse_capture_date;
use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::payload::build_metadata_payload;
use negpy_backend::features::metadata::writer::{
    TAG_DATE_TIME_ORIGINAL, TAG_EXPOSURE_BIAS, TAG_EXPOSURE_TIME, TAG_F_NUMBER, TAG_GPS_LATITUDE,
    TAG_GPS_LATITUDE_REF, TAG_ISO, TAG_MAX_APERTURE, TAG_OFFSET_TIME_ORIGINAL, TAG_USER_COMMENT,
    build_custom_exif, exif_ascii, parse_exposure,
};

#[test]
fn a_capture_date_keeps_its_offset_and_exif_form() {
    let date = parse_capture_date("1998-07-14 16:30+02:00").unwrap();
    assert_eq!(date.exif_text(), "1998:07:14 16:30:00");
    assert_eq!(date.tz_offset, "+02:00");
    assert_eq!(
        parse_capture_date("1998-07-14 16:30Z").unwrap().tz_offset,
        "+00:00"
    );
}

#[test]
fn the_custom_exif_carries_gear_and_development_notes() {
    let mut config = MetadataConfig::default();
    config.camera_make = "Olympus".into();
    config.camera_model = "PEN F".into();
    config.lens_model = "Zuiko 38".into();
    config.focal_length_mm = Some(38.0);
    config.max_aperture = Some(1.8);
    config.film_iso = Some(400);
    config.film = "HP5 Plus".into();
    config.developer = "ID-11".into();
    config.process_dilution = "1+1".into();
    let payload = build_metadata_payload(&config, None, None);
    let data = build_custom_exif(&payload);
    assert_eq!(
        data.zeroth[&270],
        ExifValue::Bytes(b"Olympus PEN F - Zuiko 38 - HP5 Plus - ISO 400".to_vec())
    );
    assert!(matches!(
        data.exif[&TAG_MAX_APERTURE],
        ExifValue::Rational(_, _)
    ));
    assert!(matches!(data.exif[&TAG_USER_COMMENT], ExifValue::Bytes(_)));
}

#[test]
fn a_negative_bias_and_fractional_shutter_survive_the_write() {
    let tags = parse_exposure("1/2.5s f/8 -2/3ev");
    assert!(tags.contains(&(TAG_EXPOSURE_TIME, ExifValue::Rational(2, 5))));
    assert!(tags.contains(&(TAG_F_NUMBER, ExifValue::Rational(8, 1))));
    assert!(tags.contains(&(TAG_EXPOSURE_BIAS, ExifValue::Rational(-2, 3))));
}

#[test]
fn gps_and_the_capture_timestamp_are_emitted() {
    let mut config = MetadataConfig::default();
    config.gps_latitude = Some(48.5);
    config.gps_longitude = Some(-17.25);
    config.capture_date = "1998-07-14 16:30+02:00".into();
    let payload = build_metadata_payload(&config, None, None);
    let data = build_custom_exif(&payload);
    assert!(matches!(data.gps[&TAG_GPS_LATITUDE_REF], ExifValue::Text(ref text) if text == "N"));
    assert!(matches!(
        data.gps[&TAG_GPS_LATITUDE],
        ExifValue::Rationals(_)
    ));
    assert!(matches!(
        data.exif[&TAG_DATE_TIME_ORIGINAL],
        ExifValue::Bytes(_)
    ));
    assert_eq!(
        data.exif[&TAG_OFFSET_TIME_ORIGINAL],
        ExifValue::Bytes(b"+02:00".to_vec())
    );
    assert_eq!(exif_ascii("20 °C"), b"20 C");
    assert!(parse_exposure("ISO 400").contains(&(TAG_ISO, ExifValue::UInt(400))));
}
