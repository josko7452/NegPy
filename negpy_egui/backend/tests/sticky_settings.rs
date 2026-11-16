//! Ports the store and migration assertions in `tests/test_sticky_settings.py`.

use std::collections::HashSet;

use serde_json::{Map, Value, json};
use tempfile::TempDir;

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::session::settings_catalog::{all_rows, default_sticky_ids, rows_by_id};
use negpy_backend::session::sticky::{
    ALWAYS_STICKY_PROCESS, STICKY_CONFIG_KEY, STICKY_ROWS_KEY, export_remainder, load_sticky_ids,
    migrate_auto_crop_sticky_row, migrate_legacy, migrate_legacy_export_destination,
    migrate_square_before_crop_sticky_row, save_sticky_ids, sticky_snapshot,
};

fn test_repo() -> (TempDir, StorageRepository) {
    let dir = TempDir::new().expect("temp dir");
    let edits = dir.path().join("edits.db").display().to_string();
    let settings = dir.path().join("settings.db").display().to_string();
    let repo = StorageRepository::new(edits, settings);
    repo.initialize().expect("initialize");
    (dir, repo)
}

fn put(repo: &mut StorageRepository, key: &str, value: Value) {
    let mut settings = Map::new();
    settings.insert(key.to_string(), value);
    repo.save_global_settings(&settings).expect("save setting");
}

fn get(repo: &StorageRepository, key: &str) -> Option<Value> {
    repo.global_setting(key).expect("load setting")
}

#[test]
fn row_ids_are_unique_and_defaults_resolve() {
    assert_eq!(rows_by_id().len(), all_rows().len());
    let rows = rows_by_id();
    assert!(default_sticky_ids().iter().all(|id| rows.contains_key(id)));
}

#[test]
fn every_export_field_has_a_row_or_carries_in_the_remainder() {
    let catalog_export: HashSet<String> = all_rows()
        .into_iter()
        .filter(|row| row.section == "export")
        .flat_map(|row| row.fields.iter().map(|field| (*field).to_string()))
        .collect();
    let remainder = export_remainder();
    let all: HashSet<String> = catalog_export.union(&remainder).cloned().collect();
    let expected: HashSet<String> = negpy_backend::domain::models::ExportConfig::field_names()
        .into_iter()
        .collect();
    assert_eq!(all, expected);
    assert!(catalog_export.is_disjoint(&remainder));
}

#[test]
fn the_default_choice_applies_when_the_user_never_chose() {
    let (_dir, repo) = test_repo();
    let ids: HashSet<String> = load_sticky_ids(&repo).into_iter().collect();
    assert_eq!(ids, default_sticky_ids());
}

#[test]
fn a_stored_choice_wins_and_unknown_ids_drop() {
    let (_dir, mut repo) = test_repo();
    put(
        &mut repo,
        STICKY_ROWS_KEY,
        json!(["exposure.density", "gone.retired_field"]),
    );
    assert_eq!(load_sticky_ids(&repo), ["exposure.density"]);
}

#[test]
fn an_empty_choice_carries_nothing() {
    let (_dir, mut repo) = test_repo();
    put(&mut repo, STICKY_ROWS_KEY, json!([]));
    assert!(load_sticky_ids(&repo).is_empty());
}

#[test]
fn a_saved_choice_round_trips() {
    let (_dir, mut repo) = test_repo();
    save_sticky_ids(
        &mut repo,
        &["lab.saturation".to_string(), "exposure.density".to_string()],
    );
    let ids: HashSet<String> = load_sticky_ids(&repo).into_iter().collect();
    assert_eq!(
        ids,
        HashSet::from(["lab.saturation".to_string(), "exposure.density".to_string()])
    );
}

#[test]
fn the_snapshot_excludes_description_fields() {
    let mut config = WorkspaceConfig::default();
    config.metadata.description_fields = Some(vec!["camera".to_string(), "iso".to_string()]);
    let snapshot = sticky_snapshot(&config);
    assert!(!snapshot.contains_key("description_fields"));
}

#[test]
fn the_snapshot_covers_every_other_catalog_field() {
    let snapshot = sticky_snapshot(&WorkspaceConfig::default());
    let expected: HashSet<String> = all_rows()
        .into_iter()
        .flat_map(|row| row.fields.iter().map(|field| (*field).to_string()))
        .filter(|field| field != "description_fields")
        .collect();
    assert_eq!(snapshot.keys().cloned().collect::<HashSet<_>>(), expected);
}

#[test]
fn legacy_keys_seed_the_snapshot_once() {
    let (_dir, mut repo) = test_repo();
    put(&mut repo, "last_process_mode", json!("B&W Negative"));
    put(&mut repo, "last_aspect_ratio", json!("1:1"));
    put(&mut repo, "last_lab_config", json!({"saturation": 1.4}));
    migrate_legacy(&mut repo);
    let snapshot = get(&repo, STICKY_CONFIG_KEY).expect("snapshot");
    assert_eq!(snapshot["process_mode"], "B&W Negative");
    assert_eq!(snapshot["autocrop_ratio"], "1:1");
    assert_eq!(snapshot["saturation"], 1.4);
}

