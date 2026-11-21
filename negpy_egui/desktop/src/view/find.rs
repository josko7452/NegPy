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

/// Paint the Find field in `rect`. Returns whether the query changed and the matches.
pub fn paint(ui: &mut Ui, rect: egui::Rect, query: &mut String, focus: &mut bool) -> FindPaint {
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
    if !response.has_focus() {
        query.clear();
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
    FindPaint {
        changed,
        results: rank(query),
        open: response.has_focus(),
    }
}

/// What one paint of the field reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindPaint {
    pub changed: bool,
    pub results: Vec<FindEntry>,
    pub open: bool,
}

/// The chosen row a key press reports from the open result list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResultNav {
    pub picked: Option<FindTarget>,
    /// The row the pointer or the arrow keys select.
    pub active: usize,
}

/// Paint the result list under the field and report a click or a key press.
pub fn results(
    ui: &mut Ui,
    rect: egui::Rect,
    entries: &[FindEntry],
    active: &mut usize,
    open: bool,
) -> ResultNav {
    if entries.is_empty() {
        return ResultNav::default();
    }
    let shown = entries.len().min(12);
    if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
        *active = (*active + 1).min(shown - 1);
    }
    if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
        *active = active.saturating_sub(1);
    }
    *active = (*active).min(shown - 1);
    let enter = open && ui.input(|input| input.key_pressed(egui::Key::Enter));
    if enter {
        return ResultNav {
            picked: Some(entries[*active].target),
            active: *active,
        };
    }
    let theme = Theme::default();
    let height = shown as f32 * 30.0;
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
    for (index, entry) in entries.iter().take(shown).enumerate() {
        let row = egui::Rect::from_min_size(
            egui::pos2(list.left(), list.top() + index as f32 * 30.0),
            egui::Vec2::new(list.width(), 30.0),
        );
        let response = ui.interact(row, ui.id().with(("find", index)), egui::Sense::click());
        if response.hovered() {
            *active = index;
            ui.painter().rect_filled(row, 0.0, theme.bg_hover);
        } else if index == *active {
            ui.painter().rect_filled(row, 0.0, theme.bg_checked_hover);
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
    ResultNav {
        picked,
        active: *active,
    }
}

/// The height the field uses.
pub const FIELD_HEIGHT: f32 = SEARCH_HEIGHT;

/// The Find box state across a repaint.
#[derive(Default)]
pub struct FindState {
    pub query: String,
    pub focus: bool,
    pub results: Vec<FindEntry>,
    pub active: usize,
    pub open: bool,
}

impl FindState {
    /// Whether the field is open on a query, so Enter opens the active row.
    #[must_use]
    pub fn query_settled(&self) -> bool {
        self.open && !self.query.trim().is_empty()
    }
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

    #[test]
    fn the_active_row_never_passes_the_list() {
        let mut nav = ResultNav::default();
        let ctx = egui::Context::default();
        let entries = rank("frame");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                nav = results(
                    ui,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(200.0, 30.0)),
                    &entries,
                    &mut 99,
                    false,
                );
            });
        });
        assert!(nav.active < entries.len());
    }

    #[test]
    fn enter_opens_the_active_row() {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        let entries = rank("crop");
        let mut active = 0;
        let ctx = egui::Context::default();
        let mut nav = ResultNav::default();
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                nav = results(
                    ui,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(200.0, 30.0)),
                    &entries,
                    &mut active,
                    true,
                );
            });
        });
        assert_eq!(nav.picked, Some(entries[0].target));
    }
}
