//! Turn off the negative-default autos on edits already marked Positive.

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::rolls::{rolls_containing_path, saved_rolls};

use super::common::{save_setting, setting_truthy, string, truthy};

const DONE_FLAG: &str = "auto_meter_positive_migrated_v1";

fn positive_rolls(repo: &StorageRepository) -> Vec<String> {
    saved_rolls(repo)
        .into_iter()
        .filter(|(_, entry)| {
            entry
                .get("defaults")
                .and_then(Value::as_object)
                .and_then(|defaults| defaults.get("positive_source"))
                .is_some_and(truthy)
        })
        .map(|(roll_id, _)| roll_id)
        .collect()
}

fn effective_positive(
    repo: &StorageRepository,
    positive_rolls: &[String],
    data: &Map<String, Value>,
    file_path: Option<&str>,
) -> bool {
    if string(data.get("process_mode")) != "Transparency" {
        return false;
    }
    if data.get("positive_source").is_some_and(truthy) {
        return true;
    }
    let Some(file_path) = file_path.filter(|path| !path.is_empty()) else {
        return false;
    };
    positive_rolls
        .iter()
        .any(|roll_id| rolls_containing_path(repo, file_path).contains(roll_id))
}

fn turn_off_autos(data: &mut Map<String, Value>) -> bool {
    let mut changed = false;
    for field in ["auto_exposure", "auto_normalize_contrast"] {
        if data.get(field).is_none_or(truthy) {
            data.insert(field.to_string(), json!(false));
            changed = true;
        }
    }
    changed
}

/// Turn off the negative-default Auto Density and Auto Grade on Positive frames, once.
///
/// A saved choice that differs from the default stays. A missing row or a malformed edit
/// is skipped, and a failed sweep still records the flag as the Python migration does.
pub fn migrate_auto_meter_for_positive_frames(repo: &mut StorageRepository) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let positive_rolls = positive_rolls(repo);
    let result = (|| -> rusqlite::Result<()> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        let mut statement =
            transaction.prepare("SELECT file_hash, settings_json, file_path FROM file_settings")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?;
        let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for (file_hash, settings, file_path) in rows {
            let Some(Value::Object(mut data)) = settings
                .filter(|settings| !settings.is_empty())
                .and_then(|settings| serde_json::from_str::<Value>(&settings).ok())
            else {
                continue;
            };
            if !effective_positive(repo, &positive_rolls, &data, file_path.as_deref()) {
                continue;
            }
            if turn_off_autos(&mut data) {
                transaction.execute(
                    "UPDATE file_settings SET settings_json = ? WHERE file_hash = ?",
                    rusqlite::params![Value::Object(data).to_string(), file_hash],
                )?;
            }
        }
        transaction.commit()
    })();
    if let Err(error) = result {
        log::error!(
            "Auto Density/Grade Positive-frame migration failed; continuing without it: {error}"
        );
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, json!(true)) {
        log::error!("Could not record the Positive-frame migration: {error}");
    }
}
