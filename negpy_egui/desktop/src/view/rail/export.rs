//! The Export tab: the output intent, the file form, the proof and the export action.

use eframe::egui::Ui;

use crate::styles::icons::Icon;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::hint;
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::segmented_choice::SegmentedChoice;
use crate::view::widgets::split_action::split_action;
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::sub_tabs::SUB_TAB_HEIGHT;
use crate::view::widgets::tab_header::TabSummary;

/// The Export tab's sub-tabs.
#[must_use]
pub fn export_tabs() -> [PanelTab; 4] {
    [
        PanelTab::beside(Icon::WhitePaper, "File").with_divider(),
        PanelTab::beside(Icon::Print, "Proof"),
        PanelTab::beside(Icon::Stack, "Presets").icon_only(),
        PanelTab::beside(Icon::Export, "Alongside Exports").icon_only(),
    ]
}

/// The Export tab's state.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportState {
    pub page: usize,
    pub intent: SegmentedChoice<'static>,
    pub format: StepCombo,
    pub jpeg_quality: CompactSlider,
    pub progressive: bool,
    pub resolution: SegmentedChoice<'static>,
    pub long_edge: CompactSlider,
    pub paper_ratio: StepCombo,
    pub input_icc: StepCombo,
    pub export_profile: StepCombo,
    pub destination: StepCombo,
    pub subfolder: String,
    pub filename: String,
    pub overwrite: bool,
    pub soft_proof: bool,
    pub proof_profile: StepCombo,
    pub presets: Vec<String>,
    pub contact_sheet: bool,
    pub alongside: bool,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            page: 0,
            intent: SegmentedChoice::new(&["Print", "Flat", "Linear"]),
            format: StepCombo::new(vec![
                Choice::new("JPEG"),
                Choice::new("TIFF"),
                Choice::new("PNG"),
                Choice::new("WebP"),
            ]),
            jpeg_quality: CompactSlider::new(
                SliderSpec::new(1.0, 100.0, 1.0, 0)
                    .with_default(90.0)
                    .with_unit(""),
            ),
            progressive: true,
            resolution: SegmentedChoice::new(&["Original", "Print", "Pixels"]),
            long_edge: CompactSlider::new(
                SliderSpec::new(1000.0, 12000.0, 10.0, 0)
                    .with_default(5500.0)
                    .with_unit(" px"),
            ),
            paper_ratio: StepCombo::new(vec![
                Choice::new("Original"),
                Choice::new("3:2"),
                Choice::new("4:5"),
            ]),
            input_icc: StepCombo::new(vec![Choice::new("None"), Choice::new("Adobe RGB")]),
            export_profile: StepCombo::new(vec![Choice::new("sRGB"), Choice::new("Adobe RGB")]),
            destination: StepCombo::new(vec![
                Choice::new("Subfolder of source"),
                Choice::new("Fixed folder"),
            ]),
            subfolder: "export".to_string(),
            filename: "{{ original_name }}".to_string(),
            overwrite: false,
            soft_proof: false,
            proof_profile: StepCombo::new(vec![
                Choice::new("Use Export profile"),
                Choice::new("Printer profile"),
            ]),
            presets: vec!["No presets yet".to_string()],
            contact_sheet: true,
            alongside: false,
        }
    }
}

impl ExportState {
    /// Select a sub-tab by its manifest key.
    pub fn show_subtab(&mut self, key: &str) {
        self.page = match key {
            "proof" => 1,
            "presets" => 2,
            "alongside" => 3,
            _ => 0,
        };
    }

    /// Paint the tab. Returns whether the edit changed.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let tabs = export_tabs();
        let mut page = self.page;
        if let Some(picked) = panel_tab_strip(ui, &tabs, page, SUB_TAB_HEIGHT) {
            page = picked;
            self.page = picked;
        }
        ui.add_space(f32::from(Theme::default().space_sm));
        let changed = match page {
            0 => self.show_file(ui),
            1 => self.show_proof(ui),
            2 => self.show_presets(ui),
            3 => self.show_alongside(ui),
            _ => false,
        };
        ui.add_space(f32::from(Theme::default().space_md));
        let _ = split_action(ui, Icon::Export, "Export Roll", ActionRank::Primary);
        changed
    }

    fn show_file(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        let _ = section_subheader(ui, "OUTPUT INTENT");
        changed |= self.intent.show(ui).is_some();
        let _ = section_subheader(ui, "FORMAT");
        field_combo(ui, "Format", &mut self.format, &mut changed);
        changed |= self.jpeg_quality.show(ui, "JPEG Quality").1;
        changed |= labeled_toggle(ui, "Progressive", &mut self.progressive).clicked();
        let _ = section_subheader(ui, "SIZE");
        changed |= self.resolution.show(ui).is_some();
        changed |= self.long_edge.show(ui, "Long edge").1;
        field_combo(ui, "Paper ratio", &mut self.paper_ratio, &mut changed);
        let _ = section_subheader(ui, "COLOR MANAGEMENT");
        field_combo(ui, "Input ICC", &mut self.input_icc, &mut changed);
        field_combo(ui, "Export profile", &mut self.export_profile, &mut changed);
        let _ = section_subheader(ui, "DESTINATION");
        field_combo(ui, "Folder", &mut self.destination, &mut changed);
        ui.horizontal(|ui| {
            ui.label("Subfolder");
            let _ = ui.text_edit_singleline(&mut self.subfolder);
        });
        ui.horizontal(|ui| {
            ui.label("Filename");
            let _ = ui.text_edit_singleline(&mut self.filename);
        });
        changed |= labeled_toggle(ui, "Overwrite Existing Files", &mut self.overwrite).clicked();
        ui.add_space(f32::from(theme.space_sm));
        changed
    }

    fn show_proof(&mut self, ui: &mut Ui) -> bool {
        let mut changed = labeled_toggle(ui, "Soft Proof", &mut self.soft_proof).clicked();
        ui.horizontal(|ui| {
            ui.label("Proof profile");
            changed |= self.proof_profile.show(ui);
        });
        let _ = hint(ui, "The printer or paper profile to preview.");
        changed
    }

    fn show_presets(&mut self, ui: &mut Ui) -> bool {
        let _ = TabSummary::new(0)
            .without_apply()
            .titled("Export Presets")
            .show(ui);
        for preset in &self.presets {
            let _ = hint(ui, preset);
        }
        let _ = labeled_action(ui, "Save Preset...", ActionRank::Normal);
        false
    }

    fn show_alongside(&mut self, ui: &mut Ui) -> bool {
        let mut changed = labeled_toggle(ui, "Contact Sheet", &mut self.contact_sheet).clicked();
        changed |= labeled_toggle(ui, "Alongside Exports", &mut self.alongside).clicked();
        let _ = hint(ui, "Save a sheet and the matching files together.");
        changed
    }
}

fn field_combo(ui: &mut Ui, label: &str, combo: &mut StepCombo, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        *changed |= combo.show(ui);
    });
}

/// A fixed-size preview of the export form, used by the capture test.
#[must_use]
pub fn export_summary(state: &ExportState) -> String {
    format!(
        "{} · {} · {}",
        state.intent.current_label(),
        state.format.current_label(),
        state.resolution.current_label()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_export_tab_opens_on_the_file_page() {
        let state = ExportState::default();
        assert_eq!(state.page, 0);
        assert_eq!(state.intent.current_label(), "Print");
        assert_eq!(state.format.current_label(), "JPEG");
        assert_eq!(state.resolution.current_label(), "Original");
        assert_eq!(export_summary(&state), "Print · JPEG · Original");
    }

    #[test]
    fn the_export_tab_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ExportState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
