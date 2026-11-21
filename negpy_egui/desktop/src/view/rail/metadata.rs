//! The Metadata tab: a pinned preview, then the Shot, Process, Presets and Gear pages.

use eframe::egui::{self, Align2, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};
use negpy_backend::domain::models::MetadataConfig;
use negpy_backend::domain::models::metadata::{FORMAT_OPTIONS, format_label, format_value};
use negpy_backend::features::metadata::capture::{format_dev_time, format_temperature};
use negpy_backend::features::metadata::gear::GearLibrary;

use crate::edit::FrameEdit;
use crate::styles::icons::Icon;
use crate::styles::layout::DEFAULT_BUTTON_HEIGHT;
use crate::styles::templates::{ActionRank, labeled_action, labeled_toggle, section_subheader};
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card, hint};
use crate::view::widgets::field_row::{RowColumns, field_row};
use crate::view::widgets::panel_tab::{PanelTab, panel_tab_strip};
use crate::view::widgets::step_combo::{Choice, StepCombo};
use crate::view::widgets::sub_tabs::SUB_TAB_HEIGHT;
use crate::view::widgets::tab_header::{TabSummary, TabSummaryAction};

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
    pub preset_names: Vec<String>,
    pub cards_open: std::collections::BTreeMap<String, bool>,
    gear: GearLibrary,
    pub clear_gear: bool,
    pub infer_gear: bool,
    pub clear_capture: bool,
    pub clear_exposure: bool,
    pub clear_process: bool,
    pub clear_scanning: bool,
    pub folder_name: String,
    /// Whether the tab summary collapsed every card.
    pub cards_collapsed: bool,
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
            film_format: StepCombo::new(
                FORMAT_OPTIONS
                    .iter()
                    .map(|name| Choice::new(*name))
                    .collect(),
            ),
            developer: String::new(),
            dilution: String::new(),
            push_pull: StepCombo::new(vec![
                Choice::new("Push +3"),
                Choice::new("Push +2"),
                Choice::new("Push +1"),
                Choice::new("Normal"),
                Choice::new("Pull -1"),
                Choice::new("Pull -2"),
                Choice::new("Pull -3"),
            ]),
            time: String::new(),
            temperature: String::new(),
            saved_setup: StepCombo::new(vec![
                Choice::new("Search scan setups..."),
                Choice::new("DSLR copy stand"),
            ]),
            scanning: String::new(),
            capture_roll: String::new(),
            capture_frame: String::new(),
            presets: Vec::new(),
            preset_names: Vec::new(),
            cards_open: std::collections::BTreeMap::new(),
            gear: GearLibrary::default(),
            clear_gear: false,
            infer_gear: false,
            clear_capture: false,
            clear_exposure: false,
            clear_process: false,
            clear_scanning: false,
            folder_name: String::new(),
            cards_collapsed: false,
        }
    }
}

impl MetadataState {
    /// Set the folder name the Infer action reads.
    pub fn set_folder_name(&mut self, name: impl Into<String>) {
        self.folder_name = name.into();
    }

    /// Show the metadata presets the profile holds.
    pub fn set_presets(&mut self, names: Vec<String>) {
        self.preset_names = names;
    }

    /// Use the gear library the profile holds for the choice lists.
    pub fn set_gear(&mut self, gear: GearLibrary) {
        self.camera = search_combo(
            "Search cameras...",
            gear.cameras
                .iter()
                .map(|item| (item.resolved_display_name(), item.id.clone())),
        );
        self.lens = search_combo(
            "Search lenses...",
            gear.lenses
                .iter()
                .map(|item| (item.resolved_display_name(), item.id.clone())),
        );
        self.film_stock = search_combo(
            "Search film stocks...",
            gear.film_stocks
                .iter()
                .map(|item| (item.full_film_label(), item.id.clone())),
        );
        self.saved_process = search_combo(
            "Search processes...",
            gear.processes
                .iter()
                .map(|item| (item.display_name.clone(), item.id.clone())),
        );
        self.saved_setup = search_combo(
            "Search scan setups...",
            gear.scan_setups
                .iter()
                .map(|item| (item.display_name.clone(), item.id.clone())),
        );
        self.gear = gear;
    }

