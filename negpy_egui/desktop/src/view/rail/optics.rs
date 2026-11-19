//! The Roll tab's Optics card: lens correction and flat-field correction.

use eframe::egui::Ui;

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
    /// Replace the profile list and keep the selected id when it still exists.
    pub fn set_profiles(&mut self, profiles: Vec<FlatFieldChoice>, selected: &str) {
        self.profile = profile_combo(&profiles, selected);
        self.profiles = profiles;
    }

    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut section = CollapsibleSection::new("Optics")
            .with_guide()
            .with_icon(Icon::Camera);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        result.unwrap_or(false)
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
            let _ = action_with_icon(
                ui,
                Icon::FolderOpen,
                "Add",
                ActionRank::Normal,
                "Pick a reference image and save it as a named profile",
            );
            let _ = action_with_icon(
                ui,
                Icon::Close,
                "Remove",
                ActionRank::Normal,
                "Remove the selected profile",
            );
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
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
