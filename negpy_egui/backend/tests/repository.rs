//! Ports of tests/test_storage_repo.py and tests/test_sidecar.py's storage rows.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use serde_json::json;

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

#[test]
fn a_saved_edit_round_trips_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let mut config = WorkspaceConfig::default();
    config.exposure.density = 0.42;
    repo.save_file_settings("h1", &config, "/photos/1.nef")
        .unwrap();

    let loaded = repo.load_file_settings("h1").unwrap().unwrap();
    assert_eq!(loaded.exposure.density, 0.42);
    assert_eq!(
        repo.path_for_file_hash("h1").unwrap().as_deref(),
        Some("/photos/1.nef")
    );
    assert_eq!(
        repo.load_file_settings_by_path("/photos/1.nef")
            .unwrap()
            .unwrap()
            .0,
        "h1"
    );
}

#[test]
fn saved_hashes_are_the_rows_that_exist() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("a", &WorkspaceConfig::default(), "")
        .unwrap();
    let hashes = vec!["a".to_string(), "b".to_string()];
    assert_eq!(repo.saved_hashes(&hashes).unwrap(), vec!["a"]);
    assert!(repo.saved_hashes(&[]).unwrap().is_empty());
}

#[test]
fn rehoming_moves_the_row_and_never_clobbers_a_live_hash() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("old", &WorkspaceConfig::default(), "/photos/1.nef")
        .unwrap();
    repo.rehome_file_settings("old", "new", "/photos/1.nef")
        .unwrap();
    assert!(repo.load_file_settings("old").unwrap().is_none());
    assert!(repo.load_file_settings("new").unwrap().is_some());

    repo.rehome_file_settings("live", "target", "/photos/2.nef")
        .unwrap();
    repo.save_file_settings("target", &WorkspaceConfig::default(), "/photos/2.nef")
        .unwrap();
    assert!(repo.load_file_settings("live").unwrap().is_none());
    assert!(repo.load_file_settings("target").unwrap().is_some());
}

#[test]
fn global_settings_round_trip_through_their_own_database() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut values = serde_json::Map::new();
    values.insert("mode".to_string(), json!("Color Negative"));
    values.insert("clip".to_string(), json!(0.01));
    repo.save_global_settings(&values).unwrap();
    assert_eq!(
        repo.global_setting("mode").unwrap(),
        Some(json!("Color Negative"))
    );
    assert_eq!(repo.global_setting("clip").unwrap(), Some(json!(0.01)));
}
