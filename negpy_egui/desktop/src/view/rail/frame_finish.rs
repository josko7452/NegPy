//! The Frame tab's Finish page: retouch, then the post-crop finishing effects.

use eframe::egui::Ui;

use crate::edit::FrameEdit;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::tab_header::TabSummary;

/// The Retouch card.
#[derive(Clone, Debug, PartialEq)]
pub struct RetouchState {
    pub overlay: SegmentedChoice<'static>,
    pub optical: bool,
    pub spot_threshold: CompactSlider,
    pub hair_threshold: CompactSlider,
    pub dust_size: CompactSlider,
    pub ir_removal: bool,
    pub ir_threshold: CompactSlider,
    pub heal: bool,
    pub scratch: bool,
    pub line: bool,
    pub brush_size: CompactSlider,
    pub line_sensitivity: CompactSlider,
    pub clone: bool,
    pub clone_match: bool,
    pub clone_strength: CompactSlider,
    pub clone_feather: CompactSlider,
    pub open: bool,
}

impl Default for RetouchState {
    fn default() -> Self {
        Self {
            overlay: SegmentedChoice::new(&["Off", "Marked", "IR"]),
            optical: false,
            spot_threshold: CompactSlider::new(
                SliderSpec::new(0.01, 1.0, 0.01, 2).with_default(0.5),
            ),
            hair_threshold: CompactSlider::new(
                SliderSpec::new(0.01, 1.0, 0.01, 2).with_default(0.5),
            ),
            dust_size: CompactSlider::new(
                SliderSpec::new(2.0, 8.0, 1.0, 0)
                    .with_default(4.0)
                    .with_unit(" px"),
            ),
            ir_removal: false,
            ir_threshold: CompactSlider::new(
                SliderSpec::new(0.05, 0.95, 0.01, 2).with_default(0.5),
            ),
            heal: false,
            scratch: false,
            line: false,
            brush_size: CompactSlider::new(
                SliderSpec::new(2.0, 64.0, 1.0, 0)
                    .with_default(12.0)
                    .with_unit(" px"),
            ),
            line_sensitivity: CompactSlider::new(
                SliderSpec::new(0.05, 0.95, 0.01, 2).with_default(0.5),
            ),
            clone: false,
            clone_match: false,
            clone_strength: CompactSlider::new(
                SliderSpec::new(0.0, 100.0, 1.0, 0)
                    .with_default(100.0)
                    .with_unit("%"),
            ),
            clone_feather: CompactSlider::new(
                SliderSpec::new(0.0, 100.0, 1.0, 0)
                    .with_default(50.0)
                    .with_unit("%"),
            ),
            open: false,
        }
    }
}

/// The Finishing card.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishingState {
    pub vignette_burn: CompactSlider,
    pub vignette_size: CompactSlider,
    pub vignette_roundness: CompactSlider,
    pub carrier_width: CompactSlider,
    pub carrier_roughness: CompactSlider,
    pub carrier_flare: CompactSlider,
    pub carrier_corners: CompactSlider,
    pub border_width: CompactSlider,
    pub border_bottom: CompactSlider,
    pub border_color: StepCombo,
    pub open: bool,
}

impl Default for FinishingState {
    fn default() -> Self {
        Self {
            vignette_burn: CompactSlider::new(
                SliderSpec::new(-2.0, 2.0, 0.01, 2)
                    .with_default(0.0)
                    .with_unit(" st"),
            ),
            vignette_size: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.5)),
            vignette_roundness: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.5),
            ),
            carrier_width: CompactSlider::new(SliderSpec::new(0.0, 5.0, 0.01, 2).with_default(0.0)),
            carrier_roughness: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            carrier_flare: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            carrier_corners: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            border_width: CompactSlider::new(SliderSpec::new(0.0, 2.5, 0.01, 2).with_default(0.0)),
            border_bottom: CompactSlider::new(SliderSpec::new(1.0, 2.0, 0.01, 2).with_default(1.0)),
            border_color: StepCombo::new(vec![Choice::new("Paper White"), Choice::new("Custom")]),
            open: false,
        }
    }
}

/// The Finish page's state.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishState {
    pub retouch: RetouchState,
    pub finishing: FinishingState,
}

impl Default for FinishState {
    fn default() -> Self {
        Self {
            retouch: RetouchState::default(),
            finishing: FinishingState::default(),
        }
    }
}

