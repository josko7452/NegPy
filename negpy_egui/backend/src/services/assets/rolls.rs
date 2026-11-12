//! Roll membership and roll-scoped edit identities.
//!
//! Mirrors `negpy/services/assets/rolls.py`: a roll decides which files a Library opens.
//! Edits stay under each frame's own hash, except a forked path, which gets a
//! `#roll:<id>` suffix.

use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::StorageRepository;

pub const ROLLS_KEY: &str = "rolls_by_id";
const FORK_SEP: &str = "#roll:";

/// A roll record, stored in the `ROLLS_KEY` global setting.
pub type RollEntry = Map<String, Value>;

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
