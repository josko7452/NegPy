//! Ports of tests/test_source_encoding.py's profile and decode rows.

use negpy_backend::domain::models::SourceEncoding;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::infrastructure::loaders::source_encoding::{
    PROFILE, SRGB_COLORANTS_D50, SampleDepth, WORKING_COLORANTS_D50, apply_decode, decode_positive,
    is_device_profile, parse_profile, resolve_decode, to_working,
};

fn adobe_rgb_icc() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../icc/AdobeCompat-v4.icc");
    std::fs::read(path).unwrap()
}

fn srgb_icc() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../icc/sRGB-v4.icc");
    std::fs::read(path).unwrap()
}

fn gray16(value: u16) -> ImageBuffer {
    ImageBuffer::from_rgb16(2, 1, vec![value, value, value, value, value, value])
}

#[test]
fn an_untagged_16_bit_scan_is_linear() {
    let resolved = resolve_decode(SourceEncoding::Auto, None, SampleDepth::Sixteen);
    assert_eq!(resolved.ran, "linear");
    assert!(resolved.decode.identity);
    let decoded = apply_decode(&gray16(32768), &resolved.decode);
    assert!((decoded.pixels[0] - 32768.0 / 65535.0).abs() < 1e-6);
}

#[test]
fn an_untagged_8_bit_scan_is_srgb() {
    let resolved = resolve_decode(SourceEncoding::Auto, None, SampleDepth::Eight);
    assert_eq!(resolved.ran, "srgb");
    let image = ImageBuffer::from_rgb(1, 1, vec![128.0 / 255.0; 3]);
    let decoded = apply_decode(&image, &resolved.decode);
    assert!(decoded.pixels[0] < 128.0 / 255.0);
}

#[test]
fn an_adobe_profile_decodes_with_the_working_trc() {
    let resolved = resolve_decode(
        SourceEncoding::Auto,
        Some(&adobe_rgb_icc()),
        SampleDepth::Sixteen,
    );
    assert_eq!(resolved.ran, PROFILE);
    assert!(resolved.decode.to_working.is_none());
    let decoded = apply_decode(&gray16(32768), &resolved.decode);
    let expected = (32768.0f64 / 65535.0).powf(563.0 / 256.0);
    assert!((decoded.pixels[0] as f64 - expected).abs() < 1e-3);
}

#[test]
fn an_srgb_profile_converts_to_the_working_primaries() {
    let matrix = parse_profile(&srgb_icc(), 256).unwrap().to_working.unwrap();
    let red = ImageBuffer::from_rgb16(1, 1, vec![65535, 0, 0]);
    let resolved = resolve_decode(
        SourceEncoding::Auto,
        Some(&srgb_icc()),
        SampleDepth::Sixteen,
    );
    let (source, to_working) = decode_positive(&red, &resolved.decode);
    assert_eq!(to_working, Some(matrix));
    assert_eq!(source.rgb_xyz_matrix, Some(matrix));
    assert!((source.pixels[0] - 1.0).abs() < 1e-6);
    assert!(source.pixels[1].abs() < 1e-6);
}

#[test]
fn linear_and_gamma_override_the_profile() {
    let linear = resolve_decode(
        SourceEncoding::Linear,
        Some(&srgb_icc()),
        SampleDepth::Sixteen,
    );
    assert_eq!(linear.ran, "linear");
    assert!(linear.decode.to_working.is_none());
    let gamma = resolve_decode(
        SourceEncoding::Gamma22,
        Some(&srgb_icc()),
        SampleDepth::Sixteen,
    );
    assert_eq!(gamma.ran, "gamma22");
    assert!(gamma.decode.to_working.is_none());
    let decoded = apply_decode(&gray16(32768), &gamma.decode);
    let expected = (32768.0f64 / 65535.0).powf(2.2);
    assert!((decoded.pixels[0] as f64 - expected).abs() < 1e-6);
}

#[test]
fn a_scanner_device_profile_is_ignored() {
    let mut icc = adobe_rgb_icc();
    icc[12..16].copy_from_slice(b"scnr");
    assert!(is_device_profile(&icc));
    let resolved = resolve_decode(SourceEncoding::Auto, Some(&icc), SampleDepth::Sixteen);
    assert_eq!(resolved.ran, "linear");
}

#[test]
fn the_bundled_profiles_resolve_to_their_primaries() {
    assert!(
        parse_profile(&adobe_rgb_icc(), 256)
            .unwrap()
            .to_working
            .is_none()
    );
    assert!(to_working(WORKING_COLORANTS_D50).is_none());
    let srgb = to_working(SRGB_COLORANTS_D50).unwrap();
    assert!(srgb[0][0] > 0.6 && srgb[1][0] < 0.3);
}

#[test]
fn a_wider_source_is_converted_and_gamut_fitted() {
    let prophoto = to_working([
        [0.7977, 0.1352, 0.0313],
        [0.2880, 0.7119, 0.0001],
        [0.0, 0.0, 0.8249],
    ])
    .unwrap();
    let mut decode = parse_profile(&srgb_icc(), 65536).unwrap();
    decode.to_working = Some(prophoto);
    let image = ImageBuffer::from_rgb16(1, 1, vec![65535, 0, 0]);
    let (decoded, matrix) = decode_positive(&image, &decode);
    assert!(matrix.is_none());
    assert!(
        decoded
            .pixels
            .iter()
            .all(|value| (0.0..=1.0).contains(value))
    );
}
