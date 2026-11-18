//! The Contact Sheet dialog capture view.
//!
//! The dialog has a paper preview on the left and its controls on the right. The
//! capture paints the same regions the golden comparator scores; widget factories
//! replace the painter helpers as the Layer 3 port lands.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Stroke, TextureId, Vec2};

use crate::styles::icons::Icon;
use crate::styles::layout::{DEFAULT_BUTTON_HEIGHT, FIELD_LABEL_WIDTH, HEADER_BUTTON_SIZE};
use crate::styles::theme::Theme;

const PANEL_WIDTH: f32 = 340.0;
const MARGIN: f32 = 16.0;
const GAP: f32 = 16.0;
const PAPER_WIDTH_MM: f32 = 241.3;
const PAPER_HEIGHT_MM: f32 = 304.8;

/// Paint the whole dialog into `screen`.
pub fn paint(painter: &egui::Painter, screen: Rect, texture: Option<TextureId>) {
    let theme = Theme::default();
    painter.rect_filled(screen, 0.0, theme.bg_panel);
    let panel = Rect::from_min_size(
        egui::pos2(screen.right() - MARGIN - PANEL_WIDTH, screen.top() + MARGIN),
        Vec2::new(PANEL_WIDTH, screen.height() - 2.0 * MARGIN),
    );
    let canvas = Rect::from_min_max(
        egui::pos2(screen.left() + MARGIN, screen.top() + MARGIN),
        egui::pos2(panel.left() - GAP, screen.bottom() - MARGIN),
    );
    painter.rect_filled(canvas, 0.0, theme.canvas_bg_dark_grey);
    paint_sheet(painter, canvas, &theme, texture);
    paint_navigation(painter, canvas, &theme);
    paint_controls(painter, panel, &theme);
}

