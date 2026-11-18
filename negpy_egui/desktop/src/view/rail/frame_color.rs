//! The Frame tab's Look page: color, detail, alternative processes and toning.

use eframe::egui::Ui;

use crate::styles::templates::{TextRole, hint_label, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::tab_header::TabSummary;

/// The Lab card: chroma, detail and effects.
#[derive(Clone, Debug, PartialEq)]
pub struct LabState {
    pub chroma: CompactSlider,
    pub skin_protection: CompactSlider,
    pub chroma_denoise: CompactSlider,
    pub sharpening: CompactSlider,
    pub method: StepCombo,
    pub radius: CompactSlider,
    pub masking: CompactSlider,
    pub clahe: CompactSlider,
    pub glow: CompactSlider,
    pub halation: CompactSlider,
    pub open: bool,
}

impl Default for LabState {
    fn default() -> Self {
        Self {
            chroma: CompactSlider::new(SliderSpec::new(0.0, 2.0, 0.01, 2).with_default(1.0)),
            skin_protection: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            chroma_denoise: CompactSlider::new(
                SliderSpec::new(0.0, 5.0, 0.01, 2).with_default(0.0),
            ),
            sharpening: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            method: StepCombo::new(vec![
                Choice::new("Unsharp Mask"),
                Choice::new("Deconvolution"),
            ]),
            radius: CompactSlider::new(
                SliderSpec::new(0.5, 3.0, 0.01, 2)
                    .with_default(1.0)
                    .with_unit(" px"),
            ),
            masking: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            clahe: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            glow: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            halation: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            open: false,
        }
    }
}

/// The Alternative Processes card.
#[derive(Clone, Debug, PartialEq)]
pub struct AltProcessState {
    pub mode: SegmentedChoice<'static>,
    pub sensitizer: StepCombo,
    pub lith_exposure: CompactSlider,
    pub lith_snatch: CompactSlider,
    pub lith_abruptness: CompactSlider,
    pub cyano_exposure: CompactSlider,
    pub cyano_scale: CompactSlider,
    pub cyano_bleach: CompactSlider,
    pub cyano_tannin: CompactSlider,
    pub sabattier_strength: CompactSlider,
    pub sabattier_reexposure: CompactSlider,
    pub sabattier_agitation: CompactSlider,
    pub open: bool,
}

impl Default for AltProcessState {
    fn default() -> Self {
        Self {
            mode: SegmentedChoice::new(&["None", "Lith", "Cyanotype", "Sabattier"]),
            sensitizer: StepCombo::new(vec![
                Choice::new("Classic (Herschel)"),
                Choice::new("New (Ware)"),
            ]),
            lith_exposure: CompactSlider::new(
                SliderSpec::new(0.0, 5.0, 0.1, 1)
                    .with_default(2.0)
                    .with_unit(" st"),
            ),
            lith_snatch: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.5)),
            lith_abruptness: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.5),
            ),
            cyano_exposure: CompactSlider::new(
                SliderSpec::new(-2.0, 4.0, 0.1, 1)
                    .with_default(1.0)
                    .with_unit(" st"),
            ),
            cyano_scale: CompactSlider::new(SliderSpec::new(0.8, 2.8, 0.05, 2).with_default(1.8)),
            cyano_bleach: CompactSlider::new(SliderSpec::new(0.0, 0.5, 0.01, 2).with_default(0.0)),
            cyano_tannin: CompactSlider::new(SliderSpec::new(0.0, 0.5, 0.01, 2).with_default(0.0)),
            sabattier_strength: CompactSlider::new(
                SliderSpec::new(0.0, 2.0, 0.05, 2).with_default(1.0),
            ),
            sabattier_reexposure: CompactSlider::new(
                SliderSpec::new(0.1, 0.9, 0.05, 2).with_default(0.5),
            ),
            sabattier_agitation: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.05, 2).with_default(0.5),
            ),
            open: false,
        }
    }
}

/// The Toning card.
#[derive(Clone, Debug, PartialEq)]
pub struct ToningState {
    pub selenium: CompactSlider,
    pub sepia: CompactSlider,
    pub gold: CompactSlider,
    pub iron_blue: CompactSlider,
    pub copper: CompactSlider,
    pub vanadium: CompactSlider,
    pub shadow_hue: CompactSlider,
    pub shadow_strength: CompactSlider,
    pub highlight_hue: CompactSlider,
    pub highlight_strength: CompactSlider,
    pub open: bool,
}

impl Default for ToningState {
    fn default() -> Self {
        let toner = || CompactSlider::new(SliderSpec::new(0.0, 2.0, 0.01, 2).with_default(0.0));
        Self {
            selenium: toner(),
            sepia: toner(),
            gold: toner(),
            iron_blue: toner(),
            copper: toner(),
            vanadium: toner(),
            shadow_hue: CompactSlider::new(
                SliderSpec::new(0.0, 360.0, 1.0, 0)
                    .with_default(0.0)
                    .with_unit("°"),
            ),
            shadow_strength: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            highlight_hue: CompactSlider::new(
                SliderSpec::new(0.0, 360.0, 1.0, 0)
                    .with_default(0.0)
                    .with_unit("°"),
            ),
            highlight_strength: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            open: false,
        }
    }
}

