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

const BASELINE_SPLIT_FLAG: &str = "baseline_card_split_v1";

/// Give every frame locked on the old Normalization card the new baseline half.
///
/// The old `process` lock covered Use Luma/Color Average before they moved to the
/// `baseline` card. Keeping both halves means the frame renders as it did.
pub fn migrate_baseline_card_split(repo: &mut StorageRepository) {
    if setting_truthy(repo, BASELINE_SPLIT_FLAG) {
        return;
    }
    let saved = crate::services::assets::rolls::saved_rolls(repo);
    for (roll_id, entry) in saved {
        let overrides = entry
            .get("frame_overrides")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (file_hash, cards) in overrides {
            let process_locked = cards
                .as_array()
                .is_some_and(|cards| cards.iter().any(|card| card.as_str() == Some("process")));
            if process_locked {
                set_frame_override(repo, &roll_id, &file_hash, "baseline", true, None);
            }
        }
    }
    if let Err(error) = save_setting(repo, BASELINE_SPLIT_FLAG, Value::Bool(true)) {
        log::error!("Could not record the baseline card split: {error}");
    }
}

const CAST_REMOVAL_FLAG: &str = "cast_removal_to_frames_v1";

/// Move each roll's Cast Removal onto the frames that followed it.
///
/// A saved frame keeps its resolved value, a frame with no saved edit takes the value from
/// the roll's frame seed, and the retired card locks are dropped.
pub fn migrate_cast_removal_to_frames(repo: &mut StorageRepository) {
    if setting_truthy(repo, CAST_REMOVAL_FLAG) {
        return;
    }
    let saved = crate::services::assets::rolls::saved_rolls(repo);
    let roll_values: Map<String, Value> = saved
        .iter()
        .filter_map(|(roll_id, entry)| {
            entry
                .get("defaults")
                .and_then(Value::as_object)
                .and_then(|defaults| defaults.get("cast_removal_strength"))
                .map(|value| (roll_id.clone(), value.clone()))
        })
        .collect();
    let result = (|| -> rusqlite::Result<()> {
        let connection = rusqlite::Connection::open(&repo.edits_db_path)?;
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
            let base = unforked_hash(&file_hash);
            let roll_id = rolls_for_row(repo, &file_hash, file_path.as_deref())
                .into_iter()
                .find(|roll_id| {
                    roll_values.contains_key(roll_id)
                        && !crate::services::assets::rolls::frame_override_cards(
                            repo, roll_id, &base,
                        )
                        .iter()
                        .any(|card| card == "cast_removal")
                });
            let Some(roll_id) = roll_id else {
                continue;
            };
            let mut data = match settings.filter(|settings| !settings.is_empty()) {
                Some(settings) => match serde_json::from_str::<Value>(&settings) {
                    Ok(Value::Object(data)) => data,
                    _ => continue,
                },
                None => Map::new(),
            };
            data.insert(
                "cast_removal_strength".to_string(),
                roll_values[&roll_id].clone(),
            );
            transaction.execute(
                "UPDATE file_settings SET settings_json = ? WHERE file_hash = ?",
                rusqlite::params![Value::Object(data).to_string(), file_hash],
            )?;
        }
        transaction.commit()
    })();
    if let Err(error) = result {
        log::error!("Cast Removal move to Filtration failed; continuing without it: {error}");
    }
    for (roll_id, entry) in &saved {
        let overrides = entry
            .get("frame_overrides")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (file_hash, cards) in overrides {
            let locked = cards.as_array().is_some_and(|cards| {
                cards
                    .iter()
                    .any(|card| card.as_str() == Some("cast_removal"))
            });
            if locked {
                set_frame_override(repo, roll_id, &file_hash, "cast_removal", false, None);
            }
        }
        if let Some(value) = roll_values.get(roll_id) {
            let mut seed = Map::new();
            seed.insert("cast_removal_strength".to_string(), value.clone());
            crate::services::assets::rolls::add_frame_seed(repo, roll_id, seed);
            crate::services::assets::rolls::restore_roll_fields(
                repo,
                roll_id,
                Map::new(),
                &["cast_removal_strength".to_string()],
            );
        }
    }
    if let Err(error) = save_setting(repo, CAST_REMOVAL_FLAG, Value::Bool(true)) {
        log::error!("Could not record the Cast Removal migration: {error}");
    }
}

