//! SQLite storage for edits and global settings.
//!
//! Mirrors the schema and file-settings rows of `negpy/infrastructure/storage/repository.py`.
//! The database is the primary edit store; sidecars are promoted into it.

use std::collections::HashMap;
use std::path::Path;

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
        Ok(connection
            .query_row(
                "SELECT file_path FROM file_settings
                 WHERE file_hash = ?1 AND file_path IS NOT NULL AND file_path != ''",
                [file_hash],
                |row| row.get(0),
            )
            .optional()?)
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

    /// Move a saved edit and its path from one hash to another.
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
        connection.execute(
            "INSERT OR REPLACE INTO file_settings (file_hash, settings_json, file_path)
             VALUES (?1, ?2, ?3)",
            params![new_hash, json, file_path],
        )?;
        connection.execute("DELETE FROM file_settings WHERE file_hash = ?1", [old_hash])?;
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
