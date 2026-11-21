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
    /// The configs an Undo steps back through, newest last.
    undo_stack: Vec<WorkspaceConfig>,
    /// The configs an Undo steps forward through, most recently undone last.
    redo_stack: Vec<WorkspaceConfig>,
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
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Read the saved edit, then apply the roll defaults the frame has not locked.
    #[must_use]
    pub fn load_in_roll(profile: Option<&Path>, path: &Path, roll_id: Option<&str>) -> Self {
        let mut edit = Self::load(profile, path);
        let (Some(profile), Some(roll_id), Some(hash)) = (
            profile,
            roll_id,
            edit.source.as_ref().map(|source| source.hash.clone()),
        ) else {
            return edit;
        };
        let app = AppConfig::for_user_dir(profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        edit.config = negpy_backend::services::assets::rolls::resolve_roll_config(
            &repo,
            Some(roll_id),
            &hash,
            &edit.config,
        );
        edit
    }

    /// Mark the config as changed since the last save.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Remember the current config before a change, so Undo can return to it.
    pub fn checkpoint(&mut self) {
        self.undo_stack.push(self.config.clone());
        self.redo_stack.clear();
    }

    /// Remember the config that was current before a change.
    pub fn checkpoint_before(&mut self, previous: WorkspaceConfig) {
        if previous == self.config {
            return;
        }
        self.undo_stack.push(previous);
        self.redo_stack.clear();
    }

    /// Return to the config before the last change. True when one was undone.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack
            .push(std::mem::replace(&mut self.config, previous));
        self.dirty = true;
        true
    }

    /// Return to the config the last Undo left. True when one was redone.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack
            .push(std::mem::replace(&mut self.config, next));
        self.dirty = true;
        true
    }

    /// Whether an Undo has a step to return to.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether a Redo has a step to return to.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
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
    fn an_undo_returns_to_the_config_before_the_change() {
        let mut edit = FrameEdit::default();
        edit.config.exposure.density = 1.0;
        let before = edit.config.clone();
        edit.config.exposure.density = 1.4;
        edit.checkpoint_before(before);
        assert!(edit.can_undo());
        assert!(edit.undo());
        assert_eq!(edit.config.exposure.density, 1.0);
        assert!(edit.can_redo());
        assert!(edit.redo());
        assert_eq!(edit.config.exposure.density, 1.4);
        assert!(!edit.can_redo());
    }

    #[test]
    fn a_new_change_drops_the_redo_steps() {
        let mut edit = FrameEdit::default();
        let first = edit.config.clone();
        edit.config.exposure.density = 1.4;
        edit.checkpoint_before(first);
        assert!(edit.undo());
        assert!(edit.can_redo());
        let before = edit.config.clone();
        edit.config.exposure.density = 1.9;
        edit.checkpoint_before(before);
        assert!(!edit.can_redo());
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

    #[test]
    fn a_roll_default_reaches_the_frame_that_has_not_locked_it() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().unwrap();
        let roll_id = negpy_backend::services::assets::rolls::create_virtual_roll(
            &mut repo,
            "Roll",
            &[photo.display().to_string()],
        );
        negpy_backend::services::assets::rolls::set_roll_defaults(
            &mut repo,
            &roll_id,
            serde_json::json!({"luma_range_clip": 0.12, "white_point_offset": 0.03})
                .as_object()
                .cloned()
                .unwrap(),
        );
        let edit = FrameEdit::load_in_roll(Some(dir.path()), &photo, Some(&roll_id));
        assert!((edit.config.process.luma_range_clip - 0.12).abs() < 1e-9);
        assert!((edit.config.process.white_point_offset - 0.03).abs() < 1e-9);
    }
}
