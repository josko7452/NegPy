//! The Metadata tab: a pinned preview, then the Shot, Process, Presets and Gear pages.

use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::sub_tabs::SUB_TAB_HEIGHT;
use crate::view::widgets::tab_header::TabSummary;

/// The four Metadata pages.
#[must_use]
pub fn metadata_tabs() -> [PanelTab; 4] {
    [
        PanelTab::beside(Icon::Image, "Shot").with_divider(),
        PanelTab::beside(Icon::Sliders, "Process"),
        PanelTab::beside(Icon::Stack, "Presets").icon_only(),
        PanelTab::beside(Icon::Gear, "My Gear").icon_only(),
    ]
}

/// The Metadata tab's state.
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataState {
    pub page: usize,
    pub preview_open: bool,
    pub protect: bool,
    pub camera: StepCombo,
    pub lens: StepCombo,
    pub film_stock: StepCombo,
    pub capture_date: String,
    pub place: String,
    pub exposure: String,
    pub saved_process: StepCombo,
    pub film_format: StepCombo,
    pub developer: String,
    pub dilution: String,
    pub push_pull: StepCombo,
    pub time: String,
    pub temperature: String,
    pub saved_setup: StepCombo,
    pub scanning: String,
    pub capture_roll: String,
    pub capture_frame: String,
    pub presets: Vec<String>,
}

impl Default for MetadataState {
    fn default() -> Self {
        Self {
            page: 0,
            preview_open: true,
            protect: false,
            camera: StepCombo::new(vec![
                Choice::new("Search cameras..."),
                Choice::new("OM-3"),
                Choice::new("Nikon FM2"),
            ]),
            lens: StepCombo::new(vec![
                Choice::new("Search lenses..."),
                Choice::new("Olympus 50 mm f/1.8"),
            ]),
            film_stock: StepCombo::new(vec![
                Choice::new("Search film stocks..."),
                Choice::new("Kodak Gold 200"),
            ]),
            capture_date: String::new(),
            place: String::new(),
            exposure: String::new(),
            saved_process: StepCombo::new(vec![
                Choice::new("Search processes..."),
                Choice::new("D-76 stock"),
            ]),
            film_format: StepCombo::new(vec![
                Choice::new("35 mm"),
                Choice::new("120"),
                Choice::new("Other"),
            ]),
            developer: String::new(),
            dilution: String::new(),
            push_pull: StepCombo::new(vec![Choice::new("Push / Pull")]),
            time: String::new(),
            temperature: String::new(),
            saved_setup: StepCombo::new(vec![
                Choice::new("Search scan setups..."),
                Choice::new("DSLR copy stand"),
            ]),
            scanning: String::new(),
            capture_roll: String::new(),
            capture_frame: String::new(),
            presets: vec!["No presets yet".to_string()],
        }
    }
}

impl MetadataState {
    /// Select a sub-tab by its manifest key.
    pub fn show_subtab(&mut self, key: &str) {
        self.page = match key {
            "process" => 1,
            "presets" => 2,
            "gear" => 3,
            _ => 0,
        };
    }

