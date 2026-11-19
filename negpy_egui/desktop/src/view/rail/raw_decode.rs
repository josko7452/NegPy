//! The Roll tab's Raw Decode card: white balance, demosaic and highlight recovery.

use eframe::egui::Ui;

use negpy_backend::domain::models::process::DemosaicMode;
use negpy_backend::session::settings_catalog::HIGHLIGHT_LEVELS;

use crate::styles::templates::section_subheader;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::step_combo::{Choice, StepCombo};

/// The Raw Decode card's controls.
#[derive(Clone, Debug, PartialEq)]
pub struct RawDecodeState {
    pub white_balance: SegmentedChoice<'static>,
    pub preview: StepCombo,
    pub export: StepCombo,
    pub highlights: StepCombo,
    pub open: bool,
}

impl Default for RawDecodeState {
    fn default() -> Self {
        Self {
            white_balance: SegmentedChoice::new(&["Camera WB", "Linear RAW"]),
            preview: demosaic_combo(),
            export: demosaic_combo(),
            highlights: StepCombo::new(
                HIGHLIGHT_LEVELS
                    .iter()
                    .map(|(_, label)| Choice::new(*label))
                    .collect(),
            ),
            open: false,
        }
    }
}

fn demosaic_combo() -> StepCombo {
    StepCombo::new(
        [
            DemosaicMode::Auto,
            DemosaicMode::Linear,
            DemosaicMode::Vng,
            DemosaicMode::Ppg,
            DemosaicMode::Ahd,
            DemosaicMode::Dcb,
            DemosaicMode::Dht,
            DemosaicMode::Aahd,
        ]
        .into_iter()
        .map(|mode| Choice::new(mode.as_str()))
        .collect(),
    )
}

impl RawDecodeState {
    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut section = CollapsibleSection::new("Raw Decode")
            .with_guide()
            .with_icon(crate::styles::icons::Icon::Grid);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        result.unwrap_or(false)
    }

    fn show_body(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        let _ = section_subheader(ui, "White Balance");
        changed |= self.white_balance.show(ui).is_some();
        let _ = hint(
            ui,
            "Camera WB uses the camera white balance; Linear RAW decodes neutral, for narrowband light.",
        );
        let _ = section_subheader(ui, "DEMOSAIC");
        ui.horizontal(|ui| {
            ui.label("Preview");
            changed |= self.preview.show(ui);
        });
        ui.horizontal(|ui| {
            ui.label("Export");
            changed |= self.export.show(ui);
        });
        let _ = section_subheader(ui, "HIGHLIGHTS");
        ui.horizontal(|ui| {
            ui.label("Recovery");
            changed |= self.highlights.show(ui);
        });
        let _ = hint(ui, "Not applied to a merged bracket.");
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demosaic_list_keeps_the_auto_choice_first() {
        let state = RawDecodeState::default();
        assert_eq!(state.preview.current_label(), "Auto");
        assert_eq!(state.export.current_label(), "Auto");
    }

    #[test]
    fn the_highlight_list_uses_the_catalog_labels() {
        let state = RawDecodeState::default();
        let labels: Vec<&str> = state
            .highlights
            .choices
            .iter()
            .map(|choice| choice.label.as_str())
            .collect();
        assert_eq!(labels, ["Off", "Blend", "Reconstruct"]);
    }

    #[test]
    fn the_card_paints_on_a_context() {
        let ctx = eframe::egui::Context::default();
        let mut state = RawDecodeState::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
