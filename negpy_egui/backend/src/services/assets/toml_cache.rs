//! A parsed-TOML cache keyed by path.
//!
//! Mirrors `negpy/services/assets/toml_cache.py`. A sidebar sync re-lists the same profile
//! files, so the cache checks only the stat and re-reads when the file changes.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use serde_json::Value;
use toml::Table;

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    modified: SystemTime,
    size: u64,
    value: Option<Table>,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, Entry>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

/// Parsed TOML for `path`, re-read when the file changes; `None` when unreadable or invalid.
pub fn load_toml_cached(path: &Path) -> Option<Table> {
    let key = path.to_string_lossy().into_owned();
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let size = metadata.len();

    let mut guard = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    if let Some(entry) = cache.entries.get(&key)
        && entry.modified == modified
        && entry.size == size
    {
        return entry.value.clone();
    }

    let value = fs::read_to_string(path)
        .ok()
        .and_then(|text| text.parse::<Table>().ok());
    cache.entries.insert(
        key,
        Entry {
            modified,
            size,
            value: value.clone(),
        },
    );
    value
}

/// A parsed JSON value, kept separate so the TOML cache does not serialize through JSON.
pub fn load_json_cached(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
