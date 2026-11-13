//! Convert legacy gear presets into metadata presets.
//!
//! Mirrors `negpy/services/assets/migrations/gear_presets.py`.

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use crate::domain::models::MetadataConfig;
use crate::features::metadata::gear::GearLibrary;
use crate::features::metadata::gear_logic::metadata_from_gear;
use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::presets::{PresetStore, sanitize_preset_name, with_preset_notes};

use super::common::{save_setting, setting_truthy, truthy};

pub const DONE_FLAG: &str = "gear_presets_migrated";
const GEAR_PRESETS_FILE: &str = "gear_presets.json";
const UNNAMED: &str = "Unnamed preset";

/// The fields a gear preset stores, in the Python tuple's order.
pub const GEAR_FIELDS: &[&str] = &[
    "camera_id",
    "lens_id",
    "film_stock_id",
    "camera_make",
    "camera_model",
    "lens_make",
    "lens_model",
    "focal_length_mm",
    "max_aperture",
    "film",
    "film_manufacturer",
    "film_iso",
    "film_color_type",
    "format",
    "format_other",
];

fn read_presets(path: &Path) -> (bool, Vec<Value>) {
    if !path.is_file() {
        return (true, Vec::new());
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return (false, Vec::new());
    };
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
        return (false, Vec::new());
    };
    (
        true,
        items
            .into_iter()
            .filter(|item| item.is_object())
            .collect(),
    )
}

fn safe_name(name: &str) -> String {
    sanitize_preset_name(name).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unique_name(base: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    (2..)
        .map(|index| format!("{base} {index}"))
        .find(|name| !taken.contains(&name.to_lowercase()))
        .unwrap_or_else(|| base.to_string())
}

fn preset_id(preset: &Value) -> String {
    preset.get("id").map_or_else(String::new, |value| match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    })
}

fn field<'a>(preset: &'a Value, names: &[&str]) -> &'a str {
    names
        .iter()
        .find_map(|name| preset.get(*name).and_then(Value::as_str))
        .unwrap_or_default()
}

fn fields(config: &MetadataConfig) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    out.insert("camera_id".into(), Value::String(config.camera_id.clone()));
    out.insert("lens_id".into(), Value::String(config.lens_id.clone()));
    out.insert(
        "film_stock_id".into(),
        Value::String(config.film_stock_id.clone()),
    );
    out.insert(
        "camera_make".into(),
        Value::String(config.camera_make.clone()),
    );
    out.insert(
        "camera_model".into(),
        Value::String(config.camera_model.clone()),
    );
    out.insert("lens_make".into(), Value::String(config.lens_make.clone()));
    out.insert(
        "lens_model".into(),
        Value::String(config.lens_model.clone()),
    );
    out.insert(
        "focal_length_mm".into(),
        config.focal_length_mm.map_or(Value::Null, |value| {
            serde_json::json!(value)
        }),
    );
    out.insert(
        "max_aperture".into(),
        config.max_aperture.map_or(Value::Null, |value| {
            serde_json::json!(value)
        }),
    );
    out.insert("film".into(), Value::String(config.film.clone()));
    out.insert(
        "film_manufacturer".into(),
        Value::String(config.film_manufacturer.clone()),
    );
    out.insert(
        "film_iso".into(),
        config.film_iso.map_or(Value::Null, |value| {
            serde_json::json!(value)
        }),
    );
    out.insert(
        "film_color_type".into(),
        Value::String(config.film_color_type.clone()),
    );
    out.insert("format".into(), Value::String(config.format.clone()));
    out.insert(
        "format_other".into(),
        Value::String(config.format_other.clone()),
    );
    out
}

/// Write every gear preset into the metadata preset store, once.
///
/// A damaged source file leaves the migration pending. A refused preset is skipped,
/// and the flag stays unset so the next launch retries.
pub fn migrate_gear_presets(
    repo: &mut StorageRepository,
    gear_dir: &Path,
    bundled_dir: &Path,
    store: &PresetStore,
) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let (bundled_ok, bundled) = read_presets(&bundled_dir.join(GEAR_PRESETS_FILE));
    let (user_ok, user) = read_presets(&gear_dir.join(GEAR_PRESETS_FILE));
    if !(bundled_ok && user_ok) {
        log::warn!("Gear preset file unreadable; migration stays pending");
        return;
    }
    let seen: HashSet<String> = bundled.iter().map(preset_id).collect();
    let presets: Vec<&Value> = bundled
        .iter()
        .chain(
            user.iter()
                .filter(|preset| !seen.contains(&preset_id(preset))),
        )
        .collect();

    let library = GearLibrary::load(gear_dir, bundled_dir);
    let mut existing: HashSet<String> = store.list().iter().map(|name| name.to_lowercase()).collect();
    let mut unstored = false;
    for preset in presets {
        let mut name = safe_name(field(
            preset,
            &["displayName", "display_name"],
        ));
        if name.is_empty() {
            name = unique_name(UNNAMED, &existing);
        }
        if existing.contains(&name.to_lowercase()) {
            continue;
        }
        let resolved = metadata_from_gear(
            &MetadataConfig::default(),
            &library,
            Some(field(preset, &["cameraId", "camera_id"])),
            Some(field(preset, &["lensId", "lens_id"])),
            Some(field(preset, &["filmStockId", "film_stock_id"])),
        );
        let notes = preset
            .get("notes")
            .filter(|value| truthy(value))
            .map_or_else(String::new, |value| {
                value.as_str().unwrap_or_default().to_string()
            });
        if store.save(&name, &with_preset_notes(&fields(&resolved), &notes)).is_err() {
            log::warn!("Gear preset {name:?} not converted");
            unstored = true;
            continue;
        }
        existing.insert(name.to_lowercase());
    }
    if !unstored && let Err(error) = save_setting(repo, DONE_FLAG, Value::Bool(true)) {
        log::error!("Could not record the gear preset migration: {error}");
    }
}