#[test]
fn true_black_maps_inverted_and_explicit_paper_black_wins() {
    let (_dir, mut repo) = test_repo();
    put(&mut repo, "last_true_black", json!(false));
    migrate_legacy(&mut repo);
    assert_eq!(get(&repo, STICKY_CONFIG_KEY).unwrap()["paper_black"], true);

    let (_dir2, mut repo2) = test_repo();
    put(&mut repo2, "last_true_black", json!(false));
    put(&mut repo2, "last_paper_black", json!(false));
    migrate_legacy(&mut repo2);
    assert_eq!(
        get(&repo2, STICKY_CONFIG_KEY).unwrap()["paper_black"],
        false
    );
}

#[test]
fn a_snapshot_stops_legacy_seeding() {
    let (_dir, mut repo) = test_repo();
    put(&mut repo, STICKY_CONFIG_KEY, json!({"density": 2.0}));
    put(&mut repo, "last_process_mode", json!("B&W Negative"));
    migrate_legacy(&mut repo);
    assert_eq!(get(&repo, STICKY_CONFIG_KEY), Some(json!({"density": 2.0})));
}

#[test]
fn a_fresh_install_does_not_create_a_snapshot() {
    let (_dir, mut repo) = test_repo();
    migrate_legacy(&mut repo);
    assert_eq!(get(&repo, STICKY_CONFIG_KEY), None);
}

#[test]
fn legacy_keys_without_catalog_rows_drop() {
    let (_dir, mut repo) = test_repo();
    put(&mut repo, "last_process_mode", json!("B&W Negative"));
    put(
        &mut repo,
        "last_export_config",
        json!({"export_path": "/out"}),
    );
    migrate_legacy(&mut repo);
    assert!(
        !get(&repo, STICKY_CONFIG_KEY)
            .unwrap()
            .get("export_path")
            .is_some()
    );
}

#[test]
fn auto_crop_and_square_rows_are_added_once() {
    let (_dir, mut repo) = test_repo();
    put(
        &mut repo,
        STICKY_ROWS_KEY,
        json!(["geometry.autocrop_mode"]),
    );
    migrate_auto_crop_sticky_row(&mut repo);
    migrate_square_before_crop_sticky_row(&mut repo);
    let ids: HashSet<String> = load_sticky_ids(&repo).into_iter().collect();
    assert_eq!(
        ids,
        HashSet::from([
            "geometry.autocrop_mode".to_string(),
            "geometry.crop_from_auto".to_string(),
            "geometry.autocrop_square".to_string(),
        ])
    );
    save_sticky_ids(&mut repo, &["geometry.autocrop_mode".to_string()]);
    migrate_auto_crop_sticky_row(&mut repo);
    migrate_square_before_crop_sticky_row(&mut repo);
    assert_eq!(load_sticky_ids(&repo), ["geometry.autocrop_mode"]);
}

#[test]
fn scan_preferences_stay_out_of_the_look_set() {
    let look_fields = ["density", "grade", "saturation"];
    for (_, field) in ALWAYS_STICKY_PROCESS {
        assert!(!look_fields.contains(&field));
    }
}

#[test]
fn an_untouched_legacy_export_destination_is_dropped_once() {
    let (_dir, mut repo) = test_repo();
    let default_export = negpy_backend::kernel::system::paths::default_user_dir()
        .join("export")
        .display()
        .to_string();
    put(
        &mut repo,
        "last_export_config",
        json!({
            "output_mode": "absolute",
            "export_path": default_export,
            "output_subfolder": "export",
            "jpeg_quality": 90,
        }),
    );
    migrate_legacy_export_destination(&mut repo);
    let remaining = get(&repo, "last_export_config").unwrap();
    assert!(remaining.get("output_mode").is_none());
    assert!(remaining.get("export_path").is_none());
    assert!(remaining.get("output_subfolder").is_none());
    assert_eq!(remaining["jpeg_quality"], 90);
    assert_eq!(
        get(&repo, "export_destination_migrated_v1"),
        Some(json!(true))
    );
}

#[test]
fn a_chosen_export_destination_is_left_alone() {
    let (_dir, mut repo) = test_repo();
    put(
        &mut repo,
        "last_export_config",
        json!({"output_mode": "absolute", "export_path": "/Volumes/Scans/export"}),
    );
    migrate_legacy_export_destination(&mut repo);
    assert_eq!(
        get(&repo, "last_export_config").unwrap()["export_path"],
        "/Volumes/Scans/export"
    );
}

#[test]
fn a_non_absolute_export_mode_is_left_alone() {
    let (_dir, mut repo) = test_repo();
    put(
        &mut repo,
        "last_export_config",
        json!({"output_mode": "subfolder_of_source", "output_subfolder": "scans"}),
    );
    migrate_legacy_export_destination(&mut repo);
    assert_eq!(
        get(&repo, "last_export_config").unwrap()["output_subfolder"],
        "scans"
    );
}
