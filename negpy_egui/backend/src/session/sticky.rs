//! Settings that carry onto a freshly opened file.
//!
//! Mirrors `negpy/desktop/sticky.py`. The user chooses catalog rows; their values come
//! from a flat snapshot of the last saved edit.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::domain::models::{ExportConfig, WorkspaceConfig};
use crate::infrastructure::storage::repository::StorageRepository;
use crate::kernel::system::paths::default_user_dir;
use crate::session::settings_catalog::{
    all_rows, default_sticky_ids, rows_by_id, selected_flat_dict,
};

pub const STICKY_CONFIG_KEY: &str = "sticky_config";
pub const STICKY_ROWS_KEY: &str = "sticky_rows";
pub const DESCRIPTION_FIELDS_KEY: &str = "last_description_fields";
pub const BRUSH_SIZE_KEY: &str = "last_brush_size";

/// Export fields the catalog does not list: workspace paths and the ICC paths.
#[must_use]
pub fn export_remainder() -> HashSet<String> {
    let export_fields = ExportConfig::field_names();
    let catalog_fields: HashSet<&str> = all_rows()
        .iter()
        .filter(|row| row.section == "export")
        .flat_map(|row| row.fields.iter().copied())
        .collect();
    export_fields
        .into_iter()
        .filter(|field| !catalog_fields.contains(field.as_str()))
        .collect()
}

/// Scan-setup preferences, carried unconditionally.
pub const ALWAYS_STICKY_PROCESS: [(&str, &str); 3] = [
    ("last_linear_raw", "linear_raw"),
    ("last_demosaic_preview", "demosaic_preview"),
    ("last_demosaic_export", "demosaic_export"),
];

/// Catalog rows the carry picker leaves out because they always carry.
#[must_use]
pub fn always_sticky_row_ids() -> HashSet<String> {
    all_rows()
        .into_iter()
        .filter(|row| {
            row.section == "process"
                && ALWAYS_STICKY_PROCESS
                    .iter()
                    .any(|(_, field)| row.fields.contains(field))
        })
        .map(|row| row.id())
        .collect()
}

/// The rows the user chose to carry, or the shipped default.
#[must_use]
pub fn load_sticky_ids(repo: &StorageRepository) -> Vec<String> {
    let stored = repo.global_setting(STICKY_ROWS_KEY).ok().flatten();
    let ids = match stored {
        Some(Value::Array(values)) => values
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<HashSet<_>>(),
        _ => default_sticky_ids(),
    };
    rows_by_id()
        .into_keys()
        .filter(|id| ids.contains(id))
        .collect()
}

/// Persist the user's row choice in catalog order.
pub fn save_sticky_ids(repo: &mut StorageRepository, ids: &[String]) {
    let rows = rows_by_id();
    let mut ordered: Vec<&String> = rows.keys().filter(|id| ids.contains(id)).collect();
    ordered.sort();
    let values: Vec<Value> = ordered.into_iter().map(|id| json!(id)).collect();
    let mut settings = Map::new();
    settings.insert(STICKY_ROWS_KEY.to_string(), Value::Array(values));
    let _ = repo.save_global_settings(&settings);
}

/// Every catalog-reachable field, so any row can be made sticky later.
#[must_use]
pub fn sticky_snapshot(config: &WorkspaceConfig) -> Map<String, Value> {
    let mut flat = selected_flat_dict(config, &all_rows());
    flat.remove("description_fields");
    flat
}

/// The stored sticky edit, or `None` when there is none.
#[must_use]
pub fn load_sticky_config(repo: &StorageRepository) -> Option<WorkspaceConfig> {
    match repo.global_setting(STICKY_CONFIG_KEY).ok().flatten() {
        Some(Value::Object(flat)) if !flat.is_empty() => {
            Some(WorkspaceConfig::from_flat_dict(&flat))
        }
        _ => None,
    }
}

const LEGACY_KEYS: [(&str, &str); 24] = [
    ("last_process_mode", "process_mode"),
    ("last_analysis_buffer", "analysis_buffer"),
    ("last_luma_range_clip", "luma_range_clip"),
    ("last_color_range_clip", "color_range_clip"),
    ("last_crosstalk_strength", "crosstalk_strength"),
    ("last_crosstalk_matrix", "crosstalk_matrix"),
    ("last_crosstalk_profile", "crosstalk_profile"),
    ("last_sensor_matrix", "sensor_matrix"),
    ("last_sensor_profile", "sensor_profile"),
    ("last_hue_trim", "hue_trim"),
    ("last_aspect_ratio", "autocrop_ratio"),
    ("last_autocrop_mode", "autocrop_mode"),
    ("last_autocrop_offset", "autocrop_offset"),
    ("last_autocrop_rebate_trim", "autocrop_rebate_trim"),
    ("last_flip_horizontal", "flip_horizontal"),
    ("last_flip_vertical", "flip_vertical"),
    ("last_auto_exposure", "auto_exposure"),
    ("last_auto_normalize_contrast", "auto_normalize_contrast"),
    ("last_paper_dmin", "paper_dmin"),
    ("last_paper_black", "paper_black"),
    ("last_cast_removal_strength", "cast_removal_strength"),
    ("last_paper_profile", "paper_profile"),
    ("last_dust_remove", "dust_remove"),
    (
        "last_protect_original_metadata",
        "protect_original_metadata",
    ),
];

