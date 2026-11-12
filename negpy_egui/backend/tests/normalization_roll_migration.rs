//! Ports tests/test_normalization_roll_migration.py.

use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::migrations::normalization_roll::migrate_legacy_normalization_rolls;
use negpy_backend::services::assets::rolls::{
    all_rolls_sorted, baseline_stale, create_virtual_roll, roll_for_id, roll_normalization,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

fn repository(dir: &std::path::Path) -> StorageRepository {
    let repo = StorageRepository::new(
        dir.join("edits.db").display().to_string(),
        dir.join("settings.db").display().to_string(),
    );
    repo.initialize().unwrap();
    repo
}

fn legacy_table(repo: &StorageRepository, rows: &[(&str, Value, Value, Option<Value>)]) {
    let connection = Connection::open(&repo.edits_db_path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE normalization_rolls (
                name TEXT PRIMARY KEY,
                floors_json TEXT,
                ceils_json TEXT,
                cast_json TEXT
            );",
        )
        .unwrap();
    for (name, floors, ceils, cast) in rows {
        connection
            .execute(
                "INSERT INTO normalization_rolls VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    name,
                    floors.to_string(),
                    ceils.to_string(),
                    cast.as_ref().map(Value::to_string)
                ],
            )
            .unwrap();
    }
}

fn full_legacy_table(repo: &StorageRepository) {
    legacy_table(
        repo,
        &[
            (
                "Tri-X",
                json!([0.1, 0.1, 0.1]),
                json!([0.9, 0.9, 0.9]),
                Some(json!([0.01, 0.0, -0.01])),
            ),
            (
                "Portra 400",
                json!([0.05, 0.05, 0.05]),
                json!([0.95, 0.95, 0.95]),
                None,
            ),
            (
                "Deleted Roll",
                json!([0.2, 0.2, 0.2]),
                json!([0.8, 0.8, 0.8]),
                None,
            ),
        ],
    );
}

fn legacy_rows(repo: &StorageRepository) -> Vec<String> {
    let Ok(connection) = Connection::open(&repo.edits_db_path) else {
        return Vec::new();
    };
    let Ok(mut statement) = connection.prepare("SELECT name FROM normalization_rolls") else {
        return Vec::new();
    };
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect()
}

fn roll_id(repo: &StorageRepository, name: &str) -> String {
    all_rolls_sorted(repo)
        .into_iter()
        .find(|(_, entry)| entry.get("name").and_then(Value::as_str) == Some(name))
        .map(|(id, _)| id)
        .unwrap()
}

#[test]
fn migration_copies_each_row_onto_its_matching_roll() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    create_virtual_roll(&mut repo, "Tri-X", &[]);
    create_virtual_roll(&mut repo, "Portra 400", &[]);
    full_legacy_table(&repo);

    migrate_legacy_normalization_rolls(&mut repo);

    let tri_x = roll_id(&repo, "Tri-X");
    let saved = roll_normalization(&repo, &tri_x).unwrap();
    assert_eq!(saved["floors"], json!([0.1, 0.1, 0.1]));
    assert_eq!(saved["ceils"], json!([0.9, 0.9, 0.9]));
    assert_eq!(saved["cast"], json!([0.01, 0.0, -0.01]));
    assert_eq!(saved["axis"], Value::Null);
    assert_eq!(saved["outliers"], json!([]));
    assert_eq!(saved["frames"], Value::Null);
    assert_eq!(saved["inputs"], Value::Null);
    assert!(baseline_stale(roll_for_id(&repo, &tri_x).as_ref()));
}

#[test]
fn migration_defaults_a_missing_cast_to_zero() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    create_virtual_roll(&mut repo, "Portra 400", &[]);
    full_legacy_table(&repo);

    migrate_legacy_normalization_rolls(&mut repo);

    let saved = roll_normalization(&repo, &roll_id(&repo, "Portra 400")).unwrap();
    assert_eq!(saved["cast"], json!([0.0, 0.0, 0.0]));
}

#[test]
fn migration_keeps_an_unmatched_row_and_stays_pending() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    create_virtual_roll(&mut repo, "Tri-X", &[]);
    create_virtual_roll(&mut repo, "Portra 400", &[]);
    full_legacy_table(&repo);

    migrate_legacy_normalization_rolls(&mut repo);

    assert_eq!(legacy_rows(&repo), vec!["Deleted Roll"]);
    assert_eq!(
        repo.global_setting("normalization_rolls_migrated_v1")
            .unwrap(),
        None
    );
}

#[test]
fn migration_survives_a_first_launch_with_no_rolls_yet() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    legacy_table(
        &repo,
        &[(
            "Tri-X",
            json!([0.1, 0.1, 0.1]),
            json!([0.9, 0.9, 0.9]),
            None,
        )],
    );

    migrate_legacy_normalization_rolls(&mut repo);

    create_virtual_roll(&mut repo, "Tri-X", &[]);
    migrate_legacy_normalization_rolls(&mut repo);

    let saved = roll_normalization(&repo, &roll_id(&repo, "Tri-X")).unwrap();
    assert_eq!(saved["floors"], json!([0.1, 0.1, 0.1]));
    assert_eq!(
        repo.global_setting("normalization_rolls_migrated_v1")
            .unwrap(),
        Some(json!(true))
    );
    assert!(legacy_rows(&repo).is_empty());
    let connection = Connection::open(&repo.edits_db_path).unwrap();
    let table: Option<String> = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE name='normalization_rolls'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    assert!(table.is_none());
}

#[test]
fn migration_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let mut repo = repository(dir.path());
    create_virtual_roll(&mut repo, "Tri-X", &[]);
    full_legacy_table(&repo);

    migrate_legacy_normalization_rolls(&mut repo);
    migrate_legacy_normalization_rolls(&mut repo);

    let saved = roll_normalization(&repo, &roll_id(&repo, "Tri-X")).unwrap();
    assert_eq!(saved["floors"], json!([0.1, 0.1, 0.1]));
}
