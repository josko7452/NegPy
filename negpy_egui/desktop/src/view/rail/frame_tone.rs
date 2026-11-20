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
    pub region_yellow: [f64; 3],
    pub region_magenta: [f64; 3],
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
            region_yellow: [0.0; 3],
            region_magenta: [0.0; 3],
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
    pub channel_trims: [[f64; 3]; 3],
    pub channel_grade: CompactSlider,
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
            channel_trims: [[0.0; 3]; 3],
            channel_grade: CompactSlider::new(
                SliderSpec::new(-20.0, 20.0, 1.0, 0).with_default(0.0),
            ),
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
    pub dye_separation_trim_red: CompactSlider,
    pub dye_separation_trim_green: CompactSlider,
    pub dye_separation_trim_blue: CompactSlider,
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
            dye_separation_trim_red: CompactSlider::new(
                SliderSpec::new(-0.4, 0.4, 0.01, 2).with_default(0.0),
            ),
            dye_separation_trim_green: CompactSlider::new(
                SliderSpec::new(-0.4, 0.4, 0.01, 2).with_default(0.0),
            ),
            dye_separation_trim_blue: CompactSlider::new(
                SliderSpec::new(-0.4, 0.4, 0.01, 2).with_default(0.0),
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
    pub is_bw: bool,
    /// The selected filtration region and tone channel survive a frame repaint.
    pub region: usize,
    pub channel: usize,
    /// The masks the canvas lasso has stored for this frame.
    pub mask_count: usize,
    clear_masks: bool,
}

impl Default for ExposureState {
    fn default() -> Self {
        Self {
            filtration: FiltrationState::default(),
            tone: ToneState::default(),
            paper: PaperState::default(),
            local: LocalState::default(),
            temperature_dirty: false,
            is_bw: false,
            region: 0,
            channel: 0,
            mask_count: 0,
            clear_masks: false,
        }
    }
}

