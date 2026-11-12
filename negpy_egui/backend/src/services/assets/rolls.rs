//! Roll membership and roll-scoped edit identities.
//!
//! Mirrors `negpy/services/assets/rolls.py`: a roll decides which files a Library opens.
//! Edits stay under each frame's own hash, except a forked path, which gets a
//! `#roll:<id>` suffix.

use serde_json::{Map, Value, json};

use crate::domain::models::process::ProcessConfig;
use crate::domain::models::with_film_fields;
use crate::domain::models::workspace::WorkspaceConfig;
use crate::infrastructure::storage::repository::StorageRepository;

pub const ROLLS_KEY: &str = "rolls_by_id";
pub const IMPORT_SOURCES_KEY: &str = "roll_import_sources";
pub const DISMISSED_FOLDERS_KEY: &str = "dismissed_folder_rolls";
pub const DISCOVERY_FILTERS_KEY: &str = "roll_discovery_filters";
pub const DEFAULT_DISCOVERY_FILTERS: &[&str] = &["export"];
const FORK_SEP: &str = "#roll:";

/// A roll record, stored in the `ROLLS_KEY` global setting.
pub type RollEntry = Map<String, Value>;

/// The fields each Roll card edits, as `(section, field names)` keyed by the card that
/// owns them. Mirrors `ROLL_DEFAULT_FIELDS`.
pub const ROLL_DEFAULT_FIELDS: &[(&str, &str, &[&str])] = &[
    (
        "film",
        "process",
        &[
            "process_mode",
            "positive_source",
            "slide_bw",
            "source_encoding",
        ],
    ),
    (
        "sensor",
        "process",
        &[
            "sensor_profile",
            "sensor_matrix",
            "sensor_unmix",
            "crosstalk_strength",
            "crosstalk_profile",
            "crosstalk_matrix",
            "crosstalk_process",
            "hue_trim",
        ],
    ),
    (
        "fade",
        "process",
        &[
            "fade_ratio_r",
            "fade_density_shift",
            "fade_ratio_g",
            "fade_ratio_b",
            "fade_white_anchor",
            "fade_white_r",
            "fade_white_g",
            "fade_white_b",
            "fade_white_space",
            "fade_crossover",
            "fade_crossover_table",
            "fade_crossover_space",
            "fade_auto_red",
            "fade_neutrals_mix",
        ],
    ),
    (
        "demosaic",
        "process",
        &[
            "linear_raw",
            "demosaic_preview",
            "demosaic_export",
            "highlight_reconstruction",
        ],
    ),
    (
        "process",
        "process",
        &[
            "analysis_buffer",
            "luma_range_clip",
            "color_range_clip",
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
    (
        "baseline",
        "process",
        &["use_luma_average", "use_color_average", "use_cast_average"],
    ),
    (
        "lens",
        "geometry",
        &[
            "distortion_k1",
            "lens_distortion_from_metadata",
            "lens_ca_from_metadata",
        ],
    ),
    ("flatfield", "flatfield", &["apply", "profile_id"]),
    (
        "metadata_gear",
        "metadata",
        &[
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
        ],
    ),
    (
        "metadata_capture",
        "metadata",
        &[
            "capture_date",
            "gps_latitude",
            "gps_longitude",
            "location_city",
            "location_state",
            "location_country",
        ],
    ),
    (
        "metadata_process",
        "metadata",
        &[
            "developer",
            "process_dilution",
            "push_pull",
            "process_time_seconds",
            "process_temperature_c",
            "process_id",
        ],
    ),
    (
        "metadata_scanning",
        "metadata",
        &["scanning", "scanning_id", "capture_roll"],
    ),
    ("metadata_exposure", "metadata", &["exposure_override"]),
];

/// A camera RAW frame's own fade white, which moves with its exposure.
pub const DECODE_WHITE_FIELDS: &[&str] = &[
    "fade_white_r",
    "fade_white_g",
    "fade_white_b",
    "fade_white_space",
];

/// The independent edit identity a fork of `from_hash` uses under `roll_id`.
#[must_use]
pub fn roll_edit_hash(from_hash: &str, roll_id: &str) -> String {
    format!("{from_hash}{FORK_SEP}{roll_id}")
}

/// `file_hash` with only a trailing `#roll:<id>` suffix removed.
#[must_use]
pub fn unforked_hash(file_hash: &str) -> String {
    file_hash
        .find(FORK_SEP)
        .map_or(file_hash, |index| &file_hash[..index])
        .to_string()
}

fn read(repo: &StorageRepository) -> Map<String, Value> {
    match repo.global_setting(ROLLS_KEY).ok().flatten() {
        Some(Value::Object(store)) => store,
        _ => Map::new(),
    }
}

fn write(repo: &mut StorageRepository, store: Map<String, Value>) {
    let mut values = Map::new();
    values.insert(ROLLS_KEY.to_string(), Value::Object(store));
    let _ = repo.save_global_settings(&values);
}

/// Every remembered roll, keyed by id.
#[must_use]
pub fn saved_rolls(repo: &StorageRepository) -> Map<String, Value> {
    read(repo)
}

/// One roll record, if it exists.
#[must_use]
pub fn roll_for_id(repo: &StorageRepository, roll_id: &str) -> Option<Value> {
    read(repo).get(roll_id).cloned()
}

/// `path` in the form two spellings of one folder share: separators and case.
#[must_use]
pub fn folder_key(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if cfg!(windows) {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// The id of the folder roll recognizing `path`.
#[must_use]
pub fn folder_roll_id_for_path(repo: &StorageRepository, path: &str) -> Option<String> {
    let key = folder_key(path);
    read(repo).into_iter().find_map(|(roll_id, entry)| {
        (entry.get("kind").and_then(Value::as_str) == Some("folder")
            && entry
                .get("folder_path")
                .and_then(Value::as_str)
                .is_some_and(|folder| folder_key(folder) == key))
        .then_some(roll_id)
    })
}

fn string_list(entry: &Value, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The files a walk of `folders` skips: those removed from the roll of each.
#[must_use]
pub fn excluded_paths(repo: &StorageRepository, folders: &[String]) -> Vec<String> {
    let keys: Vec<String> = folders.iter().map(|folder| folder_key(folder)).collect();
    read(repo)
        .into_values()
        .filter(|entry| entry.get("kind").and_then(Value::as_str) == Some("folder"))
        .filter(|entry| {
            entry
                .get("folder_path")
                .and_then(Value::as_str)
                .is_some_and(|folder| keys.contains(&folder_key(folder)))
        })
        .flat_map(|entry| string_list(&entry, "excluded_paths"))
        .map(|path| folder_key(&path))
        .collect()
}

/// Whether `path` is one of the excluded keys.
#[must_use]
pub fn is_excluded_key(path: &str, excluded: &[String]) -> bool {
    excluded.iter().any(|key| key == &folder_key(path))
}

/// The removed files of a folder roll that are still on disk.
#[must_use]
pub fn removed_paths(repo: &StorageRepository, roll_id: &str) -> Vec<String> {
    let Some(entry) = roll_for_id(repo, roll_id) else {
        return Vec::new();
    };
    if entry.get("kind").and_then(Value::as_str) != Some("folder") {
        return Vec::new();
    }
    string_list(&entry, "excluded_paths")
        .into_iter()
        .filter(|path| std::path::Path::new(path).is_file())
        .collect()
}

/// Empty a folder roll's excluded paths, returning the files still on disk.
pub fn restore_removed(repo: &mut StorageRepository, roll_id: &str) -> Vec<String> {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return Vec::new();
    };
    if entry.get("kind").and_then(Value::as_str) != Some("folder") {
        return Vec::new();
    }
    let Some(excluded) = entry
        .get("excluded_paths")
        .and_then(Value::as_array)
        .cloned()
    else {
        return Vec::new();
    };
    let back: Vec<String> = excluded
        .iter()
        .filter_map(Value::as_str)
        .filter(|path| std::path::Path::new(path).is_file())
        .map(str::to_string)
        .collect();
    entry.insert("excluded_paths".to_string(), Value::Array(Vec::new()));
    write(repo, store);
    back
}

/// Create a virtual roll and return its id.
pub fn create_virtual_roll(
    repo: &mut StorageRepository,
    name: &str,
    member_paths: &[String],
) -> String {
    let roll_id = uuid::Uuid::new_v4().simple().to_string();
    let mut store = read(repo);
    let mut entry = Map::new();
    entry.insert("kind".into(), json!("virtual"));
    entry.insert("name".into(), json!(name));
    entry.insert("member_paths".into(), json!(member_paths));
    entry.insert(
        "created_at".into(),
        json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
        ),
    );
    store.insert(roll_id.clone(), Value::Object(entry));
    write(repo, store);
    roll_id
}

/// Add paths to a roll in one write.
///
/// A virtual roll gains new members. A folder roll holds only its own folder's files, so
/// the paths and their companion parts only leave its excluded paths. Returns the paths
/// that left the exclusions.
pub fn add_members(
    repo: &mut StorageRepository,
    roll_id: &str,
    paths: &[String],
    parts: &[String],
) -> Vec<String> {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return Vec::new();
    };
    let kind = entry
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("virtual");
    let mut new = Vec::new();
    if kind != "folder" {
        let known: Vec<String> = string_list(&Value::Object(entry.clone()), "member_paths");
        for path in paths {
            if !known.contains(path) && !new.contains(path) {
                new.push(path.clone());
            }
        }
    }
    let added: Vec<String> = paths
        .iter()
        .chain(parts)
        .map(|path| folder_key(path))
        .collect();
    let excluded: Vec<String> = entry
        .get("excluded_paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    let kept: Vec<String> = excluded
        .iter()
        .filter(|path| !added.contains(&folder_key(path)))
        .cloned()
        .collect();
    let changed = !new.is_empty() || kept.len() != excluded.len();
    if !new.is_empty() {
        let mut members = string_list(&Value::Object(entry.clone()), "member_paths");
        members.extend(new);
        entry.insert("member_paths".into(), json!(members));
    }
    if kept.len() != excluded.len() {
        entry.insert("excluded_paths".into(), json!(kept));
    }
    if changed {
        write(repo, store);
    }
    paths
        .iter()
        .filter(|path| {
            let key = folder_key(path);
            excluded.iter().any(|old| folder_key(old) == key)
                && !kept.iter().any(|kept| folder_key(kept) == key)
        })
        .cloned()
        .collect()
}

/// Remove paths and their records from a roll, returning the forks the caller must delete.
pub fn remove_members(
    repo: &mut StorageRepository,
    roll_id: &str,
    paths: &[String],
    hashes: &[String],
) -> Vec<String> {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return Vec::new();
    };
    let gone: Vec<String> = paths.iter().map(|path| folder_key(path)).collect();
    if entry.contains_key("member_paths") {
        let members: Vec<String> = string_list(&Value::Object(entry.clone()), "member_paths")
            .into_iter()
            .filter(|path| !gone.contains(&folder_key(path)))
            .collect();
        entry.insert("member_paths".into(), json!(members));
    }
    if entry.get("kind").and_then(Value::as_str) == Some("folder") {
        let folder = entry
            .get("folder_path")
            .and_then(Value::as_str)
            .map(folder_key)
            .unwrap_or_default();
        let own: Vec<String> = paths
            .iter()
            .filter(|path| folder_key(&parent_dir(path).unwrap_or_default()) == folder)
            .cloned()
            .collect();
        let mut excluded = string_list(&Value::Object(entry.clone()), "excluded_paths");
        for path in own {
            if !excluded
                .iter()
                .any(|known| folder_key(known) == folder_key(&path))
            {
                excluded.push(path);
            }
        }
        entry.insert("excluded_paths".into(), json!(excluded));
    }
    for key in ["frame_overrides", "frame_baselines"] {
        if let Some(records) = entry.get_mut(key).and_then(Value::as_object_mut) {
            for hash in hashes {
                records.remove(hash);
            }
            if records.is_empty() {
                entry.remove(key);
            }
        }
    }
    if let Some(scenes) = entry.get_mut("scenes").and_then(Value::as_object_mut) {
        pull_scene_members(scenes, hashes);
    }
    let forked = string_list(&Value::Object(entry.clone()), "forked_hashes")
        .into_iter()
        .filter(|hash| hashes.contains(hash))
        .collect::<Vec<_>>();
    if !forked.is_empty() {
        let kept: Vec<String> = string_list(&Value::Object(entry.clone()), "forked_hashes")
            .into_iter()
            .filter(|hash| !hashes.contains(hash))
            .collect();
        entry.insert("forked_hashes".into(), json!(kept));
    }
    write(repo, store);
    forked
}

/// Every roll `path` belongs to: directly in a folder roll's folder and not removed,
/// or listed in a virtual roll's members.
#[must_use]
pub fn rolls_containing_path(repo: &StorageRepository, path: &str) -> Vec<String> {
    let parent = parent_dir(&absolute(path)).unwrap_or_default();
    let parent_key = folder_key(&parent);
    read(repo)
        .into_iter()
        .filter_map(|(roll_id, entry)| {
            let kind = entry
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("virtual");
            if kind == "folder" {
                let folder = entry
                    .get("folder_path")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let in_folder = !folder.is_empty() && folder_key(&absolute(folder)) == parent_key;
                let excluded = string_list(&entry, "excluded_paths");
                (in_folder && !is_excluded_key(path, &excluded)).then_some(roll_id)
            } else {
                string_list(&entry, "member_paths")
                    .iter()
                    .any(|member| member == path)
                    .then_some(roll_id)
            }
        })
        .collect()
}

/// The roll a path belongs to when none is active. A folder roll wins over a virtual one.
#[must_use]
pub fn home_roll(repo: &StorageRepository, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let containing = rolls_containing_path(repo, path);
    containing
        .iter()
        .find(|roll_id| {
            roll_for_id(repo, roll_id)
                .and_then(|entry| {
                    entry
                        .get("kind")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .as_deref()
                == Some("folder")
        })
        .cloned()
        .or_else(|| containing.first().cloned())
}

/// The roll a frame resolves through: the active roll, else its home roll.
#[must_use]
pub fn frame_roll(
    repo: &StorageRepository,
    active_roll_id: Option<&str>,
    path: &str,
) -> Option<String> {
    active_roll_id
        .map(str::to_string)
        .or_else(|| home_roll(repo, path))
}

fn absolute(path: &str) -> String {
    std::path::Path::new(path)
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Rename a roll. An unknown id is a no-op.
pub fn rename_roll(repo: &mut StorageRepository, roll_id: &str, name: &str) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    entry.insert("name".into(), json!(name));
    write(repo, store);
}

/// Delete a roll. A deleted folder roll is added to the dismissed folders.
pub fn delete_roll(repo: &mut StorageRepository, roll_id: &str) {
    let mut store = read(repo);
    let Some(entry) = store.remove(roll_id) else {
        return;
    };
    write(repo, store);
    if entry.get("kind").and_then(Value::as_str) != Some("folder") {
        return;
    }
    let Some(path) = entry.get("folder_path").and_then(Value::as_str) else {
        return;
    };
    let dismissed = import_sources_of(repo, DISMISSED_FOLDERS_KEY);
    if dismissed
        .iter()
        .any(|folder| folder_key(folder) == folder_key(path))
    {
        return;
    }
    let mut settings = Map::new();
    let mut updated = dismissed.clone();
    updated.push(path.to_string());
    settings.insert(DISMISSED_FOLDERS_KEY.into(), json!(updated));
    let _ = repo.save_global_settings(&settings);
}

fn import_sources_of(repo: &StorageRepository, key: &str) -> Vec<String> {
    match repo.global_setting(key).ok().flatten() {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// Every roll in name order, folder and virtual alike.
#[must_use]
pub fn all_rolls_sorted(repo: &StorageRepository) -> Vec<(String, Value)> {
    let mut rolls: Vec<(String, Value)> = read(repo).into_iter().collect();
    rolls.sort_by(|left, right| {
        let name = |entry: &Value| {
            entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase()
        };
        name(&left.1).cmp(&name(&right.1))
    });
    rolls
}

/// Every virtual roll in name order.
#[must_use]
pub fn virtual_rolls(repo: &StorageRepository) -> Vec<(String, Value)> {
    all_rolls_sorted(repo)
        .into_iter()
        .filter(|(_, entry)| entry.get("kind").and_then(Value::as_str) == Some("virtual"))
        .collect()
}

fn parent_dir(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
}

fn pull_scene_members(scenes: &mut Map<String, Value>, hashes: &[String]) {
    let stale: Vec<String> = scenes
        .iter()
        .filter_map(|(scene_id, scene)| {
            let members = scene.get("member_hashes").and_then(Value::as_array)?;
            let kept: Vec<String> = members
                .iter()
                .filter_map(Value::as_str)
                .filter(|hash| !hashes.iter().any(|gone| gone == hash))
                .map(str::to_string)
                .collect();
            kept.is_empty().then(|| scene_id.clone())
        })
        .collect();
    for scene_id in &stale {
        scenes.remove(scene_id);
    }
    for (_, scene) in scenes.iter_mut() {
        if let Some(members) = scene
            .get("member_hashes")
            .and_then(Value::as_array)
            .cloned()
        {
            let kept: Vec<String> = members
                .iter()
                .filter_map(Value::as_str)
                .filter(|hash| !hashes.iter().any(|gone| gone == hash))
                .map(str::to_string)
                .collect();
            if !kept.is_empty() {
                if let Some(object) = scene.as_object_mut() {
                    object.insert("member_hashes".into(), json!(kept));
                }
            }
        }
    }
}

/// The horizontal card table row for `card_key`.
#[must_use]
pub fn card_fields(card_key: &str) -> Option<(&'static str, &'static [&'static str])> {
    ROLL_DEFAULT_FIELDS
        .iter()
        .find(|(key, _, _)| *key == card_key)
        .map(|(_, section, fields)| (*section, *fields))
}

/// A list read back from JSON as a tuple-like value, recursively.
#[must_use]
pub fn config_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(config_value).collect()),
        value => value.clone(),
    }
}

/// Whether two values are equal after the store's list normalization.
#[must_use]
pub fn same_value(left: &Value, right: &Value) -> bool {
    config_value(left) == config_value(right)
}

fn without_decode_white(fields: Map<String, Value>) -> Map<String, Value> {
    if fields
        .get("fade_white_space")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return fields;
    }
    fields
        .into_iter()
        .filter(|(key, _)| !DECODE_WHITE_FIELDS.contains(&key.as_str()))
        .collect()
}

/// The roll's own value for each field it has set at least once.
#[must_use]
pub fn roll_defaults(repo: &StorageRepository, roll_id: &str) -> Map<String, Value> {
    roll_for_id(repo, roll_id)
        .and_then(|entry| entry.as_object().cloned())
        .and_then(|entry| entry.get("defaults").and_then(Value::as_object).cloned())
        .map(without_decode_white)
        .unwrap_or_default()
}

/// Set roll-default field values. An unknown roll id is a no-op.
pub fn set_roll_defaults(repo: &mut StorageRepository, roll_id: &str, fields: Map<String, Value>) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    let mut defaults = entry
        .get("defaults")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    defaults.extend(without_decode_white(fields));
    entry.insert("defaults".to_string(), Value::Object(defaults));
    write(repo, store);
}

/// Put back named defaults from a snapshot; an absent marker drops the field.
pub fn restore_roll_fields(
    repo: &mut StorageRepository,
    roll_id: &str,
    snapshot: Map<String, Value>,
    absent: &[String],
) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    let mut defaults = entry
        .get("defaults")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for name in absent {
        defaults.remove(name);
    }
    for (name, value) in snapshot {
        defaults.insert(name, value);
    }
    entry.insert("defaults".to_string(), Value::Object(defaults));
    write(repo, store);
}

/// Copy a roll's defaults, frame seed and the named frames' card locks.
pub fn copy_roll_settings(
    repo: &mut StorageRepository,
    from_id: &str,
    to_id: &str,
    file_hashes: &[String],
) {
    let mut store = read(repo);
    let Some(Value::Object(source)) = store.get(from_id).cloned() else {
        return;
    };
    let Some(Value::Object(mut target)) = store.get(to_id).cloned() else {
        return;
    };
    for key in ["defaults", "frame_seed"] {
        if source.get(key).is_some_and(|value| !value.is_null()) {
            target.insert(key.to_string(), source[key].clone());
        }
    }
    let locks: Map<String, Value> = source
        .get("frame_overrides")
        .and_then(Value::as_object)
        .map(|overrides| {
            overrides
                .iter()
                .filter(|(hash, cards)| {
                    file_hashes.iter().any(|known| known == *hash)
                        && cards.as_array().is_some_and(|cards| !cards.is_empty())
                })
                .map(|(hash, cards)| (hash.clone(), cards.clone()))
                .collect()
        })
        .unwrap_or_default();
    if !locks.is_empty() {
        let baselines: Map<String, Value> = source
            .get("frame_baselines")
            .and_then(Value::as_object)
            .map(|baselines| {
                baselines
                    .iter()
                    .filter(|(hash, _)| locks.contains_key(*hash))
                    .map(|(hash, baseline)| (hash.clone(), baseline.clone()))
                    .collect()
            })
            .unwrap_or_default();
        target.insert("frame_overrides".to_string(), Value::Object(locks));
        if !baselines.is_empty() {
            target.insert("frame_baselines".to_string(), Value::Object(baselines));
        }
    }
    store.insert(to_id.to_string(), Value::Object(target));
    write(repo, store);
}

/// The roll values a frame with no saved edit starts from.
#[must_use]
pub fn frame_seed(repo: &StorageRepository, roll_id: &str) -> Map<String, Value> {
    roll_for_id(repo, roll_id)
        .and_then(|entry| entry.get("frame_seed").and_then(Value::as_object).cloned())
        .unwrap_or_default()
}

/// Extend the roll's frame seed.
pub fn add_frame_seed(repo: &mut StorageRepository, roll_id: &str, fields: Map<String, Value>) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    let mut seed = entry
        .get("frame_seed")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    seed.extend(fields);
    entry.insert("frame_seed".to_string(), Value::Object(seed));
    write(repo, store);
}

