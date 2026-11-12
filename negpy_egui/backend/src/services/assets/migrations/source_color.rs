//! Clear a measured source color on B&W frames with no contrast filter, once.

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::infrastructure::storage::repository::StorageRepository;

use super::common::{EDIT_TABLES, save_setting, setting_truthy, string};

const DONE_FLAG: &str = "source_color_chroma_spread_v1";

fn clear_bw_row(data: &mut Map<String, Value>) -> bool {
    if !matches!(
        string(data.get("process_mode")).as_str(),
        "B&W Negative" | "B&W"
    ) {
        return false;
    }
    if data.get("source_color").is_none_or(Value::is_null) {
        return false;
    }
    if string(data.get("bw_filter").or(Some(&json!("none")))) != "none" {
        return false;
    }
    data.insert("source_color".into(), Value::Null);
    true
}

/// Clear source color on B&W rows with no filter, once, so the next load measures it again.
pub fn migrate_bw_source_color(repo: &mut StorageRepository) {
    if setting_truthy(repo, DONE_FLAG) {
        return;
    }
    let result = (|| -> rusqlite::Result<()> {
        let connection = Connection::open(&repo.edits_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        for (table, keys) in EDIT_TABLES {
            super::common::rewrite_rows(&transaction, table, keys, clear_bw_row)?;
        }
        transaction.commit()
    })();
    if let Err(error) = result {
        log::error!("source_color migration failed; continuing without it: {error}");
    }
    if let Err(error) = save_setting(repo, DONE_FLAG, json!(true)) {
        log::error!("Could not record the source_color migration: {error}");
    }
}
