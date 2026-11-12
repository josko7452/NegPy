//! JSON preset files under the app's preset directory.
//!
//! Mirrors `negpy/services/assets/presets.py`. A preset name is a filename, so it is
//! sanitized before it reaches a path.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::kernel::system::paths::{replace_file, safe_unlink};

pub const PRESET_NOTES_KEY: &str = "__notes";

/// The unusable characters in a preset name, plus the control range.
const INVALID_NAME_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The usable part of a name: unusable characters become spaces, and leading or trailing
/// dots go, so `../escaped` and `..` cannot address anything.
#[must_use]
pub fn sanitize_preset_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if INVALID_NAME_CHARS.contains(&ch) || ch.is_control() {
                ' '
            } else {
                ch
            }
        })
        .collect();
    cleaned.trim().trim_matches('.').trim().to_string()
}

/// True when the name can be written as is, with nothing silently rewritten.
#[must_use]
pub fn is_valid_preset_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && sanitize_preset_name(name) == trimmed
}

/// Just the config fields, without the preset's own bookkeeping.
#[must_use]
pub fn preset_fields(data: &Map<String, Value>) -> Map<String, Value> {
    data.iter()
        .filter(|(key, _)| key.as_str() != PRESET_NOTES_KEY)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// The preset notes, or an empty string.
#[must_use]
pub fn preset_notes(data: &Map<String, Value>) -> String {
    data.get(PRESET_NOTES_KEY)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A preset payload ready to save: fields plus notes, which are dropped when empty.
#[must_use]
pub fn with_preset_notes(fields: &Map<String, Value>, notes: &str) -> Map<String, Value> {
    let mut out = preset_fields(fields);
    let trimmed = notes.trim();
    if !trimmed.is_empty() {
        out.insert(
            PRESET_NOTES_KEY.to_string(),
            Value::String(trimmed.to_string()),
        );
    }
    out
}

/// JSON I/O for one preset namespace.
#[derive(Clone, Debug)]
pub struct PresetStore {
    dir: PathBuf,
}

impl PresetStore {
    /// The store under `root`, with an optional subdirectory for metadata presets.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, subdir: &str) -> Self {
        let dir = if subdir.is_empty() {
            root.into()
        } else {
            root.into().join(subdir)
        };
        Self { dir }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    /// Case-folded: on macOS and Windows a differently cased name is the same file.
    #[must_use]
    pub fn exists(&self, name: &str) -> bool {
        let wanted = name.trim().to_lowercase();
        self.list()
            .into_iter()
            .any(|listed| listed.to_lowercase() == wanted)
    }

    /// One replace. Writing the new file and deleting the old one would delete the file it
    /// just wrote whenever the two names differ only in case.
    pub fn rename(&self, old: &str, new: &str) -> bool {
        if !is_valid_preset_name(new) {
            return false;
        }
        let source = self.path(old);
        if !source.is_file() {
            return false;
        }
        replace_file(&source, &self.path(new.trim())).is_ok()
    }

    /// Write a preset through a temporary file, then replace the target atomically.
    pub fn save(&self, name: &str, settings: &Map<String, Value>) -> Result<(), PresetError> {
        if !is_valid_preset_name(name) {
            return Err(PresetError::UnusableName(name.to_string()));
        }
        fs::create_dir_all(&self.dir)?;
        let path = self.path(name.trim());
        let temporary = path.with_extension("tmp");
        let text = serde_json::to_string_pretty(settings)
            .map_err(|_| PresetError::UnusableName(name.to_string()))?;
        let result = fs::write(&temporary, text).and_then(|()| replace_file(&temporary, &path));
        if result.is_err() {
            safe_unlink(&temporary);
        }
        result.map_err(PresetError::Io)
    }

    /// The preset's fields, or `None` when it does not exist or is not an object.
    #[must_use]
    pub fn load(&self, name: &str) -> Option<Map<String, Value>> {
        let text = fs::read_to_string(self.path(name)).ok()?;
        match serde_json::from_str::<Value>(&text).ok()? {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }

    /// Every preset name in this namespace.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter_map(|name| name.strip_suffix(".json").map(str::to_string))
            .collect();
        names.sort();
        names
    }

    /// Remove a preset; `false` when it does not exist.
    pub fn delete(&self, name: &str) -> bool {
        let path = self.path(name);
        if !path.exists() {
            return false;
        }
        fs::remove_file(path).is_ok()
    }
}

#[derive(Debug)]
pub enum PresetError {
    UnusableName(String),
    Io(std::io::Error),
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnusableName(name) => write!(formatter, "Unusable preset name: {name:?}"),
            Self::Io(err) => err.fmt(formatter),
        }
    }
}

impl std::error::Error for PresetError {}

impl From<std::io::Error> for PresetError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
