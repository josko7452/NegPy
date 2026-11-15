//! Path-keyed records follow a folder renamed on disk.
//!
//! Mirrors `negpy/services/assets/rehome.py`.

use std::path::Path;

use crate::infrastructure::storage::repository::StorageRepository;
use crate::services::assets::composites;
use crate::services::assets::rolls;

pub const RELINK_NOT_A_FOLDER_ROLL: &str = "not_a_folder_roll";
pub const RELINK_NOT_A_FOLDER: &str = "not_a_folder";
pub const RELINK_FOLDER_TAKEN: &str = "folder_taken";

fn trimmed(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
}

/// A path mover that rewrites `old_prefix` and its children.
#[must_use]
pub fn prefix_mover(old_prefix: &str, new_prefix: &str) -> impl Fn(&str) -> String {
    let old = trimmed(old_prefix).to_string();
    let new = trimmed(new_prefix).to_string();
    move |path: &str| {
        if path == old {
            return new.clone();
        }
        for separator in ['/', '\\'] {
            let prefix = format!("{old}{separator}");
            if let Some(rest) = path.strip_prefix(&prefix) {
                return format!("{new}{separator}{rest}");
            }
        }
        path.to_string()
    }
}

/// Repoint the edits DB, rolls and composites from `old_prefix` to `new_prefix`.
pub fn rehome_path_prefix(repo: &mut StorageRepository, old_prefix: &str, new_prefix: &str) {
    let old = trimmed(old_prefix);
    let new = trimmed(new_prefix);
    if let Err(error) = repo.rehome_path_prefix(old, new) {
        log::error!("Could not rehome the edits database: {error}");
        return;
    }
    let move_path = prefix_mover(old, new);
    rolls::rehome_paths(repo, &move_path);
    composites::rehome_paths(repo, &move_path);
}

