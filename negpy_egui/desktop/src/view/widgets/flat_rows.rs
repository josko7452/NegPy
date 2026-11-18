//! Flat list rows: a current or hovered fill, a pill and a ⋮ affordance.
//!
//! The History steps, the Library rolls and the metadata presets share this row.
//! A press on the ⋮ opens that row's menu instead of picking the row.

use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::styles::icons::Icon;
use crate::styles::theme::Theme;

/// The ⋮ icon's box.
pub const ROW_ICON: f32 = 16.0;

/// Where a row draws its ⋮.
#[must_use]
pub fn more_rect(row: Rect) -> Rect {
    let theme = Theme::default();
    Rect::from_min_size(
        egui::pos2(
            row.right() - f32::from(theme.space_lg) - ROW_ICON,
            row.center().y - ROW_ICON / 2.0,
        ),
        Vec2::splat(ROW_ICON),
    )
}

/// Whether a pointer at `pos` lands on the row's ⋮.
#[must_use]
pub fn more_press(row: Rect, pos: egui::Pos2) -> bool {
    more_rect(row).contains(pos)
}

/// Paint the row's fill for its current, hover and selected states.
pub fn paint_fill(painter: &egui::Painter, rect: Rect, current: bool, hover: bool, selected: bool) {
    let theme = Theme::default();
    let box_rect = rect.shrink(0.5);
    if current {
        let fill = if hover {
            theme.bg_checked_hover
        } else {
            theme.bg_checked
        };
        let edge = if hover {
            theme.border_checked_hover
        } else {
            theme.border_checked
        };
        painter.rect_filled(box_rect, f32::from(theme.radius_md), fill);
        painter.rect_stroke(
            box_rect,
            f32::from(theme.radius_md),
            Stroke::new(1.0_f32, edge),
            egui::StrokeKind::Inside,
        );
    } else if selected || hover {
        let fill = if selected {
            theme.bg_selected
        } else {
            theme.bg_hover
        };
        painter.rect_filled(box_rect, f32::from(theme.radius_md), fill);
    }
}

/// The width a pill takes at the base font.
#[must_use]
pub fn pill_width(text: &str) -> f32 {
    text.chars().count() as f32 * 8.0 + 2.0 * f32::from(Theme::default().space_md)
}

/// A rounded tag that ends at `right`; returns its left edge.
pub fn paint_pill(
    painter: &egui::Painter,
    right: f32,
    center_y: f32,
    text: &str,
    accent: bool,
) -> f32 {
    let theme = Theme::default();
    let width = pill_width(text);
    let height = f32::from(theme.font_size_base) + 2.0;
    let pill = Rect::from_min_size(
        egui::pos2(right - width, center_y - height / 2.0),
        Vec2::new(width, height),
    );
    painter.rect_stroke(
        pill,
        height / 2.0,
        Stroke::new(
            1.0_f32,
            if accent {
                theme.border_checked_hover
            } else {
                theme.border_color
            },
        ),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(f32::from(theme.font_size_small)),
        if accent {
            theme.text_primary
        } else {
            theme.text_hint
        },
    );
    pill.left()
}

/// Paint the row's ⋮.
pub fn paint_more(painter: &egui::Painter, row: Rect) {
    let theme = Theme::default();
    painter.text(
        more_rect(row).center(),
        Align2::CENTER_CENTER,
        Icon::Menu.glyph(),
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
}

/// A name's line over a detail line, centered as a pair.
#[must_use]
pub fn two_line_rects(row: Rect, left: f32, right: f32) -> (Rect, Rect) {
    let theme = Theme::default();
    let line = f32::from(theme.font_size_base) + 2.0;
    let gap = f32::from(theme.space_xs);
    let top = row.center().y - (2.0 * line + gap) / 2.0;
    (
        Rect::from_min_size(egui::pos2(left, top), Vec2::new(right - left, line)),
        Rect::from_min_size(
            egui::pos2(left, top + line + gap),
            Vec2::new(right - left, line),
        ),
    )
}

/// One two-line row, ready for a fill and a click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    pub name: String,
    pub detail: String,
    pub current: bool,
    pub selected: bool,
}

impl FlatRow {
    /// A row with its name and detail line.
    #[must_use]
    pub fn new(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            current: false,
            selected: false,
        }
    }

    /// Paint the row and report its click and ⋮ press.
    pub fn show(&self, ui: &mut Ui, height: f32) -> FlatRowResponse {
        let theme = Theme::default();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        let hover = response.hovered();
        paint_fill(ui.painter(), rect, self.current, hover, self.selected);
        let (name_rect, detail_rect) = two_line_rects(
            rect,
            rect.left() + f32::from(theme.space_xl),
            more_rect(rect).left() - f32::from(theme.space_md),
        );
        ui.painter().text(
            name_rect.left_center(),
            Align2::LEFT_CENTER,
            &self.name,
            FontId::proportional(f32::from(theme.font_size_base)),
            if self.current || self.selected {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
        ui.painter().text(
            detail_rect.left_center(),
            Align2::LEFT_CENTER,
            &self.detail,
            FontId::proportional(f32::from(theme.font_size_small)),
            theme.text_hint,
        );
        if hover {
            paint_more(ui.painter(), rect);
        }
        let menu = response
            .interact_pointer_pos()
            .is_some_and(|pos| more_press(rect, pos));
        FlatRowResponse { response, menu }
    }
}

/// What a flat row reported.
pub struct FlatRowResponse {
    pub response: egui::Response,
    pub menu: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_more_press_stays_inside_the_row() {
        let row = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(300.0, 28.0));
        assert!(more_press(row, more_rect(row).center()));
        assert!(!more_press(row, row.left_center()));
    }

    #[test]
    fn a_pill_ends_at_its_right_edge() {
        let width = pill_width("✓");
        assert!(width > 0.0);
    }

    #[test]
    fn two_lines_share_the_row_center() {
        let row = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(300.0, 40.0));
        let (name, detail) = two_line_rects(row, 0.0, 300.0);
        assert!(name.bottom() < detail.top());
        assert!((name.center().y + detail.center().y) / 2.0 - row.center().y < 1.0);
    }
}
