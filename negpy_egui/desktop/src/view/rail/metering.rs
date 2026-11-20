//! The Roll tab's Metering card: the frame analysis and its fine trims.

use eframe::egui::Ui;

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::templates::{
    ActionRank, action_with_icon, labeled_action, labeled_toggle, section_subheader,
};
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::fold::Fold;
use crate::view::widgets::segmented_choice::SegmentedChoice;

/// The Metering card's controls.
#[derive(Clone, Debug, PartialEq)]
pub struct MeteringState {
    pub analysis_buffer: CompactSlider,
    pub lock_bounds: bool,
    pub draw_region: bool,
    pub luma_clip: CompactSlider,
    pub color_clip: CompactSlider,
    pub channel: SegmentedChoice<'static>,
    pub white_point: CompactSlider,
    pub black_point: CompactSlider,
    pub picks: Fold,
    pub points: Fold,
    pub open: bool,
    pub measure_alone: bool,
    pub pick: MeteringPick,
    pub reset_requested: bool,
}

/// The manual bound the next canvas click sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeteringPick {
    #[default]
    Off,
    Dmin,
    Dmax,
}

impl Default for MeteringState {
    fn default() -> Self {
        Self {
            analysis_buffer: CompactSlider::new(
                SliderSpec::new(0.0, 0.25, 0.01, 2).with_default(0.05),
            ),
            lock_bounds: false,
            draw_region: false,
            luma_clip: CompactSlider::new(SliderSpec::new(-100.0, 100.0, 1.0, 1).with_default(0.0)),
            color_clip: CompactSlider::new(
                SliderSpec::new(-100.0, 100.0, 1.0, 1).with_default(0.0),
            ),
            channel: SegmentedChoice::new(&["Global", "R", "G", "B"]),
            white_point: CompactSlider::new(
                SliderSpec::new(-0.25, 0.25, 0.01, 2).with_default(0.0),
            ),
            black_point: CompactSlider::new(
                SliderSpec::new(-0.25, 0.25, 0.01, 2).with_default(0.0),
            ),
            picks: Fold::new("metering_picks"),
            points: Fold::new("metering_points"),
            open: false,
            measure_alone: false,
            pick: MeteringPick::Off,
            reset_requested: false,
        }
    }
}

impl MeteringState {
    /// Replace the controls with the metering values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let process = &edit.config.process;
        self.analysis_buffer.set_value(process.analysis_buffer);
        self.lock_bounds = process.lock_bounds;
        self.draw_region = process.analysis_rect.is_some();
        self.luma_clip.set_value(process.luma_range_clip);
        self.color_clip.set_value(process.color_range_clip);
        self.white_point.set_value(process.white_point_offset);
        self.black_point.set_value(process.black_point_offset);
        self.measure_alone = !process.use_luma_average && !process.use_color_average;
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&self, edit: &mut FrameEdit) -> bool {
        let process = &mut edit.config.process;
        let draw_region = self.draw_region;
        let changed = !same(process.analysis_buffer, self.analysis_buffer.value)
            || process.lock_bounds != self.lock_bounds
            || process.analysis_rect.is_some() != draw_region
            || !same(process.luma_range_clip, self.luma_clip.value)
            || !same(process.color_range_clip, self.color_clip.value)
            || !same(process.white_point_offset, self.white_point.value)
            || !same(process.black_point_offset, self.black_point.value)
            || process.use_luma_average == self.measure_alone
            || process.use_color_average == self.measure_alone;
        process.analysis_buffer = self.analysis_buffer.value;
        process.lock_bounds = self.lock_bounds;
        if !draw_region {
            process.analysis_rect = None;
        }
        process.luma_range_clip = self.luma_clip.value;
        process.color_range_clip = self.color_clip.value;
        process.white_point_offset = self.white_point.value;
        process.black_point_offset = self.black_point.value;
        process.use_luma_average = !self.measure_alone;
        process.use_color_average = !self.measure_alone;
        changed
    }

    /// Reset every metering trim to the defaults of a fresh frame.
    pub fn reset(&mut self, edit: &mut FrameEdit) {
        let process = &mut edit.config.process;
        process.analysis_buffer = 0.05;
        process.lock_bounds = false;
        process.analysis_rect = None;
        process.luma_range_clip = 0.0;
        process.color_range_clip = 0.0;
        process.white_point_offset = 0.0;
        process.black_point_offset = 0.0;
        process.manual_dmin = None;
        process.manual_dmax = None;
        process.manual_dmin_at = None;
        process.manual_dmax_at = None;
        process.use_luma_average = true;
        process.use_color_average = true;
        self.load(edit);
        edit.mark_dirty();
    }

    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut section = CollapsibleSection::new("Metering")
            .with_guide()
            .with_icon(Icon::Sliders);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        let mut changed = result.unwrap_or(false);
        if self.reset_requested {
            self.reset_requested = false;
            self.reset(edit);
            changed = true;
        }
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_body(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            let _ = section_subheader(ui, "ANALYSIS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                changed |= labeled_toggle(ui, "Lock", &mut self.lock_bounds).clicked();
            });
        });
        ui.horizontal(|ui| {
            if action_with_icon(
                ui,
                Icon::Reset,
                "Measure Alone",
                ActionRank::Normal,
                "Take this frame off the roll average and measure it on its own",
            )
            .clicked()
            {
                self.measure_alone = !self.measure_alone;
                changed = true;
            }
            changed |= labeled_toggle(ui, "Draw Region", &mut self.draw_region).clicked();
        });
        changed |= self.analysis_buffer.show(ui, "Analysis Buffer").1;
        changed |= self.luma_clip.show(ui, "Luma Range Clip").1;
        changed |= self.color_clip.show(ui, "Color Clip").1;
        let _ = self.picks.group_header(ui, "Pick by Hand");
        if self.picks.open {
            ui.horizontal(|ui| {
                let mut dmin = self.pick == MeteringPick::Dmin;
                let mut dmax = self.pick == MeteringPick::Dmax;
                if labeled_toggle(ui, "Dmin", &mut dmin).clicked() {
                    self.pick = if self.pick == MeteringPick::Dmin {
                        MeteringPick::Off
                    } else {
                        MeteringPick::Dmin
                    };
                    dmax = false;
                    changed = true;
                }
                if labeled_toggle(ui, "Dmax", &mut dmax).clicked() {
                    self.pick = if self.pick == MeteringPick::Dmax {
                        MeteringPick::Off
                    } else {
                        MeteringPick::Dmax
                    };
                    changed = true;
                }
            });
            let _ = hint(
                ui,
                "Click the film base or the deepest shadow, then the brightest highlight.",
            );
        }
        let _ = self.points.group_header(ui, "White / Black Point");
        if self.points.open {
            changed |= self.channel.show(ui).is_some();
            changed |= self.white_point.show(ui, "White Point").1;
            changed |= self.black_point.show(ui, "Black Point").1;
        }
        if labeled_action(ui, "Reset Metering", ActionRank::Normal).clicked() {
            self.reset_requested = true;
            changed = true;
        }
        changed
    }
}

