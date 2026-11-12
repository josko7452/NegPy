//! Composite membership: the files a stitch or an HDR merge is built from.
//!
//! Mirrors `negpy/services/assets/composites.py`: the grouping is keyed by the
//! composite's primary path and lives until the user dissolves it.

use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::StorageRepository;

pub const COMPOSITES_KEY: &str = "composites_by_path";
const LEGACY_STITCHES_KEY: &str = "session_stitches";
const LEGACY_HDR_KEY: &str = "session_hdr_merges";

fn read(repo: &StorageRepository) -> Map<String, Value> {
    match repo.global_setting(COMPOSITES_KEY).ok().flatten() {
        Some(Value::Object(store)) => store,
        _ => Map::new(),
    }
}

fn save(repo: &mut StorageRepository, store: Map<String, Value>) {
    let mut values = Map::new();
    values.insert(COMPOSITES_KEY.to_string(), Value::Object(store));
    let _ = repo.save_global_settings(&values);
}

fn legacy_entry(repo: &StorageRepository, key: &str, kind: &str) -> Option<Map<String, Value>> {
    let legacy = repo.global_setting(key).ok().flatten()?;
    let Value::Object(entries) = legacy else {
        return None;
    };
    let mut out = Map::new();
    for (path, entry) in entries {
        let Some(mut entry) = entry.as_object().cloned() else {
            continue;
        };
        entry.insert("kind".to_string(), Value::String(kind.to_string()));
        out.insert(path, Value::Object(entry));
    }
    (!out.is_empty()).then_some(out)
}

/// Every remembered composite, keyed by primary path.
///
/// Legacy session-manifest entries are promoted on first read, then the legacy keys
/// are cleared, so an emptied store never promotes them again.
#[must_use]
pub fn saved_composites(repo: &mut StorageRepository) -> Map<String, Value> {
    let store = read(repo);
    if !store.is_empty() {
        return store;
    }
    let mut promoted = Map::new();
    for (key, kind) in [(LEGACY_STITCHES_KEY, "stitch"), (LEGACY_HDR_KEY, "hdr")] {
        if let Some(entries) = legacy_entry(repo, key, kind) {
            promoted.extend(entries);
        }
    }
    if promoted.is_empty() {
        return store;
    }
    let mut values = Map::new();
    values.insert(COMPOSITES_KEY.to_string(), Value::Object(promoted.clone()));
    values.insert(LEGACY_STITCHES_KEY.to_string(), Value::Null);
    values.insert(LEGACY_HDR_KEY.to_string(), Value::Null);
    let _ = repo.save_global_settings(&values);
    promoted
}

/// The remembered composites split into the two maps asset discovery re-attaches.
#[must_use]
pub fn restore_maps(repo: &mut StorageRepository) -> (Map<String, Value>, Map<String, Value>) {
    let store = saved_composites(repo);
    let mut stitches = Map::new();
    let mut merges = Map::new();
    for (path, entry) in store {
        match entry.get("kind").and_then(Value::as_str) {
            Some("stitch") => stitches.insert(path, entry),
            Some("hdr") => merges.insert(path, entry),
            _ => None,
        };
    }
    (stitches, merges)
}

/// Upsert the composites among `assets`; composites in every other folder survive.
pub fn remember_composites(repo: &mut StorageRepository, assets: &[Value]) {
    let store = read(repo);
    let mut updated = store.clone();
    for asset in assets {
        let Some(entry) = composite_entry(asset) else {
            continue;
        };
        let Some(path) = asset.get("path").and_then(Value::as_str) else {
            continue;
        };
        updated.insert(path.to_string(), entry);
    }
    if updated != store {
        save(repo, updated);
    }
}

