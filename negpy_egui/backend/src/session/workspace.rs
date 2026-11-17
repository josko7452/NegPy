//! The headless workspace session: files, the active frame and its edit.
//!
//! Mirrors `negpy/domain/session.py` without Qt: uploads stay managed cache files, local
//! paths stay in place, and the database wins over an in-memory edit.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::domain::models::WorkspaceConfig;
use crate::infrastructure::storage::local_asset_store::LocalAssetStore;
use crate::infrastructure::storage::repository::StorageRepository;
use crate::kernel::system::config::default_workspace_config;

/// One file the session manages: its display name, path and content hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionFile {
    pub name: String,
    pub path: String,
    pub hash: String,
}

/// The files an open session holds, and the edit of the selected one.
#[derive(Clone, Debug)]
pub struct WorkspaceSession {
    pub session_id: String,
    pub repository: StorageRepository,
    pub asset_store: LocalAssetStore,
    pub uploaded_files: Vec<SessionFile>,
    pub file_settings: HashMap<String, WorkspaceConfig>,
    pub selected_file_idx: usize,
    pub watched_folders: BTreeSet<String>,
}

impl WorkspaceSession {
    /// A session over an opened repository and asset store.
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        repository: StorageRepository,
        asset_store: LocalAssetStore,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            repository,
            asset_store,
            uploaded_files: Vec::new(),
            file_settings: HashMap::new(),
            selected_file_idx: 0,
            watched_folders: BTreeSet::new(),
        }
    }

    /// Drop managed uploads that left the widget while keeping local paths, and register
    /// the new names. `raw_files` maps a name to the bytes an upload carried.
    pub fn sync_files(
        &mut self,
        current_uploaded_names: &BTreeSet<String>,
        raw_files: &[(String, Vec<u8>)],
    ) {
        let cache_dir = self.asset_store.cache_dir().to_path_buf();
        let managed: Vec<SessionFile> = self
            .uploaded_files
            .iter()
            .filter(|file| path_is_under(Path::new(&file.path), &cache_dir))
            .cloned()
            .collect();
        let last_managed_names: BTreeSet<String> =
            managed.iter().map(|file| file.name.clone()).collect();
        let new_names: Vec<String> = current_uploaded_names
            .difference(&last_managed_names)
            .cloned()
            .collect();
        for name in new_names {
            let Some((_, bytes)) = raw_files.iter().find(|(raw_name, _)| raw_name == &name) else {
                continue;
            };
            if let Some((path, hash)) = self.register_upload(&name, bytes) {
                self.uploaded_files.push(SessionFile { name, path, hash });
            }
        }
        let removed: BTreeSet<String> = last_managed_names
            .difference(current_uploaded_names)
            .cloned()
            .collect();
        if !removed.is_empty() {
            self.uploaded_files
                .retain(|file| !(removed.contains(&file.name) && managed.contains(file)));
            if self.selected_file_idx >= self.uploaded_files.len() {
                self.selected_file_idx = self.uploaded_files.len().saturating_sub(1);
            }
        }
    }

    /// Register local paths; an upload buffer is not a path and is skipped.
    pub fn add_local_assets(&mut self, paths: &[PathBuf]) {
        let current: BTreeSet<String> = self
            .uploaded_files
            .iter()
            .map(|file| file.path.clone())
            .collect();
        for path in paths {
            let text = path.display().to_string();
            if current.contains(&text) {
                continue;
            }
            let Ok(Some((registered, hash))) = self.asset_store.register_path(path) else {
                continue;
            };
            let name = registered
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            self.uploaded_files.push(SessionFile {
                name,
                path: registered.display().to_string(),
                hash,
            });
        }
    }

    /// A new edit with this installation's export destination.
    #[must_use]
    pub fn create_default_config(&self, default_export_dir: &str) -> WorkspaceConfig {
        default_workspace_config(default_export_dir)
    }

    /// The edit of `file_hash`, from memory then the database then the default.
    pub fn settings_for_file(
        &mut self,
        file_hash: &str,
        default_export_dir: &str,
    ) -> WorkspaceConfig {
        if let Some(settings) = self.file_settings.get(file_hash) {
            return settings.clone();
        }
        let settings = self
            .repository
            .load_file_settings(file_hash)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.create_default_config(default_export_dir));
        self.file_settings
            .insert(file_hash.to_string(), settings.clone());
        settings
    }

    /// The selected frame's edit, or `None` when the session holds no file.
    pub fn active_settings(&mut self, default_export_dir: &str) -> Option<WorkspaceConfig> {
        let hash = self
            .uploaded_files
            .get(self.selected_file_idx)?
            .hash
            .clone();
        Some(self.settings_for_file(&hash, default_export_dir))
    }

    /// Replace the selected frame's edit, in memory and optionally in the database.
    pub fn update_active_settings(&mut self, settings: WorkspaceConfig, persist: bool) {
        let Some(file) = self.uploaded_files.get(self.selected_file_idx) else {
            return;
        };
        let hash = file.hash.clone();
        let path = file.path.clone();
        self.file_settings.insert(hash.clone(), settings.clone());
        if persist {
            let _ = self.repository.save_file_settings(&hash, &settings, &path);
        }
    }

    /// Select one row and return its file.
    pub fn select(&mut self, index: usize) -> Option<&SessionFile> {
        if index >= self.uploaded_files.len() {
            return None;
        }
        self.selected_file_idx = index;
        self.uploaded_files.get(index)
    }

    /// The selected file, or `None` when the session is empty.
    #[must_use]
    pub fn current_file(&self) -> Option<&SessionFile> {
        self.uploaded_files.get(self.selected_file_idx)
    }

    /// Remove every file and its managed cache copy.
    pub fn clear_all_files(&mut self) {
        for file in &self.uploaded_files {
            let _ = self.asset_store.remove(Path::new(&file.path));
        }
        self.uploaded_files.clear();
        self.file_settings.clear();
        self.selected_file_idx = 0;
        self.watched_folders.clear();
    }

    fn register_upload(&self, name: &str, bytes: &[u8]) -> Option<(String, String)> {
        let directory = self.asset_store.session_dir(&self.session_id).ok()?;
        let filename = format!("{}_{}", uuid::Uuid::new_v4(), name);
        let path = directory.join(filename);
        std::fs::write(&path, bytes).ok()?;
        let (registered, hash) = self.asset_store.register_path(&path).ok()??;
        Some((registered.display().to_string(), hash))
    }
}

