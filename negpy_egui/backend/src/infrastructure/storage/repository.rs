//! SQLite storage for edits and global settings.
//!
//! Mirrors the schema and file-settings rows of `negpy/infrastructure/storage/repository.py`.
//! The database is the primary edit store; sidecars are promoted into it.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::domain::models::workspace::WorkspaceConfig;

/// The edits and settings databases, opened as one repository.
pub struct StorageRepository {
    pub edits_db_path: String,
    pub settings_db_path: String,
    global_json: HashMap<String, String>,
}

impl StorageRepository {
    /// Open both paths without creating them.
    pub fn new(edits_db_path: impl Into<String>, settings_db_path: impl Into<String>) -> Self {
        Self {
            edits_db_path: edits_db_path.into(),
            settings_db_path: settings_db_path.into(),
            global_json: HashMap::new(),
        }
    }

    /// Create the parent directory and every table the Python repository creates.
    pub fn initialize(&self) -> Result<(), RepositoryError> {
        if let Some(parent) = Path::new(&self.edits_db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let edits = self.connect(&self.edits_db_path)?;
        edits.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS file_settings (
                 file_hash TEXT PRIMARY KEY,
                 settings_json TEXT,
                 file_path TEXT
             );
             CREATE TABLE IF NOT EXISTS edit_history (
                 file_hash TEXT,
                 step_index INTEGER,
                 settings_json TEXT,
                 PRIMARY KEY (file_hash, step_index)
             );
             CREATE TABLE IF NOT EXISTS work_prints (
                 file_hash TEXT,
                 name TEXT,
                 created_at REAL,
                 settings_json TEXT,
                 PRIMARY KEY (file_hash, name)
             );
             CREATE TABLE IF NOT EXISTS file_marks (
                 file_hash TEXT PRIMARY KEY,
                 mark TEXT NOT NULL,
                 file_path TEXT
             );
             CREATE TABLE IF NOT EXISTS image_embeddings (
                 file_hash TEXT PRIMARY KEY,
                 embedding BLOB,
                 model_version TEXT,
                 file_path TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_file_settings_path
                 ON file_settings(file_path);",
        )?;

        let settings = self.connect(&self.settings_db_path)?;
        settings.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS global_settings (
                 key TEXT PRIMARY KEY,
                 value_json TEXT
             );",
        )?;
        Ok(())
    }

