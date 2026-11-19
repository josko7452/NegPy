//! The Export tab: the output intent, the file form, the proof and the export action.

use eframe::egui::Ui;
use negpy_backend::domain::models::{
    EXPORT_COLOR_SPACES, ExportConfig, ExportFormat, ExportPresetOutputMode, ExportResolutionMode,
};
use negpy_backend::session::proof::ProofSettings;

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{card, hint};
use crate::view::widgets::compact_slider::{CompactSlider, SliderSpec};
use crate::view::widgets::field_row::{RowColumns, field_row};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::segmented_choice::SegmentedChoice;
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
    pub proof: ProofSettings,
    pub presets: Vec<negpy_backend::domain::models::ExportPreset>,
    pub contact_sheet: bool,
    pub alongside: bool,
    pub sidecars: bool,
    pub sidecar_path: String,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            page: 0,
            intent: SegmentedChoice::new(&["Print", "Flat"]),
            format: choice_combo(&[
                (ExportFormat::Jpeg.as_str(), ExportFormat::Jpeg.as_str()),
                (ExportFormat::Tiff.as_str(), ExportFormat::Tiff.as_str()),
                (ExportFormat::Png.as_str(), ExportFormat::Png.as_str()),
                (ExportFormat::Jxl.as_str(), ExportFormat::Jxl.as_str()),
                (ExportFormat::Webp.as_str(), ExportFormat::Webp.as_str()),
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
                    .with_default(2000.0)
                    .with_unit(" px"),
            ),
            paper_ratio: choice_combo(&[
                ("Original", "Original"),
                ("3:2", "3:2"),
                ("5:4", "5:4"),
                ("4:5", "4:5"),
                ("1:1", "1:1"),
            ]),
            input_icc: StepCombo::new(vec![Choice::new("None")]),
            export_profile: choice_combo(
                &EXPORT_COLOR_SPACES
                    .iter()
                    .map(|space| (*space, *space))
                    .collect::<Vec<_>>(),
            ),
            destination: choice_combo(&[
                ("Subfolder of source", "subfolder_of_source"),
                ("Same as source", "same_as_source"),
                ("Fixed folder", "absolute"),
            ]),
            subfolder: "export".to_string(),
            filename: "{{ original_name }}".to_string(),
            overwrite: false,
            proof: ProofSettings::default(),
            presets: Vec::new(),
            contact_sheet: true,
            alongside: false,
            sidecars: false,
            sidecar_path: String::new(),
        }
    }
}

