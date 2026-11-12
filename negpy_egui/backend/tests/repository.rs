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

#[test]
fn a_triage_mark_is_independent_of_the_edit() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_mark("h1", Some("keeper"), "/photos/1.nef")
        .unwrap();
    repo.save_file_mark("h2", Some("excluded"), "/photos/2.nef")
        .unwrap();
    assert_eq!(
        repo.file_marks().unwrap().get("h1").map(String::as_str),
        Some("keeper")
    );
    assert_eq!(
        repo.file_marks_by_path()
            .unwrap()
            .get("/photos/2.nef")
            .map(String::as_str),
        Some("excluded")
    );

    repo.delete_file_settings("h1").unwrap();
    assert_eq!(
        repo.file_marks().unwrap().get("h1").map(String::as_str),
        Some("keeper")
    );
    repo.save_file_mark("h1", None, "").unwrap();
    assert!(!repo.file_marks().unwrap().contains_key("h1"));
}

#[test]
fn history_steps_round_trip_in_order_and_truncate() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let mut first = WorkspaceConfig::default();
    first.exposure.density = 1.1;
    let mut second = WorkspaceConfig::default();
    second.exposure.density = 1.2;
    repo.save_history_step("h", 0, &first).unwrap();
    repo.save_history_step("h", 1, &second).unwrap();

    let history = repo.all_history("h").unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].1.exposure.density, 1.1);
    assert_eq!(history[1].1.exposure.density, 1.2);
    assert_eq!(repo.max_history_index("h").unwrap(), 1);

    repo.truncate_history_above("h", 0).unwrap();
    assert_eq!(repo.all_history("h").unwrap().len(), 1);
}

#[test]
fn deleting_an_edit_drops_its_history_and_work_prints() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("h", &WorkspaceConfig::default(), "")
        .unwrap();
    repo.save_history_step("h", 0, &WorkspaceConfig::default())
        .unwrap();
    repo.save_work_print("h", "print", &WorkspaceConfig::default())
        .unwrap();
    repo.delete_file_settings("h").unwrap();
    assert!(repo.load_file_settings("h").unwrap().is_none());
    assert!(repo.history_step("h", 0).unwrap().is_none());
    assert!(repo.work_prints("h").unwrap().is_empty());
}

#[test]
fn embeddings_round_trip_by_model_version() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_embedding("h1", &[1.0, 2.0], "v1", "/a/1.nef")
        .unwrap();
    repo.save_embedding("h2", &[3.0, 4.0], "v1", "/a/2.nef")
        .unwrap();
    repo.save_embedding("h3", &[5.0, 6.0], "v2", "/a/3.nef")
        .unwrap();

    let hashes = vec!["h1".to_string(), "h2".to_string(), "missing".to_string()];
    let loaded = repo.embeddings_for(&hashes, "v1").unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded["h1"], vec![1.0, 2.0]);
    assert_eq!(repo.all_embeddings("v1").unwrap()["h2"].1, vec![3.0, 4.0]);
    assert!(!repo.all_embeddings("v1").unwrap().contains_key("h3"));
}

#[test]
fn work_prints_can_be_renamed_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let config = WorkspaceConfig::default();
    repo.save_work_print("h", "draft", &config).unwrap();
    repo.rename_work_print("h", "draft", "final").unwrap();
    assert!(repo.work_print("h", "draft").unwrap().is_none());
    assert!(repo.work_print("h", "final").unwrap().is_some());
    repo.delete_work_print("h", "final").unwrap();
    assert!(repo.work_prints("h").unwrap().is_empty());
}

#[test]
fn database_stats_count_rows_and_presets() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings("h", &WorkspaceConfig::default(), "")
        .unwrap();
    repo.save_history_step("h", 0, &WorkspaceConfig::default())
        .unwrap();
    repo.save_work_print("h", "print", &WorkspaceConfig::default())
        .unwrap();
    repo.save_file_mark("h", Some("keeper"), "").unwrap();
    let mut values = serde_json::Map::new();
    values.insert("export_presets".into(), json!([{"name": "JPEG"}]));
    repo.save_global_settings(&values).unwrap();

    let stats = repo.database_stats().unwrap();
    assert_eq!(stats.file_settings, 1);
    assert_eq!(stats.edit_history, 1);
    assert_eq!(stats.work_prints, 1);
    assert_eq!(stats.file_marks, 1);
    assert_eq!(stats.export_presets, 1);
    assert_eq!(stats.app_preferences, 0);
    assert!(stats.edits_db_bytes > 0);
}

#[test]
fn reset_keeps_the_schema_and_empties_both_databases() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings("h", &WorkspaceConfig::default(), "")
        .unwrap();
    let mut values = serde_json::Map::new();
    values.insert("rolls".into(), json!({"r1": {}}));
    repo.save_global_settings(&values).unwrap();
    repo.clear_saved_edits().unwrap();
    assert!(repo.load_file_settings("h").unwrap().is_none());
    assert_eq!(
        repo.global_setting("rolls").unwrap(),
        Some(json!({"r1": {}}))
    );

    repo.save_file_settings("h", &WorkspaceConfig::default(), "")
        .unwrap();
    repo.reset_everything().unwrap();
    assert!(repo.load_file_settings("h").unwrap().is_none());
    assert_eq!(repo.global_setting("rolls").unwrap(), None);
    repo.save_global_settings(&values).unwrap();
}
