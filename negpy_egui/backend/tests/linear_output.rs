//! Ports the pure classification, gamma and quantization checks in tests/test_linear_output.py.

use negpy_backend::domain::models::{ProcessConfig, SensorUnmix};
use negpy_backend::infrastructure::loaders::constants::SUPPORTED_TIFF_EXTENSIONS;
use negpy_backend::services::export::encoders::{Resolution, TiffCompression};
use negpy_backend::services::export::linear_output::{
    CameraWb, LinearBakes, LinearRecord, SourceMeta, apply_white_balance, build_xmp,
    effective_expansion, encode_linear_tiff, expand, is_linear_output_supported,
    linear_description, linear_output_source_type, linearize, normalize_wb_rgb,
    parse_tiff_datetime, source_format_label, to_uint16, wb_bake_block_reason,
};
use std::path::Path;

fn write_pakon_header(path: &Path, width: u32, height: u32) {
    let mut data = Vec::new();
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&width.to_le_bytes());
    data.extend_from_slice(&height.to_le_bytes());
    data.extend_from_slice(&48u32.to_le_bytes());
    data.extend(std::iter::repeat_n(0u8, (width * height * 6) as usize));
    std::fs::write(path, data).unwrap();
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_tiff_is_supported_and_classified() {
    assert!(SUPPORTED_TIFF_EXTENSIONS.contains(&".tif"));
    assert!(is_linear_output_supported(Path::new("scan.tif")));
    assert_eq!(linear_output_source_type(Path::new("scan.tif")), "tiff");
    assert!(is_linear_output_supported(Path::new("scan.xyz")));
    assert_eq!(linear_output_source_type(Path::new("scan.xyz")), "camera");
}

#[test]
fn a_camera_raw_is_supported_and_classified() {
    assert!(is_linear_output_supported(Path::new("shot.dng")));
    assert_eq!(linear_output_source_type(Path::new("shot.dng")), "dng");
}

#[test]
fn only_a_tiff_or_nef_source_reads_the_input_gamma() {
    assert_eq!(effective_expansion(Path::new("scan.tif"), None), 1.0);
    assert_eq!(effective_expansion(Path::new("scan.tif"), Some(2.0)), 2.0);
    assert_eq!(effective_expansion(Path::new("shot.dng"), None), 1.0);
    assert_eq!(effective_expansion(Path::new("shot.dng"), Some(2.0)), 2.0);
}

#[test]
fn white_balance_normalizes_green() {
    assert_eq!(normalize_wb_rgb([2.0, 1.0, 0.5, 1.0]), [2.0, 1.0, 0.5]);
    // An X-Trans / three-color CFA reports the second green as zero.
    assert_eq!(normalize_wb_rgb([2.0, 1.0, 0.5, 0.0]), [2.0, 1.0, 0.5]);
    assert_eq!(normalize_wb_rgb([0.0; 4]), [0.0, 1.0, 0.0]);
}

#[test]
fn the_wb_bake_records_why_it_cannot_apply() {
    assert_eq!(wb_bake_block_reason(true, None), "trichrome");
    let mut process = ProcessConfig::default();
    process.sensor_matrix = Some(vec![1.0; 9]);
    assert_eq!(wb_bake_block_reason(false, Some(&process)), "narrowband");
    assert_eq!(
        wb_bake_block_reason(false, Some(&ProcessConfig::default())),
        ""
    );
}

#[test]
fn linear_output_reverses_the_encoding() {
    let image = [0.0, 0.04045, 0.5, 1.0];
    let linear = linearize(&image, "linear");
    assert_eq!(linear, image);
    let srgb = linearize(&image, "srgb");
    assert!(close(srgb[0], 0.0));
    assert!(close(srgb[1], 0.04045 / 12.92));
    assert!(srgb[2] > 0.2 && srgb[2] < 0.25);
    let gamma = linearize(&[0.25, 1.0], "2.2");
    assert!(gamma[0] > 0.04 && gamma[0] < 0.06);
    assert!(close(gamma[1], 1.0));
}

#[test]
fn expansion_scales_and_clips() {
    let expanded = expand(&[0.1, 0.5, 0.8], 4.0);
    assert!(close(expanded[0], 0.4));
    assert!(close(expanded[1], 1.0));
    assert!(close(expanded[2], 1.0));
}

#[test]
fn the_as_shot_gains_are_applied() {
    let out = apply_white_balance(
        &[0.25, 0.25, 0.25],
        CameraWb {
            as_shot: [2.0, 1.0, 0.5, 1.0],
            daylight: [1.0; 4],
        },
    );
    assert!(close(out[0], 0.5));
    assert!(close(out[1], 0.25));
    assert!(close(out[2], 0.125));
}

