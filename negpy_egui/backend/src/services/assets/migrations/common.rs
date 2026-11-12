//! Shared support for the startup migrations that rewrite saved edit rows.

use rusqlite::{Connection, types::Value as SqlValue};
use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::StorageRepository;

/// A JSON value read as the Python migration's truth test.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

/// Store one global setting, as the migrations' done flags do.
pub fn save_setting(
    repo: &mut StorageRepository,
    key: &str,
    value: Value,
) -> Result<(), crate::infrastructure::storage::repository::RepositoryError> {
    let mut settings = Map::new();
    settings.insert(key.to_string(), value);
    repo.save_global_settings(&settings)
}

/// Whether a global setting holds a truthy value.
#[must_use]
pub fn setting_truthy(repo: &StorageRepository, key: &str) -> bool {
    repo.global_setting(key)
        .ok()
        .flatten()
        .is_some_and(|value| truthy(&value))
}

/// A JSON string as Python `str()` would render the common scalar values.
#[must_use]
pub fn string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

/// A JSON number with Python `float()`'s common inputs.
#[must_use]
pub fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(value) => value.as_f64(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

/// Rewrite every JSON-object row in an edit table, in one transaction.
///
/// `rewrite` returns whether it changed the row. Malformed rows are left untouched, as
/// the Python migrations leave them.
pub fn rewrite_rows<F>(
    connection: &Connection,
    table: &str,
    key_columns: &[&str],
    mut rewrite: F,
) -> rusqlite::Result<()>
where
    F: FnMut(&mut Map<String, Value>) -> bool,
{
    let columns = key_columns.join(", ");
    let select = format!("SELECT {columns}, settings_json FROM {table}");
    let mut statement = connection.prepare(&select)?;
    let rows = statement.query_map([], |row| {
        let mut keys = Vec::with_capacity(key_columns.len());
        for index in 0..key_columns.len() {
            keys.push(row.get::<_, SqlValue>(index)?);
        }
        let settings: Option<String> = row.get(key_columns.len())?;
        Ok((keys, settings))
    })?;
    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let predicate = key_columns
        .iter()
        .map(|column| format!("{column} = ?"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let update = format!("UPDATE {table} SET settings_json = ? WHERE {predicate}");
    for (keys, settings) in rows.drain(..) {
        let Some(settings) = settings.filter(|settings| !settings.is_empty()) else {
            continue;
        };
        let Ok(Value::Object(mut data)) = serde_json::from_str::<Value>(&settings) else {
            continue;
        };
        if !rewrite(&mut data) {
            continue;
        }
        let mut parameters = vec![SqlValue::Text(
            serde_json::to_string(&data)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        )];
        parameters.extend(keys);
        connection.execute(&update, rusqlite::params_from_iter(parameters))?;
    }
    Ok(())
}

/// The three tables a saved edit lives in, with the keys that identify one row.
pub const EDIT_TABLES: &[(&str, &[&str])] = &[
    ("file_settings", &["file_hash"]),
    ("edit_history", &["file_hash", "step_index"]),
    ("work_prints", &["file_hash", "name"]),
];
