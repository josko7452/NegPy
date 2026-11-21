//! The Roll Analysis card's roll and baseline labels.

use std::path::Path;

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::infrastructure::storage::local_asset_store::file_hash;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::rolls;

/// The active roll and the baseline its frames read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RollAnalysisState {
    pub roll_name: Option<String>,
    /// The saved roll the baseline goes onto.
    pub roll_id: Option<String>,
    pub baseline: String,
    pub stale: bool,
    pub open: bool,
    pub use_frame_requested: bool,
    pub reanalyze_requested: bool,
    /// The Detect button, which groups the strip under another light.
    pub detect_requested: bool,
    /// The groups Detect found, as strip indices.
    pub found: Vec<Vec<usize>>,
    /// The found groups the user accepted.
    pub accept_requested: bool,
    /// The found groups the user discarded.
    pub discard_requested: bool,
}

impl RollAnalysisState {
    /// Read the saved process of `path` and the roll record it names.
    #[must_use]
    pub fn load(profile: Option<&Path>, path: &Path, config: Option<&WorkspaceConfig>) -> Self {
        let config = config.cloned().unwrap_or_default();
        let Some(profile) = profile else {
            return Self::from_config(&config);
        };
        let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        let Ok(hash) = file_hash(path) else {
            return Self::from_config(&config);
        };
        let live = repo
            .load_file_settings(&hash)
            .ok()
            .flatten()
            .unwrap_or(config);
        Self::from_repo(&repo, &live)
    }

    /// Read the labels from a repository and the frame's process.
    #[must_use]
    pub fn from_repo(repo: &StorageRepository, config: &WorkspaceConfig) -> Self {
        let process = &config.process;
        let source_id = process.baseline_source.strip_prefix("roll:");
        let roll_id = source_id.map(str::to_string).or_else(|| {
            let wanted = process.roll_name.as_deref()?;
            rolls::saved_rolls(repo)
                .into_iter()
                .find(|(_, entry)| entry.get("name").and_then(|name| name.as_str()) == Some(wanted))
                .map(|(id, _)| id)
        });
        let roll_name = process.roll_name.clone().or_else(|| {
            source_id
                .and_then(|roll_id| rolls::roll_for_id(repo, roll_id))
                .and_then(|entry| {
                    entry
                        .get("name")
                        .and_then(|name| name.as_str())
                        .map(str::to_string)
                })
        });
        let mut state = Self {
            roll_name,
            roll_id,
            baseline: rolls::baseline_label(repo, process),
            stale: rolls::frame_baseline_stale(repo, process),
            open: true,
            use_frame_requested: false,
            reanalyze_requested: false,
            detect_requested: false,
            found: Vec::new(),
            accept_requested: false,
            discard_requested: false,
        };
        if state.roll_name.is_none() {
            state.roll_name = rolls::saved_rolls(repo)
                .into_iter()
                .next()
                .and_then(|(_, entry)| {
                    entry
                        .get("name")
                        .and_then(|name| name.as_str())
                        .map(str::to_string)
                });
        }
        state
    }

    /// A state with no repository, for a frame with no saved edit.
    #[must_use]
    pub fn from_config(config: &WorkspaceConfig) -> Self {
        let process = &config.process;
        Self {
            roll_name: process.roll_name.clone(),
            roll_id: process
                .baseline_source
                .strip_prefix("roll:")
                .map(str::to_string),
            baseline: rolls::baseline_label_without_repo(process),
            stale: false,
            open: true,
            use_frame_requested: false,
            reanalyze_requested: false,
            detect_requested: false,
            found: Vec::new(),
            accept_requested: false,
            discard_requested: false,
        }
    }

    /// The roll line the card prints.
    #[must_use]
    pub fn roll_line(&self) -> String {
        self.roll_name
            .clone()
            .map_or_else(|| "No roll".to_string(), |name| name)
    }

    /// The baseline warning, or `None` when the baseline is current.
    #[must_use]
    pub fn baseline_warning(&self) -> Option<String> {
        if !self.stale {
            return None;
        }
        let name = self.roll_name.as_deref().unwrap_or("this roll");
        Some(format!("⚠ Baseline: Roll “{name}”, from an older analysis"))
    }

    /// Take the one Use Frame request.
    pub fn take_use_frame(&mut self) -> bool {
        std::mem::take(&mut self.use_frame_requested)
    }

    /// Take the one Reanalyze request.
    pub fn take_reanalyze(&mut self) -> bool {
        std::mem::take(&mut self.reanalyze_requested)
    }

    /// Take the one Detect request.
    pub fn take_detect(&mut self) -> bool {
        std::mem::take(&mut self.detect_requested)
    }

    /// Take the one Accept request.
    pub fn take_accept(&mut self) -> bool {
        std::mem::take(&mut self.accept_requested)
    }

    /// Take the one Discard request.
    pub fn take_discard(&mut self) -> bool {
        std::mem::take(&mut self.discard_requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_roll_name_becomes_the_roll_line() {
        let config = WorkspaceConfig {
            process: negpy_backend::domain::models::process::ProcessConfig {
                roll_name: Some("w9".to_string()),
                ..Default::default()
            },
            ..WorkspaceConfig::default()
        };
        let state = RollAnalysisState::from_config(&config);
        assert_eq!(state.roll_line(), "w9");
        assert!(state.baseline_warning().is_none());
    }

    #[test]
    fn an_empty_process_has_no_roll_line() {
        let state = RollAnalysisState::from_config(&WorkspaceConfig::default());
        assert_eq!(state.roll_line(), "No roll");
    }

    #[test]
    fn the_average_toggles_live_in_the_frame_config() {
        let config = WorkspaceConfig {
            process: negpy_backend::domain::models::process::ProcessConfig {
                use_luma_average: true,
                use_color_average: true,
                use_cast_average: false,
                ..Default::default()
            },
            ..WorkspaceConfig::default()
        };
        let state = RollAnalysisState::from_config(&config);
        assert!(state.roll_line() == "No roll");
    }

    #[test]
    fn the_card_actions_leave_one_request() {
        let mut state = RollAnalysisState::default();
        state.use_frame_requested = true;
        state.reanalyze_requested = true;
        assert!(state.take_use_frame());
        assert!(state.take_reanalyze());
        assert!(!state.take_use_frame());
        assert!(!state.take_reanalyze());
    }

    #[test]
    fn a_named_roll_resolves_the_saved_id_for_the_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let mut repo = StorageRepository::new(
            dir.path().join("edits.db").display().to_string(),
            dir.path().join("settings.db").display().to_string(),
        );
        repo.initialize().unwrap();
        let roll_id =
            rolls::create_virtual_roll(&mut repo, "Portra 400", &["/roll/a.orf".to_string()]);
        let config = WorkspaceConfig {
            process: negpy_backend::domain::models::process::ProcessConfig {
                roll_name: Some("Portra 400".to_string()),
                ..Default::default()
            },
            ..WorkspaceConfig::default()
        };
        let state = RollAnalysisState::from_repo(&repo, &config);
        assert_eq!(state.roll_id.as_deref(), Some(roll_id.as_str()));
    }
}
