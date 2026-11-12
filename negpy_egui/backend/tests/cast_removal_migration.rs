//! Ports tests/test_cast_removal_migration.py.

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::cast_removal::migrate_legacy_slide_cast_removal;
use rusqlite::Connection;
use serde_json::{Value, json};

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn insert(repo: &StorageRepository, table: &str, values: Value) {
    let connection = Connection::open(&repo.edits_db_path).unwrap();
    let fields = values.as_object().unwrap();
    let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
    names.sort();
    let placeholder = vec!["?"; names.len()].join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholder})",
        names.join(", ")
    );
    let parameters: Vec<rusqlite::types::Value> = names
        .iter()
        .map(|name| match &fields[*name] {
            Value::String(value) => rusqlite::types::Value::Text(value.clone()),
            Value::Number(value) => {
                rusqlite::types::Value::Real(value.as_f64().unwrap_or_default())
            }
            Value::Null => rusqlite::types::Value::Null,
            _ => panic!("unsupported test value for {name}"),
        })
        .collect();
    connection
        .execute(&sql, rusqlite::params_from_iter(parameters))
        .unwrap();
}

fn settings(repo: &StorageRepository, table: &str, key: &str) -> Value {
    let connection = Connection::open(&repo.edits_db_path).unwrap();
    let text: String = connection
        .query_row(
            &format!("SELECT settings_json FROM {table} WHERE file_hash = ?1"),
            [key],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn save(repo: &StorageRepository, table: &str, hash: &str, mode: &str, strength: f64) {
    let mut row = json!({
        "file_hash": hash,
        "settings_json": json!({
            "process_mode": mode,
            "cast_removal_strength": strength,
        })
        .to_string(),
    });
    if table == "edit_history" {
        row["step_index"] = json!(0);
    }
    if table == "work_prints" {
        row["name"] = json!("v1");
        row["created_at"] = json!(0.0);
    }
    insert(repo, table, row);
}

#[test]
fn a_legacy_slide_at_the_shipped_default_is_zeroed() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "file_settings", "hash1", "Transparency", 0.5);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.0)
    );
}

#[test]
fn a_chosen_slide_strength_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "file_settings", "hash1", "Transparency", 0.8);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.8)
    );
}

#[test]
fn a_negative_at_the_shipped_default_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "file_settings", "hash1", "Color Negative", 0.5);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.5)
    );
}

#[test]
fn the_legacy_e6_mode_name_is_swept_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "file_settings", "hash1", "E-6", 0.5);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.0)
    );
}

#[test]
fn edit_history_and_work_prints_are_swept_too() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "edit_history", "hash1", "Transparency", 0.5);
    save(&repo, "work_prints", "hash1", "Transparency", 0.5);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "edit_history", "hash1")["cast_removal_strength"],
        json!(0.0)
    );
    assert_eq!(
        settings(&repo, "work_prints", "hash1")["cast_removal_strength"],
        json!(0.0)
    );
}

#[test]
fn migration_sets_the_done_flag() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    migrate_legacy_slide_cast_removal(&mut repo);
    assert_eq!(
        repo.global_setting("cast_removal_slide_migrated_v1")
            .unwrap(),
        Some(json!(true))
    );
}

#[test]
fn migration_does_not_rerun_on_a_later_deliberate_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    save(&repo, "file_settings", "hash1", "Transparency", 0.5);
    migrate_legacy_slide_cast_removal(&mut repo);
    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.0)
    );

    let connection = Connection::open(&repo.edits_db_path).unwrap();
    connection
        .execute(
            "UPDATE file_settings SET settings_json = ?1 WHERE file_hash = 'hash1'",
            [json!({"process_mode": "Transparency", "cast_removal_strength": 0.5}).to_string()],
        )
        .unwrap();
    drop(connection);

    migrate_legacy_slide_cast_removal(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "hash1")["cast_removal_strength"],
        json!(0.5)
    );
}