impl ExportState {
    /// Replace the controls with the export values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let export = &edit.config.export;
        self.intent.current = usize::from(
            edit.config.exposure.render_intent == negpy_backend::domain::models::RenderIntent::Flat,
        );
        self.format.pick(export.export_fmt.as_str());
        self.jpeg_quality.set_value(export.jpeg_quality as f64);
        self.progressive = export.jpeg_progressive;
        self.resolution.current = match export.export_resolution_mode {
            ExportResolutionMode::Original => 0,
            ExportResolutionMode::Print => 1,
            ExportResolutionMode::TargetPx => 2,
        };
        self.long_edge
            .set_value(export.export_target_long_edge_px as f64);
        self.paper_ratio.pick(&export.paper_aspect_ratio);
        self.export_profile.pick(&export.export_color_space);
        self.destination.pick(export.output_mode.as_str());
        self.subfolder = export.output_subfolder.clone();
        self.filename = export.filename_pattern.clone();
        self.overwrite = export.overwrite;
        self.sidecars = export.export_sidecars_enabled;
        self.sidecar_path = export.contact_sheet_output_path.clone();
    }

    /// Show the export presets the profile holds.
    pub fn set_presets(&mut self, presets: Vec<negpy_backend::domain::models::ExportPreset>) {
        self.presets = presets;
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        let export = &mut edit.config.export;
        let next = ExportConfig {
            export_fmt: ExportFormat::coerce(self.format.current_value()),
            jpeg_quality: self.jpeg_quality.value.round() as i64,
            jpeg_progressive: self.progressive,
            export_resolution_mode: match self.resolution.current {
                1 => ExportResolutionMode::Print,
                2 => ExportResolutionMode::TargetPx,
                _ => ExportResolutionMode::Original,
            },
            export_target_long_edge_px: self.long_edge.value.round() as i64,
            paper_aspect_ratio: self.paper_ratio.current_value().to_string(),
            export_color_space: self.export_profile.current_value().to_string(),
            output_mode: ExportPresetOutputMode::coerce(self.destination.current_value()),
            output_subfolder: self.subfolder.clone(),
            filename_pattern: self.filename.clone(),
            overwrite: self.overwrite,
            export_sidecars_enabled: self.sidecars,
            contact_sheet_output_path: self.sidecar_path.clone(),
            ..export.clone()
        };
        edit.config.exposure.render_intent = if self.intent.current == 1 {
            negpy_backend::domain::models::RenderIntent::Flat
        } else {
            negpy_backend::domain::models::RenderIntent::Print
        };
        let changed = *export != next;
        *export = next;
        changed
    }

    /// Select a sub-tab by its manifest key.
    pub fn show_subtab(&mut self, key: &str) {
        self.page = match key {
            "proof" => 1,
            "presets" => 2,
            "alongside" => 3,
            _ => 0,
        };
    }

    /// Paint the tab. Returns whether the edit or the session proof changed.
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
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
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_file(&mut self, ui: &mut Ui) -> bool {
        let theme = Theme::default();
        let mut changed = false;
        let columns = RowColumns::new();
        let _ = card(ui, |ui| {
            let _ = section_subheader(ui, "Output Intent");
            changed |= self.intent.show(ui).is_some();
            ui.add_space(f32::from(theme.space_lg));
            let _ = section_subheader(ui, "Format");
            field_combo(ui, columns, "Format", &mut self.format, &mut changed);
            ui.add_space(f32::from(theme.space_md));
            changed |= self.jpeg_quality.show(ui, "JPEG Quality").1;
            changed |= labeled_toggle(ui, "Progressive", &mut self.progressive).clicked();
            ui.add_space(f32::from(theme.space_lg));
            let _ = section_subheader(ui, "Size");
            ui.add_space(f32::from(theme.space_md));
            field_row(ui, columns, "Resolution", |ui| {
                changed |= self.resolution.show(ui).is_some();
            });
            ui.add_space(f32::from(theme.space_md));
            changed |= self.long_edge.show(ui, "Long edge").1;
            ui.add_space(f32::from(theme.space_md));
            field_combo(
                ui,
                columns,
                "Paper ratio",
                &mut self.paper_ratio,
                &mut changed,
            );
            ui.add_space(f32::from(theme.space_lg));
            let _ = section_subheader(ui, "Color Management");
            ui.add_space(f32::from(theme.space_md));
            field_combo(ui, columns, "Input ICC", &mut self.input_icc, &mut changed);
            ui.add_space(f32::from(theme.space_md));
            field_combo(
                ui,
                columns,
                "Export profile",
                &mut self.export_profile,
                &mut changed,
            );
            ui.add_space(f32::from(theme.space_lg));
            let _ = section_subheader(ui, "Destination");
            ui.add_space(f32::from(theme.space_md));
            field_combo(ui, columns, "Folder", &mut self.destination, &mut changed);
            ui.add_space(f32::from(theme.space_md));
            entry_row(ui, columns, "Subfolder", &mut self.subfolder);
            ui.add_space(f32::from(theme.space_md));
            entry_row(ui, columns, "Filename", &mut self.filename);
            ui.add_space(f32::from(theme.space_md));
            changed |=
                labeled_toggle(ui, "Overwrite Existing Files", &mut self.overwrite).clicked();
        });
        changed
    }

    fn show_proof(&mut self, ui: &mut Ui) -> bool {
        let proof = &mut self.proof;
        let mut changed = labeled_toggle(ui, "Soft Proof", &mut proof.enabled).clicked();
        ui.horizontal(|ui| {
            ui.label("Proof profile");
            let mut intent = choice_combo(&[
                ("Relative Colorimetric", "relative_colorimetric"),
                ("Perceptual", "perceptual"),
                ("Saturation", "saturation"),
            ]);
            intent.pick(&proof.intent);
            if intent.show(ui) {
                proof.intent = intent.current_value().to_string();
                changed = true;
            }
        });
        changed |= labeled_toggle(ui, "Black Point Compensation", &mut proof.black_point).clicked();
        changed |= labeled_toggle(ui, "Simulate Paper White", &mut proof.paper_white).clicked();
        changed |= labeled_toggle(ui, "Simulate Ink Black", &mut proof.ink_black).clicked();
        changed |= labeled_toggle(ui, "Gamut Warning", &mut proof.gamut_warning).clicked();
        let _ = hint(ui, "The printer or paper profile to preview.");
        changed
    }

    fn show_presets(&mut self, ui: &mut Ui) -> bool {
        let _ = TabSummary::new(0)
            .without_apply()
            .titled("Export Presets")
            .show(ui);
        if self.presets.is_empty() {
            let _ = hint(ui, "No presets yet.");
        } else {
            for preset in &self.presets {
                let _ = hint(
                    ui,
                    &negpy_backend::domain::models::preset_display_name(preset),
                );
            }
        }
        let _ = labeled_action(ui, "Save Preset...", ActionRank::Normal);
        false
    }

    fn show_alongside(&mut self, ui: &mut Ui) -> bool {
        let changed = labeled_toggle(ui, "Save Sidecar on Export", &mut self.sidecars).clicked();
        entry_row(
            ui,
            RowColumns::new(),
            "Contact sheet folder",
            &mut self.sidecar_path,
        );
        let _ = hint(
            ui,
            "Leave the folder empty to follow the export destination.",
        );
        changed | labeled_toggle(ui, "Alongside Exports", &mut self.alongside).clicked()
    }
}

