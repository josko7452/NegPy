//! The frame edit the desktop shell reads and saves.
//!
//! The backend owns the config value and the store. This type keeps the frame identity
//! beside the config and saves only after a control reported a change.

use std::path::{Path, PathBuf};

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::domain::types::AppConfig;
use negpy_backend::infrastructure::storage::local_asset_store::file_hash;
use negpy_backend::infrastructure::storage::repository::StorageRepository;

/// The edit of one source frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameEdit {
    pub config: WorkspaceConfig,
    source: Option<EditSource>,
    dirty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EditSource {
    profile: PathBuf,
    path: PathBuf,
    hash: String,
}

impl FrameEdit {
    /// Read the saved edit for `path`, or the defaults when the profile has none.
    #[must_use]
    pub fn load(profile: Option<&Path>, path: &Path) -> Self {
        let Some(profile) = profile else {
            return Self::default();
        };
        let app = AppConfig::for_user_dir(profile);
        let Ok(hash) = file_hash(path) else {
            return Self::default();
        };
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        let config = repo
            .load_file_settings(&hash)
            .ok()
            .flatten()
            .unwrap_or_default();
        Self {
            config,
            source: Some(EditSource {
                profile: profile.to_path_buf(),
                path: path.to_path_buf(),
                hash,
            }),
            dirty: false,
        }
    }

    /// Mark the config as changed since the last save.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// The same config with no save target, for a capture.
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.source = None;
        self.dirty = false;
        self
    }

    /// Whether a control changed the config since the last save.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The stored history edit at `index`, when this frame has a store.
    #[must_use]
    pub fn history_step(&self, index: i64) -> Option<WorkspaceConfig> {
        let source = self.source.as_ref()?;
        let app = AppConfig::for_user_dir(&source.profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.history_step(&source.hash, index).ok().flatten()
    }

    /// The open frame's profile and source path.
    #[must_use]
    pub fn source(&self) -> Option<(&Path, &Path)> {
        self.source
            .as_ref()
            .map(|source| (source.profile.as_path(), source.path.as_path()))
    }

    /// Save this config as a named work print.
    pub fn save_work_print(&self, name: &str) -> Result<(), String> {
        let Some(source) = &self.source else {
            return Err("Open a profile first.".to_string());
        };
        let app = AppConfig::for_user_dir(&source.profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().map_err(|error| error.to_string())?;
        repo.save_work_print(&source.hash, name, &self.config)
            .map_err(|error| error.to_string())
    }

    /// The stored edit of a named work print.
    #[must_use]
    pub fn work_print(&self, name: &str) -> Option<WorkspaceConfig> {
        let source = self.source.as_ref()?;
        let app = AppConfig::for_user_dir(&source.profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.work_print(&source.hash, name).ok().flatten()
    }

    /// Restore the named work print, or the stored step at `index`.
    pub fn restore(&mut self, stored: WorkspaceConfig) {
        self.config = stored;
        self.dirty = true;
    }

    /// Save a dirty edit. Returns whether a row was written.
    pub fn save(&mut self) -> Result<bool, String> {
        if !self.dirty {
            return Ok(false);
        }
        self.dirty = false;
        let Some(source) = &self.source else {
            return Ok(false);
        };
        let app = AppConfig::for_user_dir(&source.profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().map_err(|error| error.to_string())?;
        repo.save_file_settings(
            &source.hash,
            &self.config,
            &source.path.display().to_string(),
        )
        .map_err(|error| error.to_string())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dirty_edit_round_trips_through_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let mut edit = FrameEdit::load(Some(dir.path()), &photo);
        edit.config.exposure.density = 1.25;
        edit.mark_dirty();
        assert!(edit.save().unwrap());
        assert!(!edit.is_dirty());
        let reloaded = FrameEdit::load(Some(dir.path()), &photo);
        assert_eq!(reloaded.config.exposure.density, 1.25);
    }

    #[test]
    fn a_clean_edit_writes_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let mut edit = FrameEdit::load(Some(dir.path()), &photo);
        assert!(!edit.save().unwrap());
    }

    #[test]
    fn a_profile_free_edit_keeps_the_defaults() {
        let edit = FrameEdit::load(None, Path::new("/missing/frame.orf"));
        assert_eq!(edit.config, WorkspaceConfig::default());
        assert!(!edit.is_dirty());
    }

    #[test]
    fn a_work_print_restores_the_stored_edit() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let mut edit = FrameEdit::load(Some(dir.path()), &photo);
        edit.config.exposure.density = 1.4;
        edit.mark_dirty();
        edit.save().unwrap();
        edit.save_work_print("Print").unwrap();
        edit.restore(WorkspaceConfig::default());
        let stored = edit.work_print("Print").unwrap();
        edit.restore(stored);
        assert_eq!(edit.config.exposure.density, 1.4);
    }

    #[test]
    fn a_history_step_restores_the_stored_edit() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let mut edit = FrameEdit::load(Some(dir.path()), &photo);
        let mut stored = edit.config.clone();
        stored.exposure.density = 0.8;
        let app = AppConfig::for_user_dir(dir.path());
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().unwrap();
        let hash = file_hash(&photo).unwrap();
        repo.save_history_step(&hash, 0, &stored).unwrap();
        let restored = edit.history_step(0).unwrap();
        edit.restore(restored);
        assert_eq!(edit.config.exposure.density, 0.8);
    }
}
