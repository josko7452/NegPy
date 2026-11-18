//! The Frame tab's Exposure page: filtration, tone, paper response and local work.

use eframe::egui::Ui;

use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::tab_header::TabSummary;

/// The Filtration card.
#[derive(Clone, Debug, PartialEq)]
pub struct FiltrationState {
    pub filter: StepCombo,
    pub region: SegmentedChoice<'static>,
    pub temperature: CompactSlider,
    pub cyan: CompactSlider,
    pub magenta: CompactSlider,
    pub yellow: CompactSlider,
    pub cast_removal: CompactSlider,
    pub open: bool,
}

impl Default for FiltrationState {
    fn default() -> Self {
        Self {
            filter: StepCombo::new(vec![
                Choice::new("None"),
                Choice::new("Yellow #8"),
                Choice::new("Deep Yellow #15"),
                Choice::new("Orange #21"),
                Choice::new("Red #25"),
            ]),
            region: SegmentedChoice::new(&["Global", "Shadows", "Highlights"]),
            temperature: CompactSlider::new(
                SliderSpec::new(2000.0, 12000.0, 50.0, 0)
                    .with_default(5500.0)
                    .with_unit(" K"),
            ),
            cyan: CompactSlider::new(SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)),
            magenta: CompactSlider::new(SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)),
            yellow: CompactSlider::new(SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)),
            cast_removal: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            open: false,
        }
    }
}

/// The Tone card.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneState {
    pub channel: SegmentedChoice<'static>,
    pub print_density: CompactSlider,
    pub grade: CompactSlider,
    pub shadows_density: CompactSlider,
    pub highlights_density: CompactSlider,
    pub shadows_grade: CompactSlider,
    pub highlights_grade: CompactSlider,
    pub contrast_mask: CompactSlider,
    pub preflash: CompactSlider,
    pub diffusion: CompactSlider,
    pub aids_open: bool,
    pub open: bool,
}

impl Default for ToneState {
    fn default() -> Self {
        Self {
            channel: SegmentedChoice::new(&["Global", "R", "G", "B"]),
            print_density: CompactSlider::new(
                SliderSpec::new(0.0, 2.0, 0.01, 2)
                    .with_default(1.08)
                    .with_unit(""),
            ),
            grade: CompactSlider::new(
                SliderSpec::new(30.0, 130.0, 1.0, 0)
                    .with_default(83.0)
                    .with_unit(" R"),
            ),
            shadows_density: CompactSlider::new(
                SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            highlights_density: CompactSlider::new(
                SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            shadows_grade: CompactSlider::new(
                SliderSpec::new(-50.0, 50.0, 1.0, 0).with_default(0.0),
            ),
            highlights_grade: CompactSlider::new(
                SliderSpec::new(-50.0, 50.0, 1.0, 0).with_default(0.0),
            ),
            contrast_mask: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            preflash: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            diffusion: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            aids_open: true,
            open: true,
        }
    }
}

/// The Paper Response card.
#[derive(Clone, Debug, PartialEq)]
pub struct PaperState {
    pub paper: StepCombo,
    pub paper_black: bool,
    pub paper_white: bool,
    pub dye_separation: CompactSlider,
    pub separation_damping: CompactSlider,
    pub snap: CompactSlider,
    pub toe: CompactSlider,
    pub toe_width: CompactSlider,
    pub shoulder: CompactSlider,
    pub shoulder_width: CompactSlider,
    pub open: bool,
}

impl Default for PaperState {
    fn default() -> Self {
        Self {
            paper: StepCombo::new(vec![
                Choice::new("Digital (Adobe RGB)"),
                Choice::new("Kodak Portra"),
                Choice::new("Ilford Multigrade"),
            ]),
            paper_black: false,
            paper_white: false,
            dye_separation: CompactSlider::new(
                SliderSpec::new(0.5, 1.5, 0.01, 2).with_default(1.0),
            ),
            separation_damping: CompactSlider::new(
                SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0),
            ),
            snap: CompactSlider::new(SliderSpec::new(-0.5, 0.5, 0.01, 2).with_default(0.0)),
            toe: CompactSlider::new(SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)),
            toe_width: CompactSlider::new(SliderSpec::new(0.1, 5.0, 0.01, 2).with_default(1.0)),
            shoulder: CompactSlider::new(SliderSpec::new(-1.0, 1.0, 0.01, 2).with_default(0.0)),
            shoulder_width: CompactSlider::new(
                SliderSpec::new(0.1, 5.0, 0.01, 2).with_default(1.0),
            ),
            open: false,
        }
    }
}

/// The Dodge & Burn card.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalState {
    pub draw: bool,
    pub oval: bool,
    pub card: bool,
    pub burn: CompactSlider,
    pub grade: CompactSlider,
    pub flash: CompactSlider,
    pub feather: CompactSlider,
    pub tone_zone: CompactSlider,
    pub tone_softness: CompactSlider,
    pub open: bool,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            draw: false,
            oval: false,
            card: false,
            burn: CompactSlider::new(
                SliderSpec::new(-2.0, 2.0, 0.05, 2)
                    .with_default(0.0)
                    .with_unit(" st"),
            ),
            grade: CompactSlider::new(SliderSpec::new(-40.0, 40.0, 5.0, 0).with_default(0.0)),
            flash: CompactSlider::new(SliderSpec::new(0.0, 1.0, 0.01, 2).with_default(0.0)),
            feather: CompactSlider::new(SliderSpec::new(0.0, 0.15, 0.005, 3).with_default(0.04)),
            tone_zone: CompactSlider::new(SliderSpec::new(0.0, 10.0, 0.333, 3).with_default(6.0)),
            tone_softness: CompactSlider::new(
                SliderSpec::new(0.333, 3.0, 0.333, 3).with_default(1.0),
            ),
            open: false,
        }
    }
}

