//! Move legacy name-keyed roll baselines onto their Library rolls.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::rolls::{saved_rolls, set_roll_normalization};

use super::common::{save_setting, setting_truthy};

const DONE_FLAG: &str = "normalization_rolls_migrated_v1";

fn numbers(value: &str) -> Vec<f64> {
    serde_json::from_str::<Vec<Value>>(value)
        .ok()
        .map(|values| {
            values
                .iter()
                .map(|value| value.as_f64().unwrap_or(0.0))
                .collect()
        })
        .unwrap_or_default()
}

/// Copy each legacy baseline onto the roll with the same name.
///
/// A name with no roll stays in the legacy table, so the migration retries after the
/// folder is imported.
pub fn migrate_legacy_normalization_rolls(repo: &mut StorageRepository) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let result = (|| -> rusqlite::Result<()> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let exists: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='normalization_rolls'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(());
        }
        let by_name: HashMap<String, String> = saved_rolls(repo)
            .into_iter()
            .filter_map(|(roll_id, entry)| {
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|name| (name.to_string(), roll_id))
            })
            .collect();
        let transaction = connection.unchecked_transaction()?;
        let mut statement = transaction
            .prepare("SELECT name, floors_json, ceils_json, cast_json FROM normalization_rolls")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut unmatched = false;
        for (name, floors, ceils, cast) in rows {
            let Some(roll_id) = by_name.get(&name) else {
                unmatched = true;
                continue;
            };
            set_roll_normalization(
                repo,
                roll_id,
                &numbers(&floors),
                &numbers(&ceils),
                &cast.map_or([0.0; 3].to_vec(), |value| numbers(&value)),
                &[],
                None,
                0,
                None,
                None,
            );
            transaction.execute("DELETE FROM normalization_rolls WHERE name = ?1", [&name])?;
        }
        if !unmatched {
            transaction.execute("DROP TABLE normalization_rolls", [])?;
        }
        transaction.commit()?;
        if unmatched {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    })();
    if let Err(error) = result {
        if !matches!(error, rusqlite::Error::QueryReturnedNoRows) {
            log::error!("Normalization-roll migration failed; continuing without it: {error}");
        }
        return;
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, json!(true)) {
        log::error!("Could not record the normalization-roll migration: {error}");
    }
}
