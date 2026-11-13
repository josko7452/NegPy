//! Ports the observable rules of `tests/test_flatfield_migration.py`.

use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::flatfield::{Baked, FlatFieldProfiles};
use negpy_backend::services::assets::migrations::flatfield::migrate_legacy_flatfield_profiles;
use serde_json::Value;

fn baked(_: &str) -> Option<Baked> {
    Some(Baked {
        gain: ImageBuffer::from_rgb(8, 8, vec![1.0; 8 * 8 * 3]),
        check: negpy_backend::features::flatfield::logic::Evenness::default(),
        reference: ImageBuffer::from_rgb(8, 8, vec![0.4; 8 * 8 * 3]),
    })
}

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn store(dir: &std::path::Path) -> FlatFieldProfiles {
    FlatFieldProfiles {
        directory: dir.join("flatfield"),
        bake: baked,
    }
}

fn legacy_repo(dir: &std::path::Path) -> StorageRepository {
    let mut repo = repository(dir);
    let connection = rusqlite::Connection::open(&repo.edits_db_path).unwrap();
    connection
        .execute(
            "CREATE TABLE flatfield_profiles (name TEXT PRIMARY KEY, path TEXT, k1 REAL DEFAULT 0.0)",
            [],
        )
        .unwrap();
    for (name, path, k1) in [
        ("rig-a", dir.join("a.dng").display().to_string(), -0.05),
        ("rig-b", dir.join("b.dng").display().to_string(), 0.0),
    ] {
        connection
            .execute(
                "INSERT INTO flatfield_profiles (name, path, k1) VALUES (?1, ?2, ?3)",
                rusqlite::params![name, path, k1],
            )
            .unwrap();
    }
    let edit = serde_json::json!({
        "apply": true,
        "reference_path": dir.join("a.dng").display().to_string(),
        "k1": -0.05,
        "density": 1.0,
    });
    connection
        .execute(
            "INSERT INTO file_settings (file_hash, settings_json) VALUES (?1, ?2)",
            rusqlite::params!["hash1", edit.to_string()],
        )
        .unwrap();
    let mut settings = serde_json::Map::new();
    settings.insert(
        "flatfield_active_profile".to_string(),
        Value::String("rig-a".to_string()),
    );
    repo.save_global_settings(&settings).unwrap();
    repo
}

fn config(repo: &StorageRepository, file_hash: &str) -> Value {
    let connection = rusqlite::Connection::open(&repo.edits_db_path).unwrap();
    let settings: String = connection
        .query_row(
            "SELECT settings_json FROM file_settings WHERE file_hash = ?1",
            [file_hash],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&settings).unwrap()
}

fn has_legacy_table(repo: &StorageRepository) -> bool {
    let connection = rusqlite::Connection::open(&repo.edits_db_path).unwrap();
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE name='flatfield_profiles'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .is_ok()
}

#[test]
fn the_migration_bakes_profiles_and_repoints_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = legacy_repo(dir.path());
    let store = store(dir.path());

    migrate_legacy_flatfield_profiles(&mut repo, &store);

    let profiles: std::collections::HashMap<String, String> = store
        .list_profiles()
        .into_iter()
        .map(|(id, name)| (name, id))
        .collect();
    assert_eq!(
        profiles
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        ["rig-a".to_string(), "rig-b".to_string()]
            .into_iter()
            .collect()
    );
    assert_eq!(store.get(&profiles["rig-a"]).unwrap().k1, -0.05);
    let edit = config(&repo, "hash1");
    assert_eq!(edit["profile_id"], Value::String(profiles["rig-a"].clone()));
    assert!(edit.get("reference_path").is_none());
    assert_eq!(
        repo.global_setting("flatfield_active_profile").unwrap(),
        Some(Value::String(profiles["rig-a"].clone()))
    );
    assert!(!has_legacy_table(&repo));
    assert_eq!(
        repo.global_setting("flatfield_migrated_v2").unwrap(),
        Some(Value::Bool(true))
    );
}

#[test]
fn the_migration_runs_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = legacy_repo(dir.path());
    let store = store(dir.path());
    migrate_legacy_flatfield_profiles(&mut repo, &store);
    let count = store.list_profiles().len();

    migrate_legacy_flatfield_profiles(&mut repo, &store);

    assert_eq!(store.list_profiles().len(), count);
}

#[test]
fn no_legacy_table_only_sets_the_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let store = store(dir.path());

    migrate_legacy_flatfield_profiles(&mut repo, &store);

    assert!(store.list_profiles().is_empty());
    assert_eq!(
        repo.global_setting("flatfield_migrated_v2").unwrap(),
        Some(Value::Bool(true))
    );
}