    /// Replace the controls with the metadata values the edit holds.
    pub fn load(&mut self, edit: &FrameEdit) {
        let config = &edit.config.metadata;
        self.protect = config.protect_original_metadata;
        self.camera.pick(&config.camera_id);
        self.lens.pick(&config.lens_id);
        self.film_stock.pick(&config.film_stock_id);
        self.capture_date = config.capture_date.clone();
        self.place = join_place(config);
        self.exposure = config.exposure_override.clone();
        self.film_format.current = FORMAT_OPTIONS
            .iter()
            .position(|option| *option == format_label(&config.format))
            .unwrap_or(0);
        self.saved_process.pick(&config.process_id);
        self.developer = config.developer.clone();
        self.dilution = config.process_dilution.clone();
        self.push_pull.current = push_pull_index(config.push_pull);
        self.time = format_dev_time(config.process_time_seconds);
        self.temperature = format_temperature(config.process_temperature_c);
        self.saved_setup.pick(&config.scanning_id);
        self.scanning = config.scanning.clone();
        self.capture_roll = config.capture_roll.clone();
        self.capture_frame = config
            .capture_frame
            .map_or_else(String::new, |frame| frame.to_string());
    }

    /// Write the controls to the edit and return whether any value changed.
    pub fn store(&mut self, edit: &mut FrameEdit) -> bool {
        let mut action = false;
        if self.clear_gear {
            action = true;
            self.clear_gear = false;
            self.camera.current = 0;
            self.lens.current = 0;
            self.film_stock.current = 0;
        }
        if self.clear_capture {
            action = true;
            self.clear_capture = false;
            self.capture_date.clear();
            self.place.clear();
            edit.config.metadata.gps_latitude = None;
            edit.config.metadata.gps_longitude = None;
        }
        if self.clear_exposure {
            action = true;
            self.clear_exposure = false;
            self.exposure.clear();
        }
        if self.clear_process {
            action = true;
            self.clear_process = false;
            self.saved_process.current = 0;
            self.developer.clear();
            self.dilution.clear();
            self.push_pull.current = push_pull_index(0);
            self.time.clear();
            self.temperature.clear();
        }
        if self.clear_scanning {
            action = true;
            self.clear_scanning = false;
            self.saved_setup.current = 0;
            self.scanning.clear();
            self.capture_roll.clear();
            self.capture_frame.clear();
        }
        if self.infer_gear {
            action = true;
            self.infer_gear = false;
            let matched = negpy_backend::services::assets::gear_match::match_gear_for_folder(
                &self.folder_name,
                &self.gear,
            );
            if !matched.camera_id.is_empty() {
                self.camera.pick(&matched.camera_id);
            }
            if !matched.film_stock_id.is_empty() {
                self.film_stock.pick(&matched.film_stock_id);
            }
            if !matched.capture_date.is_empty() && self.capture_date.is_empty() {
                self.capture_date = matched.capture_date;
            }
        }
        let config = &mut edit.config.metadata;
        let push_pull = PUSH_PULL_VALUES[self.push_pull.current.min(PUSH_PULL_VALUES.len() - 1)];
        let format =
            format_value(&FORMAT_OPTIONS[self.film_format.current.min(FORMAT_OPTIONS.len() - 1)])
                .to_string();
        let frame = self.capture_frame.trim().parse::<i64>().ok();
        let place = parse_place(&self.place);
        let next = MetadataConfig {
            camera_id: picked(&self.camera),
            lens_id: picked(&self.lens),
            film_stock_id: picked(&self.film_stock),
            capture_date: self.capture_date.clone(),
            exposure_override: self.exposure.clone(),
            process_id: picked(&self.saved_process),
            developer: self.developer.clone(),
            process_dilution: self.dilution.clone(),
            push_pull,
            process_time_seconds: parse_dev_time(&self.time),
            process_temperature_c: parse_temperature(&self.temperature),
            scanning_id: picked(&self.saved_setup),
            scanning: self.scanning.clone(),
            capture_roll: self.capture_roll.clone(),
            capture_frame: frame,
            protect_original_metadata: self.protect,
            format,
            location_city: place.0,
            location_state: place.1,
            location_country: place.2,
            ..config.clone()
        };
        let changed = action || *config != next;
        *config = next;
        changed
    }

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
    pub fn show(&mut self, ui: &mut Ui, edit: &mut FrameEdit) -> bool {
        self.load(edit);
        let mut changed = false;
        let mut preview = CollapsibleSection::new("Metadata Preview").with_guide();
        preview.expanded = self.preview_open && !self.cards_collapsed;
        let (_, preview_result) = preview.show(ui, |ui| {
            card(ui, |ui| {
                let preview_changed =
                    labeled_toggle(ui, "Protect Original Metadata", &mut self.protect).clicked();
                ui.horizontal_wrapped(|ui| {
                    let _ = hint(ui, "Written to exported files on export.");
                    let _ = ui.add_enabled(
                        false,
                        egui::Button::new("Description...")
                            .min_size(Vec2::new(0.0, DEFAULT_BUTTON_HEIGHT)),
                    );
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
        if !self.cards_collapsed {
            self.preview_open = preview.expanded;
        }
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
        changed |= changed_page;
        if changed {
            self.store(edit);
        }
        changed
    }

    fn show_shot(&mut self, ui: &mut Ui) -> bool {
        let summary = TabSummary::new(3)
            .with_collapsed(self.cards_collapsed)
            .show(ui);
        let mut changed = matches!(summary, TabSummaryAction::Apply | TabSummaryAction::Reset);
        if summary == TabSummaryAction::Collapse {
            self.cards_collapsed = !self.cards_collapsed;
        }
        let columns = RowColumns::new();
        let (gear_open, gear_changed) =
            metadata_card(ui, "Analog Gear", self.card_open("Analog Gear"), |ui| {
                let _ = hint(ui, "Searches the gear that you declared as your own.");
                let mut gear_changed = false;
                field_row(ui, columns, "Camera", |ui| {
                    gear_changed |= self.camera.show(ui);
                });
                field_row(ui, columns, "Lens", |ui| {
                    gear_changed |= self.lens.show(ui);
                });
                field_row(ui, columns, "Film stock", |ui| {
                    gear_changed |= self.film_stock.show(ui);
                });
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - f32::from(Theme::default().space_sm)) / 2.0;
                    if ui
                        .add_sized(
                            Vec2::new(width, DEFAULT_BUTTON_HEIGHT),
                            egui::Button::new("Clear")
                                .fill(Theme::default().bg_input)
                                .stroke(Stroke::new(1.0_f32, Theme::default().border_primary)),
                        )
                        .clicked()
                    {
                        self.clear_gear = true;
                        gear_changed = true;
                    }
                    if ui
                        .add_sized(
                            Vec2::new(width, DEFAULT_BUTTON_HEIGHT),
                            egui::Button::new("Infer from Folder Name")
                                .fill(Theme::default().bg_input)
                                .stroke(Stroke::new(1.0_f32, Theme::default().border_primary)),
                        )
                        .clicked()
                    {
                        self.infer_gear = true;
                        gear_changed = true;
                    }
                });
                gear_changed
            });
        self.record_open("Analog Gear", gear_open);
        changed |= gear_changed;
        let (capture_open, capture_changed) =
            metadata_card(ui, "Capture", self.card_open("Capture"), |ui| {
                let mut capture_changed = false;
                field_row(ui, columns, "Date", |ui| {
                    capture_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.capture_date)
                                .hint_text("YYYY, YYYY-MM, YYYY-MM-DD or YYYY-MM-DD HH:MM"),
                        )
                        .changed();
                });
                field_row(ui, columns, "Place", |ui| {
                    capture_changed |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.place)
                                .hint_text("Pick on a map, or paste coordinates"),
                        )
                        .changed();
                });
                capture_changed
            });
        self.record_open("Capture", capture_open);
        changed |= capture_changed;
        let (exposure_open, exposure_changed) = metadata_card(
            ui,
            "Exposure",
            self.card_open("Exposure"),
            |ui| {
                let _ = hint(
                    ui,
                    "The exposure of the original shot, if you know it. Click the lock to type free text such as 1/125s f/2.8 ISO 400 -1/3ev.",
                );
                let mut exposure_changed = false;
                field_row(ui, columns, "Exposure", |ui| {
                    exposure_changed |= ui
                        .add(egui::TextEdit::singleline(&mut self.exposure).hint_text("—"))
                        .changed();
                    let _ = ui.add_sized(
                        Vec2::splat(DEFAULT_BUTTON_HEIGHT),
                        egui::Button::new("🔒")
                            .fill(Theme::default().bg_input)
                            .stroke(Stroke::new(1.0_f32, Theme::default().border_primary)),
                    );
                });
                exposure_changed
            },
        );
        self.record_open("Exposure", exposure_open);
        changed |= exposure_changed;
        changed
    }

    fn show_process(&mut self, ui: &mut Ui) -> bool {
        let summary = TabSummary::new(2)
            .with_collapsed(self.cards_collapsed)
            .show(ui);
        let mut changed = matches!(summary, TabSummaryAction::Apply | TabSummaryAction::Reset);
        if summary == TabSummaryAction::Collapse {
            self.cards_collapsed = !self.cards_collapsed;
        }
        let (process_open, process_changed) =
            metadata_card(ui, "Process", self.card_open("Process"), |ui| {
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
                    process_changed |= ui.text_edit_singleline(&mut self.developer).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Dilution");
                    process_changed |= ui.text_edit_singleline(&mut self.dilution).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Push / Pull");
                    process_changed |= self.push_pull.show(ui);
                });
                ui.horizontal(|ui| {
                    ui.label("Time");
                    process_changed |= ui.text_edit_singleline(&mut self.time).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Temperature (°C)");
                    process_changed |= ui.text_edit_singleline(&mut self.temperature).changed();
                });
                if labeled_action(ui, "Clear", ActionRank::Normal).clicked() {
                    self.clear_process = true;
                    process_changed = true;
                }
                process_changed
            });
        self.record_open("Process", process_open);
        changed |= process_changed;
        let (scan_open, scan_changed) =
            metadata_card(ui, "Scanning", self.card_open("Scanning"), |ui| {
                let mut scan_changed = false;
                ui.horizontal(|ui| {
                    ui.label("Saved setup");
                    scan_changed |= self.saved_setup.show(ui);
                });
                ui.horizontal(|ui| {
                    ui.label("Scanning");
                    scan_changed |= ui.text_edit_singleline(&mut self.scanning).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Roll");
                    scan_changed |= ui.text_edit_singleline(&mut self.capture_roll).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Frame");
                    scan_changed |= ui.text_edit_singleline(&mut self.capture_frame).changed();
                    let _ = ui.add_enabled(
                        false,
                        egui::Button::new("Sequence...")
                            .min_size(Vec2::new(0.0, DEFAULT_BUTTON_HEIGHT)),
                    );
                });
                if labeled_action(ui, "Clear", ActionRank::Normal).clicked() {
                    self.clear_scanning = true;
                    scan_changed = true;
                }
                scan_changed
            });
        self.record_open("Scanning", scan_open);
        changed |= scan_changed;
        changed
    }

    fn show_presets(&mut self, ui: &mut Ui) -> bool {
        let summary = TabSummary::new(1)
            .with_collapsed(self.cards_collapsed)
            .show(ui);
        if summary == TabSummaryAction::Collapse {
            self.cards_collapsed = !self.cards_collapsed;
        }
        let (open, changed) = metadata_card(ui, "Presets", self.card_open("Presets"), |ui| {
            if self.preset_names.is_empty() {
                let _ = hint(ui, "No presets yet.");
            } else {
                for preset in &self.preset_names {
                    let _ = hint(ui, preset);
                }
            }
            false
        });
        self.record_open("Presets", open);
        changed
    }

    fn show_gear(&mut self, ui: &mut Ui) -> bool {
        let summary = TabSummary::new(1)
            .with_collapsed(self.cards_collapsed)
            .show(ui);
        if summary == TabSummaryAction::Collapse {
            self.cards_collapsed = !self.cards_collapsed;
        }
        let (open, changed) = metadata_card(ui, "My Gear", self.card_open("My Gear"), |ui| {
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
        });
        self.record_open("My Gear", open);
        changed
    }

    fn card_open(&self, title: &str) -> bool {
        !self.cards_collapsed && self.cards_open.get(title).copied().unwrap_or(true)
    }

    fn record_open(&mut self, title: &str, open: bool) {
        if !self.cards_collapsed {
            self.cards_open.insert(title.to_string(), open);
        }
    }
}