const LEGACY_DICT_KEYS: [&str; 2] = ["last_lab_config", "last_export_config"];

/// Seed the snapshot from the superseded per-key store, once.
pub fn migrate_legacy(repo: &mut StorageRepository) {
    if repo
        .global_setting(STICKY_CONFIG_KEY)
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }
    let mut flat = Map::new();
    for (legacy_key, field) in LEGACY_KEYS {
        if let Ok(Some(value)) = repo.global_setting(legacy_key) {
            flat.insert(field.to_string(), value);
        }
    }
    for legacy_key in LEGACY_DICT_KEYS {
        if let Ok(Some(Value::Object(stored))) = repo.global_setting(legacy_key) {
            flat.extend(stored);
        }
    }
    if !flat.contains_key("paper_black")
        && let Ok(Some(legacy_bpc)) = repo.global_setting("last_true_black")
    {
        flat.insert(
            "paper_black".to_string(),
            json!(!legacy_bpc.as_bool().unwrap_or(false)),
        );
    }
    if flat.is_empty() {
        return;
    }
    let known: HashSet<&str> = all_rows()
        .iter()
        .flat_map(|row| row.fields.iter().copied())
        .collect();
    let filtered: Map<String, Value> = flat
        .into_iter()
        .filter(|(key, _)| known.contains(key.as_str()))
        .collect();
    let mut settings = Map::new();
    settings.insert(STICKY_CONFIG_KEY.to_string(), Value::Object(filtered));
    let _ = repo.save_global_settings(&settings);
}

const EXPORT_DESTINATION_MIGRATED_KEY: &str = "export_destination_migrated_v1";
const AUTO_CROP_ROW_MIGRATED_KEY: &str = "auto_crop_sticky_row_v1";
const SQUARE_ROW_MIGRATED_KEY: &str = "sticky_rows_square_before_crop_v1";

/// Drop an untouched legacy export destination once, so it tracks the current default.
pub fn migrate_legacy_export_destination(repo: &mut StorageRepository) {
    if flag_set(repo, EXPORT_DESTINATION_MIGRATED_KEY) {
        return;
    }
    let mut settings = Map::new();
    settings.insert(EXPORT_DESTINATION_MIGRATED_KEY.to_string(), json!(true));
    let _ = repo.save_global_settings(&settings);
    let Ok(Some(Value::Object(sticky_export))) = repo.global_setting("last_export_config") else {
        return;
    };
    if sticky_export.get("output_mode").and_then(Value::as_str) != Some("absolute") {
        return;
    }
    let default_export = default_user_dir().join("export").display().to_string();
    if sticky_export.get("export_path").and_then(Value::as_str) != Some(default_export.as_str()) {
        return;
    }
    let remaining: Map<String, Value> = sticky_export
        .into_iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "output_mode" | "export_path" | "output_subfolder"
            )
        })
        .collect();
    let mut settings = Map::new();
    settings.insert("last_export_config".to_string(), Value::Object(remaining));
    let _ = repo.save_global_settings(&settings);
}

/// Add Square Before Crop to a saved row choice, once.
pub fn migrate_square_before_crop_sticky_row(repo: &mut StorageRepository) {
    add_sticky_row_once(repo, SQUARE_ROW_MIGRATED_KEY, "geometry.autocrop_square");
}

/// Add Auto Crop to a saved row choice, once.
pub fn migrate_auto_crop_sticky_row(repo: &mut StorageRepository) {
    add_sticky_row_once(repo, AUTO_CROP_ROW_MIGRATED_KEY, "geometry.crop_from_auto");
}

fn flag_set(repo: &StorageRepository, key: &str) -> bool {
    repo.global_setting(key)
        .ok()
        .flatten()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn add_sticky_row_once(repo: &mut StorageRepository, flag: &str, row_id: &str) {
    if flag_set(repo, flag) {
        return;
    }
    let mut settings = Map::new();
    settings.insert(flag.to_string(), json!(true));
    let _ = repo.save_global_settings(&settings);
    let Ok(Some(Value::Array(stored))) = repo.global_setting(STICKY_ROWS_KEY) else {
        return;
    };
    let mut ids: Vec<String> = stored
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    if !ids.iter().any(|id| id == row_id) {
        ids.push(row_id.to_string());
    }
    save_sticky_ids(repo, &ids);
}