impl FinishState {
    /// Replace the controls with the retouch and finishing values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let retouch = &edit.config.retouch;
        self.retouch.optical = retouch.dust_remove;
        self.retouch
            .spot_threshold
            .set_value(retouch.dust_threshold);
        self.retouch
            .hair_threshold
            .set_value(retouch.dust_hair_threshold);
        self.retouch.dust_size.set_value(retouch.dust_size as f64);
        self.retouch.ir_removal = retouch.ir_dust_remove;
        self.retouch.ir_threshold.set_value(retouch.ir_threshold);
        self.retouch
            .brush_size
            .set_value(retouch.manual_dust_size as f64);
        self.retouch
            .line_sensitivity
            .set_value(retouch.scratch_threshold);
        self.retouch.clone_match = retouch.clone_match_tone;
        self.retouch
            .clone_strength
            .set_value(retouch.clone_strength * 100.0);
        self.retouch
            .clone_feather
            .set_value(retouch.clone_feather * 100.0);
        let finish = &edit.config.finish;
        self.finishing
            .vignette_burn
            .set_value(finish.vignette_stops);
        self.finishing.vignette_size.set_value(finish.vignette_size);
        self.finishing
            .vignette_roundness
            .set_value(finish.vignette_roundness);
        self.finishing.carrier_width.set_value(finish.carrier_width);
        self.finishing
            .carrier_roughness
            .set_value(finish.carrier_rough);
        self.finishing.carrier_flare.set_value(finish.carrier_flare);
        self.finishing
            .carrier_corners
            .set_value(finish.carrier_corner);
        self.finishing.border_width.set_value(finish.border_size);
        self.finishing
            .border_bottom
            .set_value(finish.border_bottom_weight);
        self.finishing.border_color.current = usize::from(!finish.border_match_paper);
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        let retouch = &mut edit.config.retouch;
        let optical = self.retouch.optical;
        let spot = self.retouch.spot_threshold.value;
        let hair = self.retouch.hair_threshold.value;
        let dust = self.retouch.dust_size.value.round() as i64;
        let ir = self.retouch.ir_removal;
        let ir_threshold = self.retouch.ir_threshold.value;
        let brush = self.retouch.brush_size.value.round() as i64;
        let sensitivity = self.retouch.line_sensitivity.value;
        let clone_match = self.retouch.clone_match;
        let clone_strength = self.retouch.clone_strength.value / 100.0;
        let clone_feather = self.retouch.clone_feather.value / 100.0;
        let changed = retouch.dust_remove != optical
            || !near(retouch.dust_threshold, spot)
            || !near(retouch.dust_hair_threshold, hair)
            || retouch.dust_size != dust
            || retouch.ir_dust_remove != ir
            || !near(retouch.ir_threshold, ir_threshold)
            || retouch.manual_dust_size != brush
            || !near(retouch.scratch_threshold, sensitivity)
            || retouch.clone_match_tone != clone_match
            || !near(retouch.clone_strength, clone_strength)
            || !near(retouch.clone_feather, clone_feather);
        retouch.dust_remove = optical;
        retouch.dust_threshold = spot;
        retouch.dust_hair_threshold = hair;
        retouch.dust_size = dust;
        retouch.ir_dust_remove = ir;
        retouch.ir_threshold = ir_threshold;
        retouch.manual_dust_size = brush;
        retouch.scratch_threshold = sensitivity;
        retouch.clone_match_tone = clone_match;
        retouch.clone_strength = clone_strength;
        retouch.clone_feather = clone_feather;
        let finish = &mut edit.config.finish;
        let match_paper = self.finishing.border_color.current == 0;
        let finish_changed = !near(finish.vignette_stops, self.finishing.vignette_burn.value)
            || !near(finish.vignette_size, self.finishing.vignette_size.value)
            || !near(
                finish.vignette_roundness,
                self.finishing.vignette_roundness.value,
            )
            || !near(finish.carrier_width, self.finishing.carrier_width.value)
            || !near(finish.carrier_rough, self.finishing.carrier_roughness.value)
            || !near(finish.carrier_flare, self.finishing.carrier_flare.value)
            || !near(finish.carrier_corner, self.finishing.carrier_corners.value)
            || !near(finish.border_size, self.finishing.border_width.value)
            || !near(
                finish.border_bottom_weight,
                self.finishing.border_bottom.value,
            )
            || finish.border_match_paper != match_paper;
        finish.vignette_stops = self.finishing.vignette_burn.value;
        finish.vignette_size = self.finishing.vignette_size.value;
        finish.vignette_roundness = self.finishing.vignette_roundness.value;
        finish.carrier_width = self.finishing.carrier_width.value;
        finish.carrier_rough = self.finishing.carrier_roughness.value;
        finish.carrier_flare = self.finishing.carrier_flare.value;
        finish.carrier_corner = self.finishing.carrier_corners.value;
        finish.border_size = self.finishing.border_width.value;
        finish.border_bottom_weight = self.finishing.border_bottom.value;
        finish.border_match_paper = match_paper;
        changed || finish_changed
    }

    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut changed = TabSummary::new(2).show(ui) != Default::default();
        let (retouch_open, retouch_changed) =
            show_card(ui, "Retouch", self.retouch.open, |ui| self.show_retouch(ui));
        self.retouch.open = retouch_open;
        changed |= retouch_changed;
        let (finishing_open, finishing_changed) =
            show_card(ui, "Finishing", self.finishing.open, |ui| {
                self.show_finishing(ui)
            });
        self.finishing.open = finishing_open;
        changed |= finishing_changed;
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_retouch(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Overlay");
            changed |= self.retouch.overlay.show(ui).is_some();
        });
        changed |= labeled_toggle(ui, "Optical Removal", &mut self.retouch.optical).clicked();
        changed |= self.retouch.spot_threshold.show(ui, "Spot Threshold").1;
        changed |= self.retouch.hair_threshold.show(ui, "Hair Threshold").1;
        changed |= self.retouch.dust_size.show(ui, "Size").1;
        changed |= labeled_toggle(ui, "IR Removal", &mut self.retouch.ir_removal).clicked();
        changed |= self.retouch.ir_threshold.show(ui, "IR Threshold").1;
        let _ = section_subheader(ui, "MANUAL HEAL · 0");
        ui.horizontal(|ui| {
            changed |= labeled_toggle(ui, "Heal", &mut self.retouch.heal).clicked();
            changed |= labeled_toggle(ui, "Scratch", &mut self.retouch.scratch).clicked();
            changed |= labeled_toggle(ui, "Line", &mut self.retouch.line).clicked();
        });
        changed |= self.retouch.brush_size.show(ui, "Brush Size").1;
        changed |= self.retouch.line_sensitivity.show(ui, "Line Sensitivity").1;
        let _ = section_subheader(ui, "CLONE · 0");
        ui.horizontal(|ui| {
            changed |= labeled_toggle(ui, "Clone", &mut self.retouch.clone).clicked();
            changed |= labeled_toggle(ui, "Match Tone", &mut self.retouch.clone_match).clicked();
        });
        changed |= self.retouch.clone_strength.show(ui, "Strength").1;
        changed |= self.retouch.clone_feather.show(ui, "Feather").1;
        let _ = hint(ui, "Right-drag on the picture to protect an area.");
        ui.add_space(f32::from(theme.space_xs));
        changed
    }

    fn show_finishing(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        let _ = section_subheader(ui, "VIGNETTE");
        changed |= self.finishing.vignette_burn.show(ui, "Burn").1;
        changed |= self.finishing.vignette_size.show(ui, "Size").1;
        changed |= self.finishing.vignette_roundness.show(ui, "Roundness").1;
        let _ = section_subheader(ui, "FILED CARRIER");
        changed |= self.finishing.carrier_width.show(ui, "Width").1;
        changed |= self.finishing.carrier_roughness.show(ui, "Roughness").1;
        changed |= self.finishing.carrier_flare.show(ui, "Flare").1;
        changed |= self.finishing.carrier_corners.show(ui, "Corners").1;
        let _ = section_subheader(ui, "BORDER");
        changed |= self.finishing.border_width.show(ui, "Width").1;
        changed |= self.finishing.border_bottom.show(ui, "Bottom Weight").1;
        ui.horizontal(|ui| {
            ui.label("Color");
            changed |= self.finishing.border_color.show(ui);
        });
        let _ = labeled_action(ui, "Drop Empty Corners", ActionRank::Normal);
        ui.add_space(f32::from(theme.space_xs));
        changed
    }
}

