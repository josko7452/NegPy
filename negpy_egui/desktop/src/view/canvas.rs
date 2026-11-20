//! Canvas geometry: the photo rect, the crop cursor and the crop box.

use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, Vec2};

use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

/// The smallest crop a drag can make, as a fraction of the photo.
pub const MIN_CROP: f64 = 0.02;

/// The fitted photo rect inside the canvas area.
#[must_use]
pub fn photo_rect(area: Rect, width: u32, height: u32) -> Rect {
    let content = Vec2::new(width as f32, height as f32);
    if content.x <= 0.0 || content.y <= 0.0 {
        return area;
    }
    let scale = (area.width() / content.x).min(area.height() / content.y);
    Rect::from_center_size(area.center(), content * scale)
}

/// A normalized crop rect that follows `from` to `to` inside `photo`.
#[must_use]
pub fn drag_rect(photo: Rect, from: egui::Pos2, to: egui::Pos2) -> [f64; 4] {
    let clamp = |point: egui::Pos2| {
        egui::pos2(
            point.x.clamp(photo.left(), photo.right()),
            point.y.clamp(photo.top(), photo.bottom()),
        )
    };
    let from = clamp(from);
    let to = clamp(to);
    let x1 = f64::from((from.x.min(to.x) - photo.left()) / photo.width().max(1.0));
    let y1 = f64::from((from.y.min(to.y) - photo.top()) / photo.height().max(1.0));
    let x2 = f64::from((from.x.max(to.x) - photo.left()) / photo.width().max(1.0));
    let y2 = f64::from((from.y.max(to.y) - photo.top()) / photo.height().max(1.0));
    if x2 - x1 < MIN_CROP || y2 - y1 < MIN_CROP {
        [0.0, 0.0, 1.0, 1.0]
    } else {
        [x1, y1, x2, y2]
    }
}

/// A stored normalized crop rect as a screen rect inside `photo`.
#[must_use]
pub fn stored_rect(photo: Rect, rect: &[f64]) -> Option<Rect> {
    if rect.len() != 4 {
        return None;
    }
    let x = |value: f64| photo.left() + value as f32 * photo.width();
    let y = |value: f64| photo.top() + value as f32 * photo.height();
    let bounds = Rect::from_min_max(
        egui::pos2(x(rect[0]), y(rect[1])),
        egui::pos2(x(rect[2]), y(rect[3])),
    );
    bounds.intersects(photo).then(|| bounds.intersect(photo))
}

/// Paint the crop box and let a drag on the photo replace it.
///
/// Returns the new normalized rect when a drag ends. `drag_start` keeps the press
/// position, because egui drops the pointer when the drag stops.
pub fn crop_tool(
    ui: &mut egui::Ui,
    photo: Rect,
    rect: Option<&[f64]>,
    draw: bool,
    drag_start: &mut Option<egui::Pos2>,
) -> Option<[f64; 4]> {
    let theme = Theme::default();
    let response = ui.interact(photo, ui.id().with("crop-photo"), Sense::drag());
    if draw {
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
    }
    if draw {
        if response.drag_started() {
            *drag_start = response.interact_pointer_pos();
        }
        if response.drag_stopped()
            && let (Some(from), Some(to)) = (*drag_start, response.interact_pointer_pos())
        {
            *drag_start = None;
            return Some(drag_rect(photo, from, to));
        }
    }
    if let Some(bounds) = draw
        .then(|| rect.and_then(|rect| stored_rect(photo, rect)))
        .flatten()
    {
        let shade = with_alpha(theme.mark_shadow, 110);
        ui.painter().rect_filled(
            Rect::from_min_max(photo.min, egui::pos2(photo.right(), bounds.top())),
            0.0,
            shade,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(egui::pos2(photo.left(), bounds.bottom()), photo.max),
            0.0,
            shade,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(photo.left(), bounds.top()),
                egui::pos2(bounds.left(), bounds.bottom()),
            ),
            0.0,
            shade,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(bounds.right(), bounds.top()),
                egui::pos2(photo.right(), bounds.bottom()),
            ),
            0.0,
            shade,
        );
        ui.painter().rect_stroke(
            bounds,
            0.0,
            Stroke::new(2.0_f32, theme.mark_photo),
            egui::StrokeKind::Inside,
        );
        for corner in [
            bounds.left_top(),
            bounds.right_top(),
            bounds.left_bottom(),
            bounds.right_bottom(),
        ] {
            ui.painter().rect_filled(
                Rect::from_center_size(corner, Vec2::splat(6.0)),
                1.0,
                theme.mark_photo,
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo() -> Rect {
        Rect::from_min_size(egui::pos2(100.0, 50.0), Vec2::new(400.0, 200.0))
    }

    #[test]
    fn the_photo_rect_keeps_its_aspect() {
        let rect = photo_rect(
            Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(500.0, 500.0)),
            400,
            200,
        );
        assert!((rect.width() - 500.0).abs() < 1e-4);
        assert!((rect.height() - 250.0).abs() < 1e-4);
    }

    #[test]
    fn a_drag_becomes_a_normalized_rect() {
        let rect = drag_rect(photo(), egui::pos2(200.0, 100.0), egui::pos2(400.0, 200.0));
        assert!((rect[0] - 0.25).abs() < 1e-9);
        assert!((rect[1] - 0.25).abs() < 1e-9);
        assert!((rect[2] - 0.75).abs() < 1e-9);
        assert!((rect[3] - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_drag_outside_the_photo_is_clamped() {
        let rect = drag_rect(photo(), egui::pos2(-20.0, -20.0), egui::pos2(900.0, 900.0));
        assert_eq!(rect, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn a_click_without_a_size_makes_no_crop() {
        let rect = drag_rect(photo(), egui::pos2(200.0, 100.0), egui::pos2(201.0, 101.0));
        assert_eq!(rect, [0.0, 0.0, 1.0, 1.0]);
    }
}