/// The roll's current value for each named field; `None` where it has no default.
#[must_use]
pub fn roll_fields_snapshot(
    repo: &StorageRepository,
    roll_id: &str,
    names: &[&str],
) -> Vec<(String, Option<Value>)> {
    let defaults = roll_defaults(repo, roll_id);
    names
        .iter()
        .map(|name| ((*name).to_string(), defaults.get(*name).cloned()))
        .collect()
}

/// The cards this frame has locked within this roll.
#[must_use]
pub fn frame_override_cards(
    repo: &StorageRepository,
    roll_id: &str,
    file_hash: &str,
) -> Vec<String> {
    roll_for_id(repo, roll_id)
        .and_then(|entry| {
            entry
                .get("frame_overrides")
                .and_then(Value::as_object)
                .and_then(|overrides| overrides.get(file_hash).cloned())
        })
        .and_then(|cards| cards.as_array().cloned())
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Unlock every card for one frame within one roll.
pub fn clear_frame_overrides(repo: &mut StorageRepository, roll_id: &str, file_hash: &str) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    if !entry
        .get("frame_overrides")
        .and_then(Value::as_object)
        .is_some_and(|overrides| overrides.contains_key(file_hash))
    {
        return;
    }
    for key in ["frame_overrides", "frame_baselines"] {
        if let Some(overrides) = entry.get_mut(key).and_then(Value::as_object_mut) {
            overrides.remove(file_hash);
        }
    }
    write(repo, store);
}

