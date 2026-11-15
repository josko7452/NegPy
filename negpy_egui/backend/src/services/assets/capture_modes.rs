//! Capture modes: how NegPy shot a single-file frame.
//!
//! Mirrors `negpy/services/assets/capture_modes.py`: one global setting per folder, a
//! file name to mode map, read back for the frame's mark.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde_json::{Map, Value};

use crate::infrastructure::storage::repository::StorageRepository;

pub const BLEND: &str = "blend";
pub const SINGLE: &str = "single";
pub const MODES: [&str; 2] = [BLEND, SINGLE];
const KEY_PREFIX: &str = "capture_modes:";

fn key(folder: &str) -> String {
    format!(
        "{KEY_PREFIX}{}",
        crate::kernel::system::paths::normalize(Path::new(folder)).display()
    )
}

fn folder_modes(repo: &StorageRepository, folder: &str) -> Map<String, Value> {
    match repo.global_setting(&key(folder)).ok().flatten() {
        Some(Value::Object(modes)) => modes,
        _ => Map::new(),
    }
}

/// Record one path's mode in its folder's setting.
pub fn record_capture_mode(
    repo: &mut StorageRepository,
    path: &str,
    mode: &str,
) -> Result<(), String> {
    if !MODES.contains(&mode) {
        return Err(format!("unknown capture mode: {mode}"));
    }
    let path = Path::new(path);
    let folder = path
        .parent()
        .map_or_else(String::new, |folder| folder.display().to_string());
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut modes = folder_modes(repo, &folder);
    if modes.get(&name).and_then(Value::as_str) == Some(mode) {
        return Ok(());
    }
    modes.insert(name, Value::String(mode.to_string()));
    let mut values = Map::new();
    values.insert(key(&folder), Value::Object(modes));
    repo.save_global_settings(&values)
        .map_err(|error| error.to_string())
}

/// `{path: mode}` for each path a capture recorded.
#[must_use]
pub fn capture_modes(repo: &StorageRepository, paths: &[String]) -> HashMap<String, String> {
    let mut by_folder: BTreeMap<String, Vec<&String>> = BTreeMap::new();
    for path in paths {
        let folder = Path::new(path)
            .parent()
            .map_or_else(String::new, |folder| folder.display().to_string());
        by_folder.entry(folder).or_default().push(path);
    }
    let mut found = HashMap::new();
    for (folder, members) in by_folder {
        let modes = folder_modes(repo, &folder);
        for path in members {
            let name = Path::new(path)
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            if let Some(mode) = modes.get(&name).and_then(Value::as_str)
                && MODES.contains(&mode)
            {
                found.insert(path.clone(), mode.to_string());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> (tempfile::TempDir, StorageRepository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = StorageRepository::new(
            dir.path().join("edits.db").display().to_string(),
            dir.path().join("settings.db").display().to_string(),
        );
        repo.initialize().unwrap();
        (dir, repo)
    }

    #[test]
    fn a_mode_is_stored_by_its_folder_and_file_name() {
        let (_dir, mut repo) = repository();
        record_capture_mode(&mut repo, "/roll/F1.ARW", BLEND).unwrap();
        let found = capture_modes(&repo, &["/roll/F1.ARW".to_string()]);
        assert_eq!(found["/roll/F1.ARW"], BLEND);
        assert!(capture_modes(&repo, &["/other/F1.ARW".to_string()]).is_empty());
    }

    #[test]
    fn an_unknown_mode_is_refused() {
        let (_dir, mut repo) = repository();
        assert!(record_capture_mode(&mut repo, "/roll/F1.ARW", "bracket").is_err());
    }
}
