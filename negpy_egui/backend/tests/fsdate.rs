//! Export filesystem dates mirror the source.

use std::time::{Duration, SystemTime};

use negpy_backend::features::metadata::fsdate::sync_export_filesystem_dates;

#[test]
fn mtime_and_atime_follow_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.arw");
    let output = dir.path().join("export.jpg");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&output, b"export").unwrap();
    let old = SystemTime::now() - Duration::from_secs(5 * 24 * 3600);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&source)
        .unwrap();
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(old)
            .set_modified(old),
    )
    .unwrap();
    sync_export_filesystem_dates(&output, &source);
    let stat = std::fs::metadata(&output).unwrap();
    assert!(stat.modified().unwrap().duration_since(old).unwrap() < Duration::from_secs(1));
    assert!(stat.accessed().unwrap().duration_since(old).unwrap() < Duration::from_secs(1));
}

#[test]
fn a_missing_source_is_a_noop() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("export.jpg");
    std::fs::write(&output, b"export").unwrap();
    let before = std::fs::metadata(&output).unwrap().modified().unwrap();
    sync_export_filesystem_dates(&output, &dir.path().join("does-not-exist.arw"));
    assert_eq!(
        std::fs::metadata(&output).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn a_missing_destination_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.arw");
    std::fs::write(&source, b"source").unwrap();
    sync_export_filesystem_dates(&dir.path().join("does-not-exist.jpg"), &source);
}
