//! Ports of tests/test_source_encoding.py's scanner default rows.

use std::path::Path;

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::domain::models::{ProcessConfig, ProcessMode};
use negpy_backend::infrastructure::loaders::source_kind::{
    LoaderKind, detect_noritsu_dims, is_pakon, pakon_header, select_loader,
    with_scanner_positive_default,
};

fn slide() -> WorkspaceConfig {
    WorkspaceConfig {
        process: ProcessConfig {
            process_mode: ProcessMode::E6,
            ..ProcessConfig::default()
        },
        ..WorkspaceConfig::default()
    }
}

fn write_pakon(path: &Path, width: u32, height: u32) {
    let mut bytes = Vec::with_capacity(16 + width as usize * height as usize * 6);
    for value in [16u32, width, height, 48] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.resize(16 + width as usize * height as usize * 6, 0);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_finished_image_or_scanner_raw_starts_positive() {
    for path in ["a.tif", "a.TIFF", "a.jpg", "a.jxl"] {
        assert!(
            with_scanner_positive_default(slide(), Path::new(path))
                .process
                .positive_source
        );
        assert!(
            !with_scanner_positive_default(WorkspaceConfig::default(), Path::new(path))
                .process
                .positive_source
        );
    }
    assert!(
        !with_scanner_positive_default(slide(), Path::new("camera.nef"))
            .process
            .positive_source
    );
    assert!(
        !with_scanner_positive_default(slide(), Path::new("a.dng"))
            .process
            .positive_source
    );
}

#[test]
fn a_pakon_header_selects_the_pakon_loader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("frame.raw");
    write_pakon(&path, 1000, 1500);

    assert_eq!(pakon_header(&path), Some((1500, 1000)));
    assert!(is_pakon(&path));
    assert_eq!(select_loader(&path), LoaderKind::Pakon);
}

#[test]
fn a_noritsu_size_selects_the_noritsu_loader() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scan.raw");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(3711 * 5028 * 6).unwrap();
    assert_eq!(detect_noritsu_dims(&path), Some((3711, 5028)));
    assert_eq!(select_loader(&path), LoaderKind::Noritsu);
}
