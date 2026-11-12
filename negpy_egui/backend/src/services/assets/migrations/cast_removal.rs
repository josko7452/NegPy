//! Zero the shipped-default Cast Removal on legacy slide edits, once.

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::infrastructure::storage::repository::StorageRepository;

use super::common::{EDIT_TABLES, number, save_setting, setting_truthy, string};

const DONE_FLAG: &str = "cast_removal_slide_migrated_v1";
const SHIPPED_CAST_STRENGTH: f64 = 0.5;

fn zero_legacy_row(data: &mut Map<String, Value>) -> bool {
    if !matches!(
        string(data.get("process_mode")).as_str(),
        "Transparency" | "E-6"
    ) {
        return false;
    }
    if number(data.get("cast_removal_strength")) != Some(SHIPPED_CAST_STRENGTH) {
        return false;
    }
    data.insert("cast_removal_strength".into(), json!(0.0));
    true
}

/// Zero the shipped-default Cast Removal on legacy slide edits, once.
pub fn migrate_legacy_slide_cast_removal(repo: &mut StorageRepository) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let result = (|| -> rusqlite::Result<()> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        for (table, keys) in EDIT_TABLES {
            super::common::rewrite_rows(&transaction, table, keys, zero_legacy_row)?;
        }
        transaction.commit()
    })();
    if let Err(error) = result {
        log::error!("Cast Removal migration failed; continuing without it: {error}");
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, json!(true)) {
        log::error!("Could not record the Cast Removal migration: {error}");
    }
}