/// True when `entry` is a folder roll whose folder is not on disk.
#[must_use]
pub fn entry_folder_missing(entry: Option<&serde_json::Value>) -> bool {
    entry.is_some_and(|entry| {
        entry.get("kind").and_then(serde_json::Value::as_str) == Some("folder")
            && !Path::new(
                entry
                    .get("folder_path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            )
            .is_dir()
    })
}

/// True when this folder roll's folder is not on disk.
#[must_use]
pub fn folder_roll_missing(repo: &StorageRepository, roll_id: &str) -> bool {
    entry_folder_missing(rolls::roll_for_id(repo, roll_id).as_ref())
}

/// False when `path` holds none of the edited files the roll saved under its old folder.
#[must_use]
pub fn folder_fits_roll(repo: &StorageRepository, roll_id: &str, path: &str) -> bool {
    let entry = rolls::roll_for_id(repo, roll_id).unwrap_or_default();
    let folder = entry
        .get("folder_path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let known: Vec<String> = repo
        .edited_file_names_under(folder)
        .unwrap_or_default()
        .into_iter()
        .map(|name| name.to_lowercase())
        .collect();
    if known.is_empty() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    let present: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| name.to_lowercase())
        .collect();
    known.iter().any(|name| present.contains(name))
}

/// Point a folder roll at its renamed folder; `(old path, "")` or `(None, reason)`.
pub fn relink_folder_roll(
    repo: &mut StorageRepository,
    roll_id: &str,
    new_path: &str,
) -> (Option<String>, &'static str) {
    let Some(entry) = rolls::roll_for_id(repo, roll_id) else {
        return (None, RELINK_NOT_A_FOLDER_ROLL);
    };
    if entry.get("kind").and_then(serde_json::Value::as_str) != Some("folder") {
        return (None, RELINK_NOT_A_FOLDER_ROLL);
    }
    if !Path::new(new_path).is_dir() {
        return (None, RELINK_NOT_A_FOLDER);
    }
    if let Some(other) = rolls::folder_roll_id_for_path(repo, new_path)
        && other != roll_id
    {
        return (None, RELINK_FOLDER_TAKEN);
    }
    let old_path = entry
        .get("folder_path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let default_name = !old_path.is_empty()
        && entry.get("name").and_then(serde_json::Value::as_str)
            == Some(rolls::folder_roll_name(repo, &old_path).as_str());
    if !old_path.is_empty() {
        rehome_path_prefix(repo, &old_path, new_path);
    }
    if default_name {
        let name = rolls::folder_roll_name(repo, new_path);
        rolls::rename_roll(repo, roll_id, &name);
    }
    (Some(old_path), "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn repository(dir: &Path) -> StorageRepository {
        let repo = StorageRepository::new(
            dir.join("edits.db").display().to_string(),
            dir.join("settings.db").display().to_string(),
        );
        repo.initialize().unwrap();
        repo
    }

    #[test]
    fn a_prefix_move_rewrites_children_and_leaves_a_sibling_prefix_alone() {
        let move_path = prefix_mover("/roll1/", "/roll1-portra");
        assert_eq!(move_path("/roll1/a.tif"), "/roll1-portra/a.tif");
        assert_eq!(move_path("/roll1"), "/roll1-portra");
        assert_eq!(move_path("/roll10/b.tif"), "/roll10/b.tif");
    }

    #[test]
    fn every_path_keyed_store_follows_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = repository(dir.path());
        let old = dir.path().join("roll1");
        let new = dir.path().join("roll1-portra");
        let a = old.join("a.tif").display().to_string();
        let b = old.join("b.tif").display().to_string();
        let elsewhere = dir
            .path()
            .join("roll10")
            .join("c.tif")
            .display()
            .to_string();
        repo.save_file_settings("ha", &Default::default(), &a)
            .unwrap();
        repo.save_file_settings("hc", &Default::default(), &elsewhere)
            .unwrap();
        repo.save_file_mark("ha", Some("keeper"), &a).unwrap();
        let roll_id =
            rolls::create_virtual_roll(&mut repo, "picks", &[a.clone(), elsewhere.clone()]);
        let composites =
            json!({a.clone(): {"kind": "stitch", "paths": [a.clone(), b.clone()], "hash": "hs"}});
        let mut settings = serde_json::Map::new();
        settings.insert(composites::COMPOSITES_KEY.to_string(), composites);
        repo.save_global_settings(&settings).unwrap();

        rehome_path_prefix(
            &mut repo,
            &old.display().to_string(),
            &new.display().to_string(),
        );

        assert!(
            repo.load_file_settings_by_path(&new.join("a.tif").display().to_string())
                .unwrap()
                .is_some()
        );
        assert!(
            repo.load_file_settings_by_path(&elsewhere)
                .unwrap()
                .is_some()
        );
        assert!(
            repo.file_marks_by_path()
                .unwrap()
                .contains_key(&new.join("a.tif").display().to_string())
        );
        assert_eq!(
            rolls::roll_for_id(&repo, &roll_id).unwrap()["member_paths"][0],
            json!(new.join("a.tif").display().to_string())
        );
        let saved = composites::saved_composites(&mut repo);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved.keys().next().unwrap(),
            &new.join("a.tif").display().to_string()
        );
    }

    #[test]
    fn relink_refuses_a_folder_another_roll_owns() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = repository(dir.path());
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let gone = rolls::recognize_folder(
            &mut repo,
            &dir.path().join("gone").display().to_string(),
            None,
        );
        rolls::recognize_folder(&mut repo, &b.display().to_string(), None);
        assert_eq!(
            relink_folder_roll(&mut repo, &gone, &b.display().to_string()),
            (None, RELINK_FOLDER_TAKEN)
        );
        assert_eq!(
            relink_folder_roll(
                &mut repo,
                &gone,
                &dir.path().join("nope").display().to_string()
            ),
            (None, RELINK_NOT_A_FOLDER)
        );
        assert_eq!(
            relink_folder_roll(&mut repo, "no-such-roll", &b.display().to_string()),
            (None, RELINK_NOT_A_FOLDER_ROLL)
        );
    }
}
