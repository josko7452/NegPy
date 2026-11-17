//! Ports the workspace-session rows in `tests/test_session_manager.py`.

use std::collections::BTreeSet;

use negpy_backend::infrastructure::storage::local_asset_store::LocalAssetStore;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::session::workspace::{SessionFile, WorkspaceSession};

fn session(dir: &std::path::Path) -> WorkspaceSession {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    let store = LocalAssetStore::new(dir.join("cache"), dir.join("icc"), 256);
    store.initialize().unwrap();
    WorkspaceSession::new("test_session_123", repo, store)
}

fn frame() -> SessionFile {
    SessionFile {
        name: "test.dng".into(),
        path: "/tmp/test.dng".into(),
        hash: "abc123hash".into(),
    }
}

#[test]
fn the_default_config_takes_the_export_destination() {
    let dir = tempfile::tempdir().unwrap();
    let session = session(dir.path());
    let config = session.create_default_config("/custom/env/path/export");
    assert_eq!(config.export.export_path, "/custom/env/path/export");
    assert_eq!(config.process.crosstalk_strength, 0.0);
    assert_eq!(config.retouch.dust_size, 4);
    assert_eq!(config.export.export_fmt.as_str(), "JPEG");
    assert_eq!(config.export.export_print_size, 30.0);
    assert_eq!(config.export.export_dpi, 300);
}

#[test]
fn a_missing_edit_reads_as_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = session(dir.path());
    session.uploaded_files = vec![frame()];
    session.selected_file_idx = 0;
    let settings = session.active_settings("/tmp/export").unwrap();
    assert_eq!(settings.lab.sharpen, 0.25);
    assert_eq!(settings.lab.sharpen_method.as_str(), "usm");
    assert_eq!(session.file_settings["abc123hash"], settings);
}

#[test]
fn a_saved_edit_wins_over_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = session(dir.path());
    let mut saved = session.create_default_config("/tmp/export");
    saved.exposure.grade = 72.0;
    session
        .repository
        .save_file_settings("saved_hash", &saved, "/tmp/test.dng")
        .unwrap();
    session.uploaded_files = vec![SessionFile {
        hash: "saved_hash".into(),
        ..frame()
    }];
    assert_eq!(session.active_settings("/tmp/export").unwrap(), saved);
}

#[test]
fn syncing_uploads_keeps_local_paths_and_drops_removed_uploads() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("local.tif");
    std::fs::write(&local, b"local").unwrap();
    let mut session = session(dir.path());
    session.add_local_assets(&[local.clone()]);
    session.sync_files(
        &BTreeSet::from(["upload.tif".to_string()]),
        &[("upload.tif".to_string(), b"upload".to_vec())],
    );
    assert_eq!(session.uploaded_files.len(), 2);
    session.sync_files(&BTreeSet::new(), &[]);
    assert_eq!(session.uploaded_files.len(), 1);
    assert_eq!(session.uploaded_files[0].path, local.display().to_string());
}
