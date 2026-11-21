//! The Roll tab's Calibration card: the spectral-crosstalk profile.

use std::path::Path;

use eframe::egui::Ui;
use negpy_backend::domain::models::{ProcessMode, WorkspaceConfig};
use negpy_backend::domain::types::AppConfig;
use negpy_backend::services::assets::crosstalk::{self, CrosstalkDirs};

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::templates::section_subheader;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::step_combo::{Choice, StepCombo};

/// The Calibration card's controls.
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationState {
    pub open: bool,
    pub profiles: Vec<String>,
    pub matrix: StepCombo,
    pub strength: CompactSlider,
    dirs: Option<std::path::PathBuf>,
}

impl Default for CalibrationState {
    fn default() -> Self {
        let profiles = vec![crosstalk::DEFAULT_NAME.to_string()];
        Self {
            open: true,
            matrix: profile_combo(&profiles, crosstalk::DEFAULT_NAME),
            profiles,
            strength: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2)
                    .with_default(0.0)
                    .with_unit(""),
            ),
            dirs: None,
        }
    }
}

impl CalibrationState {
    /// Read the profile gallery the app ships and the user's matrix files.
    pub fn set_profile(&mut self, profile: Option<&Path>, config: &WorkspaceConfig) {
        let Some(profile) = profile else {
            self.dirs = None;
            self.profiles = vec![crosstalk::DEFAULT_NAME.to_string()];
            self.load(config);
            return;
        };
        let app = AppConfig::for_user_dir(profile);
        let dirs = CrosstalkDirs::new(app.crosstalk_dir);
        self.profiles = crosstalk::list_profiles(&dirs);
        if !self
            .profiles
            .iter()
            .any(|name| name == &config.process.crosstalk_profile)
        {
            self.profiles.push(config.process.crosstalk_profile.clone());
            self.profiles.sort();
        }
        self.matrix = profile_combo(&self.profiles, &config.process.crosstalk_profile);
        self.dirs = Some(dirs.user.clone());
        self.load(config);
    }

    fn load(&mut self, config: &WorkspaceConfig) {
        self.strength.set_value(config.process.crosstalk_strength);
        self.matrix.pick(&config.process.crosstalk_profile);
    }

    fn selected(&self) -> &str {
        self.matrix.current_value()
    }

    /// The matrix and film process the selected profile carries.
    fn selected_definition(&self) -> (Option<Vec<f64>>, ProcessMode) {
        let Some(dirs) = &self.dirs else {
            return (None, ProcessMode::C41);
        };
        let dirs = CrosstalkDirs::new(dirs.clone());
        (
            crosstalk::get_matrix(&dirs, self.selected()).map(|matrix| matrix.to_vec()),
            crosstalk::get_process(&dirs, self.selected()),
        )
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&self, edit: &mut FrameEdit) -> bool {
        let (matrix, process) = self.selected_definition();
        let strength = self.strength.value;
        let next = (
            self.selected().to_string(),
            matrix,
            process.as_str().to_string(),
            strength,
        );
        let process_config = &edit.config.process;
        let changed = process_config.crosstalk_profile != next.0
            || process_config.crosstalk_matrix != next.1
            || process_config.crosstalk_process != next.2
            || (process_config.crosstalk_strength - next.3).abs() > 1e-9;
        if changed {
            let process_config = &mut edit.config.process;
            process_config.crosstalk_profile = next.0;
            process_config.crosstalk_matrix = next.1;
            process_config.crosstalk_process = next.2;
            process_config.crosstalk_strength = next.3;
            let (invalidate, floors, ceils) =
                negpy_backend::domain::models::process::invalidate_local_bounds(process_config);
            if invalidate {
                process_config.local_floors = floors;
                process_config.local_ceils = ceils;
            }
        }
        changed
    }

    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(&edit.config);
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Calibration")
            .with_guide()
            .with_icon(Icon::Grid);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| {
            card(ui, |ui| {
                let mut changed = false;
                let _ = section_subheader(ui, "CROSSTALK");
                ui.add_space(f32::from(theme.space_sm));
                ui.horizontal(|ui| {
                    ui.label("Matrix");
                    changed |= self.matrix.show(ui);
                });
                ui.add_space(f32::from(theme.space_sm));
                changed |= self.strength.show(ui, "Strength").1;
                let _ = hint(
                    ui,
                    "Separates colors that the film dyes, the light and the sensor mix together.",
                );
                changed
            })
        });
        self.open = section.expanded;
        let changed = result.unwrap_or(false);
        if changed {
            self.store(edit);
        }
        changed
    }
}

fn profile_combo(profiles: &[String], selected: &str) -> StepCombo {
    let mut combo = StepCombo::new(
        profiles
            .iter()
            .map(|profile| Choice::with_id(profile.clone(), profile.clone()))
            .collect(),
    );
    combo.pick(selected);
    combo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_card_names_the_builtin_profile() {
        let state = CalibrationState::default();
        assert_eq!(state.matrix.current_value(), crosstalk::DEFAULT_NAME);
        assert_eq!(state.strength.value, 0.0);
    }

    #[test]
    fn a_saved_profile_writes_its_matrix_and_process() {
        let mut edit = FrameEdit::default();
        edit.config.process.crosstalk_profile = crosstalk::DEFAULT_NAME.to_string();
        edit.config.process.crosstalk_strength = 0.0;
        let mut state = CalibrationState::default();
        assert!(!state.store(&mut edit));
        state.strength.set_value(0.5);
        assert!(state.store(&mut edit));
        assert_eq!(edit.config.process.crosstalk_strength, 0.5);
        assert_eq!(
            edit.config.process.crosstalk_process,
            ProcessMode::C41.as_str()
        );
    }

    #[test]
    fn a_profile_change_clears_the_stored_bounds() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = CrosstalkDirs::new(temp.path());
        crosstalk::save(
            &dirs,
            "Portra",
            [1.0, -0.1, 0.0, 0.0, 1.0, -0.1, 0.0, 0.0, 1.0],
            crosstalk::CrosstalkType::Measured,
            Some(ProcessMode::C41),
        )
        .unwrap();
        let mut state = CalibrationState::default();
        state.dirs = Some(dirs.user.clone());
        state.profiles = vec![crosstalk::DEFAULT_NAME.to_string(), "Portra".to_string()];
        state.matrix = profile_combo(&state.profiles, "Portra");
        let mut edit = FrameEdit::default();
        edit.config.process.local_floors = [0.1, 0.2, 0.3];
        assert!(state.store(&mut edit));
        assert_eq!(
            edit.config.process.crosstalk_matrix.as_deref(),
            Some([1.0, -0.1, 0.0, 0.0, 1.0, -0.1, 0.0, 0.0, 1.0].as_slice())
        );
        assert_eq!(edit.config.process.local_floors, [0.0; 3]);
    }

    #[test]
    fn the_card_paints_on_a_context() {
        let ctx = eframe::egui::Context::default();
        let mut state = CalibrationState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }
}
