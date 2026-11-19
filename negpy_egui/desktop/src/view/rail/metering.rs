//! The Roll tab's Metering card: the frame analysis and its fine trims.

use eframe::egui::Ui;

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
        }
    }
}

impl MeteringState {
    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut section = CollapsibleSection::new("Metering")
            .with_guide()
            .with_icon(Icon::Sliders);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        result.unwrap_or(false)
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
            changed |= action_with_icon(
                ui,
                Icon::Reset,
                "Measure Alone",
                ActionRank::Normal,
                "Take this frame off the roll average and measure it on its own",
            )
            .clicked();
            changed |= labeled_toggle(ui, "Draw Region", &mut self.draw_region).clicked();
        });
        changed |= self.analysis_buffer.show(ui, "Analysis Buffer").1;
        changed |= self.luma_clip.show(ui, "Luma Range Clip").1;
        changed |= self.color_clip.show(ui, "Color Clip").1;
        let _ = self.picks.group_header(ui, "Pick by Hand");
        if self.picks.open {
            ui.horizontal(|ui| {
                changed |= labeled_toggle(ui, "Dmin", &mut false).clicked();
                changed |= labeled_toggle(ui, "Dmax", &mut false).clicked();
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
        let _ = labeled_action(ui, "Reset Metering", ActionRank::Normal);
        changed
    }
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
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
