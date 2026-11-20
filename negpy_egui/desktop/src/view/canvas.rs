//! Canvas geometry: the photo rect, the crop cursor and the crop box.

use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, Vec2};

use negpy_backend::domain::models::{LocalMask, MaskShape};

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

/// A mask the canvas lasso drew, in normalized photo coordinates.
pub type DrawnMask = (MaskShape, Vec<[f64; 2]>);

/// The in-progress local mask lasso.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaskDrag {
    pub points: Vec<egui::Pos2>,
    pub start: Option<egui::Pos2>,
}

/// Paint every stored mask and let the lasso collect points inside the photo.
pub fn local_tool(
    ui: &mut egui::Ui,
    photo: Rect,
    masks: &[LocalMask],
    drawing: bool,
    drag: &mut MaskDrag,
    oval: bool,
) -> Option<DrawnMask> {
    let theme = Theme::default();
    for mask in masks {
        let points: Vec<egui::Pos2> = mask
            .vertices
            .iter()
            .map(|point| {
                egui::pos2(
                    photo.left() + point[0] as f32 * photo.width(),
                    photo.top() + point[1] as f32 * photo.height(),
                )
            })
            .collect();
        if points.len() >= 2 {
            ui.painter().add(egui::Shape::closed_line(
                points,
                Stroke::new(1.5_f32, theme.mark_photo),
            ));
        }
    }
    let response = ui.interact(
        photo,
        ui.id().with("local-photo"),
        if oval { Sense::drag() } else { Sense::click() },
    );
    if oval {
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
        if response.drag_started() {
            drag.start = response.interact_pointer_pos();
        }
        if response.drag_stopped()
            && let (Some(start), Some(end)) = (drag.start, response.interact_pointer_pos())
        {
            drag.start = None;
            if (end - start).length() >= 8.0 {
                let center = egui::pos2((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
                let points = [
                    center,
                    egui::pos2(end.x, center.y),
                    egui::pos2(center.x, end.y),
                ];
                let vertices = points
                    .iter()
                    .map(|point| {
                        [
                            f64::from((point.x - photo.left()) / photo.width().max(1.0)),
                            f64::from((point.y - photo.top()) / photo.height().max(1.0)),
                        ]
                    })
                    .collect();
                return Some((MaskShape::Oval, vertices));
            }
        }
        if let (Some(start), Some(current)) = (drag.start, response.interact_pointer_pos()) {
            let box_rect = Rect::from_two_pos(start, current);
            ui.painter().rect_stroke(
                box_rect,
                0.0,
                Stroke::new(1.5_f32, theme.warn_amber),
                egui::StrokeKind::Inside,
            );
        }
        return None;
    }
    if drawing {
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
        if response.clicked() {
            if let Some(point) = response.interact_pointer_pos() {
                drag.points.push(point);
            }
        }
        if response.double_clicked() {
            let points: Vec<[f64; 2]> = drag
                .points
                .drain(..)
                .map(|point| {
                    [
                        f64::from((point.x - photo.left()) / photo.width().max(1.0)),
                        f64::from((point.y - photo.top()) / photo.height().max(1.0)),
                    ]
                })
                .collect();
            if points.len() >= 3 {
                return Some((MaskShape::Polygon, points));
            }
        }
    } else {
        drag.points.clear();
    }
    if drag.points.len() >= 2 {
        ui.painter().add(egui::Shape::line(
            drag.points.clone(),
            Stroke::new(1.5_f32, theme.warn_amber),
        ));
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

    #[test]
    fn a_drawn_point_becomes_a_normalized_vertex() {
        let photo = photo();
        let point = egui::pos2(200.0, 100.0);
        let normalized = [
            f64::from((point.x - photo.left()) / photo.width()),
            f64::from((point.y - photo.top()) / photo.height()),
        ];
        assert!((normalized[0] - 0.25).abs() < 1e-9);
        assert!((normalized[1] - 0.25).abs() < 1e-9);
    }

    #[test]
    fn an_oval_drag_makes_center_and_axis_points() {
        let start = egui::pos2(100.0, 100.0);
        let end = egui::pos2(300.0, 200.0);
        let center = egui::pos2((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
        assert!((center.x - 200.0).abs() < 1e-9);
        assert!((center.y - 150.0).abs() < 1e-9);
    }
}