#[test]
fn the_dump_quantizes_to_sixteen_bits() {
    assert_eq!(to_uint16(&[0.0, 0.5, 1.0]), vec![0, 32768, 65535]);
}

#[test]
fn the_description_records_the_source_and_bakes() {
    let meta = SourceMeta {
        demosaic: Some("AHD".into()),
        lens_corrected: true,
        ..SourceMeta::default()
    };
    let description = linear_description(
        LinearRecord {
            source_name: "shot.dng",
            camera_wb: Some(CameraWb {
                as_shot: [2.0, 1.0, 0.5, 1.0],
                daylight: [1.0; 4],
            }),
            expansion: 2.0,
            source_format: "camera RAW",
            sensor_applied: Some(&ProcessConfig {
                sensor_matrix: Some(vec![1.0; 9]),
                sensor_unmix: SensorUnmix::Linear,
                ..ProcessConfig::default()
            }),
            gamma_key: "2.2",
            meta: Some(&meta),
        },
        LinearBakes {
            wb_applied: true,
            flatfield: true,
            ice: true,
        },
    );
    assert!(description.contains("source: camera RAW"));
    assert!(description.contains("expansion: x2"));
    assert!(description.contains("demosaic: AHD"));
    assert!(description.contains("WB applied"));
    assert!(description.contains("flatfield"));
    assert!(description.contains("lens"));
    assert!(description.contains("Linear"));
    assert!(description.contains("ICE"));
    assert!(description.ends_with("no color management."));
}

#[test]
fn the_linear_tiff_is_untagged_sixteen_bit() {
    let image = vec![0.5; 4 * 4 * 3];
    let bytes = encode_linear_tiff(
        &image,
        4,
        4,
        Some(Resolution {
            x_dpi: 300.0,
            y_dpi: 300.0,
            unit: 2,
        }),
        TiffCompression::Deflate,
    )
    .expect("tiff");
    assert!(bytes.starts_with(b"II"));
    assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 42);
}

#[test]
fn the_tiff_datetime_accepts_the_writer_stamps() {
    assert_eq!(
        parse_tiff_datetime(Some("2025:01:15 12:00:00")).as_deref(),
        Some("2025:01:15 12:00:00")
    );
    assert_eq!(
        parse_tiff_datetime(Some("2025-01-15 12:00:00")).as_deref(),
        Some("2025:01:15 12:00:00")
    );
    assert_eq!(
        parse_tiff_datetime(Some("2025.01.15 12.00.00")).as_deref(),
        Some("2025:01:15 12:00:00")
    );
    assert_eq!(
        parse_tiff_datetime(Some("2025:01:15")).as_deref(),
        Some("2025:01:15 00:00:00")
    );
    assert_eq!(parse_tiff_datetime(Some("not a date")), None);
    assert_eq!(parse_tiff_datetime(None), None);
}

#[test]
fn the_xmp_escapes_the_name_and_title() {
    let xmp = build_xmp(
        Path::new("/scans/Mom & Dad <1>.NEF"),
        CameraWb {
            as_shot: [398.0, 302.0, 873.0, 304.0],
            daylight: [1.0; 4],
        },
        "A & B",
        false,
    );
    let body = String::from_utf8(xmp).unwrap();
    assert!(body.contains("Mom &amp; Dad &lt;1&gt;.NEF"));
    assert!(body.contains("<dc:title>"));
    assert!(body.contains("RAW-WB:"));
    assert!(body.contains("crs:RawFileName"));
    assert!(body.contains("dc:description"));
    assert!(body.contains("1.000000"));
}

#[test]
fn the_xmp_omits_the_gains_once_they_are_applied() {
    let xmp = build_xmp(
        Path::new("shot.nef"),
        CameraWb {
            as_shot: [2.0, 1.0, 0.5, 1.0],
            daylight: [1.0; 4],
        },
        "",
        true,
    );
    let body = String::from_utf8(xmp).unwrap();
    assert!(!body.contains("dc:description"));
}

#[test]
fn a_pakon_source_label_names_the_model() {
    let folder = tempfile::tempdir().unwrap();
    let f135 = folder.path().join("f135.raw");
    let file = std::fs::File::create(&f135).unwrap();
    file.set_len(36_000_000).unwrap();
    assert_eq!(source_format_label(&f135), "Pakon F135 Plus High Res");

    let header = folder.path().join("header.raw");
    write_pakon_header(&header, 2500, 2000);
    assert_eq!(source_format_label(&header), "Pakon 2500x2000");
}
