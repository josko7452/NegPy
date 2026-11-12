//! Drop per-frame bounds measured under an older [`BOUNDS_VERSION`].

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::domain::models::workspace::BOUNDS_VERSION;
use crate::infrastructure::storage::repository::StorageRepository;

use super::common::{EDIT_TABLES, number, save_setting, truthy};

const VERSION_KEY: &str = "bounds_version";

fn drop_stale_bounds(data: &mut Map<String, Value>) -> bool {
    if data.get("lock_bounds").is_some_and(truthy) {
        return false;
    }
    let measured = data
        .get("local_floors")
        .and_then(Value::as_array)
        .is_some_and(|floors| floors.iter().any(|value| number(Some(value)) != Some(0.0)));
    if !measured {
        return false;
    }
    data.insert("local_floors".into(), json!([0.0, 0.0, 0.0]));
    data.insert("local_ceils".into(), json!([0.0, 0.0, 0.0]));
    true
}

/// Drop saved per-frame bounds measured under an older analysis version.
///
/// A locked frame keeps its bounds. A failed sweep leaves the version unset, so the next
/// startup retries it.
pub fn migrate_stale_frame_bounds(repo: &mut StorageRepository) {
    let current = repo
        .global_setting(VERSION_KEY)
        .ok()
        .flatten()
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    if current >= BOUNDS_VERSION {
        return;
    }
    let result = (|| -> rusqlite::Result<()> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        for (table, keys) in EDIT_TABLES {
            super::common::rewrite_rows(&transaction, table, keys, drop_stale_bounds)?;
        }
        transaction.commit()
    })();
    if let Err(error) = result {
        log::error!("Stale-bounds migration failed; continuing without it: {error}");
        return;
    }
    if let Err(error) = save_setting(repo, VERSION_KEY, json!(BOUNDS_VERSION)) {
        log::error!("Could not record the bounds migration: {error}");
    }
}