/// Drop a composite, so its parts stay separate frames from now on.
pub fn forget_composite(repo: &mut StorageRepository, primary_path: Option<&str>) {
    let mut store = read(repo);
    let Some(path) = primary_path else {
        return;
    };
    if store.remove(path).is_some() {
        save(repo, store);
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
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

fn list_list(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().filter(|v| v.is_array()).cloned().collect())
        .unwrap_or_default()
}

fn list(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn number_list(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().filter(|v| v.is_number()).cloned().collect())
        .unwrap_or_default()
}

/// The storable record of an asset's composite membership, or `None` for a plain frame.
#[must_use]
pub fn composite_entry(asset: &Value) -> Option<Value> {
    let hash = asset.get("hash").and_then(Value::as_str)?;
    let shared_hash = utils::unforked_hash(hash);
    let process_mode = asset.get("process_mode").cloned().unwrap_or_default();
    if let Some(paths) = asset.get("stitch_paths").and_then(Value::as_array) {
        let mut entry = Map::new();
        entry.insert("kind".into(), Value::String("stitch".into()));
        entry.insert("paths".into(), Value::Array(paths.clone()));
        entry.insert(
            "transforms".into(),
            Value::Array(list_list(asset.get("stitch_transforms"))),
        );
        entry.insert(
            "canvas".into(),
            Value::Array(list(asset.get("stitch_canvas"))),
        );
        entry.insert(
            "sizes".into(),
            Value::Array(list_list(asset.get("stitch_sizes"))),
        );
        entry.insert(
            "triplets".into(),
            Value::Array(list_list(asset.get("stitch_triplets"))),
        );
        entry.insert(
            "align".into(),
            Value::Bool(
                asset
                    .get("stitch_align")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
        );
        entry.insert("hash".into(), Value::String(shared_hash));
        entry.insert("process_mode".into(), process_mode);
        return Some(Value::Object(entry));
    }
    if let Some(paths) = asset.get("hdr_paths").and_then(Value::as_array) {
        let mut entry = Map::new();
        entry.insert("kind".into(), Value::String("hdr".into()));
        entry.insert("paths".into(), Value::Array(paths.clone()));
        entry.insert(
            "ratios".into(),
            Value::Array(number_list(asset.get("hdr_ratios"))),
        );
        entry.insert(
            "align".into(),
            Value::Bool(
                asset
                    .get("hdr_align")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
        );
        entry.insert(
            "anchor".into(),
            Value::String(
                asset
                    .get("hdr_anchor")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
        );
        entry.insert(
            "anchor_ev".into(),
            Value::from(
                asset
                    .get("hdr_anchor_ev")
                    .and_then(Value::as_f64)
                    .unwrap_or(crate::domain::models::hdr::ANCHOR_EV_UNSET),
            ),
        );
        entry.insert("hash".into(), Value::String(shared_hash));
        entry.insert("process_mode".into(), process_mode);
        return Some(Value::Object(entry));
    }
    None
}

/// The non-primary files of the given composite records.
#[must_use]
pub fn part_paths(entries: impl IntoIterator<Item = Value>) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for entry in entries {
        out.extend(string_list(entry.get("paths")));
        for triplet in list_list(entry.get("triplets")) {
            out.extend(string_list(Some(&triplet)));
        }
    }
    out.retain(|path| !path.is_empty());
    out
}

/// Pass every primary and part path of every composite through `move_path`.
pub fn rehome_paths(repo: &mut StorageRepository, move_path: impl Fn(&str) -> String) {
    let saved = saved_composites(repo);
    let mut moved = Map::new();
    for (primary, entry) in saved.clone() {
        let mut entry = entry.as_object().cloned().unwrap_or_default();
        entry.insert(
            "paths".into(),
            Value::Array(
                string_list(entry.get("paths"))
                    .iter()
                    .map(|p| Value::String(move_path(p)))
                    .collect(),
            ),
        );
        if let Some(triplets) = entry.get("triplets").and_then(Value::as_array) {
            entry.insert(
                "triplets".into(),
                Value::Array(
                    triplets
                        .iter()
                        .map(|triplet| {
                            Value::Array(
                                string_list(Some(triplet))
                                    .iter()
                                    .map(|p| {
                                        Value::String(if p.is_empty() {
                                            p.clone()
                                        } else {
                                            move_path(p)
                                        })
                                    })
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            );
        }
        if let Some(anchor) = entry.get("anchor").and_then(Value::as_str)
            && !anchor.is_empty()
        {
            entry.insert("anchor".into(), Value::String(move_path(anchor)));
        }
        moved.insert(move_path(&primary), Value::Object(entry));
    }
    if moved != saved {
        save(repo, moved);
    }
}

/// How many frames `paths` open as, counting a composite or triplet as one frame.
#[must_use]
pub fn frame_count(
    paths: &[String],
    composites: &Map<String, Value>,
    triplets: &Map<String, Value>,
) -> usize {
    let files: std::collections::HashSet<String> = paths.iter().cloned().collect();
    let mut parts = part_paths(
        composites
            .iter()
            .filter(|(primary, _)| files.contains(*primary))
            .map(|(_, entry)| entry.clone()),
    );
    for (red, record) in triplets {
        if files.contains(red) {
            parts.extend(string_list(Some(record)).into_iter().take(2));
        }
    }
    files.iter().filter(|path| !parts.contains(*path)).count()
}

mod utils {
    pub use super::super::rolls::unforked_hash;
}