const PUSH_PULL_VALUES: [i64; 7] = [3, 2, 1, 0, -1, -2, -3];

fn search_combo(placeholder: &str, items: impl Iterator<Item = (String, String)>) -> StepCombo {
    let mut choices = vec![Choice::new(placeholder)];
    choices.extend(items.map(|(label, id)| Choice::with_id(label, id)));
    StepCombo::new(choices)
}

fn picked(combo: &StepCombo) -> String {
    if combo.current == 0 {
        String::new()
    } else {
        combo.current_value().to_string()
    }
}

fn push_pull_index(value: i64) -> usize {
    PUSH_PULL_VALUES
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(3)
}

fn join_place(config: &MetadataConfig) -> String {
    [
        &config.location_city,
        &config.location_state,
        &config.location_country,
    ]
    .iter()
    .filter(|part| !part.is_empty())
    .map(|part| (*part).clone())
    .collect::<Vec<_>>()
    .join(", ")
}

fn parse_place(text: &str) -> (String, String, String) {
    let mut parts = text.split(',').map(str::trim);
    (
        parts.next().unwrap_or_default().to_string(),
        parts.next().unwrap_or_default().to_string(),
        parts.next().unwrap_or_default().to_string(),
    )
}

fn parse_dev_time(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some((minutes, seconds)) = text.split_once(':') {
        return Some(
            minutes.trim().parse::<i64>().ok()? * 60 + seconds.trim().parse::<i64>().ok()?,
        );
    }
    text.parse::<f64>()
        .ok()
        .map(|minutes| (minutes * 60.0).round() as i64)
}

