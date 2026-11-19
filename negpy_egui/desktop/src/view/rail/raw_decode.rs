//! The Roll tab's Raw Decode card: white balance, demosaic and highlight recovery.

use eframe::egui::Ui;

use negpy_backend::domain::models::process::DemosaicMode;
use negpy_backend::session::settings_catalog::HIGHLIGHT_LEVELS;

use crate::edit::FrameEdit;
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

const DEMOSAIC_MODES: [DemosaicMode; 8] = [
    DemosaicMode::Auto,
    DemosaicMode::Linear,
    DemosaicMode::Vng,
    DemosaicMode::Ppg,
    DemosaicMode::Ahd,
    DemosaicMode::Dcb,
    DemosaicMode::Dht,
    DemosaicMode::Aahd,
];

fn demosaic_index(mode: DemosaicMode) -> usize {
    DEMOSAIC_MODES
        .iter()
        .position(|candidate| *candidate == mode)
        .unwrap_or(0)
}

fn highlight_index(level: i64) -> usize {
    HIGHLIGHT_LEVELS
        .iter()
        .position(|(value, _)| *value == level)
        .unwrap_or(0)
}

fn demosaic_combo() -> StepCombo {
    StepCombo::new(
        DEMOSAIC_MODES
            .into_iter()
            .map(|mode| Choice::new(mode.as_str()))
            .collect(),
    )
}

impl RawDecodeState {
    /// Replace the controls with the values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let process = &edit.config.process;
        self.white_balance.current = usize::from(process.linear_raw);
        self.preview.current = demosaic_index(process.demosaic_preview);
        self.export.current = demosaic_index(process.demosaic_export);
        self.highlights.current = highlight_index(process.highlight_reconstruction);
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&self, edit: &mut FrameEdit) -> bool {
        let process = &mut edit.config.process;
        let linear_raw = self.white_balance.current == 1;
        let preview = DEMOSAIC_MODES[self.preview.current.min(DEMOSAIC_MODES.len() - 1)];
        let export = DEMOSAIC_MODES[self.export.current.min(DEMOSAIC_MODES.len() - 1)];
        let highlights =
            HIGHLIGHT_LEVELS[self.highlights.current.min(HIGHLIGHT_LEVELS.len() - 1)].0;
        let changed = process.linear_raw != linear_raw
            || process.demosaic_preview != preview
            || process.demosaic_export != export
            || process.highlight_reconstruction != highlights;
        process.linear_raw = linear_raw;
        process.demosaic_preview = preview;
        process.demosaic_export = export;
        process.highlight_reconstruction = highlights;
        changed
    }

    /// Paint the card. Returns whether a control changed the edit.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut section = CollapsibleSection::new("Raw Decode")
            .with_guide()
            .with_icon(crate::styles::icons::Icon::Grid);
        section.expanded = self.open;
        let (_, result) = section.show(ui, |ui| card(ui, |ui| self.show_body(ui)));
        self.open = section.expanded;
        let changed = result.unwrap_or(false);
        if changed {
            self.store(edit);
        }
        changed
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
        let mut edit = FrameEdit::default();
        let _ = ctx.run(eframe::egui::RawInput::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_loaded_edit_fills_the_card_and_stores_the_same_values() {
        let mut edit = FrameEdit::default();
        edit.config.process.linear_raw = true;
        edit.config.process.demosaic_preview = DemosaicMode::Dcb;
        edit.config.process.demosaic_export = DemosaicMode::Aahd;
        edit.config.process.highlight_reconstruction = 5;
        let mut state = RawDecodeState::default();
        state.load(&edit);
        assert_eq!(state.white_balance.current_label(), "Linear RAW");
        assert_eq!(state.preview.current_label(), "DCB");
        assert_eq!(state.export.current_label(), "AAHD");
        assert_eq!(state.highlights.current_label(), "Reconstruct");
        assert!(!state.store(&mut edit));
    }
}
