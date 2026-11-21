//! The Find box: a filter over the shell's cards, controls and actions.

use eframe::egui::{self, Ui};

use crate::app::Tab;
use crate::styles::icons::Icon;
use crate::styles::theme::Theme;
use crate::view::rail::SEARCH_HEIGHT;

/// One result a query can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindTarget {
    Tab(Tab),
    Tool,
    Zoom,
    Export,
    CropAll,
    RollAverage,
}

/// A fixed entry the shell can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FindEntry {
    pub name: &'static str,
    pub where_: &'static str,
    pub target: FindTarget,
}

/// The shell's searchable entries, in panel order.
pub const ENTRIES: &[FindEntry] = &[
    FindEntry {
        name: "Geometry",
        where_: "Frame",
        target: FindTarget::Tab(Tab::Frame),
    },
    FindEntry {
        name: "Crop All Frames",
        where_: "Frame › Crop",
        target: FindTarget::CropAll,
    },
    FindEntry {
        name: "Exposure",
        where_: "Frame",
        target: FindTarget::Tab(Tab::Frame),
    },
    FindEntry {
        name: "Look",
        where_: "Frame",
        target: FindTarget::Tab(Tab::Frame),
    },
    FindEntry {
        name: "Finish",
        where_: "Frame",
        target: FindTarget::Tab(Tab::Frame),
    },
    FindEntry {
        name: "Metering",
        where_: "Roll",
        target: FindTarget::Tab(Tab::Roll),
    },
    FindEntry {
        name: "Roll Average",
        where_: "Toolbar",
        target: FindTarget::RollAverage,
    },
    FindEntry {
        name: "Analysis",
        where_: "Roll",
        target: FindTarget::Tab(Tab::Roll),
    },
    FindEntry {
        name: "Export",
        where_: "Export",
        target: FindTarget::Export,
    },
    FindEntry {
        name: "Print",
        where_: "Toolbar",
        target: FindTarget::Export,
    },
    FindEntry {
        name: "Library",
        where_: "Library",
        target: FindTarget::Tab(Tab::Library),
    },
    FindEntry {
        name: "Metadata",
        where_: "Metadata",
        target: FindTarget::Tab(Tab::Metadata),
    },
    FindEntry {
        name: "Scan",
        where_: "Scan",
        target: FindTarget::Tab(Tab::Scan),
    },
    FindEntry {
        name: "Tools",
        where_: "Toolbar",
        target: FindTarget::Tool,
    },
    FindEntry {
        name: "Zoom",
        where_: "Toolbar",
        target: FindTarget::Zoom,
    },
];

/// The entries holding every query word, in shell order.
#[must_use]
pub fn rank(query: &str) -> Vec<FindEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    ENTRIES
        .iter()
        .copied()
        .filter(|entry| {
            let haystack = format!("{} {}", entry.name, entry.where_).to_lowercase();
            query.split_whitespace().all(|word| haystack.contains(word))
        })
        .collect()
}

/// Paint the Find field in `rect`. Returns the query and whether the field has focus.
pub fn paint(
    ui: &mut Ui,
    rect: egui::Rect,
    query: &mut String,
    focus: &mut bool,
) -> (bool, Vec<FindEntry>) {
    let theme = Theme::default();
    ui.painter().rect_filled(rect, 4.0, theme.bg_input);
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    let icon = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top()),
        egui::Vec2::new(32.0, rect.height()),
    );
    ui.painter().text(
        icon.center(),
        egui::Align2::CENTER_CENTER,
        Icon::Search.glyph(),
        egui::FontId::proportional(f32::from(theme.font_size_header)),
        theme.text_hint,
    );
    let text = egui::Rect::from_min_max(
        egui::pos2(icon.right(), rect.top()),
        egui::pos2(rect.right() - 52.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let response = child.add(
        egui::TextEdit::singleline(query)
            .hint_text("Find a control, card or action...")
            .frame(false)
            .desired_width(text.width()),
    );
    if *focus {
        response.request_focus();
        *focus = false;
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        query.clear();
        response.surrender_focus();
    }
    let changed = response.changed();
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "Ctrl+K",
        egui::FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
    (changed, rank(query))
}

/// Paint the result list under the field and return the chosen target.
pub fn results(ui: &mut Ui, rect: egui::Rect, entries: &[FindEntry]) -> Option<FindTarget> {
    if entries.is_empty() {
        return None;
    }
    let theme = Theme::default();
    let height = (entries.len() as f32 * 30.0).min(360.0);
    let list = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() + 4.0),
        egui::Vec2::new(rect.width(), height),
    );
    ui.painter().rect_filled(list, 4.0, theme.bg_card);
    ui.painter().rect_stroke(
        list,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    let mut picked = None;
    for (index, entry) in entries.iter().take(12).enumerate() {
        let row = egui::Rect::from_min_size(
            egui::pos2(list.left(), list.top() + index as f32 * 30.0),
            egui::Vec2::new(list.width(), 30.0),
        );
        let response = ui.interact(row, ui.id().with(("find", index)), egui::Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(row, 0.0, theme.bg_hover);
        }
        ui.painter().text(
            row.left_center() + egui::Vec2::new(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            entry.name,
            egui::FontId::proportional(f32::from(theme.font_size_base)),
            theme.text_primary,
        );
        ui.painter().text(
            row.right_center() - egui::Vec2::new(12.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            entry.where_,
            egui::FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        if response.clicked() {
            picked = Some(entry.target);
        }
    }
    picked
}

/// The height the field uses.
pub const FIELD_HEIGHT: f32 = SEARCH_HEIGHT;

/// The Find box state across a repaint.
#[derive(Default)]
pub struct FindState {
    pub query: String,
    pub focus: bool,
    pub results: Vec<FindEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_reads_each_word_against_the_name_and_place() {
        assert_eq!(rank("crop all")[0].target, FindTarget::CropAll);
        assert!(
            rank("crop")
                .iter()
                .any(|entry| entry.name == "Crop All Frames")
        );
        assert_eq!(rank("frame export"), Vec::new());
    }

    #[test]
    fn an_empty_query_lists_nothing() {
        assert!(rank("   ").is_empty());
    }

    #[test]
    fn a_place_word_matches_its_tab() {
        let hits = rank("frame");
        assert!(hits.iter().any(|entry| entry.name == "Geometry"));
        assert!(hits.iter().all(|entry| entry.where_.contains("Frame")));
    }
}
