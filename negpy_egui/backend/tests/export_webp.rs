//! Ports the observable rules of `tests/test_export_webp.py`: lossless WebP writes, and the
//! lossy setting reports a reason instead of writing a different file.

use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::domain::models::{ExportConfig, ExportFormat, WorkspaceConfig};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::export::encode::{can_encode, encode_export, unsupported_reason};
use negpy_backend::services::export::pixels::ExportPixels;

fn pixels() -> ExportPixels {
    ExportPixels {
        samples: vec![10, 128, 240, 20, 100, 200],
        samples16: Vec::new(),
        width: 2,
        height: 1,
        channels: 3,
        depth: 8,
        icc: None,
    }
}

#[test]
fn a_lossless_export_writes_a_riff_container() {
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Webp;
    config.webp_lossless = true;
    let out = encode_export(&pixels(), &config).expect("lossless webp");
    assert_eq!(&out.bytes[..4], b"RIFF");
    assert_eq!(&out.bytes[8..12], b"WEBP");
    assert_eq!(out.extension, "webp");
    assert!(can_encode(ExportFormat::Webp));
}

#[test]
fn a_lossy_setting_reports_the_missing_encoder() {
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Webp;
    let error = encode_export(&pixels(), &config).unwrap_err();
    assert!(error.contains("Lossless WebP"));
    assert!(unsupported_reason(ExportFormat::Webp).is_none());
}

#[test]
fn a_lossless_export_ignores_the_sixteen_bit_setting() {
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Webp;
    config.webp_lossless = true;
    config.export_bit_depth = 16;
    let workspace = WorkspaceConfig::default();
    let _ = &workspace.metadata;
    let out = encode_export(&pixels(), &config).expect("lossless webp");
    assert_eq!(&out.bytes[..4], b"RIFF");
}

#[test]
fn a_write_puts_a_webp_beside_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("frame.tif");
    std::fs::write(&source, b"x").unwrap();
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Webp;
    config.webp_lossless = true;
    config.export_path = dir.path().join("out").display().to_string();
    let workspace = WorkspaceConfig {
        export: config.clone(),
        metadata: MetadataConfig::default(),
        ..WorkspaceConfig::default()
    };
    let path = negpy_backend::services::export::service::write_export(
        &ImageBuffer::from_rgb(8, 6, vec![0.5; 8 * 6 * 3]),
        &source,
        &config,
        &workspace,
        "sRGB",
        dir.path(),
    )
    .expect("write");
    assert_eq!(std::fs::read(&path).unwrap()[..4], *b"RIFF");
}