/// The Exposure page's state.
#[derive(Clone, Debug, PartialEq)]
pub struct ExposureState {
    pub filtration: FiltrationState,
    pub tone: ToneState,
    pub paper: PaperState,
    pub local: LocalState,
}

impl Default for ExposureState {
    fn default() -> Self {
        Self {
            filtration: FiltrationState::default(),
            tone: ToneState::default(),
            paper: PaperState::default(),
            local: LocalState::default(),
        }
    }
}

impl ExposureState {
    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = TabSummary::new(4).show(ui) != Default::default();
        let (filtration_open, filtration_changed) =
            show_card(ui, "Filtration", self.filtration.open, |ui| {
                self.show_filtration(ui)
            });
        self.filtration.open = filtration_open;
        changed |= filtration_changed;
        let (tone_open, tone_changed) =
            show_card(ui, "Tone", self.tone.open, |ui| self.show_tone(ui));
        self.tone.open = tone_open;
        changed |= tone_changed;
        let (paper_open, paper_changed) = show_card(ui, "Paper Response", self.paper.open, |ui| {
            self.show_paper(ui)
        });
        self.paper.open = paper_open;
        changed |= paper_changed;
        let (local_open, local_changed) = show_card(ui, "Dodge & Burn", self.local.open, |ui| {
            self.show_local(ui)
        });
        self.local.open = local_open;
        changed |= local_changed;
        changed
    }

    fn show_filtration(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Filter");
            changed |= self.filtration.filter.show(ui);
        });
        ui.add_space(f32::from(theme.space_sm));
        changed |= self.filtration.region.show(ui).is_some();
        ui.add_space(f32::from(theme.space_sm));
        changed |= self.filtration.temperature.show(ui, "Temperature").1;
        changed |= self.filtration.cyan.show(ui, "Cyan").1;
        changed |= self.filtration.magenta.show(ui, "Magenta").1;
        changed |= self.filtration.yellow.show(ui, "Yellow").1;
        changed |= self.filtration.cast_removal.show(ui, "Cast Removal").1;
        changed
    }

    fn show_tone(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = self.tone.channel.show(ui).is_some();
        ui.add_space(f32::from(theme.space_sm));
        changed |= self.tone.print_density.show(ui, "Print Density").1;
        changed |= self.tone.grade.show(ui, "ISO-R Grade").1;
        changed |= self.tone.shadows_density.show(ui, "Shadows Density").1;
        changed |= self
            .tone
            .highlights_density
            .show(ui, "Highlights Density")
            .1;
        changed |= self.tone.shadows_grade.show(ui, "Shadows Grade").1;
        changed |= self.tone.highlights_grade.show(ui, "Highlights Grade").1;
        ui.add_space(f32::from(theme.space_sm));
        let _ = section_subheader(ui, "Printing Aids");
        changed |= self.tone.preflash.show(ui, "Preflash").1;
        changed |= self.tone.diffusion.show(ui, "Diffusion").1;
        changed |= self.tone.contrast_mask.show(ui, "Contrast Mask").1;
        changed
    }

    fn show_paper(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Print stock");
            changed |= self.paper.paper.show(ui);
        });
        ui.horizontal(|ui| {
            changed |= labeled_toggle(ui, "Paper Black", &mut self.paper.paper_black).clicked();
            changed |= labeled_toggle(ui, "Paper White", &mut self.paper.paper_white).clicked();
        });
        changed |= self.paper.dye_separation.show(ui, "Dye Separation").1;
        changed |= self
            .paper
            .separation_damping
            .show(ui, "Separation Damping")
            .1;
        changed |= self.paper.snap.show(ui, "Snap").1;
        changed |= self.paper.toe.show(ui, "Toe").1;
        changed |= self.paper.toe_width.show(ui, "Toe Width").1;
        changed |= self.paper.shoulder.show(ui, "Shoulder").1;
        changed |= self.paper.shoulder_width.show(ui, "Shoulder Width").1;
        let _ = hint(
            ui,
            "Paper White and Paper Black show the stock's real white and black.",
        );
        ui.add_space(f32::from(theme.space_xs));
        changed
    }

    fn show_local(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= labeled_toggle(ui, "Draw", &mut self.local.draw).clicked();
            changed |= labeled_toggle(ui, "Oval", &mut self.local.oval).clicked();
            changed |= labeled_toggle(ui, "Card", &mut self.local.card).clicked();
        });
        let _ = section_subheader(ui, "MASKS · 0");
        changed |= self.local.burn.show(ui, "Burn").1;
        changed |= self.local.grade.show(ui, "Grade").1;
        changed |= self.local.flash.show(ui, "Flash").1;
        changed |= self.local.feather.show(ui, "Feather").1;
        let _ = section_subheader(ui, "Tone Limit");
        changed |= self.local.tone_zone.show(ui, "Tone Zone").1;
        changed |= self.local.tone_softness.show(ui, "Tone Softness").1;
        let _ = labeled_action(ui, "Clear All", ActionRank::Normal);
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
    fn the_exposure_page_opens_its_tone_card() {
        let state = ExposureState::default();
        assert!(state.tone.open);
        assert!(!state.filtration.open);
        assert!(!state.paper.open);
        assert!(!state.local.open);
        assert_eq!(state.tone.channel.current_label(), "Global");
        assert_eq!(state.paper.paper.current_label(), "Digital (Adobe RGB)");
    }

    #[test]
    fn the_exposure_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ExposureState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
