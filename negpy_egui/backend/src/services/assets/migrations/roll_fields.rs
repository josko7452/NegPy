//! Lock the frame cards whose new roll-default fields already carry a frame value.

use serde_json::{Map, Value};

use crate::domain::models::ProcessConfig;
use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::rolls::{rolls_containing_path, set_frame_override, unforked_hash};

use super::common::{save_setting, setting_truthy};

const DONE_FLAG: &str = "roll_field_locks_migrated_v1";
const FORK_SEPARATOR: &str = "#roll:";

const NEW_ROLL_FIELDS: &[(&str, &[&str])] = &[
    (
        "process",
        &[
            "white_point_offset",
            "black_point_offset",
            "white_point_trim_red",
            "white_point_trim_green",
            "white_point_trim_blue",
            "black_point_trim_red",
            "black_point_trim_green",
            "black_point_trim_blue",
        ],
    ),
    ("demosaic", &["highlight_reconstruction"]),
];

fn changed_cards(data: &Map<String, Value>, defaults: &Map<String, Value>) -> Vec<&'static str> {
    NEW_ROLL_FIELDS
        .iter()
        .filter_map(|(card, fields)| {
            let changed = fields.iter().any(|field| {
                let default = defaults.get(*field);
                data.get(*field).unwrap_or(default.unwrap_or(&Value::Null))
                    != default.unwrap_or(&Value::Null)
            });
            changed.then_some(*card)
        })
        .collect()
}

fn rolls_for_row(
    repo: &StorageRepository,
    file_hash: &str,
    file_path: Option<&str>,
) -> Vec<String> {
    let base = unforked_hash(file_hash);
    if base != file_hash {
        return file_hash
            .find(FORK_SEPARATOR)
            .map(|index| file_hash[index + FORK_SEPARATOR.len()..].to_string())
            .into_iter()
            .collect();
    }
    file_path
        .filter(|path| !path.is_empty())
        .map_or_else(Vec::new, |path| rolls_containing_path(repo, path))
}

/// Lock each affected card on every frame whose saved value differs from its default.
///
/// A fork locks only its own roll, and the lock uses the unforked hash. A failed sweep
/// still records the flag, as the Python migration does.
pub fn migrate_new_roll_field_locks(repo: &mut StorageRepository) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let defaults = serde_json::to_value(ProcessConfig::default())
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let result = (|| -> rusqlite::Result<()> {
        let connection = rusqlite::Connection::open(&repo.edits_db_path)?;
        let mut statement =
            connection.prepare("SELECT file_hash, settings_json, file_path FROM file_settings")?;
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
            let data = settings
                .as_deref()
                .and_then(|settings| serde_json::from_str::<Value>(settings).ok())
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();
            let cards = changed_cards(&data, &defaults);
            if cards.is_empty() {
                continue;
            }
            let base = unforked_hash(&file_hash);
            for roll_id in rolls_for_row(repo, &file_hash, file_path.as_deref()) {
                for card in &cards {
                    set_frame_override(repo, &roll_id, &base, card, true, None);
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        log::error!("New roll-field lock migration failed; continuing without it: {error}");
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, Value::Bool(true)) {
        log::error!("Could not record the roll-field lock migration: {error}");
    }
}
