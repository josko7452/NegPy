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
/// A cell's target width, one frame pitch on the Light Table.
pub const CELL_TARGET: f32 = 220.0;

/// A triage action the Light Table bar asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightTableAction {
    Keep,
    Reject,
    AutoCrop,
    Export,
}

/// What a Light Table paint reported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LightTableHit {
    pub cell: Option<usize>,
    pub action: Option<LightTableAction>,
    /// Whether the click held Ctrl or Shift.
    pub modified: bool,
    /// Whether the modifier was Shift.
    pub shift: bool,
}

/// The grid geometry for `count` frames at `width`.
#[must_use]
pub fn grid(count: usize, area: Rect, aspects: &[f32]) -> Vec<Rect> {
    if count == 0 || area.width() <= 0.0 {
        return Vec::new();
    }
    let columns = (((area.width() + CELL_GAP) / (CELL_TARGET + CELL_GAP)).floor() as usize).max(1);
    let width =
        ((area.width() - CELL_GAP * (columns.saturating_sub(1)) as f32) / columns as f32).max(1.0);
    let mut cells = Vec::with_capacity(count);
    let mut top = area.top();
    for start in (0..count).step_by(columns) {
        let end = (start + columns).min(count);
        let row = &aspects[start..end];
        let aspect = row.iter().copied().sum::<f32>() / row.len().max(1) as f32;
        let height = (width / aspect.max(0.05)).max(1.0);
        for column in 0..(end - start) {
            cells.push(Rect::from_min_size(
                egui::pos2(area.left() + column as f32 * (width + CELL_GAP), top),
                Vec2::new(width, height),
            ));
        }
        top += height + CELL_GAP;
    }
    cells
}

/// Paint the Light Table over `screen`. `selected` is the open frame; the return value
/// is the cell the user clicked.
pub fn paint(
    ui: &mut egui::Ui,
    screen: Rect,
    strip: &[(egui::TextureId, Vec2)],
    selected: usize,
    selected_frames: &std::collections::BTreeSet<usize>,
    marks: &[Option<&str>],
) -> LightTableHit {
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

    let aspects: Vec<f32> = strip
        .iter()
        .map(|(_, size)| (size.x / size.y.max(1.0)).max(0.05))
        .collect();
    let cells = grid(strip.len(), grid_area, &aspects);
    let mut hit = LightTableHit::default();
    for (index, cell) in cells.iter().enumerate() {
        if !grid_area.intersects(*cell) {
            continue;
        }
        let response = ui.interact(
            *cell,
            ui.id().with(("light-table-cell", index)),
            Sense::click(),
        );
        let hovered = response.hovered();
        if response.clicked() {
            hit.cell = Some(index);
            hit.modified = ui.input(|input| input.modifiers.command || input.modifiers.shift);
            hit.shift = ui.input(|input| input.modifiers.shift);
        }
        ui.painter()
            .rect_filled(cell.shrink(1.0), 0.0, Color32::BLACK);
        if let Some((texture, size)) = strip.get(index) {
            let content = cover_rect(cell.shrink(1.0), *size);
            ui.painter().with_clip_rect(cell.shrink(1.0)).image(
                *texture,
                content,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let edge = if index == selected || selected_frames.contains(&index) {
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
        match marks.get(index).copied().flatten() {
            Some("keeper") => marks::draw_mark_badge(ui.painter(), *cell, true),
            Some("excluded") => marks::draw_mark_badge(ui.painter(), *cell, false),
            _ => {}
        }
    }
    hit.action = paint_bar(ui, bar, strip.len());
    hit
}

fn cover_rect(outer: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return outer;
    }
    let scale = (outer.width() / content.x).max(outer.height() / content.y);
    Rect::from_center_size(outer.center(), content * scale)
}

fn paint_bar(ui: &mut egui::Ui, bar: Rect, frames: usize) -> Option<LightTableAction> {
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
    let mut action = None;
    if crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Check,
        "Keep",
        crate::styles::templates::ActionRank::Normal,
        "Keep the selected frames",
    )
    .clicked()
    {
        action = Some(LightTableAction::Keep);
    }
    actions.add_space(f32::from(theme.space_md));
    if crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Close,
        "Reject",
        crate::styles::templates::ActionRank::Normal,
        "Reject the selected frames: batch export skips them",
    )
    .clicked()
    {
        action = Some(LightTableAction::Reject);
    }
    actions.add_space(f32::from(theme.space_md));
    if crate::styles::templates::action_with_icon(
        &mut actions,
        Icon::Wrench,
        "Auto Crop",
        crate::styles::templates::ActionRank::Normal,
        "Auto Crop the selected frames; crops you drew stay",
    )
    .clicked()
    {
        action = Some(LightTableAction::AutoCrop);
    }
    actions.add_space(f32::from(theme.space_md));
    if crate::styles::templates::labeled_action(
        &mut actions,
        "Export",
        crate::styles::templates::ActionRank::Normal,
    )
    .clicked()
    {
        action = Some(LightTableAction::Export);
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_justifies_each_row_to_the_reference_width() {
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1350.0, 1300.0));
        let cells = grid(7, area, &[1.2; 7]);
        assert_eq!(cells.len(), 7);
        let columns = (0..7)
            .take_while(|index| cells[*index].top() == cells[0].top())
            .count();
        assert!(columns >= 2);
        assert!((cells[columns - 1].right() - area.right()).abs() < 0.01);
        assert_eq!(cells[columns].left(), cells[0].left());
        assert!(cells[columns].top() >= cells[0].bottom() + CELL_GAP);
    }

    #[test]
    fn an_empty_grid_has_no_cells() {
        assert!(
            grid(
                0,
                Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::splat(10.0)),
                &[]
            )
            .is_empty()
        );
    }

    #[test]
    fn a_wide_row_keeps_the_frame_shape_and_fills_the_width() {
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1350.0, 1300.0));
        let cells = grid(2, area, &[1.2, 1.2]);
        let width = cells[0].width();
        assert!(((width / cells[0].height()) - 1.2).abs() < 0.01);
        assert_eq!(cells[1].left(), cells[0].right() + CELL_GAP);
    }

    #[test]
    fn a_cover_crop_fills_the_cell_without_letterboxing() {
        let cell = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(100.0, 50.0));
        let content = cover_rect(cell, Vec2::new(100.0, 100.0));
        assert!(content.width() >= cell.width());
        assert!(content.height() >= cell.height());
    }

    #[test]
    fn the_light_table_paints_a_mark_badge_for_the_stored_mark() {
        let ctx = egui::Context::default();
        let strip = [(egui::TextureId::Managed(0), Vec2::new(200.0, 100.0))];
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let hit = paint(
                    ui,
                    ui.max_rect(),
                    &strip,
                    0,
                    &std::collections::BTreeSet::new(),
                    &[Some("keeper")],
                );
                assert!(hit.cell.is_none());
            });
        });
    }

    #[test]
    fn the_bar_reports_the_actions_it_owns() {
        assert_ne!(LightTableAction::Keep, LightTableAction::Reject);
        assert_ne!(LightTableAction::AutoCrop, LightTableAction::Export);
    }
}