/// The values a locked card held before its lock, for fields the roll had no default for.
#[must_use]
pub fn frame_baseline(
    repo: &StorageRepository,
    roll_id: &str,
    file_hash: &str,
    card_key: &str,
) -> Map<String, Value> {
    roll_for_id(repo, roll_id)
        .and_then(|entry| {
            entry
                .get("frame_baselines")
                .and_then(Value::as_object)
                .and_then(|baselines| baselines.get(file_hash).cloned())
        })
        .and_then(|frame| frame.get(card_key).and_then(Value::as_object).cloned())
        .map(|baseline| {
            baseline
                .into_iter()
                .map(|(name, value)| (name, config_value(&value)))
                .collect()
        })
        .unwrap_or_default()
}

/// Lock or unlock one card for one frame within one roll.
///
/// A new lock keeps `baseline`, a lock that already holds keeps its own, and an unlock
/// drops it. An unknown roll id is a no-op.
pub fn set_frame_override(
    repo: &mut StorageRepository,
    roll_id: &str,
    file_hash: &str,
    card_key: &str,
    locked: bool,
    baseline: Option<Map<String, Value>>,
) {
    let mut store = read(repo);
    let Some(Value::Object(entry)) = store.get_mut(roll_id) else {
        return;
    };
    let mut overrides = entry
        .get("frame_overrides")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut cards: Vec<String> = overrides
        .get(file_hash)
        .and_then(Value::as_array)
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let relock = locked && cards.iter().any(|card| card == card_key);
    if locked {
        if !cards.iter().any(|card| card == card_key) {
            cards.push(card_key.to_string());
            cards.sort();
        }
    } else {
        cards.retain(|card| card != card_key);
    }
    if cards.is_empty() {
        overrides.remove(file_hash);
    } else {
        overrides.insert(
            file_hash.to_string(),
            Value::Array(cards.into_iter().map(Value::String).collect()),
        );
    }
    entry.insert("frame_overrides".to_string(), Value::Object(overrides));

    let mut baselines = entry
        .get("frame_baselines")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut frame = baselines
        .get(file_hash)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let kept_baseline = if relock {
        frame.get(card_key).cloned()
    } else {
        None
    };
    frame.remove(card_key);
    if locked {
        if let Some(kept) = kept_baseline {
            frame.insert(card_key.to_string(), kept);
        } else if let Some(baseline) = baseline {
            if !baseline.is_empty() {
                frame.insert(card_key.to_string(), Value::Object(baseline));
            }
        }
    }
    if frame.is_empty() {
        baselines.remove(file_hash);
    } else {
        baselines.insert(file_hash.to_string(), Value::Object(frame));
    }
    if baselines.is_empty() {
        entry.remove("frame_baselines");
    } else {
        entry.insert("frame_baselines".to_string(), Value::Object(baselines));
    }
    write(repo, store);
}