fn path_is_under(path: &Path, root: &Path) -> bool {
    let absolute =
        |value: &Path| std::fs::canonicalize(value).unwrap_or_else(|_| value.to_path_buf());
    absolute(path).starts_with(absolute(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(dir: &Path) -> WorkspaceSession {
        let repo = StorageRepository::new(
            dir.join("edits.db").display().to_string(),
            dir.join("settings.db").display().to_string(),
        );
        repo.initialize().unwrap();
        let store = LocalAssetStore::new(dir.join("cache"), dir.join("icc"), 256);
        store.initialize().unwrap();
        WorkspaceSession::new("session", repo, store)
    }

    #[test]
    fn a_local_path_is_registered_and_its_edit_persists() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.tif");
        std::fs::write(&photo, b"frame").unwrap();
        let mut session = session(dir.path());
        session.add_local_assets(std::slice::from_ref(&photo));
        assert_eq!(session.uploaded_files.len(), 1);
        let mut config = session.active_settings("/tmp/export").unwrap();
        config.exposure.grade = 90.0;
        session.update_active_settings(config.clone(), true);
        let hash = session.uploaded_files[0].hash.clone();
        assert_eq!(
            session.repository.load_file_settings(&hash).unwrap(),
            Some(config)
        );
    }

    #[test]
    fn an_empty_session_has_no_active_settings() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = session(dir.path());
        assert!(session.active_settings("/tmp/export").is_none());
        assert!(session.current_file().is_none());
        assert!(session.select(3).is_none());
    }

    #[test]
    fn a_upload_is_copied_into_the_session_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = session(dir.path());
        let names = BTreeSet::from(["upload.tif".to_string()]);
        session.sync_files(&names, &[("upload.tif".to_string(), b"pixels".to_vec())]);
        assert_eq!(session.uploaded_files.len(), 1);
        assert!(session.uploaded_files[0].path.contains("session"));
    }
}
