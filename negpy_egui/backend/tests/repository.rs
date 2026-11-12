//! Ports of tests/test_storage_repo.py and tests/test_sidecar.py's storage rows.

use negpy_backend::domain::models::workspace::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use rusqlite::Connection;
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
fn many_edits_load_in_one_read_and_skip_missing_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    for index in 0..501 {
        let mut config = WorkspaceConfig::default();
        config.exposure.density = index as f64;
        repo.save_file_settings(&format!("h{index}"), &config, "")
            .unwrap();
    }
    let mut hashes: Vec<String> = (0..501).map(|index| format!("h{index}")).collect();
    hashes.push("missing".into());

    let loaded = repo.load_file_settings_many(&hashes).unwrap();

    assert_eq!(loaded.len(), 501);
    assert_eq!(loaded["h500"].exposure.density, 500.0);
    assert!(!loaded.contains_key("missing"));
    assert!(repo.load_file_settings_many(&[]).unwrap().is_empty());
}

#[test]
fn settings_by_path_skip_rows_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let mut config = WorkspaceConfig::default();
    config.exposure.density = 0.7;
    repo.save_file_settings("h1", &config, "/photos/1.nef")
        .unwrap();
    repo.save_file_settings("h2", &WorkspaceConfig::default(), "")
        .unwrap();

    let by_path = repo.load_settings_by_path().unwrap();

    assert_eq!(by_path.len(), 1);
    assert_eq!(by_path["/photos/1.nef"].exposure.density, 0.7);
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
    repo.save_history_step("old", 0, &WorkspaceConfig::default())
        .unwrap();

    repo.rehome_file_settings("live", "target", "/photos/2.nef")
        .unwrap();
    repo.save_file_settings("target", &WorkspaceConfig::default(), "/photos/2.nef")
        .unwrap();
    assert!(repo.load_file_settings("live").unwrap().is_none());
    assert!(repo.load_file_settings("target").unwrap().is_some());
}

#[test]
fn rehoming_moves_history_work_prints_and_marks() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("old", &WorkspaceConfig::default(), "/photos/1.nef")
        .unwrap();
    repo.save_history_step("old", 0, &WorkspaceConfig::default())
        .unwrap();
    repo.save_work_print("old", "print", &WorkspaceConfig::default())
        .unwrap();
    repo.save_file_mark("old", Some("keeper"), "/photos/1.nef")
        .unwrap();

    repo.rehome_file_settings("old", "new", "/photos/2.nef")
        .unwrap();

    assert!(repo.history_step("new", 0).unwrap().is_some());
    assert_eq!(repo.work_prints("new").unwrap(), vec!["print"]);
    assert_eq!(
        repo.file_marks().unwrap().get("new").map(String::as_str),
        Some("keeper")
    );
}

#[test]
fn a_path_rehome_moves_rows_under_one_folder_only() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("a", &WorkspaceConfig::default(), "/old/a.nef")
        .unwrap();
    repo.save_file_settings("b", &WorkspaceConfig::default(), "/old/b.nef")
        .unwrap();
    repo.save_file_settings("c", &WorkspaceConfig::default(), "/older/c.nef")
        .unwrap();
    repo.save_file_mark("a", Some("keeper"), "/old/a.nef")
        .unwrap();
    repo.save_embedding("a", &[1.0], "v1", "/old/a.nef")
        .unwrap();

    repo.rehome_path_prefix("/old", "/new").unwrap();

    assert_eq!(
        repo.path_for_file_hash("a").unwrap().as_deref(),
        Some("/new/a.nef")
    );
    assert_eq!(repo.file_marks_by_path().unwrap()["/new/a.nef"], "keeper");
    assert_eq!(repo.all_embeddings("v1").unwrap()["a"].0, "/new/a.nef");
    assert_eq!(
        repo.path_for_file_hash("c").unwrap().as_deref(),
        Some("/older/c.nef")
    );
}

#[test]
fn edited_names_are_the_files_directly_under_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    repo.save_file_settings("a", &WorkspaceConfig::default(), "/photos/a.nef")
        .unwrap();
    repo.save_file_settings("b", &WorkspaceConfig::default(), "/photos/sub/b.nef")
        .unwrap();

    let mut names = repo.edited_file_names_under("/photos").unwrap();
    names.sort();

    assert_eq!(names, vec!["a.nef"]);
    assert!(repo.edited_file_names_under("/photo").unwrap().is_empty());
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

#[test]
fn statistics_on_an_empty_database_are_zero() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    let stats = repo.database_stats().unwrap();
    assert_eq!(stats.file_settings, 0);
    assert_eq!(stats.edit_history, 0);
    assert_eq!(stats.work_prints, 0);
    assert_eq!(stats.file_marks, 0);
    assert_eq!(stats.export_presets, 0);
    assert_eq!(stats.app_preferences, 0);
    assert!(stats.edits_db_bytes > 0);
    assert!(stats.settings_db_bytes > 0);
}

#[test]
fn app_preferences_count_every_global_row_except_presets() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let mut values = serde_json::Map::new();
    values.insert("export_presets".into(), json!([{"name": "JPEG"}]));
    values.insert("rolls_by_id".into(), json!({"r1": {"kind": "virtual"}}));
    values.insert("window_geometry".into(), json!([0, 0, 800, 600]));
    values.insert("last_open_folder".into(), json!("/photos"));
    repo.save_global_settings(&values).unwrap();

    let stats = repo.database_stats().unwrap();
    assert_eq!(stats.export_presets, 1);
    assert_eq!(stats.app_preferences, 3);
}

#[test]
fn clearing_saved_edits_keeps_rig_and_app_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings("h", &WorkspaceConfig::default(), "/photos/1.raw")
        .unwrap();
    repo.save_history_step("h", 0, &WorkspaceConfig::default())
        .unwrap();
    repo.save_file_mark("h", Some("keeper"), "/photos/1.raw")
        .unwrap();
    let mut values = serde_json::Map::new();
    values.insert("rolls_by_id".into(), json!({"r1": {"defaults": {}}}));
    values.insert("export_presets".into(), json!([{"name": "JPEG"}]));
    repo.save_global_settings(&values).unwrap();

    repo.clear_saved_edits().unwrap();

    let stats = repo.database_stats().unwrap();
    assert_eq!(stats.file_settings, 0);
    assert_eq!(stats.edit_history, 0);
    assert_eq!(stats.file_marks, 0);
    assert_eq!(stats.export_presets, 1);
    assert_eq!(stats.app_preferences, 1);
    assert_eq!(
        repo.global_setting("rolls_by_id").unwrap(),
        Some(json!({"r1": {"defaults": {}}}))
    );
    assert!(
        repo.load_file_settings_by_path("/photos/1.raw")
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_repository_stays_usable_after_a_full_reset() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.save_file_settings("old", &WorkspaceConfig::default(), "/photos/old.raw")
        .unwrap();
    repo.reset_everything().unwrap();

    repo.save_file_settings("new", &WorkspaceConfig::default(), "/photos/new.raw")
        .unwrap();
    assert!(repo.load_file_settings("new").unwrap().is_some());
    assert_eq!(repo.database_stats().unwrap().file_settings, 1);
}

#[test]
fn clear_and_reset_accept_an_empty_database() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    repo.clear_saved_edits().unwrap();
    repo.reset_everything().unwrap();
    assert_eq!(repo.database_stats().unwrap().file_settings, 0);
}

#[test]
fn both_databases_use_write_ahead_logging() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path());
    for path in [&repo.edits_db_path, &repo.settings_db_path] {
        let connection = Connection::open(path).unwrap();
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