/// Apply this roll's defaults unless the frame locked the owning card.
#[must_use]
pub fn resolve_roll_config(
    repo: &StorageRepository,
    roll_id: Option<&str>,
    file_hash: &str,
    config: &WorkspaceConfig,
) -> WorkspaceConfig {
    let Some(roll_id) = roll_id else {
        return config.clone();
    };
    let defaults = frame_roll_defaults(roll_defaults(repo, roll_id), &config.process);
    if defaults.is_empty() {
        return config.clone();
    }
    let locked = frame_override_cards(repo, roll_id, file_hash);
    let mut process = Map::new();
    let mut geometry = Map::new();
    let mut flatfield = Map::new();
    let mut metadata = Map::new();
    for (card_key, section, field_names) in ROLL_DEFAULT_FIELDS {
        if locked.iter().any(|card| card == card_key) {
            continue;
        }
        for name in *field_names {
            let Some(value) = defaults.get(*name) else {
                continue;
            };
            match *section {
                "process" => {
                    process.insert((*name).to_string(), value.clone());
                }
                "geometry" => {
                    geometry.insert((*name).to_string(), value.clone());
                }
                "flatfield" => {
                    flatfield.insert((*name).to_string(), value.clone());
                }
                "metadata" => {
                    metadata.insert((*name).to_string(), value.clone());
                }
                _ => {}
            }
        }
    }
    let mut resolved = config.clone();
    resolved = with_film_fields(resolved, &mut process);
    resolved.process = merge_section(&resolved.process, &process);
    resolved.geometry = merge_section(&resolved.geometry, &geometry);
    resolved.flatfield = merge_section(&resolved.flatfield, &flatfield);
    resolved.metadata = merge_section(&resolved.metadata, &metadata);
    resolved
}