/// The Look page's state.
#[derive(Clone, Debug, PartialEq)]
pub struct LookState {
    pub lab: LabState,
    pub alt: AltProcessState,
    pub toning: ToningState,
}

impl Default for LookState {
    fn default() -> Self {
        Self {
            lab: LabState::default(),
            alt: AltProcessState::default(),
            toning: ToningState::default(),
        }
    }
}

impl LookState {
    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = TabSummary::new(3).show(ui) != Default::default();
        let (lab_open, lab_changed) = show_card(ui, "Lab", self.lab.open, |ui| self.show_lab(ui));
        self.lab.open = lab_open;
        changed |= lab_changed;
        let (alt_open, alt_changed) = show_card(ui, "Alternative Processes", self.alt.open, |ui| {
            self.show_alt(ui)
        });
        self.alt.open = alt_open;
        changed |= alt_changed;
        let (toning_open, toning_changed) =
            show_card(ui, "Toning", self.toning.open, |ui| self.show_toning(ui));
        self.toning.open = toning_open;
        changed |= toning_changed;
        changed
    }

    fn show_lab(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        let _ = section_subheader(ui, "COLOR");
        changed |= self.lab.chroma.show(ui, "Chroma").1;
        changed |= self.lab.skin_protection.show(ui, "Skin Protection").1;
        changed |= self.lab.chroma_denoise.show(ui, "Chroma Denoise").1;
        ui.add_space(f32::from(Theme::default().space_sm));
        let _ = section_subheader(ui, "DETAIL");
        changed |= self.lab.sharpening.show(ui, "Sharpening").1;
        ui.horizontal(|ui| {
            ui.label("Method");
            changed |= self.lab.method.show(ui);
        });
        changed |= self.lab.radius.show(ui, "Radius").1;
        changed |= self.lab.masking.show(ui, "Masking").1;
        changed |= self.lab.clahe.show(ui, "CLAHE").1;
        let _ = section_subheader(ui, "EFFECTS");
        changed |= self.lab.glow.show(ui, "Glow").1;
        changed |= self.lab.halation.show(ui, "Halation").1;
        changed
    }

    fn show_alt(&mut self, ui: &mut Ui) -> bool {
        let mut changed = self.alt.mode.show(ui).is_some();
        let _ = hint_label(ui, "Black and white film only", TextRole::Hint);
        match self.alt.mode.current {
            1 => {
                changed |= self.alt.lith_exposure.show(ui, "Exposure").1;
                changed |= self.alt.lith_snatch.show(ui, "Snatch Point").1;
                changed |= self.alt.lith_abruptness.show(ui, "Abruptness").1;
            }
            2 => {
                ui.horizontal(|ui| {
                    ui.label("Sensitizer");
                    changed |= self.alt.sensitizer.show(ui);
                });
                changed |= self.alt.cyano_exposure.show(ui, "Exposure").1;
                changed |= self.alt.cyano_scale.show(ui, "Exposure Scale").1;
                changed |= self.alt.cyano_bleach.show(ui, "Bleach").1;
                changed |= self.alt.cyano_tannin.show(ui, "Tannin").1;
            }
            3 => {
                changed |= self.alt.sabattier_strength.show(ui, "Strength").1;
                changed |= self.alt.sabattier_reexposure.show(ui, "Re-exposure").1;
                changed |= self.alt.sabattier_agitation.show(ui, "Agitation").1;
            }
            _ => {}
        }
        changed
    }

    fn show_toning(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        let _ = section_subheader(ui, "CHEMICAL TONING");
        changed |= self.toning.selenium.show(ui, "Selenium").1;
        changed |= self.toning.sepia.show(ui, "Sepia").1;
        changed |= self.toning.gold.show(ui, "Gold").1;
        changed |= self.toning.iron_blue.show(ui, "Iron Blue").1;
        changed |= self.toning.copper.show(ui, "Copper").1;
        changed |= self.toning.vanadium.show(ui, "Vanadium").1;
        let _ = section_subheader(ui, "SPLIT TONING");
        changed |= self.toning.shadow_strength.show(ui, "Shadow Strength").1;
        if !self.toning.shadow_strength.is_neutral() {
            changed |= self.toning.shadow_hue.show(ui, "Shadow Hue").1;
        }
        changed |= self
            .toning
            .highlight_strength
            .show(ui, "Highlight Strength")
            .1;
        if !self.toning.highlight_strength.is_neutral() {
            changed |= self.toning.highlight_hue.show(ui, "Highlight Hue").1;
        }
        changed
    }
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
    fn the_look_page_holds_its_three_cards_closed() {
        let state = LookState::default();
        assert!(!state.lab.open);
        assert!(!state.alt.open);
        assert!(!state.toning.open);
        assert_eq!(state.alt.mode.current_label(), "None");
        assert_eq!(state.lab.method.current_label(), "Unsharp Mask");
    }

    #[test]
    fn the_look_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = LookState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