    /// Paint the tab. Returns whether the edit changed.
    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        let mut preview = CollapsibleSection::new("Metadata Preview").with_guide();
        preview.expanded = self.preview_open;
        let (_, preview_result) = preview.show(ui, |ui| {
            card(ui, |ui| {
                let preview_changed =
                    labeled_toggle(ui, "Protect Original Metadata", &mut self.protect).clicked();
                ui.horizontal_wrapped(|ui| {
                    let _ = hint(ui, "Written to exported files on export.");
                    let _ = labeled_action(ui, "Description...", ActionRank::Normal);
                });
                ui.add_space(f32::from(Theme::default().space_sm));
                let _ = section_subheader(ui, "SCAN");
                for (label, value) in [
                    ("Camera make", "OM Digital Solutions"),
                    ("Camera model", "OM-3"),
                    ("Lens model", "OLYMPUS M. 30mm F3.5 Macro"),
                    ("Focal length", "30 mm"),
                    ("Aperture", "f/4"),
                    ("Exposure", "1/500s"),
                ] {
                    preview_row(ui, label, value);
                }
                preview_changed
            })
        });
        self.preview_open = preview.expanded;
        changed |= preview_result.unwrap_or(false);
        let tabs = metadata_tabs();
        let mut page = self.page;
        if let Some(picked) = panel_tab_strip(ui, &tabs, page, SUB_TAB_HEIGHT) {
            page = picked;
            self.page = picked;
        }
        ui.add_space(f32::from(Theme::default().space_sm));
        let changed_page = match page {
            0 => self.show_shot(ui),
            1 => self.show_process(ui),
            2 => self.show_presets(ui),
            3 => self.show_gear(ui),
            _ => false,
        };
        changed | changed_page
    }

    fn show_shot(&mut self, ui: &mut Ui) -> bool {
        let mut changed = TabSummary::new(3).show(ui) != Default::default();
        let (gear_open, gear_changed) = metadata_card(ui, "Analog Gear", true, |ui| {
            let _ = hint(ui, "Searches the gear that you declared as your own.");
            let mut gear_changed = false;
            ui.horizontal(|ui| {
                ui.label("Camera");
                gear_changed |= self.camera.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Lens");
                gear_changed |= self.lens.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Film stock");
                gear_changed |= self.film_stock.show(ui);
            });
            let _ = labeled_action(ui, "Clear", ActionRank::Normal);
            let _ = labeled_action(ui, "Infer from Folder Name", ActionRank::Normal);
            gear_changed
        });
        let _ = gear_open;
        changed |= gear_changed;
        let (capture_open, capture_changed) = metadata_card(ui, "Capture", true, |ui| {
            ui.horizontal(|ui| {
                ui.label("Date");
                let _ = ui.text_edit_singleline(&mut self.capture_date);
            });
            ui.horizontal(|ui| {
                ui.label("Place");
                let _ = ui.text_edit_singleline(&mut self.place);
            });
            false
        });
        let _ = capture_open;
        changed |= capture_changed;
        let (exposure_open, exposure_changed) = metadata_card(ui, "Exposure", true, |ui| {
            let _ = hint(ui, "The exposure of the original shot.");
            ui.horizontal(|ui| {
                ui.label("Exposure");
                let _ = ui.text_edit_singleline(&mut self.exposure);
                let _ = ui.add(egui::Button::new("Lock"));
            });
            false
        });
        let _ = exposure_open;
        changed |= exposure_changed;
        changed
    }

    fn show_process(&mut self, ui: &mut Ui) -> bool {
        let mut changed = TabSummary::new(2).show(ui) != Default::default();
        let (process_open, process_changed) = metadata_card(ui, "Process", true, |ui| {
            let mut process_changed = false;
            ui.horizontal(|ui| {
                ui.label("Saved process");
                process_changed |= self.saved_process.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Film format");
                process_changed |= self.film_format.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Developer");
                let _ = ui.text_edit_singleline(&mut self.developer);
            });
            ui.horizontal(|ui| {
                ui.label("Dilution");
                let _ = ui.text_edit_singleline(&mut self.dilution);
            });
            ui.horizontal(|ui| {
                ui.label("Push / Pull");
                process_changed |= self.push_pull.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Time");
                let _ = ui.text_edit_singleline(&mut self.time);
            });
            ui.horizontal(|ui| {
                ui.label("Temperature (°C)");
                let _ = ui.text_edit_singleline(&mut self.temperature);
            });
            let _ = labeled_action(ui, "Clear", ActionRank::Normal);
            process_changed
        });
        let _ = process_open;
        changed |= process_changed;
        let (scan_open, scan_changed) = metadata_card(ui, "Scanning", true, |ui| {
            let mut scan_changed = false;
            ui.horizontal(|ui| {
                ui.label("Saved setup");
                scan_changed |= self.saved_setup.show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Scanning");
                let _ = ui.text_edit_singleline(&mut self.scanning);
            });
            ui.horizontal(|ui| {
                ui.label("Roll");
                let _ = ui.text_edit_singleline(&mut self.capture_roll);
            });
            ui.horizontal(|ui| {
                ui.label("Frame");
                let _ = ui.text_edit_singleline(&mut self.capture_frame);
                let _ = labeled_action(ui, "Sequence...", ActionRank::Normal);
            });
            let _ = labeled_action(ui, "Clear", ActionRank::Normal);
            scan_changed
        });
        let _ = scan_open;
        changed |= scan_changed;
        changed
    }

    fn show_presets(&mut self, ui: &mut Ui) -> bool {
        let _ = TabSummary::new(1).show(ui);
        metadata_card(ui, "Presets", true, |ui| {
            for preset in &self.presets {
                let _ = hint(ui, preset);
            }
            false
        })
        .1
    }

    fn show_gear(&mut self, ui: &mut Ui) -> bool {
        let _ = TabSummary::new(1).show(ui);
        metadata_card(ui, "My Gear", true, |ui| {
            let _ = hint(ui, "The gear that you declared as your own.");
            for item in ["Camera", "Lens", "Film stock", "Process", "Scan setup"] {
                let (row, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
                    Sense::hover(),
                );
                let rect = Rect::from_min_size(
                    row.min,
                    Vec2::new(ui.available_width(), DEFAULT_BUTTON_HEIGHT),
                );
                ui.painter()
                    .rect_filled(rect, 4.0, Theme::default().bg_input);
                ui.painter().text(
                    rect.left_center() + Vec2::new(12.0, 0.0),
                    Align2::LEFT_CENTER,
                    item,
                    FontId::proportional(f32::from(Theme::default().font_size_base)),
                    Theme::default().text_primary,
                );
            }
            false
        })
        .1
    }
}

fn preview_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(78.0, DEFAULT_BUTTON_HEIGHT),
            egui::Label::new(
                RichText::new(label)
                    .color(Theme::default().text_hint)
                    .size(f32::from(Theme::default().font_size_small)),
            )
            .halign(egui::Align::Max),
        );
        let _ = ui.label(
            RichText::new(value)
                .color(Theme::default().text_primary)
                .size(f32::from(Theme::default().font_size_small)),
        );
    });
}

fn metadata_card(
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

fn _unused_stroke(ui: &Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, Theme::default().border_primary),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_metadata_tab_opens_the_shot_page_and_preview() {
        let state = MetadataState::default();
        assert_eq!(state.page, 0);
        assert!(state.preview_open);
        assert_eq!(state.film_format.current_label(), "35 mm");
        assert_eq!(state.saved_process.current_label(), "Search processes...");
    }

    #[test]
    fn the_metadata_tab_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = MetadataState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!state.show(ui));
            });
        });
    }
}