fn merge_section<T>(config: &T, updates: &Map<String, Value>) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + Default,
{
    if updates.is_empty() {
        return config.clone();
    }
    let mut merged = match serde_json::to_value(config) {
        Ok(Value::Object(fields)) => fields,
        _ => return config.clone(),
    };
    merged.extend(updates.clone());
    crate::domain::models::build_lenient(&merged)
}

/// Defaults as they apply to a frame whose own decode measured a white.
#[must_use]
pub fn frame_roll_defaults(
    defaults: Map<String, Value>,
    process: &ProcessConfig,
) -> Map<String, Value> {
    if process.fade_white_space.is_empty() {
        return defaults;
    }
    defaults
        .into_iter()
        .filter(|(key, _)| !DECODE_WHITE_FIELDS.contains(&key.as_str()))
        .collect()
}

/// Settle a card's lock from the frame's values against the roll's defaults.
pub fn settle_card_lock(
    repo: &mut StorageRepository,
    roll_id: &str,
    file_hash: &str,
    card_key: &str,
    values: &Map<String, Value>,
    before: Option<&Map<String, Value>>,
    process: Option<&ProcessConfig>,
) {
    let mut defaults = frame_roll_defaults(
        roll_defaults(repo, roll_id),
        process.unwrap_or(&ProcessConfig::default()),
    );
    let values: Map<String, Value> =
        if process.is_some_and(|process| !process.fade_white_space.is_empty()) {
            values
                .iter()
                .filter(|(key, _)| !DECODE_WHITE_FIELDS.contains(&key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        } else {
            values.clone()
        };
    let locked = frame_override_cards(repo, roll_id, file_hash)
        .iter()
        .any(|card| card == card_key);
    let baseline = if locked {
        frame_baseline(repo, roll_id, file_hash, card_key)
    } else {
        before
            .map(|before| {
                before
                    .iter()
                    .filter(|(key, _)| !defaults.contains_key(*key))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    for (key, value) in &baseline {
        defaults.insert(key.clone(), value.clone());
    }
    let diverged = values.iter().any(|(key, value)| {
        defaults
            .get(key)
            .is_none_or(|default| !same_value(value, default))
    });
    if diverged != locked {
        set_frame_override(repo, roll_id, file_hash, card_key, diverged, Some(baseline));
    }
}

fn entry_map(entry: Value) -> Option<RollEntry> {
    entry.as_object().cloned()
}

fn forked_hashes(entry: &RollEntry) -> Vec<String> {
    entry
        .get("forked_hashes")
        .and_then(Value::as_array)
        .map(|hashes| {
            hashes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Every roll's own edit hash for `from_hash`, where a roll forked it.
#[must_use]
pub fn forked_edit_hashes(repo: &StorageRepository, from_hash: &str) -> Vec<String> {
    read(repo)
        .iter()
        .filter(|(_, entry)| {
            entry_map((*entry).clone())
                .is_some_and(|entry| forked_hashes(&entry).iter().any(|hash| hash == from_hash))
        })
        .map(|(roll_id, _)| roll_edit_hash(from_hash, roll_id))
        .collect()
}

/// Whether `from_hash` has its own edit under `roll_id`.
#[must_use]
pub fn is_forked(repo: &StorageRepository, roll_id: &str, from_hash: &str) -> bool {
    roll_for_id(repo, roll_id)
        .and_then(entry_map)
        .is_some_and(|entry| forked_hashes(&entry).iter().any(|hash| hash == from_hash))
}

/// Give `from_hash` its own edit under `roll_id`, seeded from `config`.
///
/// Re-forking an already-forked hash only re-seeds it. An unknown roll returns the
/// shared hash and writes nothing.
pub fn fork_edit(
    repo: &mut StorageRepository,
    roll_id: &str,
    from_hash: &str,
    source_path: &str,
    config: &crate::domain::models::workspace::WorkspaceConfig,
) -> Result<String, crate::infrastructure::storage::repository::RepositoryError> {
    let mut store = read(repo);
    let Some(Value::Object(mut entry)) = store.get(roll_id).cloned() else {
        return Ok(from_hash.to_string());
    };
    let mut hashes = forked_hashes(&entry);
    if !hashes.iter().any(|hash| hash == from_hash) {
        hashes.push(from_hash.to_string());
        entry.insert(
            "forked_hashes".to_string(),
            Value::Array(hashes.into_iter().map(Value::String).collect()),
        );
        store.insert(roll_id.to_string(), Value::Object(entry));
        write(repo, store);
    }
    let forked = roll_edit_hash(from_hash, roll_id);
    repo.save_file_settings(&forked, config, source_path)?;
    Ok(forked)
}

/// Undo `fork_edit`: forget the fork and delete its edit, history and work prints.
pub fn unfork_edit(
    repo: &mut StorageRepository,
    roll_id: &str,
    from_hash: &str,
) -> Result<(), crate::infrastructure::storage::repository::RepositoryError> {
    let mut store = read(repo);
    if let Some(Value::Object(mut entry)) = store.get(roll_id).cloned() {
        let hashes: Vec<Value> = forked_hashes(&entry)
            .into_iter()
            .filter(|hash| hash != from_hash)
            .map(Value::String)
            .collect();
        entry.insert("forked_hashes".to_string(), Value::Array(hashes));
        store.insert(roll_id.to_string(), Value::Object(entry));
        write(repo, store);
    }
    repo.delete_file_settings(&roll_edit_hash(from_hash, roll_id))?;
    Ok(())
}