fn parse_temperature(text: &str) -> Option<f64> {
    text.trim().parse().ok()
}

fn preview_row(ui: &mut Ui, label: &str, value: &str) {
    let theme = Theme::default();
    ui.horizontal(|ui| {
        ui.add_sized(
            Vec2::new(84.0, 18.0),
            egui::Label::new(
                RichText::new(label)
                    .color(theme.text_secondary)
                    .size(f32::from(theme.font_size_small)),
            )
            .halign(egui::Align::Max),
        );
        let _ = ui.label(
            RichText::new(value)
                .color(theme.text_primary)
                .size(f32::from(theme.font_size_small)),
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
        assert_eq!(state.film_format.current_label(), "—");
        assert_eq!(state.saved_process.current_label(), "Search processes...");
    }

    #[test]
    fn the_metadata_tab_round_trips_the_edit() {
        let mut state = MetadataState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.developer = "D-76".to_string();
        state.push_pull.current = 0;
        state.film_format.current = 1;
        assert!(state.store(&mut edit));
        assert_eq!(edit.config.metadata.developer, "D-76");
        assert_eq!(edit.config.metadata.push_pull, 3);
        assert_eq!(edit.config.metadata.format, "35mm");
    }

    #[test]
    fn the_metadata_gear_picks_the_stored_id() {
        let mut state = MetadataState::default();
        let gear = GearLibrary {
            cameras: vec![negpy_backend::features::metadata::gear::Camera {
                id: "cam-om3".to_string(),
                display_name: "OM-3".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        state.set_gear(gear);
        let mut edit = FrameEdit::default();
        edit.config.metadata.camera_id = "cam-om3".to_string();
        state.load(&edit);
        assert_eq!(state.camera.current_label(), "OM-3");
        state.camera.current = 0;
        assert!(state.store(&mut edit));
        assert!(edit.config.metadata.camera_id.is_empty());
    }

    #[test]
    fn the_metadata_presets_come_from_the_store() {
        let mut state = MetadataState::default();
        state.set_presets(vec!["Golden Hour".to_string()]);
        assert_eq!(state.preset_names, ["Golden Hour"]);
    }

    #[test]
    fn typed_capture_fields_are_stored() {
        let mut state = MetadataState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.capture_date = "2024-05-01".to_string();
        state.place = "Bratislava".to_string();
        state.exposure = "1/125s f/2.8".to_string();
        assert!(state.store(&mut edit));
        assert_eq!(edit.config.metadata.capture_date, "2024-05-01");
        assert_eq!(edit.config.metadata.location_city, "Bratislava");
        assert_eq!(edit.config.metadata.exposure_override, "1/125s f/2.8");
    }

    #[test]
    fn clear_actions_empty_their_card() {
        let mut state = MetadataState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.capture_date = "2024".to_string();
        state.place = "Bratislava".to_string();
        state.exposure = "1/125".to_string();
        state.developer = "D-76".to_string();
        state.scanning = "DSLR".to_string();
        state.clear_capture = true;
        state.clear_exposure = true;
        state.clear_process = true;
        state.clear_scanning = true;
        assert!(state.store(&mut edit));
        assert!(edit.config.metadata.capture_date.is_empty());
        assert!(edit.config.metadata.location_city.is_empty());
        assert!(edit.config.metadata.exposure_override.is_empty());
        assert!(edit.config.metadata.developer.is_empty());
        assert!(edit.config.metadata.scanning.is_empty());
    }

    #[test]
    fn infer_reads_the_folder_name() {
        let mut state = MetadataState::default();
        let mut edit = FrameEdit::default();
        state.load(&edit);
        state.folder_name = "2024-05-01".to_string();
        state.infer_gear = true;
        assert!(state.store(&mut edit));
        assert_eq!(edit.config.metadata.capture_date, "2024-05-01");
    }

    #[test]
    fn the_metadata_tab_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = MetadataState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut edit = FrameEdit::default();
                assert!(!state.show(ui, &mut edit));
            });
        });
    }

    #[test]
    fn a_collapsed_metadata_page_keeps_each_card_state() {
        let ctx = egui::Context::default();
        let mut state = MetadataState::default();
        let mut edit = FrameEdit::default();
        state.cards_open.insert("Capture".to_string(), false);
        state.cards_collapsed = true;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = state.show(ui, &mut edit);
            });
        });
        assert_eq!(state.cards_open.get("Capture"), Some(&false));
        assert!(state.card_open("Analog Gear") == false);
    }
}
