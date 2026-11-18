//! The Frame tab's Geometry page: the crop and the frame's own straightening.

use eframe::egui::{self, RichText, Ui};

use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::tab_header::TabSummary;

/// The crop options in the Crop card.
#[derive(Clone, Debug, PartialEq)]
pub struct CropState {
    pub draw: bool,
    pub auto_crop: bool,
    pub ratio: StepCombo,
    pub guide: StepCombo,
    pub mode: StepCombo,
    pub offset: CompactSlider,
    pub rebate_trim: CompactSlider,
    pub square_first: bool,
    pub settings_open: bool,
}

impl Default for CropState {
    fn default() -> Self {
        Self {
            draw: false,
            auto_crop: false,
            ratio: StepCombo::new(vec![
                Choice::new("Original"),
                Choice::new("1:1"),
                Choice::new("4:5"),
                Choice::new("2:3"),
            ]),
            guide: StepCombo::new(vec![
                Choice::new("Thirds"),
                Choice::new("Golden Ratio"),
                Choice::new("Diagonals"),
            ]),
            mode: StepCombo::new(vec![Choice::new("Image only"), Choice::new("Film edge")]),
            offset: CompactSlider::new(
                SliderSpec::new(-5.0, 100.0, 1.0, 0)
                    .with_default(0.0)
                    .with_unit(" px"),
            ),
            rebate_trim: CompactSlider::new(
                SliderSpec::new(0.0, 150.0, 5.0, 0)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            square_first: false,
            settings_open: true,
        }
    }
}

/// The Straighten card's state.
#[derive(Clone, Debug, PartialEq)]
pub struct StraightenState {
    pub level: bool,
    pub draw_lines: bool,
    pub fine_rotation: CompactSlider,
    pub tilt: CompactSlider,
    pub swing: CompactSlider,
    pub crop_empty: bool,
}

impl Default for StraightenState {
    fn default() -> Self {
        Self {
            level: false,
            draw_lines: false,
            fine_rotation: CompactSlider::new(
                SliderSpec::new(-5.0, 5.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("°"),
            ),
            tilt: CompactSlider::new(
                SliderSpec::new(-15.0, 15.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            swing: CompactSlider::new(
                SliderSpec::new(-15.0, 15.0, 0.1, 2)
                    .with_default(0.0)
                    .with_unit("%"),
            ),
            crop_empty: true,
        }
    }
}

/// The Geometry page's state.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryState {
    pub crop: CropState,
    pub straighten: StraightenState,
}

impl Default for GeometryState {
    fn default() -> Self {
        Self {
            crop: CropState::default(),
            straighten: StraightenState::default(),
        }
    }
}

impl GeometryState {
    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = TabSummary::new(2).show(ui) != Default::default();
        ui.add_space(f32::from(Theme::default().space_sm));
        changed |= self.show_crop(ui);
        ui.add_space(f32::from(Theme::default().space_sm));
        changed |= self.show_straighten(ui);
        changed
    }

    fn show_crop(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Crop").with_guide();
        section.modified = usize::from(self.crop.auto_crop || self.crop.draw);
        let mut changed = false;
        section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                ui.horizontal(|ui| {
                    changed |= labeled_toggle(ui, "Draw", &mut self.crop.draw).clicked();
                    changed |= labeled_toggle(ui, "Auto Crop", &mut self.crop.auto_crop).clicked();
                });
                ui.add_space(f32::from(theme.space_sm));
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Ratio")
                            .color(theme.text_secondary)
                            .size(f32::from(theme.font_size_base)),
                    );
                    changed |= self.crop.ratio.show(ui);
                });
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Guide")
                            .color(theme.text_secondary)
                            .size(f32::from(theme.font_size_base)),
                    );
                    changed |= self.crop.guide.show(ui);
                });
                self.crop.settings_open = true;
                let _ = section_subheader(ui, "AUTO CROP SETTINGS");
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Mode")
                            .color(theme.text_secondary)
                            .size(f32::from(theme.font_size_base)),
                    );
                    changed |= self.crop.mode.show(ui);
                });
                let (_, offset) = self.crop.offset.show(ui, "Crop Offset");
                changed |= offset;
                let (_, rebate) = self.crop.rebate_trim.show(ui, "Rebate Trim");
                changed |= rebate;
                ui.horizontal(|ui| {
                    changed |=
                        labeled_toggle(ui, "Square First", &mut self.crop.square_first).clicked();
                    let _ = labeled_action(ui, "Crop All Frames", ActionRank::Normal);
                });
            });
        });
        changed
    }

    fn show_straighten(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Straighten").with_guide();
        section.modified = usize::from(
            !self.straighten.fine_rotation.is_neutral()
                || !self.straighten.tilt.is_neutral()
                || !self.straighten.swing.is_neutral(),
        );
        let mut changed = false;
        section.show(ui, |ui| {
            let _ = card(ui, |ui| {
                let _ = section_subheader(ui, "ROTATE AND FLIP");
                ui.horizontal(|ui| {
                    changed |= labeled_toggle(ui, "Level", &mut self.straighten.level).clicked();
                    let _ = labeled_action(ui, "Square to Film", ActionRank::Normal);
                    changed |=
                        labeled_toggle(ui, "Draw Lines", &mut self.straighten.draw_lines).clicked();
                });
                let (_, fine) = self.straighten.fine_rotation.show(ui, "Fine Rotation");
                changed |= fine;
                let (_, tilt) = self.straighten.tilt.show(ui, "Tilt");
                changed |= tilt;
                let (_, swing) = self.straighten.swing.show(ui, "Swing");
                changed |= swing;
                ui.add_space(f32::from(theme.space_sm));
                changed |=
                    labeled_toggle(ui, "Crop Empty Corners", &mut self.straighten.crop_empty)
                        .clicked();
            });
        });
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_geometry_page_names_its_two_cards() {
        let state = GeometryState::default();
        assert_eq!(state.crop.ratio.current_label(), "Original");
        assert_eq!(state.crop.guide.current_label(), "Thirds");
        assert_eq!(state.crop.mode.current_label(), "Image only");
        assert!(state.straighten.crop_empty);
    }

    #[test]
    fn the_geometry_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = GeometryState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
