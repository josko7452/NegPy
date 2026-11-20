//! The Roll tab's Optics card: lens correction and flat-field correction.

use std::path::{Path, PathBuf};

use eframe::egui::Ui;
use negpy_backend::domain::types::AppConfig;
use negpy_backend::services::assets::flatfield::FlatFieldProfiles;

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::templates::{ActionRank, action_with_icon, labeled_toggle, section_subheader};
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::field_row::{RowColumns, field_row};
use crate::view::widgets::step_combo::{Choice, StepCombo};

/// The flat-field set the frame selects from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatFieldChoice {
    pub label: String,
    pub id: String,
}

/// The Optics card's controls.
#[derive(Clone, Debug, PartialEq)]
pub struct OpticsState {
    pub distortion: CompactSlider,
    pub embedded_distortion: bool,
    pub embedded_ca: bool,
    pub profiles: Vec<FlatFieldChoice>,
    pub profile: StepCombo,
    pub apply: bool,
    pub open: bool,
    /// The profile directory a flat-field action reads and writes, when a profile is open.
    pub directory: Option<PathBuf>,
    pub action: Option<FlatFieldAction>,
}

/// A flat-field action the card asked the app to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatFieldAction {
    Add,
    Remove,
}

impl Default for OpticsState {
    fn default() -> Self {
        let profiles = vec![FlatFieldChoice {
            label: "None".to_string(),
            id: String::new(),
        }];
        Self {
            distortion: CompactSlider::new(
                SliderSpec::new(-0.10, 0.10, 0.001, 3).with_default(0.0),
            ),
            embedded_distortion: false,
            embedded_ca: false,
            profile: profile_combo(&profiles, ""),
            profiles,
            apply: false,
            open: false,
            directory: None,
            action: None,
        }
    }
}

fn profile_combo(profiles: &[FlatFieldChoice], selected: &str) -> StepCombo {
    let mut combo = StepCombo::new(
        profiles
            .iter()
            .map(|profile| Choice::new(profile.label.clone()))
            .collect(),
    );
    if let Some(index) = profiles.iter().position(|profile| profile.id == selected) {
        combo.current = index;
    }
    combo
}

impl OpticsState {
    /// Read the saved flat-field profiles for `profile`, keeping the frame's selection.
    pub fn set_profile(&mut self, profile: Option<&Path>, selected: &str) {
        let Some(profile) = profile else {
            self.directory = None;
            self.set_profiles(
                vec![FlatFieldChoice {
                    label: "None".to_string(),
                    id: String::new(),
                }],
                selected,
            );
            return;
        };
        let app = AppConfig::for_user_dir(profile);
        let store = FlatFieldProfiles::new(&app.flatfield_dir);
        let mut profiles = vec![FlatFieldChoice {
            label: "None".to_string(),
            id: String::new(),
        }];
        profiles.extend(
            store
                .list_profiles()
                .into_iter()
                .map(|(id, label)| FlatFieldChoice { label, id }),
        );
        self.directory = Some(PathBuf::from(app.flatfield_dir));
        self.set_profiles(profiles, selected);
    }

    /// Add `reference` as a named profile and select it.
    pub fn add_profile(&mut self, reference: &Path, k1: f64) -> Option<String> {
        let directory = self.directory.clone()?;
        let name = reference
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Flat field")
            .to_string();
        let store = FlatFieldProfiles::new(directory);
        let id = store.create(&name, &reference.display().to_string(), k1)?;
        let selected = self
            .profiles
            .get(self.profile.current)
            .map_or_else(String::new, |profile| profile.id.clone());
        let mut profiles = vec![FlatFieldChoice {
            label: "None".to_string(),
            id: String::new(),
        }];
        profiles.extend(
            store
                .list_profiles()
                .into_iter()
                .map(|(profile_id, label)| FlatFieldChoice {
                    label,
                    id: profile_id,
                }),
        );
        self.set_profiles(profiles, if selected.is_empty() { &id } else { &selected });
        Some(id)
    }

    /// Remove the selected profile and select None.
    pub fn remove_profile(&mut self) -> bool {
        let Some(directory) = self.directory.clone() else {
            return false;
        };
        let Some(profile) = self.profiles.get(self.profile.current) else {
            return false;
        };
        if profile.id.is_empty() {
            return false;
        }
        let store = FlatFieldProfiles::new(directory);
        store.delete(&profile.id);
        let mut profiles = vec![FlatFieldChoice {
            label: "None".to_string(),
            id: String::new(),
        }];
        profiles.extend(
            store
                .list_profiles()
                .into_iter()
                .map(|(profile_id, label)| FlatFieldChoice {
                    label,
                    id: profile_id,
                }),
        );
        self.set_profiles(profiles, "");
        true
    }

    /// Replace the profile list and keep the selected id when it still exists.
    pub fn set_profiles(&mut self, profiles: Vec<FlatFieldChoice>, selected: &str) {
        self.profile = profile_combo(&profiles, selected);
        self.profiles = profiles;
    }

