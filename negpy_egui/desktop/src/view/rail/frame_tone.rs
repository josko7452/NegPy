//! The Frame tab's Exposure page: filtration, tone, paper response and local work.

use eframe::egui::Ui;

use negpy_backend::domain::models::exposure::BwFilter;
use negpy_backend::domain::models::process::ProcessMode;
use negpy_backend::features::exposure::papers::profiles_for_mode;
use negpy_backend::features::pickbetter::{kelvin_to_wb, wb_to_kelvin};

use crate::edit::FrameEdit;
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
            paper: StepCombo::new(vec![Choice::new("Digital (Adobe RGB)")]),
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
    pub temperature_dirty: bool,
}

impl Default for ExposureState {
    fn default() -> Self {
        Self {
            filtration: FiltrationState::default(),
            tone: ToneState::default(),
            paper: PaperState::default(),
            local: LocalState::default(),
            temperature_dirty: false,
        }
    }
}

impl ExposureState {
    /// Replace the controls with the exposure values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let exposure = &edit.config.exposure;
        let process = &edit.config.process;
        self.filtration.filter.current = bw_filter_index(exposure.bw_filter);
        if !self.temperature_dirty {
            self.filtration
                .temperature
                .set_value(wb_to_kelvin(exposure.wb_magenta, exposure.wb_yellow));
        }
        self.temperature_dirty = false;
        self.filtration.cyan.set_value(exposure.wb_cyan);
        self.filtration.magenta.set_value(exposure.wb_magenta);
        self.filtration.yellow.set_value(exposure.wb_yellow);
        self.filtration
            .cast_removal
            .set_value(exposure.cast_removal_strength);
        self.tone.print_density.set_value(exposure.density);
        self.tone.grade.set_value(exposure.grade);
        self.tone.shadows_density.set_value(exposure.shadow_density);
        self.tone
            .highlights_density
            .set_value(exposure.highlight_density);
        self.tone.shadows_grade.set_value(exposure.shadow_grade);
        self.tone
            .highlights_grade
            .set_value(exposure.highlight_grade);
        self.tone.preflash.set_value(exposure.preflash);
        self.tone.diffusion.set_value(exposure.diffusion);
        self.tone
            .contrast_mask
            .set_value(exposure.contrast_mask_strength);
        let papers = profiles_for_mode(edit.config.process.process_mode)
            .into_iter()
            .map(|(key, profile)| Choice::with_id(profile.label, key))
            .collect();
        self.paper.paper = StepCombo::new(papers);
        self.paper.paper.pick(&exposure.paper_profile);
        self.paper.paper_black = exposure.paper_black;
        self.paper.paper_white = exposure.paper_dmin;
        self.paper.dye_separation.set_value(exposure.dye_separation);
        self.paper
            .separation_damping
            .set_value(exposure.separation_damping);
        self.paper.toe.set_value(exposure.toe);
        self.paper.toe_width.set_value(exposure.toe_width);
        self.paper.shoulder.set_value(exposure.shoulder);
        self.paper.shoulder_width.set_value(exposure.shoulder_width);
        self.local.burn.set_value(exposure.shadow_density);
        self.local.grade.set_value(exposure.shadow_grade);
        self.local.flash.set_value(process.analysis_buffer);
        self.local
            .feather
            .set_value(process.analysis_buffer.min(0.15));
        self.local
            .tone_zone
            .set_value(process.analysis_buffer * 10.0);
        self.local
            .tone_softness
            .set_value(process.analysis_buffer + 1.0);
        self.tone.open = self.tone.open || edit.config.process.process_mode == ProcessMode::BW;
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        let exposure = &mut edit.config.exposure;
        let filter = BW_FILTERS[self.filtration.filter.current.min(BW_FILTERS.len() - 1)];
        let cyan = self.filtration.cyan.value;
        let temperature = self.filtration.temperature.value;
        let (magenta, yellow) = if !near(
            wb_to_kelvin(self.filtration.magenta.value, self.filtration.yellow.value),
            temperature,
        ) {
            kelvin_to_wb(
                temperature,
                self.filtration.magenta.value,
                self.filtration.yellow.value,
            )
        } else {
            (self.filtration.magenta.value, self.filtration.yellow.value)
        };
        let cast = self.filtration.cast_removal.value;
        let density = self.tone.print_density.value;
        let grade = self.tone.grade.value;
        let shadow_density = self.tone.shadows_density.value;
        let highlight_density = self.tone.highlights_density.value;
        let shadow_grade = self.tone.shadows_grade.value;
        let highlight_grade = self.tone.highlights_grade.value;
        let preflash = self.tone.preflash.value;
        let diffusion = self.tone.diffusion.value;
        let contrast = self.tone.contrast_mask.value;
        let paper_profile = self.paper.paper.current_value().to_string();
        let paper_black = self.paper.paper_black;
        let paper_white = self.paper.paper_white;
        let dye = self.paper.dye_separation.value;
        let damping = self.paper.separation_damping.value;
        let toe = self.paper.toe.value;
        let toe_width = self.paper.toe_width.value;
        let shoulder = self.paper.shoulder.value;
        let shoulder_width = self.paper.shoulder_width.value;
        let changed = exposure.bw_filter != filter
            || !near(exposure.wb_cyan, cyan)
            || !near(exposure.wb_magenta, magenta)
            || !near(exposure.wb_yellow, yellow)
            || !near(exposure.cast_removal_strength, cast)
            || !near(exposure.density, density)
            || !near(exposure.grade, grade)
            || !near(exposure.shadow_density, shadow_density)
            || !near(exposure.highlight_density, highlight_density)
            || !near(exposure.shadow_grade, shadow_grade)
            || !near(exposure.highlight_grade, highlight_grade)
            || !near(exposure.preflash, preflash)
            || !near(exposure.diffusion, diffusion)
            || !near(exposure.contrast_mask_strength, contrast)
            || exposure.paper_profile != paper_profile
            || exposure.paper_black != paper_black
            || exposure.paper_dmin != paper_white
            || !near(exposure.dye_separation, dye)
            || !near(exposure.separation_damping, damping)
            || !near(exposure.toe, toe)
            || !near(exposure.toe_width, toe_width)
            || !near(exposure.shoulder, shoulder)
            || !near(exposure.shoulder_width, shoulder_width);
        exposure.bw_filter = filter;
        exposure.wb_cyan = cyan;
        exposure.wb_magenta = magenta;
        exposure.wb_yellow = yellow;
        exposure.cast_removal_strength = cast;
        exposure.density = density;
        exposure.grade = grade;
        exposure.shadow_density = shadow_density;
        exposure.highlight_density = highlight_density;
        exposure.shadow_grade = shadow_grade;
        exposure.highlight_grade = highlight_grade;
        exposure.preflash = preflash;
        exposure.diffusion = diffusion;
        exposure.contrast_mask_strength = contrast;
        exposure.paper_profile = paper_profile;
        exposure.paper_black = paper_black;
        exposure.paper_dmin = paper_white;
        exposure.dye_separation = dye;
        exposure.separation_damping = damping;
        exposure.toe = toe;
        exposure.toe_width = toe_width;
        exposure.shoulder = shoulder;
        exposure.shoulder_width = shoulder_width;
        changed
    }

    /// Paint the page. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
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
        if changed {
            self.store(edit);
        }
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
        if self.filtration.temperature.show(ui, "Temperature").1 {
            self.temperature_dirty = true;
            changed = true;
        }
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