const SECTION_PUSHES_FLAG: &str = "section_pushes_dropped_v1";

/// Drop the retired whole-roll apply records, once.
pub fn migrate_drop_section_pushes(repo: &mut StorageRepository) {
    if setting_truthy(repo, SECTION_PUSHES_FLAG) {
        return;
    }
    crate::services::assets::rolls::drop_section_pushes(repo);
    if let Err(error) = save_setting(repo, SECTION_PUSHES_FLAG, Value::Bool(true)) {
        log::error!("Could not record the section-push cleanup: {error}");
    }
}

const RAW_DECODE_SPLIT_FLAG: &str = "raw_decode_linear_raw_v1";

/// Give every frame locked on the old Calibration card the new Raw Decode half.
pub fn migrate_linear_raw_card_split(repo: &mut StorageRepository) {
    if setting_truthy(repo, RAW_DECODE_SPLIT_FLAG) {
        return;
    }
    let saved = crate::services::assets::rolls::saved_rolls(repo);
    for (roll_id, entry) in saved {
        let overrides = entry
            .get("frame_overrides")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (file_hash, cards) in overrides {
            let sensor_locked = cards
                .as_array()
                .is_some_and(|cards| cards.iter().any(|card| card.as_str() == Some("sensor")));
            if sensor_locked {
                set_frame_override(repo, &roll_id, &file_hash, "demosaic", true, None);
            }
        }
    }
    if let Err(error) = save_setting(repo, RAW_DECODE_SPLIT_FLAG, Value::Bool(true)) {
        log::error!("Could not record the Raw Decode card split: {error}");
    }
}

const AUTO_CROP_FLAG: &str = "auto_crop_roll_card_v1";

/// Lock the Auto Crop card on every frame saved with Auto Crop on.
///
/// A hand-drawn crop needs no lock, so only `crop_from_auto` rows are read.
pub fn migrate_auto_crop_roll_locks(repo: &mut StorageRepository) {
    if setting_truthy(repo, AUTO_CROP_FLAG) {
        return;
    }
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
            if !data
                .get("crop_from_auto")
                .is_some_and(super::common::truthy)
            {
                continue;
            }
            let base = unforked_hash(&file_hash);
            for roll_id in rolls_for_row(repo, &file_hash, file_path.as_deref()) {
                set_frame_override(repo, &roll_id, &base, "auto_crop", true, None);
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        log::error!("Auto Crop roll-card lock migration failed; continuing without it: {error}");
    }
    if let Err(error) = save_setting(repo, AUTO_CROP_FLAG, Value::Bool(true)) {
        log::error!("Could not record the Auto Crop lock migration: {error}");
    }
}

const FILM_ENCODING_FLAG: &str = "film_card_source_encoding_v1";

/// Give each roll with Film defaults the Auto encoding, once.
pub fn migrate_film_card_source_encoding(repo: &mut StorageRepository) {
    if setting_truthy(repo, FILM_ENCODING_FLAG) {
        return;
    }
    for (roll_id, entry) in crate::services::assets::rolls::saved_rolls(repo) {
        let defaults = entry
            .get("defaults")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if !defaults.contains_key("source_encoding")
            && (defaults.contains_key("process_mode") || defaults.contains_key("positive_source"))
        {
            let mut fields = Map::new();
            fields.insert(
                "source_encoding".to_string(),
                Value::String("auto".to_string()),
            );
            crate::services::assets::rolls::set_roll_defaults(repo, &roll_id, fields);
        }
    }
    if let Err(error) = save_setting(repo, FILM_ENCODING_FLAG, Value::Bool(true)) {
        log::error!("Could not record the Film card encoding migration: {error}");
    }
}