fn near(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

fn show_card(
    ui: &mut Ui,
    title: &str,
    expanded: bool,
    body: impl FnOnce(&mut Ui) -> bool,
) -> (bool, bool) {
    let mut section = CollapsibleSection::new(title).with_guide();
    section.expanded = expanded;
    let (_, result) = section.show(ui, |ui| card(ui, body));
    ui.add_space(f32::from(Theme::default().space_sm));
    (section.expanded, result.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_finish_page_holds_its_two_cards_closed() {
        let state = FinishState::default();
        assert!(!state.retouch.open);
        assert!(!state.finishing.open);
        assert_eq!(state.retouch.overlay.current_label(), "Off");
        assert_eq!(state.finishing.border_color.current_label(), "Paper White");
    }

    #[test]
    fn the_finish_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = FinishState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_finish_page_and_stores_it() {
        let mut edit = FrameEdit::default();
        edit.config.retouch.dust_remove = true;
        edit.config.retouch.ir_threshold = 0.4;
        edit.config.retouch.clone_strength = 0.8;
        edit.config.finish.vignette_stops = -0.5;
        edit.config.finish.border_match_paper = false;
        let mut state = FinishState::default();
        state.load(&edit);
        assert!(state.retouch.optical);
        assert!((state.retouch.ir_threshold.value - 0.4).abs() < 1e-9);
        assert!((state.retouch.clone_strength.value - 80.0).abs() < 1e-9);
        assert!((state.finishing.vignette_burn.value + 0.5).abs() < 1e-9);
        assert_eq!(state.finishing.border_color.current_label(), "Custom");
        assert!(!state.store(&mut edit));
    }
}
