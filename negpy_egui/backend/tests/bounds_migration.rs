//! Ports tests/test_bounds_migration.py's stale-row sweep.

use negpy_backend::domain::models::workspace::{BOUNDS_VERSION, WorkspaceConfig};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::bounds::migrate_stale_frame_bounds;
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

#[test]
fn saved_bounds_are_dropped_in_every_edit_table() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let measured = json!({
        "local_floors": [-1.1, -1.2, -1.0],
        "local_ceils": [-0.2, -0.4, -0.6],
    });
    insert(
        &repo,
        "file_settings",
        json!({"file_hash": "h1", "settings_json": measured.to_string()}),
    );
    insert(
        &repo,
        "edit_history",
        json!({"file_hash": "h1", "step_index": 0, "settings_json": measured.to_string()}),
    );
    insert(
        &repo,
        "work_prints",
        json!({"file_hash": "h1", "name": "v1", "created_at": 0.0, "settings_json": measured.to_string()}),
    );

    migrate_stale_frame_bounds(&mut repo);

    for table in ["file_settings", "edit_history", "work_prints"] {
        let saved = settings(&repo, table, "h1");
        assert_eq!(saved["local_floors"], json!([0.0, 0.0, 0.0]), "{table}");
        assert_eq!(saved["local_ceils"], json!([0.0, 0.0, 0.0]), "{table}");
    }
    assert_eq!(
        repo.global_setting("bounds_version").unwrap(),
        Some(json!(BOUNDS_VERSION))
    );
}

#[test]
fn locked_bounds_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    let measured = json!({
        "lock_bounds": true,
        "local_floors": [-1.1, -1.2, -1.0],
        "local_ceils": [-0.2, -0.4, -0.6],
    });
    insert(
        &repo,
        "file_settings",
        json!({"file_hash": "h1", "settings_json": measured.to_string()}),
    );

    migrate_stale_frame_bounds(&mut repo);

    assert_eq!(
        settings(&repo, "file_settings", "h1")["local_floors"],
        measured["local_floors"]
    );
}

#[test]
fn bounds_saved_under_the_current_version_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    migrate_stale_frame_bounds(&mut repo);
    let mut config = WorkspaceConfig::default();
    config.process.local_floors = [-1.1, -1.2, -1.0];
    repo.save_file_settings("h1", &config, "").unwrap();

    migrate_stale_frame_bounds(&mut repo);

    assert_eq!(
        repo.load_file_settings("h1")
            .unwrap()
            .unwrap()
            .process
            .local_floors,
        [-1.1, -1.2, -1.0]
    );
}

#[test]
fn malformed_rows_do_not_stop_the_sweep() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    insert(
        &repo,
        "file_settings",
        json!({"file_hash": "bad", "settings_json": "not json"}),
    );
    insert(
        &repo,
        "file_settings",
        json!({
            "file_hash": "good",
            "settings_json": json!({"local_floors": [-1.0, -1.0, -1.0]}).to_string(),
        }),
    );

    migrate_stale_frame_bounds(&mut repo);

    let connection = Connection::open(&repo.edits_db_path).unwrap();
    let raw: String = connection
        .query_row(
            "SELECT settings_json FROM file_settings WHERE file_hash = 'bad'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(raw, "not json");
    assert_eq!(
        settings(&repo, "file_settings", "good")["local_floors"],
        json!([0.0, 0.0, 0.0])
    );
}