impl ExposureState {
    /// Replace the controls with the exposure values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let exposure = &edit.config.exposure;
        let process = &edit.config.process;
        self.filtration.region.current = self.region.min(2);
        self.tone.channel.current = self.channel.min(3);
        self.filtration.filter.current = bw_filter_index(exposure.bw_filter);
        let region_channels = [
            (exposure.wb_cyan, exposure.wb_magenta, exposure.wb_yellow),
            (
                exposure.shadow_cyan,
                exposure.shadow_magenta,
                exposure.shadow_yellow,
            ),
            (
                exposure.highlight_cyan,
                exposure.highlight_magenta,
                exposure.highlight_yellow,
            ),
        ];
        self.filtration.region_yellow = [
            region_channels[0].2,
            region_channels[1].2,
            region_channels[2].2,
        ];
        self.filtration.region_magenta = [
            region_channels[0].1,
            region_channels[1].1,
            region_channels[2].1,
        ];
        let region = self.filtration.region.current.min(2);
        if !self.temperature_dirty {
            self.filtration.temperature.set_value(wb_to_kelvin(
                region_channels[region].1,
                region_channels[region].2,
            ));
        }
        self.temperature_dirty = false;
        self.filtration.cyan.set_value(region_channels[region].0);
        self.filtration.magenta.set_value(region_channels[region].1);
        self.filtration.yellow.set_value(region_channels[region].2);
        self.filtration
            .cast_removal
            .set_value(exposure.cast_removal_strength);
        self.tone.print_density.set_value(exposure.density);
        self.tone.grade.set_value(exposure.grade);
        self.tone.shadows_density.set_value(exposure.shadow_density);
        self.tone
            .highlights_density
            .set_value(exposure.highlight_density);
        self.tone.channel_trims = [
            [
                exposure.grade_trim_red,
                exposure.grade_trim_green,
                exposure.grade_trim_blue,
            ],
            [
                exposure.shadow_grade_trim_red,
                exposure.shadow_grade_trim_green,
                exposure.shadow_grade_trim_blue,
            ],
            [
                exposure.highlight_grade_trim_red,
                exposure.highlight_grade_trim_green,
                exposure.highlight_grade_trim_blue,
            ],
        ];
        let channel = self.tone.channel.current.saturating_sub(1).min(2);
        self.tone.channel_grade.set_value(if channel > 0 {
            self.tone.channel_trims[0][channel - 1]
        } else {
            0.0
        });
        self.tone.shadows_grade.set_value(if channel > 0 {
            self.tone.channel_trims[1][channel - 1]
        } else {
            exposure.shadow_grade
        });
        self.tone.highlights_grade.set_value(if channel > 0 {
            self.tone.channel_trims[2][channel - 1]
        } else {
            exposure.highlight_grade
        });
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
            .dye_separation_trim_red
            .set_value(exposure.dye_separation_trim_red);
        self.paper
            .dye_separation_trim_green
            .set_value(exposure.dye_separation_trim_green);
        self.paper
            .dye_separation_trim_blue
            .set_value(exposure.dye_separation_trim_blue);
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
        self.is_bw = edit.config.process.process_mode == ProcessMode::BW;
        self.mask_count = edit.config.local.masks.len();
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        self.region = self.filtration.region.current.min(2);
        self.channel = self.tone.channel.current.min(3);
        let exposure = &mut edit.config.exposure;
        let filter = BW_FILTERS[self.filtration.filter.current.min(BW_FILTERS.len() - 1)];
        let region = self.filtration.region.current.min(2);
        let cyan = self.filtration.cyan.value;
        let temperature = self.filtration.temperature.value;
        let (mut magenta, mut yellow) =
            (self.filtration.magenta.value, self.filtration.yellow.value);
        if !near(wb_to_kelvin(magenta, yellow), temperature) {
            (magenta, yellow) = kelvin_to_wb(temperature, magenta, yellow);
        }
        self.filtration.region_yellow[region] = yellow;
        self.filtration.region_magenta[region] = magenta;
        let region_yellow = self.filtration.region_yellow;
        let cast = self.filtration.cast_removal.value;
        let density = self.tone.print_density.value;
        let grade = self.tone.grade.value;
        let shadow_density = self.tone.shadows_density.value;
        let highlight_density = self.tone.highlights_density.value;
        let channel = self.tone.channel.current.saturating_sub(1).min(2);
        if channel > 0 {
            self.tone.channel_trims[0][channel - 1] = self.tone.channel_grade.value;
            self.tone.channel_trims[1][channel - 1] = self.tone.shadows_grade.value;
            self.tone.channel_trims[2][channel - 1] = self.tone.highlights_grade.value;
        }
        let (shadow_grade, highlight_grade) = if channel > 0 {
            (exposure.shadow_grade, exposure.highlight_grade)
        } else {
            (
                self.tone.shadows_grade.value,
                self.tone.highlights_grade.value,
            )
        };
        let trims = self.tone.channel_trims;
        let preflash = self.tone.preflash.value;
        let diffusion = self.tone.diffusion.value;
        let contrast = self.tone.contrast_mask.value;
        let paper_profile = self.paper.paper.current_value().to_string();
        let paper_black = self.paper.paper_black;
        let paper_white = self.paper.paper_white;
        let dye = self.paper.dye_separation.value;
        let dye_red = self.paper.dye_separation_trim_red.value;
        let dye_green = self.paper.dye_separation_trim_green.value;
        let dye_blue = self.paper.dye_separation_trim_blue.value;
        let damping = self.paper.separation_damping.value;
        let toe = self.paper.toe.value;
        let toe_width = self.paper.toe_width.value;
        let shoulder = self.paper.shoulder.value;
        let shoulder_width = self.paper.shoulder_width.value;
        let region_channels = match region {
            1 => (exposure.shadow_cyan, exposure.shadow_magenta),
            2 => (exposure.highlight_cyan, exposure.highlight_magenta),
            _ => (exposure.wb_cyan, exposure.wb_magenta),
        };
        let changed = exposure.bw_filter != filter
            || !near(region_channels.0, cyan)
            || !near(region_channels.1, magenta)
            || !near(
                match region {
                    1 => exposure.shadow_yellow,
                    2 => exposure.highlight_yellow,
                    _ => exposure.wb_yellow,
                },
                region_yellow[region],
            )
            || !near(exposure.grade_trim_red, trims[0][0])
            || !near(exposure.grade_trim_green, trims[0][1])
            || !near(exposure.grade_trim_blue, trims[0][2])
            || !near(exposure.shadow_grade_trim_red, trims[1][0])
            || !near(exposure.shadow_grade_trim_green, trims[1][1])
            || !near(exposure.shadow_grade_trim_blue, trims[1][2])
            || !near(exposure.highlight_grade_trim_red, trims[2][0])
            || !near(exposure.highlight_grade_trim_green, trims[2][1])
            || !near(exposure.highlight_grade_trim_blue, trims[2][2])
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
            || !near(exposure.dye_separation_trim_red, dye_red)
            || !near(exposure.dye_separation_trim_green, dye_green)
            || !near(exposure.dye_separation_trim_blue, dye_blue)
            || !near(exposure.separation_damping, damping)
            || !near(exposure.toe, toe)
            || !near(exposure.toe_width, toe_width)
            || !near(exposure.shoulder, shoulder)
            || !near(exposure.shoulder_width, shoulder_width);
        exposure.bw_filter = filter;
        match region {
            1 => {
                exposure.shadow_cyan = cyan;
                exposure.shadow_magenta = magenta;
                exposure.shadow_yellow = region_yellow[1];
            }
            2 => {
                exposure.highlight_cyan = cyan;
                exposure.highlight_magenta = magenta;
                exposure.highlight_yellow = region_yellow[2];
            }
            _ => {
                exposure.wb_cyan = cyan;
                exposure.wb_magenta = magenta;
                exposure.wb_yellow = region_yellow[0];
            }
        }
        exposure.cast_removal_strength = cast;
        exposure.density = density;
        exposure.grade = grade;
        [
            exposure.grade_trim_red,
            exposure.grade_trim_green,
            exposure.grade_trim_blue,
        ] = trims[0];
        [
            exposure.shadow_grade_trim_red,
            exposure.shadow_grade_trim_green,
            exposure.shadow_grade_trim_blue,
        ] = trims[1];
        [
            exposure.highlight_grade_trim_red,
            exposure.highlight_grade_trim_green,
            exposure.highlight_grade_trim_blue,
        ] = trims[2];
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
        exposure.dye_separation_trim_red = dye_red;
        exposure.dye_separation_trim_green = dye_green;
        exposure.dye_separation_trim_blue = dye_blue;
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
        if changed && self.clear_masks {
            edit.config.local.masks.clear();
            self.mask_count = 0;
            self.clear_masks = false;
            changed = true;
        }
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
        let previous_region = self.filtration.region.current;
        changed |= self.filtration.region.show(ui).is_some();
        if self.filtration.region.current != previous_region {
            let region = self.filtration.region.current.min(2);
            self.filtration
                .magenta
                .set_value(self.filtration.region_magenta[region]);
            self.filtration
                .yellow
                .set_value(self.filtration.region_yellow[region]);
            self.filtration.temperature.set_value(wb_to_kelvin(
                self.filtration.region_magenta[region],
                self.filtration.region_yellow[region],
            ));
            self.temperature_dirty = false;
        }
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
        let previous_channel = self.tone.channel.current;
        let mut changed = self.tone.channel.show(ui).is_some();
        if self.tone.channel.current != previous_channel {
            let channel = self.tone.channel.current.saturating_sub(1).min(2);
            if channel > 0 {
                self.tone
                    .channel_grade
                    .set_value(self.tone.channel_trims[0][channel - 1]);
                self.tone
                    .shadows_grade
                    .set_value(self.tone.channel_trims[1][channel - 1]);
                self.tone
                    .highlights_grade
                    .set_value(self.tone.channel_trims[2][channel - 1]);
            }
        }
        ui.add_space(f32::from(theme.space_sm));
        changed |= self.tone.print_density.show(ui, "Print Density").1;
        if self.tone.channel.current == 0 {
            changed |= self.tone.grade.show(ui, "ISO-R Grade").1;
        } else {
            changed |= self.tone.channel_grade.show(ui, "Grade Trim").1;
        }
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
        let arming_trims = self.paper.dye_separation_trim_red.value != 0.0
            || self.paper.dye_separation_trim_green.value != 0.0
            || self.paper.dye_separation_trim_blue.value != 0.0;
        if !self.is_bw {
            changed |= self.paper.dye_separation.show(ui, "Dye Separation").1;
            let armed = arming_trims || (self.paper.dye_separation.value - 1.0).abs() > 1e-9;
            if armed {
                changed |= self.paper.dye_separation_trim_red.show(ui, "Red Trim").1;
                changed |= self
                    .paper
                    .dye_separation_trim_green
                    .show(ui, "Green Trim")
                    .1;
                changed |= self.paper.dye_separation_trim_blue.show(ui, "Blue Trim").1;
            }
            changed |= self
                .paper
                .separation_damping
                .show(ui, "Separation Damping")
                .1;
        }
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
        let _ = section_subheader(ui, &format!("MASKS · {}", self.mask_count));
        changed |= self.local.burn.show(ui, "Burn").1;
        changed |= self.local.grade.show(ui, "Grade").1;
        changed |= self.local.flash.show(ui, "Flash").1;
        changed |= self.local.feather.show(ui, "Feather").1;
        let _ = section_subheader(ui, "Tone Limit");
        changed |= self.local.tone_zone.show(ui, "Tone Zone").1;
        changed |= self.local.tone_softness.show(ui, "Tone Softness").1;
        if labeled_action(ui, "Clear All", ActionRank::Normal).clicked() {
            self.clear_masks = true;
        }
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
    fn the_local_card_counts_the_stored_masks() {
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        edit.config
            .local
            .masks
            .push(negpy_backend::domain::models::LocalMask::default());
        state.load(&edit);
        assert_eq!(state.mask_count, 1);
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
    fn each_region_writes_its_own_white_balance_pair() {
        let mut edit = FrameEdit::default();
        edit.config.exposure.wb_cyan = 0.11;
        edit.config.exposure.shadow_cyan = 0.22;
        edit.config.exposure.highlight_cyan = 0.33;
        edit.config.exposure.wb_magenta = 0.07;
        edit.config.exposure.shadow_magenta = 0.14;
        edit.config.exposure.highlight_magenta = 0.21;
        let mut state = ExposureState::default();
        for region in 0..3 {
            state.region = region;
            state.load(&edit);
            assert!((state.filtration.cyan.value - [0.11, 0.22, 0.33][region]).abs() < 1e-9);
            assert!((state.filtration.magenta.value - [0.07, 0.14, 0.21][region]).abs() < 1e-9);
        }
    }

    #[test]
    fn each_channel_writes_its_grade_trims() {
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.tone.channel_trims = [[3.0, 4.0, 5.0], [4.0, 5.0, 6.0], [5.0, 6.0, 7.0]];
        state.tone.channel.current = 0;
        state.store(&mut edit);
        let exposure = &edit.config.exposure;
        assert_eq!(
            [
                exposure.grade_trim_red,
                exposure.grade_trim_green,
                exposure.grade_trim_blue,
            ],
            [3.0, 4.0, 5.0]
        );
        assert_eq!(
            [
                exposure.shadow_grade_trim_red,
                exposure.shadow_grade_trim_green,
                exposure.shadow_grade_trim_blue,
            ],
            [4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn the_damping_rail_is_armed_by_a_dye_separation_trim() {
        let mut state = ExposureState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        assert!((state.paper.dye_separation.value - 1.0).abs() < 1e-9);
        state.paper.dye_separation_trim_red.set_value(0.1);
        state.store(&mut edit);
        assert!((edit.config.exposure.dye_separation_trim_red - 0.1).abs() < 1e-9);
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

#[test]
fn the_region_and_channel_choices_survive_a_reload() {
    let mut state = ExposureState::default();
    let mut edit = FrameEdit::default();
    state.load(&edit);
    state.filtration.region.current = 2;
    state.tone.channel.current = 3;
    state.store(&mut edit);
    state.load(&edit);
    assert_eq!(state.filtration.region.current, 2);
    assert_eq!(state.tone.channel.current, 3);
}