fn same(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_metering_defaults_match_the_backend() {
        let state = MeteringState::default();
        assert_eq!(state.analysis_buffer.value, 0.05);
        assert_eq!(state.luma_clip.value, 0.0);
        assert_eq!(state.color_clip.value, 0.0);
    }

    #[test]
    fn the_channel_choice_starts_global() {
        assert_eq!(MeteringState::default().channel.current_label(), "Global");
    }

    #[test]
    fn the_folds_start_closed() {
        let state = MeteringState::default();
        assert!(!state.picks.open);
        assert!(!state.points.open);
    }

    #[test]
    fn the_card_paints_on_a_context() {
        let ctx = eframe::egui::Context::default();
        let mut state = MeteringState::default();
        let mut edit = FrameEdit::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_metering_card_and_stores_it() {
        let mut edit = FrameEdit::default();
        edit.config.process.analysis_buffer = 0.12;
        edit.config.process.lock_bounds = true;
        edit.config.process.luma_range_clip = -12.0;
        edit.config.process.color_range_clip = 8.0;
        edit.config.process.white_point_offset = 0.03;
        edit.config.process.black_point_offset = -0.02;
        let mut state = MeteringState::default();
        state.load(&edit);
        assert!(state.lock_bounds);
        assert!((state.analysis_buffer.value - 0.12).abs() < 1e-9);
        assert_eq!(state.luma_clip.value, -12.0);
        assert_eq!(state.color_clip.value, 8.0);
        assert!((state.white_point.value - 0.03).abs() < 1e-9);
        assert!((state.black_point.value + 0.02).abs() < 1e-9);
        assert!(!state.store(&mut edit));
    }

    #[test]
    fn measure_alone_turns_off_both_roll_averages() {
        let mut edit = FrameEdit::default();
        edit.config.process.use_luma_average = true;
        edit.config.process.use_color_average = true;
        let mut state = MeteringState::default();
        state.load(&edit);
        state.measure_alone = true;
        assert!(state.store(&mut edit));
        assert!(!edit.config.process.use_luma_average);
        assert!(!edit.config.process.use_color_average);
    }

    #[test]
    fn reset_metering_clears_the_trims_and_manual_bounds() {
        let mut edit = FrameEdit::default();
        edit.config.process.white_point_offset = 0.1;
        edit.config.process.black_point_offset = -0.1;
        edit.config.process.manual_dmin = Some(vec![0.0; 3]);
        edit.config.process.manual_dmax = Some(vec![1.0; 3]);
        edit.config.process.lock_bounds = true;
        let mut state = MeteringState::default();
        state.reset(&mut edit);
        assert_eq!(edit.config.process.white_point_offset, 0.0);
        assert_eq!(edit.config.process.black_point_offset, 0.0);
        assert!(edit.config.process.manual_dmin.is_none());
        assert!(edit.config.process.manual_dmax.is_none());
        assert!(!edit.config.process.lock_bounds);
    }
}
