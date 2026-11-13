//! Move legacy DB-backed flat-field profiles into the NPZ store.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};
use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::flatfield::FlatFieldProfiles;

use super::common::{rewrite_rows, save_setting, setting_truthy};

const DONE_FLAG: &str = "flatfield_migrated_v2";

fn table_exists(connection: &Connection, name: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|row| row.is_some())
}

fn rewrite_configs(
    connection: &Connection,
    table: &str,
    key_columns: &[&str],
    path_to_id: &HashMap<String, String>,
) -> rusqlite::Result<()> {
    rewrite_rows(connection, table, key_columns, |data| {
        let Some(path) = data.remove("reference_path") else {
            return false;
        };
        let path = path.as_str().unwrap_or_default();
        data.insert(
            "profile_id".to_string(),
            Value::String(path_to_id.get(path).cloned().unwrap_or_default()),
        );
        true
    })
}

/// Bake each legacy profile, repoint edits and the active setting at its id, drop the table.
pub fn migrate_legacy_flatfield_profiles(repo: &mut StorageRepository, store: &FlatFieldProfiles) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let result = (|| -> rusqlite::Result<Option<String>> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let mut active_name = None;
        if table_exists(&connection, "flatfield_profiles")? {
            let rows = {
                let mut statement =
                    connection.prepare("SELECT name, path, k1 FROM flatfield_profiles")?;
                let rows = statement.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let mut name_to_id = HashMap::new();
            let mut path_to_id = HashMap::new();
            for (name, path, k1) in rows {
                let source = path.unwrap_or_default();
                let Some(new_id) = store.create(&name, &source, k1.unwrap_or(0.0)) else {
                    log::warn!("Flat-field migration: could not bake profile {name:?}");
                    continue;
                };
                name_to_id.insert(name, new_id.clone());
                if !source.is_empty() {
                    path_to_id.insert(source, new_id);
                }
            }
            let transaction = connection.unchecked_transaction()?;
            rewrite_configs(&transaction, "file_settings", &["file_hash"], &path_to_id)?;
            if table_exists(&transaction, "edit_history")? {
                rewrite_configs(&transaction, "edit_history", &["rowid"], &path_to_id)?;
            }
            transaction.execute("DROP TABLE flatfield_profiles", [])?;
            transaction.commit()?;

            active_name = Some(
                repo.global_setting("flatfield_active_profile")
                    .ok()
                    .flatten()
                    .filter(|value| !value.is_null())
                    .map(|value| value.as_str().unwrap_or_default().to_string())
                    .unwrap_or_default(),
            );
            active_name =
                active_name.map(|active| name_to_id.get(&active).cloned().unwrap_or_default());
        }
        Ok(active_name)
    })();
    match result {
        Ok(Some(active_id)) => {
            if !active_id.is_empty() {
                let mut settings = Map::new();
                settings.insert(
                    "flatfield_active_profile".to_string(),
                    Value::String(active_id),
                );
                if let Err(error) = repo.save_global_settings(&settings) {
                    log::error!("Could not remap the active Flat-field profile: {error}");
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            log::error!("Flat-field migration failed; continuing without it: {error}");
        }
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, Value::Bool(true)) {
        log::error!("Could not record the Flat-field migration: {error}");
    }
}
