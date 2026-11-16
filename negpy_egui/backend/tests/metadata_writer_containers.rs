//! The export containers that carry EXIF and XMP beside their pixels.

use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::features::metadata::payload::build_metadata_payload;
use negpy_backend::features::metadata::resolution::{INCH, Resolution};
use negpy_backend::features::metadata::writer::embed_metadata;
use negpy_backend::infrastructure::loaders::jxl_boxes::{
    JXL_SIGNATURE, read_jxl_exif, read_jxl_xmp,
};
use negpy_backend::services::export::encoders::{encode_jpeg, encode_png};

fn payload() -> negpy_backend::features::metadata::payload::MetadataPayload {
    let mut config = MetadataConfig::default();
    config.camera_make = "Olympus".into();
    config.camera_model = "PEN F".into();
    config.film = "HP5 Plus".into();
    build_metadata_payload(&config, None, None)
}

fn resolution() -> Resolution {
    Resolution {
        x: (300, 1),
        y: (300, 1),
        unit: INCH,
    }
}

#[test]
fn a_png_gains_one_exif_and_one_xmp_chunk() {
    let pixels = (0..8 * 8 * 3)
        .map(|index| (index % 255) as u8)
        .collect::<Vec<_>>();
    let png = encode_png(&pixels, 8, 8, 3, 8, 6, None, None, None, None).unwrap();
    let out = embed_metadata(&png, &payload(), None, Some(&resolution()));
    assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(
        out.windows(4).filter(|window| *window == b"eXIf").count(),
        1
    );
    assert_eq!(
        out.windows(4).filter(|window| *window == b"iTXt").count(),
        1
    );
    assert!(out.windows(b"NegPy".len()).any(|window| window == b"NegPy"));
}

#[test]
fn a_jpeg_gets_its_resolution_and_one_xmp_packet() {
    let pixels = (0..8 * 8 * 3)
        .map(|index| (index % 255) as u8)
        .collect::<Vec<_>>();
    let jpeg = encode_jpeg(&pixels, 8, 8, 3, 90, None, None).unwrap();
    let out = embed_metadata(&jpeg, &payload(), None, Some(&resolution()));
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

#[test]
fn a_webp_is_rebuilt_with_exif_and_xmp_chunks() {
    let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
    webp.extend_from_slice(b"VP8 ");
    webp.extend_from_slice(&10u32.to_le_bytes());
    webp.extend_from_slice(&[0, 0, 0, 0x9d, 0x01, 0x2a, 8, 0, 8, 0]);
    let len = (webp.len() - 8) as u32;
    webp[4..8].copy_from_slice(&len.to_le_bytes());
    let out = embed_metadata(&webp, &payload(), None, None);
    assert_eq!(&out[..4], b"RIFF");
    assert_eq!(&out[8..12], b"WEBP");
    assert!(out.windows(4).any(|window| window == b"EXIF"));
    assert!(out.windows(4).any(|window| window == b"XMP "));
    assert_eq!(&out[12..16], b"VP8X");
}

#[test]
fn a_jxl_container_replaces_its_metadata_boxes() {
    let mut jxl = JXL_SIGNATURE.to_vec();
    jxl.extend_from_slice(&[0, 0, 0, 0x14]);
    jxl.extend_from_slice(b"ftypjxl \0\0\0\0jxl ");
    jxl.extend_from_slice(&[0, 0, 0, 0x0e]);
    jxl.extend_from_slice(b"jxlc");
    jxl.extend_from_slice(b"pixels");
    let out = embed_metadata(&jxl, &payload(), None, None);
    assert!(out.starts_with(JXL_SIGNATURE));
    assert!(read_jxl_exif(&out).is_some());
    assert!(read_jxl_xmp(&out).is_some());
    assert_eq!(
        out.windows(4).filter(|window| *window == b"Exif").count(),
        1
    );
    assert_eq!(
        out.windows(4).filter(|window| *window == b"xml ").count(),
        1
    );
}
