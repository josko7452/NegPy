//! The Library tab: the Roll tree over its filter and toolbar.
//!
//! A Roll is the Library's only unit; a folder is an import source. The panel shows the
//! saved rolls for the open profile, newest state first.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::TOOLBAR_BUTTON_HEIGHT;
use crate::styles::theme::Theme;
use crate::view::widgets::collapsible::{CollapsibleSection, card};
use crate::view::widgets::flat_rows::{FlatRow, paint_pill};

const ROLL_ROW: f32 = 46.0;

/// One Roll row, as the saved store reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollRow {
    pub name: String,
    pub detail: String,
    pub open: bool,
}

impl RollRow {
    /// A row with the count and date the Library prints under the name.
    #[must_use]
    pub fn new(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            open: false,
        }
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
        Self::reference()
    }
}

impl LibraryState {
    /// The rolls the reference profile shows.
    #[must_use]
    pub fn reference() -> Self {
        let mut rows = Vec::new();
        for (name, detail) in [
            ("w1", "37 photos · 12 Nov"),
            ("w10", "13 photos · 12 Nov"),
            ("w2", "37 photos · 12 Nov"),
            ("w3", "37 photos · 12 Nov"),
            ("w4", "39 photos · 12 Nov"),
            ("w5", "37 photos · 12 Nov"),
            ("w6", "13 photos · 11 Nov"),
            ("w7", "13 photos · 12 Nov"),
            ("w8", "13 photos · 12 Nov"),
            ("w9", "13 photos · Color · 12 Nov"),
        ] {
            rows.push(RollRow::new(name, detail));
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
    pub fn show(&mut self, ui: &mut Ui) -> Option<String> {
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
                    let mut flat = FlatRow::new(row.name.clone(), row.detail.clone());
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
        ui.add_space(f32::from(theme.space_xl));
        ui.painter().text(
            egui::pos2(ui.cursor().left(), ui.cursor().top() + 8.0),
            Align2::LEFT_CENTER,
            "10 rolls · 252 photos",
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        opened
    }
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
    fn the_library_paints_on_a_context() {
        let ctx = egui::Context::default();
        let mut state = LibraryState::reference();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(state.show(ui), None);
            });
        });
    }
}