    /// Replace the controls with the lens and flat-field values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let geometry = &edit.config.geometry;
        self.embedded_distortion = geometry.lens_distortion_from_metadata;
        self.embedded_ca = geometry.lens_ca_from_metadata;
        self.distortion.set_value(geometry.distortion_k1);
        self.apply = edit.config.flatfield.apply;
        self.profile = profile_combo(&self.profiles, &edit.config.flatfield.profile_id);
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&self, edit: &mut FrameEdit) -> bool {
        let geometry = &mut edit.config.geometry;
        let flatfield = &mut edit.config.flatfield;
        let profile_id = self
            .profiles
            .get(self.profile.current)
            .map_or_else(String::new, |profile| profile.id.clone());
        let changed = geometry.lens_distortion_from_metadata != self.embedded_distortion
            || geometry.lens_ca_from_metadata != self.embedded_ca
            || (geometry.distortion_k1 - self.distortion.value).abs()
                > self.distortion.spec.step * 0.5
            || flatfield.apply != self.apply
            || flatfield.profile_id != profile_id;
        geometry.lens_distortion_from_metadata = self.embedded_distortion;
        geometry.lens_ca_from_metadata = self.embedded_ca;
        geometry.distortion_k1 = self.distortion.value;
        flatfield.apply = self.apply;
        flatfield.profile_id = profile_id;
        changed
    }

    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut section = CollapsibleSection::new("Optics")
            .with_guide()
            .with_icon(Icon::Camera);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        let changed = result.unwrap_or(false);
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_body(&mut self, ui: &mut Ui) -> bool {
        let columns = RowColumns::new();
        let mut changed = false;
        let _ = section_subheader(ui, "LENS CORRECTION");
        field_row(ui, columns, "Embedded", |ui| {
            changed |= labeled_toggle(ui, "Distortion", &mut self.embedded_distortion).clicked();
            changed |= labeled_toggle(ui, "CA", &mut self.embedded_ca).clicked();
        });
        changed |= self.distortion.show(ui, "Distortion Correction").1;
        let _ = hint(ui, "Positive bends lines out; negative bends them in.");
        let _ = section_subheader(ui, "FLAT FIELD CORRECTION");
        ui.horizontal(|ui| {
            if action_with_icon(
                ui,
                Icon::FolderOpen,
                "Add",
                ActionRank::Normal,
                "Pick a reference image and save it as a named profile",
            )
            .clicked()
            {
                self.action = Some(FlatFieldAction::Add);
                changed = true;
            }
            if action_with_icon(
                ui,
                Icon::Close,
                "Remove",
                ActionRank::Normal,
                "Remove the selected profile",
            )
            .clicked()
            {
                self.action = Some(FlatFieldAction::Remove);
                changed = true;
            }
        });
        field_row(ui, columns, "Profile", |ui| {
            changed |= self.profile.show(ui);
        });
        ui.horizontal(|ui| {
            changed |= labeled_toggle(ui, "Apply Flat Field", &mut self.apply).clicked();
        });
        if self.profiles.len() <= 1 {
            let _ = hint(ui, "Add a scan of the bare light source to enable.");
        }
        changed
    }

    /// Take the one pending profile action.
    pub fn take_action(&mut self) -> Option<FlatFieldAction> {
        self.action.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_list_starts_with_none() {
        let state = OpticsState::default();
        assert_eq!(state.profile.current_label(), "None");
        assert!(state.profiles[0].id.is_empty());
    }

    #[test]
    fn a_saved_profile_selection_survives_a_refresh() {
        let mut state = OpticsState::default();
        state.set_profiles(
            vec![
                FlatFieldChoice {
                    label: "None".to_string(),
                    id: String::new(),
                },
                FlatFieldChoice {
                    label: "Bare light".to_string(),
                    id: "p1".to_string(),
                },
            ],
            "p1",
        );
        assert_eq!(state.profile.current_label(), "Bare light");
    }

    #[test]
    fn the_distortion_slider_keeps_a_thousandth_step() {
        let state = OpticsState::default();
        assert_eq!(state.distortion.spec.step, 0.001);
        assert_eq!(state.distortion.display(), "0.000");
    }

    #[test]
    fn the_card_paints_on_a_context() {
        let ctx = eframe::egui::Context::default();
        let mut state = OpticsState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_optics_card_and_stores_it() {
        let mut edit = FrameEdit::default();
        edit.config.geometry.lens_distortion_from_metadata = true;
        edit.config.geometry.lens_ca_from_metadata = true;
        edit.config.geometry.distortion_k1 = 0.012;
        edit.config.flatfield.apply = true;
        edit.config.flatfield.profile_id = "p1".to_string();
        let mut state = OpticsState::default();
        state.set_profiles(
            vec![
                FlatFieldChoice {
                    label: "None".to_string(),
                    id: String::new(),
                },
                FlatFieldChoice {
                    label: "Bare light".to_string(),
                    id: "p1".to_string(),
                },
            ],
            "p1",
        );
        state.load(&edit);
        assert!(state.embedded_distortion);
        assert!(state.embedded_ca);
        assert!((state.distortion.value - 0.012).abs() < 1e-9);
        assert!(state.apply);
        assert_eq!(state.profile.current_label(), "Bare light");
        assert!(!state.store(&mut edit));
    }

    #[test]
    fn the_card_lists_the_profiles_in_the_profile_store() {
        let dir = tempfile::tempdir().unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let store = FlatFieldProfiles::new(&app.flatfield_dir);
        let id = store.import_gain(
            &negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer::from_rgb(
                4,
                4,
                vec![1.0; 48],
            ),
            "Bare light",
            0.0,
            "",
        );
        let mut state = OpticsState::default();
        state.set_profile(Some(dir.path()), &id);
        assert_eq!(state.profiles.len(), 2);
        assert_eq!(state.profile.current_label(), "Bare light");
    }

    #[test]
    fn removing_the_selected_profile_leaves_none() {
        let dir = tempfile::tempdir().unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let store = FlatFieldProfiles::new(&app.flatfield_dir);
        let id = store.import_gain(
            &negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer::from_rgb(
                4,
                4,
                vec![1.0; 48],
            ),
            "Bare light",
            0.0,
            "",
        );
        let mut state = OpticsState::default();
        state.set_profile(Some(dir.path()), &id);
        assert!(state.remove_profile());
        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profile.current_label(), "None");
        assert!(store.load_gain(&id).is_none());
    }
}
