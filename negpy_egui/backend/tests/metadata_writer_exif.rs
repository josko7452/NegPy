//! Custom EXIF tags built from the resolved metadata payload.

use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::features::metadata::capture::parse_capture_date;
use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::payload::build_metadata_payload;
use negpy_backend::features::metadata::writer::{
    TAG_DATE_TIME_ORIGINAL, TAG_EXPOSURE_BIAS, TAG_EXPOSURE_TIME, TAG_F_NUMBER, TAG_GPS_LATITUDE,
    TAG_GPS_LATITUDE_REF, TAG_ISO, TAG_MAX_APERTURE, TAG_OFFSET_TIME_ORIGINAL, TAG_USER_COMMENT,
    build_custom_exif, dump_exif, exif_ascii, inject_jpeg_exif, inject_jpeg_xmp, parse_exposure,
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

#[test]
fn the_exif_block_reads_back_with_its_directories() {
    let mut config = MetadataConfig::default();
    config.camera_make = "Olympus".into();
    config.capture_date = "1998-07-14 16:30".into();
    let payload = build_metadata_payload(&config, None, None);
    let bytes = dump_exif(&build_custom_exif(&payload));
    assert_eq!(&bytes[..8], b"II*\0\x08\0\0\0");
    let count = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let mut tags = Vec::new();
    for index in 0..count {
        let at = 10 + index * 12;
        tags.push(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    }
    assert!(tags.contains(&271));
    assert!(tags.windows(2).all(|pair| pair[0] < pair[1]));
    let exif_at = tags.iter().position(|tag| *tag == 34665).map(|index| {
        let at = 10 + index * 12 + 8;
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
    });
    assert!(exif_at.is_some_and(|at| at < bytes.len()));
    let exif_at = exif_at.unwrap();
    let exif_count = u16::from_le_bytes([bytes[exif_at], bytes[exif_at + 1]]) as usize;
    let exif_tags: Vec<u16> = (0..exif_count)
        .map(|index| {
            let at = exif_at + 2 + index * 12;
            u16::from_le_bytes([bytes[at], bytes[at + 1]])
        })
        .collect();
    assert!(exif_tags.contains(&36867));
}

#[test]
fn a_jpeg_gains_one_exif_segment() {
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02, 0xFF, 0xD9];
    let exif = b"II*\0payload";
    let out = inject_jpeg_exif(&jpeg, exif);
    assert_eq!(out[..2], [0xFF, 0xD8]);
    assert_eq!(out[2..4], [0xFF, 0xE1]);
    let again = inject_jpeg_exif(&out, exif);
    assert_eq!(
        again
            .windows(5)
            .filter(|window| *window == b"Exif\0")
            .count(),
        1
    );
}

#[test]
fn a_jpeg_keeps_its_scan_and_gains_one_xmp_packet() {
    let pixels = (0..16 * 16 * 3)
        .map(|index| (index % 255) as u8)
        .collect::<Vec<_>>();
    let jpeg =
        negpy_backend::services::export::encoders::encode_jpeg(&pixels, 16, 16, 3, 90, None, None)
            .unwrap();
    let mut config = MetadataConfig::default();
    config.camera_make = "Olympus".into();
    let payload = build_metadata_payload(&config, None, None);
    let exif = dump_exif(&build_custom_exif(&payload));
    let with_exif = inject_jpeg_exif(&jpeg, &exif);
    let packet = negpy_backend::features::metadata::xmp::build_xmp_bytes(&payload, true);
    let out = inject_jpeg_xmp(&with_exif, &packet);
    assert!(out.starts_with(&[0xFF, 0xD8]));
    assert_eq!(
        out.windows(5).filter(|window| *window == b"Exif\0").count(),
        1
    );
    assert_eq!(
        out.windows(29)
            .filter(|window| *window == b"http://ns.adobe.com/xap/1.0/\0")
            .count(),
        1
    );
}
