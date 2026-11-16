//! Scan naming, collision policy and the written file.

use negpy_backend::services::scanning::writer::{MONO_TIFF, ScanResult, write_scan};

fn capture(width: usize, height: usize) -> ScanResult {
    ScanResult {
        rgb: (0..width * height * 3)
            .map(|index| (index % 255) as u8)
            .collect(),
        dpi: 300,
        ..ScanResult::default()
    }
}

#[test]
fn two_scans_do_not_overwrite_one_another() {
    let dir = tempfile::tempdir().unwrap();
    let first = write_scan(
        &capture(4, 3),
        dir.path(),
        "scan_{{ seq }}",
        "TIFF",
        "20260102",
        None,
        4,
        3,
    )
    .unwrap();
    let second = write_scan(
        &capture(4, 3),
        dir.path(),
        "scan_{{ seq }}",
        "TIFF",
        "20260102",
        None,
        4,
        3,
    )
    .unwrap();
    assert_ne!(first, second);
    assert!(first.exists() && second.exists());
    assert_eq!(first.extension().unwrap(), "tif");
}

#[test]
fn a_leftover_ir_companion_reserves_the_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("scan_1_ir.TIF"), b"").unwrap();
    let path = write_scan(
        &capture(4, 3),
        dir.path(),
        "scan_{{ seq }}",
        "TIFF",
        "20260102",
        None,
        4,
        3,
    )
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "scan_2.tif");
}

#[test]
fn a_template_that_ignores_the_sequence_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let error = write_scan(
        &capture(4, 3),
        dir.path(),
        "fixed",
        "TIFF",
        "20260102",
        None,
        4,
        3,
    )
    .unwrap_err();
    assert!(error.contains("different basename"));
}

#[test]
fn the_mono_format_writes_a_tiff() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_scan(
        &capture(4, 3),
        dir.path(),
        "mono_{{ seq }}",
        MONO_TIFF,
        "20260102",
        Some(4),
        4,
        3,
    )
    .unwrap();
    assert_eq!(path.file_name().unwrap(), "mono_4.tif");
    assert!(path.exists());
}

#[test]
fn a_capture_with_ir_writes_the_rgb_and_ir_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut capture = capture(4, 3);
    capture.ir = Some(vec![64u8; 12]);
    capture.ir_valid_mask = Some(vec![1u8; 12]);
    let path = write_scan(
        &capture,
        dir.path(),
        "ir_{{ seq }}",
        "TIFF",
        "20260102",
        None,
        4,
        3,
    )
    .unwrap();
    assert!(path.exists());
    assert!(dir.path().join("ir_1_IR.tif").exists());
    assert!(dir.path().join("ir_1_IR_VALID.tif").exists());
}
