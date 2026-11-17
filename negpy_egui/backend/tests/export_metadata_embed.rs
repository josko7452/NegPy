//! Ports the write-path observations of `tests/test_flat_export_capture_date.py` and
//! `tests/test_metadata_writer.py`.

use negpy_backend::domain::models::{
    ExportConfig, ExportFormat, ExportResolutionMode, MetadataConfig, WorkspaceConfig,
};
use negpy_backend::features::metadata::exif_read::ExifValue;
use negpy_backend::features::metadata::writer::{ExifData, dump_exif};
use negpy_backend::infrastructure::loaders::exif_block::parse_exif_block;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::services::export::service::write_export;

fn source_with_exif(dir: &std::path::Path) -> std::path::PathBuf {
    let source = dir.join("frame.tiff");
    let mut data = ExifData::default();
    data.zeroth.insert(271, ExifValue::Text("Canon".into()));
    data.exif
        .insert(36867, ExifValue::Text("2024:05:12 14:30:00".into()));
    std::fs::write(&source, dump_exif(&data)).unwrap();
    source
}

fn image() -> ImageBuffer {
    ImageBuffer::from_rgb(8, 6, vec![0.4; 8 * 6 * 3])
}

#[test]
fn a_tiff_export_keeps_the_source_capture_date() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_with_exif(dir.path());
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Tiff;
    config.export_resolution_mode = ExportResolutionMode::Original;
    config.paper_aspect_ratio = "original".into();
    config.export_path = dir.path().join("out").display().to_string();
    let workspace = WorkspaceConfig {
        export: config.clone(),
        metadata: MetadataConfig::default(),
        ..WorkspaceConfig::default()
    };
    let path =
        write_export(&image(), &source, &config, &workspace, "sRGB", dir.path()).expect("write");
    let bytes = std::fs::read(&path).unwrap();
    let exif = parse_exif_block(&bytes).expect("a readable TIFF");
    assert_eq!(
        exif.exif.get(&36867),
        Some(&ExifValue::Text("2024:05:12 14:30:00".into()))
    );
}

#[test]
fn a_protected_source_keeps_its_own_camera_fields() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_with_exif(dir.path());
    let mut metadata = MetadataConfig::default();
    metadata.protect_original_metadata = true;
    let mut config = ExportConfig::default();
    config.export_fmt = ExportFormat::Tiff;
    config.export_resolution_mode = ExportResolutionMode::Original;
    config.paper_aspect_ratio = "original".into();
    config.export_path = dir.path().join("out").display().to_string();
    let workspace = WorkspaceConfig {
        export: config.clone(),
        metadata,
        ..WorkspaceConfig::default()
    };
    let path =
        write_export(&image(), &source, &config, &workspace, "sRGB", dir.path()).expect("write");
    let raw = std::fs::read(&path).unwrap();
    let exif = parse_exif_block(&raw).expect("a readable TIFF");
    assert_eq!(
        exif.zeroth.get(&271),
        Some(&ExifValue::Text("Canon".into()))
    );
}
