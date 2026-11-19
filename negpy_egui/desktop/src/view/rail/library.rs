//! The Library tab: the Roll tree over its filter and toolbar.
//!
//! A Roll is the Library's only unit; a folder is an import source. The panel shows the
//! saved rolls for the open profile, newest state first.

use std::path::Path;

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};
use negpy_backend::domain::types::AppConfig;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::assets::rolls;

use crate::styles::icons::Icon;
use crate::styles::layout::TOOLBAR_BUTTON_HEIGHT;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::flat_rows::{FlatRow, paint_pill};

const ROLL_ROW: f32 = 46.0;

/// One Roll row, as the saved store reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollRow {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub mode: Option<egui::Color32>,
    pub open: bool,
}

impl RollRow {
    /// A row with the count and date the Library prints under the name.
    #[must_use]
    pub fn new(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            name: name.into(),
            detail: detail.into(),
            mode: None,
            open: false,
        }
    }

    /// Give the row the roll id the store reports.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Mark the roll's film mode with the dot the Library prints.
    #[must_use]
    pub fn with_mode(mut self, color: egui::Color32) -> Self {
        self.mode = Some(color);
        self
    }
}

/// The Library tab's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryState {
    pub rolls: Vec<RollRow>,
    pub filter: String,
}

impl Default for LibraryState {
    fn default() -> Self {
        Self {
            rolls: Vec::new(),
            filter: String::new(),
        }
    }
}

impl LibraryState {
    /// The saved rolls of `profile`, in name order.
    #[must_use]
    pub fn load(profile: Option<&Path>) -> Self {
        let Some(profile) = profile else {
            return Self::default();
        };
        let app = AppConfig::for_user_dir(profile);
        let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        let rolls = rolls::all_rolls_sorted(&repo)
            .into_iter()
            .map(|(roll_id, entry)| RollRow::new(roll_name(&entry), "").with_id(roll_id))
            .collect();
        Self {
            rolls,
            filter: String::new(),
        }
    }

    /// The rolls the reference profile shows.
    #[must_use]
    pub fn reference() -> Self {
        let mut rows = Vec::new();
        for (name, detail, mode) in [
            ("w1", "37 photos · 12 Nov", None),
            ("w10", "13 photos · 12 Nov", None),
            ("w2", "37 photos · 12 Nov", None),
            ("w3", "37 photos · 12 Nov", None),
            ("w4", "39 photos · 12 Nov", None),
            ("w5", "37 photos · 12 Nov", None),
            ("w6", "13 photos · 11 Nov", None),
            ("w7", "13 photos · 12 Nov", None),
            ("w8", "13 photos · 12 Nov", None),
            (
                "w9",
                "13 photos · Color · 12 Nov",
                Some(Theme::default().mode_c41),
            ),
        ] {
            let mut row = RollRow::new(name, detail);
            row.mode = mode;
            rows.push(row);
        }
        if let Some(open) = rows.last_mut() {
            open.open = true;
        }
        Self {
            rolls: rows,
            filter: String::new(),
        }
    }

    /// Paint the tab and return the Roll the user opened.
    pub fn show(&mut self, ui: &mut Ui, footer_y: Option<f32>) -> Option<String> {
        let theme = Theme::default();
        let mut section = CollapsibleSection::new("Library").with_guide();
        section.expanded = true;
        let (_, result) = section.show(ui, |ui| {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let height = TOOLBAR_BUTTON_HEIGHT;
                    let (filter, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width() - 210.0, height),
                        Sense::hover(),
                    );
                    paint_field(ui, filter, &theme);
                    ui.painter().text(
                        filter.left_center() + Vec2::new(f32::from(theme.space_xl), 0.0),
                        Align2::LEFT_CENTER,
                        Icon::Search.glyph(),
                        FontId::proportional(f32::from(theme.font_size_header)),
                        theme.text_hint,
                    );
                    ui.painter().text(
                        filter.left_center() + Vec2::new(30.0, 0.0),
                        Align2::LEFT_CENTER,
                        "Filter rolls...",
                        FontId::proportional(f32::from(theme.font_size_base)),
                        theme.text_hint,
                    );
                    ui.add_space(f32::from(theme.space_sm));
                    paint_small_button(ui, "Name", 66.0, &theme);
                    ui.add_space(f32::from(theme.space_sm));
                    paint_small_button(ui, "Import", 76.0, &theme);
                    ui.add_space(f32::from(theme.space_sm));
                    paint_small_button(ui, Icon::Menu.glyph(), TOOLBAR_BUTTON_HEIGHT, &theme);
                });
                ui.add_space(f32::from(theme.space_sm));
                let mut opened = None;
                for row in &self.rolls {
                    let mut flat =
                        FlatRow::new(row.name.clone(), row.detail.clone()).with_icon(Icon::Folder);
                    if let Some(mode) = row.mode {
                        flat = flat.with_dot(mode);
                    }
                    flat.current = row.open;
                    let response = flat.show(ui, ROLL_ROW);
                    if row.open {
                        let rect = response.response.rect;
                        let right = rect.right() - f32::from(theme.space_xl);
                        paint_pill(ui.painter(), right, rect.center().y, "Open", true);
                    }
                    if response.response.clicked() {
                        opened = Some(row.name.clone());
                    }
                }
                opened
            })
        });
        let opened = result.flatten();
        let hint_y = footer_y.map_or(ui.cursor().top() + f32::from(theme.space_xl), |y| y - 8.0);
        ui.painter().text(
            egui::pos2(ui.cursor().left(), hint_y),
            Align2::LEFT_CENTER,
            "10 rolls · 252 photos",
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        opened
    }
}

fn roll_name(entry: &serde_json::Value) -> String {
    entry
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn paint_field(ui: &mut Ui, rect: Rect, theme: &Theme) {
    ui.painter().rect_filled(rect, 4.0, theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
}

fn paint_small_button(ui: &mut Ui, label: &str, width: f32, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, TOOLBAR_BUTTON_HEIGHT), Sense::click());
    ui.painter().rect_filled(rect, 4.0, theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_library_opens_one_roll() {
        let state = LibraryState::reference();
        assert_eq!(state.rolls.len(), 10);
        assert_eq!(state.rolls.iter().filter(|row| row.open).count(), 1);
        assert_eq!(state.rolls.last().unwrap().name, "w9");
    }

    #[test]
    fn the_library_reads_the_saved_rolls() {
        let dir = tempfile::tempdir().unwrap();
        let app = AppConfig::for_user_dir(dir.path());
        let mut repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
        repo.initialize().unwrap();
        rolls::recognize_folder(&mut repo, "/photos/w1", Some("w1"));
        let state = LibraryState::load(Some(dir.path()));
        assert_eq!(state.rolls.len(), 1);
        assert_eq!(state.rolls[0].name, "w1");
        assert!(!state.rolls[0].id.is_empty());
    }

    #[test]
    fn a_profile_without_rolls_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(LibraryState::load(Some(dir.path())).rolls.is_empty());
        assert!(LibraryState::load(None).rolls.is_empty());
    }

    #[test]
    fn the_library_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = LibraryState::reference();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(state.show(ui, None), None);
            });
        });
    }
}