const BW_FILTERS: [BwFilter; 5] = [
    BwFilter::None,
    BwFilter::Yellow8,
    BwFilter::DeepYellow15,
    BwFilter::Orange21,
    BwFilter::Red25,
];

fn bw_filter_index(filter: BwFilter) -> usize {
    BW_FILTERS
        .iter()
        .position(|candidate| *candidate == filter)
        .unwrap_or(0)
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
    fn the_paper_selector_writes_the_profile_key() {
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.paper.paper.pick("pakon");
        state.store(&mut edit);
        assert_eq!(edit.config.exposure.paper_profile, "pakon");
    }

    #[test]
    fn a_temperature_change_moves_the_magenta_yellow_pair() {
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        edit.config.exposure.wb_magenta = 0.0;
        edit.config.exposure.wb_yellow = 0.0;
        state.load(&edit);
        state.filtration.temperature.set_value(7000.0);
        state.store(&mut edit);
        assert!(edit.config.exposure.wb_magenta != 0.0 || edit.config.exposure.wb_yellow != 0.0);
        assert!(
            (wb_to_kelvin(
                edit.config.exposure.wb_magenta,
                edit.config.exposure.wb_yellow
            ) - 7000.0)
                .abs()
                < 50.0
        );
    }

    #[test]
    fn the_exposure_page_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_exposure_page_and_stores_it() {
        let mut edit = FrameEdit::default();
        edit.config.exposure.density = 1.22;
        edit.config.exposure.grade = 90.0;
        edit.config.exposure.bw_filter = BwFilter::Orange21;
        edit.config.exposure.paper_dmin = true;
        edit.config.exposure.toe = 0.4;
        let mut state = ExposureState::default();
        state.load(&edit);
        assert!((state.tone.print_density.value - 1.22).abs() < 1e-9);
        assert_eq!(state.tone.grade.value, 90.0);
        assert_eq!(state.filtration.filter.current_label(), "Orange #21");
        assert!(state.paper.paper_white);
        assert!((state.paper.toe.value - 0.4).abs() < 1e-9);
        assert_eq!(state.paper.paper.current_value(), "neutral");
        assert!(!state.store(&mut edit));
    }
}
