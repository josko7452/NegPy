//! Ports the gear preset conversion cases in tests/test_metadata_presets.py.

use std::path::Path;

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::gear_presets::migrate_gear_presets;
use negpy_backend::services::assets::presets::PresetStore;
use serde_json::{Value, json};

fn repository(dir: &Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn write_gear(dir: &Path, cameras: Value, stocks: Value, presets: Value) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("cameras.json"), cameras.to_string()).unwrap();
    std::fs::write(dir.join("film_stocks.json"), stocks.to_string()).unwrap();
    std::fs::write(dir.join("gear_presets.json"), presets.to_string()).unwrap();
}

#[test]
fn a_gear_preset_resolves_to_metadata_fields_once() {
    let dir = tempfile::tempdir().unwrap();
    let gear = dir.path().join("gear");
    let bundled = dir.path().join("bundled");
    let mut repo = repository(dir.path());
    write_gear(
        &gear,
        json!([{"id": "c1", "make": "Nikon", "model": "FM2"}]),
        json!([{"id": "f1", "manufacturer": "Ilford", "stockName": "HP5+", "iso": 400}]),
        json!([{"id": "p1", "displayName": "FM2 combo", "cameraId": "c1", "filmStockId": "f1"}]),
    );
    let store = PresetStore::new(dir.path().join("presets"), "metadata");

    migrate_gear_presets(&mut repo, &gear, &bundled, &store);

    let stored = store.load("FM2 combo").unwrap();
    assert_eq!(stored["camera_id"], json!("c1"));
    assert_eq!(stored["camera_make"], json!("Nikon"));
    assert_eq!(stored["film"], json!("Ilford HP5+"));
    assert_eq!(stored["film_iso"], json!(400));
    assert_eq!(stored.len(), 15);
    assert_eq!(
        repo.global_setting("gear_presets_migrated").unwrap(),
        Some(json!(true))
    );

    store
        .save(
            "FM2 combo",
            &json!({"developer": "D-76"}).as_object().unwrap().clone(),
        )
        .unwrap();
    migrate_gear_presets(&mut repo, &gear, &bundled, &store);
    assert_eq!(store.load("FM2 combo").unwrap()["developer"], json!("D-76"));
}

#[test]
fn a_case_folded_name_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let gear = dir.path().join("gear");
    let bundled = dir.path().join("bundled");
    let mut repo = repository(dir.path());
    write_gear(
        &gear,
        json!([{"id": "c1", "make": "Nikon", "model": "FM2"}]),
        json!([]),
        json!([{"id": "p1", "displayName": "portra", "cameraId": "c1"}]),
    );
    let store = PresetStore::new(dir.path().join("presets"), "metadata");
    store
        .save(
            "Portra",
            &json!({"scanning": "mine"}).as_object().unwrap().clone(),
        )
        .unwrap();

    migrate_gear_presets(&mut repo, &gear, &bundled, &store);

    assert_eq!(store.load("Portra").unwrap()["scanning"], json!("mine"));
    assert_eq!(store.list(), vec!["Portra".to_string()]);
}

#[test]
fn a_damaged_source_keeps_the_migration_pending() {
    let dir = tempfile::tempdir().unwrap();
    let gear = dir.path().join("gear");
    let bundled = dir.path().join("bundled");
    let mut repo = repository(dir.path());
    std::fs::create_dir_all(&gear).unwrap();
    std::fs::write(gear.join("gear_presets.json"), "{ not json").unwrap();
    let store = PresetStore::new(dir.path().join("presets"), "metadata");

    migrate_gear_presets(&mut repo, &gear, &bundled, &store);

    assert!(
        repo.global_setting("gear_presets_migrated")
            .unwrap()
            .is_none()
    );

    std::fs::write(
        gear.join("gear_presets.json"),
        json!([{"id": "p1", "displayName": "FM2 combo"}]).to_string(),
    )
    .unwrap();
    migrate_gear_presets(&mut repo, &gear, &bundled, &store);
    assert!(store.exists("FM2 combo"));
    assert_eq!(
        repo.global_setting("gear_presets_migrated").unwrap(),
        Some(json!(true))
    );
}

#[test]
fn unnamed_presets_are_kept_and_numbered() {
    let dir = tempfile::tempdir().unwrap();
    let gear = dir.path().join("gear");
    let bundled = dir.path().join("bundled");
    let mut repo = repository(dir.path());
    write_gear(
        &gear,
        json!([]),
        json!([]),
        json!([{"id": "p1"}, {"id": "p2"}]),
    );
    let store = PresetStore::new(dir.path().join("presets"), "metadata");

    migrate_gear_presets(&mut repo, &gear, &bundled, &store);

    let mut names = store.list();
    names.sort();
    assert_eq!(names, vec!["Unnamed preset", "Unnamed preset 2"]);
}

#[test]
fn a_separator_leaves_one_space() {
    let dir = tempfile::tempdir().unwrap();
    let gear = dir.path().join("gear");
    let bundled = dir.path().join("bundled");
    let mut repo = repository(dir.path());
    write_gear(
        &gear,
        json!([]),
        json!([]),
        json!([{"id": "p1", "displayName": "AE-1P / FD 50 f/1.4 / Portra 400"}]),
    );
    let store = PresetStore::new(dir.path().join("presets"), "metadata");

    migrate_gear_presets(&mut repo, &gear, &bundled, &store);

    assert_eq!(store.list(), vec!["AE-1P FD 50 f 1.4 Portra 400"]);
}