fn choice_combo(items: &[(&str, &str)]) -> StepCombo {
    StepCombo::new(
        items
            .iter()
            .map(|(label, id)| Choice::with_id(*label, *id))
            .collect(),
    )
}

fn field_combo(
    ui: &mut Ui,
    columns: RowColumns,
    label: &str,
    combo: &mut StepCombo,
    changed: &mut bool,
) {
    field_row(ui, columns, label, |ui| {
        ui.set_max_width(ui.available_width());
        *changed |= combo.show(ui);
    });
}

fn entry_row(ui: &mut Ui, columns: RowColumns, label: &str, value: &mut String) {
    field_row(ui, columns, label, |ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()));
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
    fn the_export_tab_round_trips_the_edit() {
        let mut state = ExportState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.page = 0;
        state.format.pick("TIFF");
        state.jpeg_quality.set_value(80.0);
        state.resolution.current = 2;
        state.long_edge.set_value(4000.0);
        assert!(state.store(&mut edit));
        let export = &edit.config.export;
        assert_eq!(export.export_fmt, ExportFormat::Tiff);
        assert_eq!(export.jpeg_quality, 80);
        assert_eq!(
            export.export_resolution_mode,
            ExportResolutionMode::TargetPx
        );
        assert_eq!(export.export_target_long_edge_px, 4000);
    }

    #[test]
    fn the_export_sidecar_fields_round_trip() {
        let mut state = ExportState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.page = 3;
        state.sidecars = true;
        state.sidecar_path = "/tmp/sheets".to_string();
        assert!(state.store(&mut edit));
        let export = &edit.config.export;
        assert!(export.export_sidecars_enabled);
        assert_eq!(export.contact_sheet_output_path, "/tmp/sheets");
    }

    #[test]
    fn the_proof_controls_read_and_write_the_session_proof() {
        let mut state = ExportState::default();
        state.proof.enabled = false;
        state.proof.intent = "perceptual".to_string();
        state.proof.black_point = true;
        assert!(!state.proof.enabled);
        assert_eq!(state.proof.intent, "perceptual");
        assert!(state.proof.black_point);
    }

    #[test]
    fn the_export_presets_show_their_display_names() {
        let mut state = ExportState::default();
        state.set_presets(vec![negpy_backend::domain::models::ExportPreset {
            name: "JPEG".to_string(),
            export_fmt: ExportFormat::Jpeg,
            ..Default::default()
        }]);
        assert_eq!(state.presets.len(), 1);
        assert_eq!(
            negpy_backend::domain::models::preset_display_name(&state.presets[0]),
            "JPEG"
        );
    }

    #[test]
    fn the_export_tab_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = ExportState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut edit = FrameEdit::default();
                assert!(!state.show(ui, &mut edit));
            });
        });
    }
}