fn paint_sheet(painter: &egui::Painter, canvas: Rect, theme: &Theme, texture: Option<TextureId>) {
    let pad = 28.0;
    let scale = ((canvas.width() - 2.0 * pad) / PAPER_WIDTH_MM)
        .min((canvas.height() - 2.0 * pad) / PAPER_HEIGHT_MM)
        .max(0.05);
    let paper = Rect::from_center_size(
        canvas.center(),
        Vec2::new(PAPER_WIDTH_MM * scale, PAPER_HEIGHT_MM * scale),
    );
    painter.rect_filled(paper, 0.0, Color32::BLACK);
    painter.rect_stroke(
        paper,
        0.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );

    let inset = 14.0;
    let inner = paper.shrink(inset);
    let columns = 3;
    let tile_gap = 8.0;
    let tile_w = (inner.width() - tile_gap * (columns as f32 - 1.0)) / columns as f32;
    let tile_h = (inner.height() * 0.45 - tile_gap) / 2.0;
    for index in 0..5 {
        let column = index % columns;
        let row = index / columns;
        let tile = Rect::from_min_size(
            egui::pos2(
                inner.left() + column as f32 * (tile_w + tile_gap),
                inner.top() + row as f32 * (tile_h + tile_gap),
            ),
            Vec2::new(tile_w, tile_h),
        );
        if let Some(texture) = texture {
            painter.image(
                texture,
                tile,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.rect_filled(tile, 0.0, Color32::from_rgb(0x80, 0x80, 0x80));
        }
        painter.text(
            tile.right_bottom() + Vec2::new(-6.0, -4.0),
            Align2::RIGHT_BOTTOM,
            format!("{}", index + 1),
            FontId::proportional(9.0),
            theme.text_primary,
        );
    }
    paint_handles(painter, paper, theme);
}

fn paint_handles(painter: &egui::Painter, paper: Rect, theme: &Theme) {
    let size = 9.0;
    let points = [
        paper.left_top(),
        paper.center_top(),
        paper.right_top(),
        paper.left_center(),
        paper.right_center(),
        paper.left_bottom(),
        paper.center_bottom(),
        paper.right_bottom(),
    ];
    for point in points {
        let handle = Rect::from_center_size(point, Vec2::splat(size));
        painter.rect_filled(handle, 1.0, theme.text_primary);
    }
}

fn paint_navigation(painter: &egui::Painter, canvas: Rect, theme: &Theme) {
    let y = canvas.bottom() - 20.0;
    paint_button(
        painter,
        Rect::from_center_size(
            egui::pos2(canvas.center().x - 56.0, y),
            Vec2::new(36.0, 28.0),
        ),
        Icon::ChevronLeft.glyph(),
        theme,
    );
    painter.text(
        egui::pos2(canvas.center().x, y),
        Align2::CENTER_CENTER,
        "Sheet 1 of 2",
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
    paint_button(
        painter,
        Rect::from_center_size(
            egui::pos2(canvas.center().x + 56.0, y),
            Vec2::new(36.0, 28.0),
        ),
        Icon::ChevronRight.glyph(),
        theme,
    );
}

fn paint_controls(painter: &egui::Painter, panel: Rect, theme: &Theme) {
    let label_width = FIELD_LABEL_WIDTH + 10.0;
    let box_left = panel.left() + label_width;
    let box_width = panel.right() - box_left;
    let base = f32::from(theme.font_size_base);

    paint_field(
        painter,
        panel,
        26.0,
        label_width,
        box_width,
        "Film format",
        "120",
        theme,
    );
    paint_field(
        painter,
        panel,
        78.0,
        label_width,
        box_width,
        "Frame size",
        "6x6",
        theme,
    );
    paint_field(
        painter,
        panel,
        126.0,
        label_width,
        box_width,
        "Paper",
        "24 × 30.5 cm",
        theme,
    );
    paint_button(
        painter,
        Rect::from_min_size(
            egui::pos2(box_left + box_width - 38.0, 126.0 - 6.0),
            Vec2::new(38.0, 38.0),
        ),
        Icon::TurnPage.glyph(),
        theme,
    );
    paint_slider(
        painter,
        panel,
        180.0,
        label_width,
        box_width,
        "Width",
        0.58,
        "241.30 mm",
        theme,
    );
    paint_slider(
        painter,
        panel,
        230.0,
        label_width,
        box_width,
        "Height",
        0.74,
        "304.80 mm",
        theme,
    );
    paint_field(
        painter,
        panel,
        264.0,
        label_width,
        box_width,
        "DPI",
        "300",
        theme,
    );
    paint_field(
        painter,
        panel,
        316.0,
        label_width,
        box_width,
        "Order",
        "Name",
        theme,
    );
    paint_field(
        painter,
        panel,
        372.0,
        label_width,
        box_width,
        "Print",
        "As Edited",
        theme,
    );

    paint_button(
        painter,
        Rect::from_min_size(
            egui::pos2(panel.left(), 420.0),
            Vec2::new(panel.width(), 38.0),
        ),
        format!("{} Pick Frames", Icon::PickFrames.glyph()).as_str(),
        theme,
    );

    let button_width = (panel.width() - theme.space_md as f32) / 2.0;
    for (index, label) in ["Film Edges", "Edge Print", "White Paper", "Roll Label"]
        .iter()
        .enumerate()
    {
        let column = index % 2;
        let row = index / 2;
        let rect = Rect::from_min_size(
            egui::pos2(
                panel.left() + column as f32 * (button_width + theme.space_md as f32),
                470.0 + row as f32 * 46.0,
            ),
            Vec2::new(button_width, 38.0),
        );
        paint_toggle(painter, rect, label, index != 2, theme);
    }

    paint_labeled_entry(
        painter,
        panel,
        589.0,
        label_width,
        box_width,
        "Label text",
        "vv9",
        theme,
    );
    painter.text(
        egui::pos2(panel.left(), 628.0),
        Align2::LEFT_TOP,
        "13 frames · 5 strips of 3 · 2 sheets · preview 5/13",
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );
    painter.text(
        egui::pos2(panel.left(), 668.0),
        Align2::LEFT_TOP,
        "Saves to /home/bruno/WORK/svetovy_vylet/skeny/vv9/\nexport",
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_hint,
    );

    let footer = panel.bottom() - DEFAULT_BUTTON_HEIGHT;
    let info = Rect::from_center_size(
        egui::pos2(
            panel.left() + HEADER_BUTTON_SIZE / 2.0,
            footer + DEFAULT_BUTTON_HEIGHT / 2.0,
        ),
        Vec2::splat(HEADER_BUTTON_SIZE),
    );
    paint_button(painter, info, Icon::Help.glyph(), theme);
    let cancel = Rect::from_min_size(
        egui::pos2(panel.right() - 2.0 * 72.0 - theme.space_md as f32, footer),
        Vec2::new(72.0, DEFAULT_BUTTON_HEIGHT),
    );
    paint_button(painter, cancel, "Cancel", theme);
    paint_primary(
        painter,
        Rect::from_min_size(
            egui::pos2(panel.right() - 72.0, footer),
            Vec2::new(72.0, DEFAULT_BUTTON_HEIGHT),
        ),
        "Export",
        theme,
    );
    let _ = base;
}

fn paint_field(
    painter: &egui::Painter,
    panel: Rect,
    y: f32,
    label_width: f32,
    box_width: f32,
    label: &str,
    value: &str,
    theme: &Theme,
) {
    let row = Rect::from_min_size(
        egui::pos2(panel.left(), y),
        Vec2::new(panel.width(), DEFAULT_BUTTON_HEIGHT),
    );
    painter.text(
        row.left_center() + Vec2::new(label_width - 10.0, 0.0),
        Align2::RIGHT_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_secondary,
    );
    let field = Rect::from_min_size(
        egui::pos2(panel.left() + label_width, y),
        Vec2::new(box_width, DEFAULT_BUTTON_HEIGHT),
    );
    painter.rect_filled(field, 4.0, theme.bg_input);
    painter.rect_stroke(
        field,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    painter.text(
        field.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
    painter.text(
        field.right_center() + Vec2::new(-10.0, 0.0),
        Align2::RIGHT_CENTER,
        Icon::ChevronDown.glyph(),
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_hint,
    );
}

fn paint_labeled_entry(
    painter: &egui::Painter,
    panel: Rect,
    y: f32,
    label_width: f32,
    box_width: f32,
    label: &str,
    value: &str,
    theme: &Theme,
) {
    painter.text(
        egui::pos2(
            panel.left() + label_width - 10.0,
            y + DEFAULT_BUTTON_HEIGHT / 2.0,
        ),
        Align2::RIGHT_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_secondary,
    );
    let field = Rect::from_min_size(
        egui::pos2(panel.left() + label_width, y),
        Vec2::new(box_width, DEFAULT_BUTTON_HEIGHT),
    );
    painter.rect_filled(field, 4.0, theme.bg_input);
    painter.rect_stroke(
        field,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    painter.text(
        field.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        value,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
}

fn paint_slider(
    painter: &egui::Painter,
    panel: Rect,
    y: f32,
    label_width: f32,
    box_width: f32,
    label: &str,
    fraction: f32,
    value: &str,
    theme: &Theme,
) {
    painter.text(
        egui::pos2(
            panel.left() + label_width - 10.0,
            y + DEFAULT_BUTTON_HEIGHT / 2.0,
        ),
        Align2::RIGHT_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_secondary,
    );
    let track_left = panel.left() + label_width;
    let track_right = panel.left() + label_width + box_width * 0.58;
    let cy = y + DEFAULT_BUTTON_HEIGHT / 2.0;
    painter.line_segment(
        [
            egui::pos2(track_left + 10.0, cy),
            egui::pos2(track_right, cy),
        ],
        Stroke::new(2.0_f32, theme.border_primary),
    );
    let knob_x = track_left + 10.0 + (track_right - track_left - 10.0) * fraction;
    painter.circle_filled(egui::pos2(knob_x, cy), 5.0, theme.text_secondary);
    painter.text(
        egui::pos2(panel.right(), cy),
        Align2::RIGHT_CENTER,
        value,
        FontId::proportional(f32::from(theme.font_size_small)),
        theme.text_secondary,
    );
}

fn paint_button(painter: &egui::Painter, rect: Rect, label: &str, theme: &Theme) {
    painter.rect_filled(rect, 4.0, theme.bg_input);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_primary),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_primary,
    );
}

fn paint_primary(painter: &egui::Painter, rect: Rect, label: &str, theme: &Theme) {
    painter.rect_filled(rect, 4.0, theme.bg_primary);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, theme.border_checked_hover),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        theme.text_on_accent,
    );
}

fn paint_toggle(painter: &egui::Painter, rect: Rect, label: &str, checked: bool, theme: &Theme) {
    let fill = if checked {
        theme.bg_checked
    } else {
        theme.bg_input
    };
    let border = if checked {
        theme.border_checked
    } else {
        theme.border_primary
    };
    painter.rect_filled(rect, 4.0, fill);
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(f32::from(theme.font_size_base)),
        if checked {
            theme.text_primary
        } else {
            theme.text_hint
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_paints_on_a_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                paint(ui.painter(), ui.max_rect(), None);
            });
        });
    }
}
