//! The Light Table: every frame of the roll in a grid over the canvas area.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::BAR_BUTTON_HEIGHT;
use crate::styles::theme::Theme;
use crate::view::widgets::marks;

/// The grid's edge inset.
pub const GRID_MARGIN: f32 = 8.0;
/// The gap between frame cells.
pub const CELL_GAP: f32 = 8.0;
/// The frame's shape inside each cell, the reference frame's landscape ratio.
pub const CELL_ASPECT: f32 = 1.2;

/// The grid geometry for `count` frames at `width`.
#[must_use]
pub fn grid(count: usize, area: Rect, cell_width: f32) -> Vec<Rect> {
    if count == 0 || cell_width <= 0.0 {
        return Vec::new();
    }
    let columns = ((area.width() + CELL_GAP) / (cell_width + CELL_GAP))
        .floor()
        .max(1.0) as usize;
    let cell_height = cell_width / CELL_ASPECT;
    let mut cells = Vec::with_capacity(count);
    for index in 0..count {
        let column = index % columns;
        let row = index / columns;
        cells.push(Rect::from_min_size(
            egui::pos2(
                area.left() + column as f32 * (cell_width + CELL_GAP),
                area.top() + row as f32 * (cell_height + CELL_GAP),
            ),
            Vec2::new(cell_width, cell_height),
        ));
    }
    cells
}

/// Paint the Light Table over `screen`. `selected` is the open frame.
pub fn paint(ui: &mut egui::Ui, screen: Rect, strip: &[(egui::TextureId, Vec2)], selected: usize) {
    let theme = Theme::default();
    crate::app::paint_top_bar(ui.painter(), screen);
    let top = crate::app::top_bar_rect(screen);

    let body = Rect::from_min_max(
        egui::pos2(screen.left() + GRID_MARGIN, top.bottom() + GRID_MARGIN),
        egui::pos2(
            crate::app::rail_rect(screen).left() - GRID_MARGIN,
            screen.bottom() - GRID_MARGIN,
        ),
    );
    let bar = Rect::from_min_max(egui::pos2(body.left(), body.bottom() - 52.0), body.max);
    let grid_area = Rect::from_min_max(body.min, egui::pos2(body.right(), bar.top() - GRID_MARGIN));
    ui.painter()
        .rect_filled(grid_area, 0.0, theme.canvas_bg_dark_grey);

    let cell_width = ((grid_area.width() - CELL_GAP) / 2.0).floor();
    let cells = grid(strip.len(), grid_area, cell_width);
    for (index, cell) in cells.iter().enumerate() {
        if !grid_area.intersects(*cell) {
            continue;
        }
        let hovered = ui
            .interact(
                *cell,
                ui.id().with(("light-table-cell", index)),
                Sense::click(),
            )
            .hovered();
        ui.painter()
            .rect_filled(cell.shrink(1.0), 0.0, Color32::BLACK);
        if let Some((texture, size)) = strip.get(index) {
            let content = fitted_rect(cell.shrink(1.0), *size);
            ui.painter().with_clip_rect(cell.shrink(1.0)).image(
                *texture,
                content,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let edge = if index == selected {
            theme.border_checked
        } else if hovered {
            theme.border_hover
        } else {
            theme.border_color
        };
        ui.painter().rect_stroke(
            cell.shrink(1.0),
            0.0,
            Stroke::new(if index == selected { 2.0_f32 } else { 1.0_f32 }, edge),
            egui::StrokeKind::Inside,
        );
        let dot = egui::pos2(cell.right() - 10.0, cell.top() + 10.0);
        ui.painter().circle_filled(dot, 5.0, theme.bg_dark);
        ui.painter()
            .circle_stroke(dot, 5.0, Stroke::new(1.0_f32, theme.border_color));
        if index == 0 {
            marks::draw_mark_badge(ui.painter(), *cell, true);
        }
    }
    paint_bar(ui, bar, strip.len());
}

fn fitted_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).min(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn paint_bar(ui: &mut egui::Ui, bar: Rect, frames: usize) {
    let theme = Theme::default();
    ui.painter().rect_filled(bar, 0.0, theme.bg_panel);
    ui.painter().line_segment(
        [bar.left_top(), bar.right_top()],
        Stroke::new(1.0_f32, theme.border_primary),
    );
    let row = Rect::from_min_size(
        egui::pos2(bar.left() + 8.0, bar.center().y - BAR_BUTTON_HEIGHT / 2.0),
        Vec2::new(bar.width() - 16.0, BAR_BUTTON_HEIGHT),
    );
    let mut left = row.left();
    let text = |ui: &egui::Ui, left: &mut f32, text: &str, width: f32| {
        ui.painter().text(
            egui::pos2(*left, row.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(f32::from(theme.font_size_base)),
            theme.text_secondary,
        );
        *left += width;
    };
    text(ui, &mut left, "Photo", 64.0);
    text(ui, &mut left, "Roll", 64.0);
    let count = if frames == 1 {
        "1 frame".to_string()
    } else {
        format!("{frames} frames")
    };
    text(ui, &mut left, &count, 92.0);
    let mut actions = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("light-table-bar")
            .max_rect(row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    actions.set_width(row.width());
    actions.add_space(left - row.left());
    let _ = crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Check,
        "Keep",
        crate::styles::templates::ActionRank::Normal,
        "Keep the selected frames",
    );
    actions.add_space(f32::from(theme.space_md));
    let _ = crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Close,
        "Reject",
        crate::styles::templates::ActionRank::Normal,
        "Reject the selected frames: batch export skips them",
    );
    actions.add_space(f32::from(theme.space_md));
    let _ = crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Wrench,
        "Auto Crop",
        crate::styles::templates::ActionRank::Normal,
        "Auto Crop the selected frames; crops you drew stay",
    );
    actions.add_space(f32::from(theme.space_md));
    let _ = crate::styles::templates::labeled_action(
        &mut actions,
        "Export",
        crate::styles::templates::ActionRank::Normal,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_uses_two_columns_at_the_reference_width() {
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1350.0, 1300.0));
        let cells = grid(4, area, (area.width() - CELL_GAP) / 2.0);
        assert_eq!(cells.len(), 4);
        assert_eq!(
            cells[0].left(),
            cells[1].left() - cells[0].width() - CELL_GAP
        );
        assert!(cells[2].top() > cells[0].bottom());
    }

    #[test]
    fn an_empty_grid_has_no_cells() {
        assert!(
            grid(
                0,
                Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::splat(10.0)),
                5.0
            )
            .is_empty()
        );
    }
}