    fn connect(&self, path: &str) -> rusqlite::Result<Connection> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        Ok(connection)
    }

    /// Save or replace one frame's edit and its optional file path.
    pub fn save_file_settings(
        &self,
        file_hash: &str,
        settings: &WorkspaceConfig,
        file_path: &str,
    ) -> rusqlite::Result<()> {
        let json = serde_json::to_string(&settings.to_dict())
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "INSERT OR REPLACE INTO file_settings (file_hash, settings_json, file_path)
             VALUES (?1, ?2, ?3)",
            params![file_hash, json, file_path],
        )?;
        Ok(())
    }

    /// The saved edit for `file_hash`, if one exists.
    pub fn load_file_settings(&self, file_hash: &str) -> rusqlite::Result<Option<WorkspaceConfig>> {
        let connection = self.connect(&self.edits_db_path)?;
        let json: Option<String> = connection
            .query_row(
                "SELECT settings_json FROM file_settings WHERE file_hash = ?1",
                [file_hash],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json
            .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
            .map(|flat| WorkspaceConfig::from_flat_dict(&flat)))
    }

    /// The path this hash's edit was last saved against.
    pub fn path_for_file_hash(&self, file_hash: &str) -> rusqlite::Result<Option<String>> {
        let connection = self.connect(&self.edits_db_path)?;
        connection
            .query_row(
                "SELECT file_path FROM file_settings
                 WHERE file_hash = ?1 AND file_path IS NOT NULL AND file_path != ''",
                [file_hash],
                |row| row.get(0),
            )
            .optional()
    }

    /// The saved edits among `hashes`, without parsing a missing row.
    pub fn saved_hashes(&self, hashes: &[String]) -> rusqlite::Result<Vec<String>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut saved = Vec::new();
        for chunk in hashes.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql =
                format!("SELECT file_hash FROM file_settings WHERE file_hash IN ({placeholders})");
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(chunk), |row| {
                row.get::<_, String>(0)
            })?;
            for row in rows {
                saved.push(row?);
            }
        }
        Ok(saved)
    }

    /// The saved edits among `hashes`, in one connection.
    ///
    /// A hash with no saved edit is absent, which marks its frame as never edited.
    pub fn load_file_settings_many(
        &self,
        hashes: &[String],
    ) -> rusqlite::Result<HashMap<String, WorkspaceConfig>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut out = HashMap::new();
        for chunk in hashes.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT file_hash, settings_json FROM file_settings WHERE file_hash IN ({placeholders})"
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(rusqlite::params_from_iter(chunk), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (hash, json) = row?;
                if let Ok(flat) = serde_json::from_str::<Map<String, Value>>(&json) {
                    out.insert(hash, WorkspaceConfig::from_flat_dict(&flat));
                }
            }
        }
        Ok(out)
    }

    /// Every saved edit that knows its path, keyed by that path.
    pub fn load_settings_by_path(&self) -> rusqlite::Result<HashMap<String, WorkspaceConfig>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT file_path, settings_json FROM file_settings
             WHERE file_path IS NOT NULL AND file_path != ''",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (path, json) = row?;
            if let Ok(flat) = serde_json::from_str::<Map<String, Value>>(&json) {
                out.insert(path, WorkspaceConfig::from_flat_dict(&flat));
            }
        }
        Ok(out)
    }

    /// Persist a triage mark. `None` clears it.
    pub fn save_file_mark(
        &self,
        file_hash: &str,
        mark: Option<&str>,
        file_path: &str,
    ) -> rusqlite::Result<()> {
        let connection = self.connect(&self.edits_db_path)?;
        if let Some(mark) = mark {
            connection.execute(
                "INSERT OR REPLACE INTO file_marks (file_hash, mark, file_path) VALUES (?1, ?2, ?3)",
                params![file_hash, mark, file_path],
            )?;
        } else {
            connection.execute("DELETE FROM file_marks WHERE file_hash = ?1", [file_hash])?;
        }
        Ok(())
    }

    /// Every triage mark, keyed by file hash.
    pub fn file_marks(&self) -> rusqlite::Result<HashMap<String, String>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare("SELECT file_hash, mark FROM file_marks")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    /// Triage marks keyed by path. Rows written before the path column exists are absent.
    pub fn file_marks_by_path(&self) -> rusqlite::Result<HashMap<String, String>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT file_path, mark FROM file_marks WHERE file_path IS NOT NULL AND file_path != ''",
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    /// Delete this hash's edit, history, work prints and embedding. A triage mark stays.
    pub fn delete_file_settings(&self, file_hash: &str) -> rusqlite::Result<()> {
        let connection = self.connect(&self.edits_db_path)?;
        for table in [
            "file_settings",
            "edit_history",
            "work_prints",
            "image_embeddings",
        ] {
            connection.execute(
                &format!("DELETE FROM {table} WHERE file_hash = ?1"),
                [file_hash],
            )?;
        }
        Ok(())
    }

    /// Store one undo-history step.
    pub fn save_history_step(
        &self,
        file_hash: &str,
        index: i64,
        settings: &WorkspaceConfig,
    ) -> rusqlite::Result<()> {
        let json = serde_json::to_string(&settings.to_dict())
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "INSERT OR REPLACE INTO edit_history (file_hash, step_index, settings_json)
             VALUES (?1, ?2, ?3)",
            params![file_hash, index, json],
        )?;
        Ok(())
    }

    /// The stored edit at one history step.
    pub fn history_step(
        &self,
        file_hash: &str,
        index: i64,
    ) -> rusqlite::Result<Option<WorkspaceConfig>> {
        let connection = self.connect(&self.edits_db_path)?;
        let json: Option<String> = connection
            .query_row(
                "SELECT settings_json FROM edit_history WHERE file_hash = ?1 AND step_index = ?2",
                params![file_hash, index],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json
            .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
            .map(|flat| WorkspaceConfig::from_flat_dict(&flat)))
    }

    /// Every history step, ordered by index.
    pub fn all_history(&self, file_hash: &str) -> rusqlite::Result<Vec<(i64, WorkspaceConfig)>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT step_index, settings_json FROM edit_history WHERE file_hash = ?1 ORDER BY step_index",
        )?;
        let rows = statement.query_map([file_hash], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (index, json): (i64, String) = row?;
            if let Ok(flat) = serde_json::from_str::<Map<String, Value>>(&json) {
                out.push((index, WorkspaceConfig::from_flat_dict(&flat)));
            }
        }
        Ok(out)
    }

    /// The highest stored history index.
    pub fn max_history_index(&self, file_hash: &str) -> rusqlite::Result<i64> {
        let connection = self.connect(&self.edits_db_path)?;
        Ok(connection
            .query_row(
                "SELECT MAX(step_index) FROM edit_history WHERE file_hash = ?1",
                [file_hash],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(0))
    }

    /// Delete every history step above `index`.
    pub fn truncate_history_above(&self, file_hash: &str, index: i64) -> rusqlite::Result<()> {
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "DELETE FROM edit_history WHERE file_hash = ?1 AND step_index > ?2",
            params![file_hash, index],
        )?;
        Ok(())
    }

    /// Store or replace a named version of this frame's edit.
    pub fn save_work_print(
        &self,
        file_hash: &str,
        name: &str,
        settings: &WorkspaceConfig,
    ) -> rusqlite::Result<()> {
        let json = serde_json::to_string(&settings.to_dict())
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "INSERT OR REPLACE INTO work_prints (file_hash, name, created_at, settings_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_hash, name, created_at, json],
        )?;
        Ok(())
    }

    /// This frame's work-print names, newest first.
    pub fn work_prints(&self, file_hash: &str) -> rusqlite::Result<Vec<String>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT name FROM work_prints WHERE file_hash = ?1 ORDER BY created_at DESC, rowid DESC",
        )?;
        let rows = statement.query_map([file_hash], |row| row.get(0))?;
        rows.collect()
    }

    /// The stored edit for one work print.
    pub fn work_print(
        &self,
        file_hash: &str,
        name: &str,
    ) -> rusqlite::Result<Option<WorkspaceConfig>> {
        let connection = self.connect(&self.edits_db_path)?;
        let json: Option<String> = connection
            .query_row(
                "SELECT settings_json FROM work_prints WHERE file_hash = ?1 AND name = ?2",
                params![file_hash, name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json
            .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
            .map(|flat| WorkspaceConfig::from_flat_dict(&flat)))
    }

    /// Rename a stored work print.
    pub fn rename_work_print(
        &self,
        file_hash: &str,
        name: &str,
        new_name: &str,
    ) -> rusqlite::Result<()> {
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "UPDATE work_prints SET name = ?1 WHERE file_hash = ?2 AND name = ?3",
            params![new_name, file_hash, name],
        )?;
        Ok(())
    }

    /// Delete a stored work print.
    pub fn delete_work_print(&self, file_hash: &str, name: &str) -> rusqlite::Result<()> {
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "DELETE FROM work_prints WHERE file_hash = ?1 AND name = ?2",
            params![file_hash, name],
        )?;
        Ok(())
    }

    /// Store one semantic-search vector for a frame.
    pub fn save_embedding(
        &self,
        file_hash: &str,
        vector: &[f32],
        model_version: &str,
        file_path: &str,
    ) -> rusqlite::Result<()> {
        let bytes: Vec<u8> = vector
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let connection = self.connect(&self.edits_db_path)?;
        connection.execute(
            "INSERT OR REPLACE INTO image_embeddings (file_hash, embedding, model_version, file_path)
             VALUES (?1, ?2, ?3, ?4)",
            params![file_hash, bytes, model_version, file_path],
        )?;
        Ok(())
    }

    /// Cached vectors for `hashes` under one model version.
    pub fn embeddings_for(
        &self,
        hashes: &[String],
        model_version: &str,
    ) -> rusqlite::Result<HashMap<String, Vec<f32>>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut out = HashMap::new();
        for chunk in hashes.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT file_hash, embedding FROM image_embeddings WHERE model_version = ? AND file_hash IN ({placeholders})"
            );
            let mut statement = connection.prepare(&sql)?;
            let mut binds: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk.len() + 1);
            binds.push(&model_version);
            binds.extend(chunk.iter().map(|hash| hash as &dyn rusqlite::ToSql));
            let rows = statement.query_map(binds.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?;
            for row in rows {
                let (hash, bytes) = row?;
                out.insert(hash, bytes_to_f32(&bytes));
            }
        }
        Ok(out)
    }

    /// Every vector under `model_version`, with its stored path.
    pub fn all_embeddings(
        &self,
        model_version: &str,
    ) -> rusqlite::Result<HashMap<String, (String, Vec<f32>)>> {
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT file_hash, file_path, embedding FROM image_embeddings WHERE model_version = ?1",
        )?;
        let rows = statement.query_map([model_version], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (hash, path, bytes) = row?;
            out.insert(hash, (path, bytes_to_f32(&bytes)));
        }
        Ok(out)
    }

    /// Look up a whole-file edit by path. Half-frame rows (`<hash>#<n>`) are excluded.
    pub fn load_file_settings_by_path(
        &self,
        file_path: &str,
    ) -> rusqlite::Result<Option<(String, WorkspaceConfig)>> {
        if file_path.is_empty() {
            return Ok(None);
        }
        let connection = self.connect(&self.edits_db_path)?;
        let row: Option<(String, String)> = connection
            .query_row(
                "SELECT file_hash, settings_json FROM file_settings
                 WHERE file_path = ?1 AND file_hash NOT LIKE '%#%'",
                [file_path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.and_then(|(hash, json)| {
            serde_json::from_str::<Map<String, Value>>(&json)
                .ok()
                .map(|flat| (hash, WorkspaceConfig::from_flat_dict(&flat)))
        }))
    }

    /// Move a saved edit, its history, work prints and triage mark to another hash.
    pub fn rehome_file_settings(
        &self,
        old_hash: &str,
        new_hash: &str,
        file_path: &str,
    ) -> rusqlite::Result<()> {
        if old_hash == new_hash {
            return Ok(());
        }
        let connection = self.connect(&self.edits_db_path)?;
        let exists: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM file_settings WHERE file_hash = ?1",
                [new_hash],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Ok(());
        }
        let json: Option<String> = connection
            .query_row(
                "SELECT settings_json FROM file_settings WHERE file_hash = ?1",
                [old_hash],
                |row| row.get(0),
            )
            .optional()?;
        let Some(json) = json else {
            return Ok(());
        };
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO file_settings (file_hash, settings_json, file_path)
             VALUES (?1, ?2, ?3)",
            params![new_hash, json, file_path],
        )?;
        transaction.execute("DELETE FROM file_settings WHERE file_hash = ?1", [old_hash])?;
        for table in ["edit_history", "work_prints", "file_marks"] {
            transaction.execute(
                &format!("UPDATE OR REPLACE {table} SET file_hash = ?1 WHERE file_hash = ?2"),
                params![new_hash, old_hash],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Base names of edited files saved directly under `folder`.
    pub fn edited_file_names_under(&self, folder: &str) -> rusqlite::Result<Vec<String>> {
        let separator = std::path::MAIN_SEPARATOR;
        let under = format!("{}{separator}", folder.trim_end_matches(['/', '\\']));
        let key = path_key(&under);
        let prefix_len = under.len();
        let connection = self.connect(&self.edits_db_path)?;
        let mut statement = connection.prepare(
            "SELECT file_path FROM file_settings
             WHERE lower(substr(file_path, 1, ?1)) = lower(?2)",
        )?;
        let rows = statement.query_map(params![prefix_len as i64, under], |row| {
            row.get::<_, String>(0)
        })?;
        let mut names = Vec::new();
        let case_insensitive = cfg!(windows);
        for row in rows {
            let path = row?;
            let Some(rest) = path.get(prefix_len..) else {
                continue;
            };
            let prefix = &path[..prefix_len];
            let matches = if case_insensitive {
                prefix.eq_ignore_ascii_case(key.trim_end_matches(['/', '\\']))
            } else {
                path_key(prefix) == key
            };
            if matches && !rest.contains(['/', '\\']) {
                if let Some(name) = path.rsplit(['/', '\\']).next() {
                    if !names.iter().any(|known| known == name) {
                        names.push(name.to_string());
                    }
                }
            }
        }
        Ok(names)
    }

    /// Repoint every path-keyed row under one folder prefix to another.
    pub fn rehome_path_prefix(&self, old_prefix: &str, new_prefix: &str) -> rusqlite::Result<()> {
        let under = format!("{old_prefix}{}", std::path::MAIN_SEPARATOR);
        let connection = self.connect(&self.edits_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        for table in ["file_settings", "file_marks", "image_embeddings"] {
            let sql = format!(
                "UPDATE {table} SET file_path = ?1 || substr(file_path, ?2)
                 WHERE file_path = ?3 OR substr(file_path, 1, ?4) = ?5"
            );
            transaction.execute(
                &sql,
                params![
                    new_prefix,
                    old_prefix.len() as i64 + 1,
                    old_prefix,
                    under.len() as i64,
                    under
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Save many global settings in one transaction.
    pub fn save_global_settings(
        &mut self,
        settings: &Map<String, Value>,
    ) -> Result<(), RepositoryError> {
        let connection = self.connect(&self.settings_db_path)?;
        let transaction = connection.unchecked_transaction()?;
        for (key, value) in settings {
            let json = serde_json::to_string(value)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
            transaction.execute(
                "INSERT OR REPLACE INTO global_settings (key, value_json) VALUES (?1, ?2)",
                params![key, json],
            )?;
            self.global_json.insert(key.clone(), json);
        }
        transaction.commit()?;
        Ok(())
    }

    /// Read a global setting, parsing the stored JSON.
    pub fn global_setting(&self, key: &str) -> Result<Option<Value>, RepositoryError> {
        let connection = self.connect(&self.settings_db_path)?;
        let json: Option<String> = connection
            .query_row(
                "SELECT value_json FROM global_settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json.and_then(|text| serde_json::from_str(&text).ok()))
    }

    /// Row counts per category and on-disk sizes for the database dialog.
    pub fn database_stats(&self) -> Result<DatabaseStats, RepositoryError> {
        let edits = self.connect(&self.edits_db_path)?;
        let settings = self.connect(&self.settings_db_path)?;
        let presets = self.global_setting("export_presets")?;
        let export_presets = presets
            .as_ref()
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let global_settings = count(&settings, "global_settings")?;
        Ok(DatabaseStats {
            file_settings: count(&edits, "file_settings")?,
            edit_history: count(&edits, "edit_history")?,
            work_prints: count(&edits, "work_prints")?,
            file_marks: count(&edits, "file_marks")?,
            export_presets,
            app_preferences: global_settings.saturating_sub(usize::from(presets.is_some())),
            edits_db_bytes: db_size_bytes(&self.edits_db_path),
            settings_db_bytes: db_size_bytes(&self.settings_db_path),
        })
    }

    /// Drop saved looks and their records, preserving rig and app preferences.
    pub fn clear_saved_edits(&self) -> Result<(), RepositoryError> {
        wipe(
            &self.edits_db_path,
            &["file_settings", "edit_history", "work_prints", "file_marks"],
        )?;
        Ok(())
    }

    /// Drop every row in both databases, preserving the schema.
    pub fn reset_everything(&mut self) -> Result<(), RepositoryError> {
        wipe(
            &self.edits_db_path,
            &["file_settings", "edit_history", "work_prints", "file_marks"],
        )?;
        wipe(&self.settings_db_path, &["global_settings"])?;
        self.global_json.clear();
        Ok(())
    }
}

/// The database dialog's row counts and on-disk sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DatabaseStats {
    pub file_settings: usize,
    pub edit_history: usize,
    pub work_prints: usize,
    pub file_marks: usize,
    pub export_presets: usize,
    pub app_preferences: usize,
    pub edits_db_bytes: u64,
    pub settings_db_bytes: u64,
}

fn count(connection: &Connection, table: &str) -> rusqlite::Result<usize> {
    let count: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })?;
    Ok(usize::try_from(count).unwrap_or(0))
}

fn db_size_bytes(path: &str) -> u64 {
    ["", "-wal", "-shm"]
        .iter()
        .filter_map(|suffix| fs::metadata(format!("{path}{suffix}")).ok())
        .map(|metadata| metadata.len())
        .sum()
}

/// The path form two spellings of one path share on this platform.
fn path_key(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if cfg!(windows) {
        trimmed.replace('/', "\\").to_lowercase()
    } else {
        trimmed.to_string()
    }
}

fn wipe(path: &str, tables: &[&str]) -> Result<(), RepositoryError> {
    let connection = Connection::open(path)?;
    for table in tables {
        connection.execute(&format!("DELETE FROM {table}"), [])?;
    }
    drop(connection);
    let connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
    Ok(())
}

#[derive(Debug)]
pub enum RepositoryError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => err.fmt(formatter),
            Self::Sqlite(err) => err.fmt(formatter),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<std::io::Error> for RepositoryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<rusqlite::Error> for RepositoryError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err)
    }
}

fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}
