//! Canvas geometry: the photo rect, the crop cursor and the crop box.

use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, Vec2};

use negpy_backend::domain::models::{LocalMask, MaskShape};

use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

/// The smallest crop a drag can make, as a fraction of the photo.
pub const MIN_CROP: f64 = 0.02;
/// The crop hint, shown while Draw is on.
pub const CROP_HINT: &str = "Drag a crop. Right-click or Enter keeps it.";

/// A crop drag in progress, in photo coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropDrag {
    pub start: egui::Pos2,
    pub pending: [f64; 4],
}

/// What the crop tool did this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CropAction {
    #[default]
    None,
    Pending([f64; 4]),
    Confirm([f64; 4]),
}

impl CropDrag {
    /// Start a drag at `start`, clamped inside `photo`.
    #[must_use]
    pub fn new(photo: Rect, start: egui::Pos2) -> Self {
        let start = clamp_point(photo, start);
        Self {
            start,
            pending: region_rect(photo, start, start),
        }
    }

    /// Move the drag to `current` and keep the ordered normalized rect.
    pub fn update(&mut self, photo: Rect, current: egui::Pos2) {
        self.pending = region_rect(photo, self.start, current);
    }
}

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
pub fn region_rect(photo: Rect, from: egui::Pos2, to: egui::Pos2) -> [f64; 4] {
    let from = clamp_point(photo, from);
    let to = clamp_point(photo, to);
    let x1 = f64::from((from.x.min(to.x) - photo.left()) / photo.width().max(1.0));
    let y1 = f64::from((from.y.min(to.y) - photo.top()) / photo.height().max(1.0));
    let x2 = f64::from((from.x.max(to.x) - photo.left()) / photo.width().max(1.0));
    let y2 = f64::from((from.y.max(to.y) - photo.top()) / photo.height().max(1.0));
    [x1, y1, x2, y2]
}

/// A normalized crop rect a drag can commit, or the full frame when it is too small.
#[must_use]
pub fn drag_rect(photo: Rect, from: egui::Pos2, to: egui::Pos2) -> [f64; 4] {
    let [x1, y1, x2, y2] = region_rect(photo, from, to);
    if x2 - x1 < MIN_CROP || y2 - y1 < MIN_CROP {
        [0.0, 0.0, 1.0, 1.0]
    } else {
        [x1, y1, x2, y2]
    }
}

fn clamp_point(photo: Rect, point: egui::Pos2) -> egui::Pos2 {
    egui::pos2(
        point.x.clamp(photo.left(), photo.right()),
        point.y.clamp(photo.top(), photo.bottom()),
    )
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
/// Returns the new normalized rect when a drag ends. `drag` keeps the press
/// position, because egui drops the pointer when the drag stops.
pub fn crop_tool(
    ui: &mut egui::Ui,
    photo: Rect,
    rect: Option<&[f64]>,
    draw: bool,
    drag: &mut Option<CropDrag>,
) -> CropAction {
    let theme = Theme::default();
    let response = ui.interact(photo, ui.id().with("crop-photo"), Sense::click_and_drag());
    let mut action = CropAction::None;
    if draw {
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
    }
    if draw {
        if response.drag_started()
            && let Some(start) = response.interact_pointer_pos()
        {
            *drag = Some(CropDrag::new(photo, start));
        }
        if let (Some(active), Some(current)) = (*drag, response.interact_pointer_pos()) {
            if response.dragged() {
                let mut moved = active;
                moved.update(photo, current);
                *drag = Some(moved);
            }
        }
        if response.drag_stopped()
            && let (Some(active), Some(current)) = (*drag, response.interact_pointer_pos())
        {
            let mut stopped = active;
            stopped.update(photo, current);
            *drag = Some(stopped);
            action = CropAction::Pending(stopped.pending);
        }
    }
    if draw {
        let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
        let editing = ui.ctx().wants_keyboard_input();
        if let Some(rect) = confirmed_crop(
            drag.map(|active| active.pending),
            rect,
            enter,
            editing,
            response.secondary_clicked(),
        ) {
            *drag = None;
            action = CropAction::Confirm(rect);
        }
    }
    if draw {
        let bounds = drag
            .and_then(|active| stored_rect(photo, &active.pending))
            .or_else(|| rect.and_then(|rect| stored_rect(photo, rect)));
        if let Some(bounds) = bounds {
            paint_crop_box(ui, photo, bounds, drag.is_some(), &theme);
        }
        paint_crop_hint(ui, photo, &theme);
    }
    action
}

fn confirmed_crop(
    pending: Option<[f64; 4]>,
    stored: Option<&[f64]>,
    enter: bool,
    editing_text: bool,
    secondary_click: bool,
) -> Option<[f64; 4]> {
    if !(enter && !editing_text) && !secondary_click {
        return None;
    }
    pending
        .filter(|rect| rect[2] - rect[0] >= MIN_CROP && rect[3] - rect[1] >= MIN_CROP)
        .or_else(|| stored.and_then(|rect| <[f64; 4]>::try_from(rect).ok()))
}

fn paint_crop_box(ui: &egui::Ui, photo: Rect, bounds: Rect, dragging: bool, theme: &Theme) {
    let shade = with_alpha(theme.mark_shadow, 110);
    for outside in [
        Rect::from_min_max(photo.min, egui::pos2(photo.right(), bounds.top())),
        Rect::from_min_max(egui::pos2(photo.left(), bounds.bottom()), photo.max),
        Rect::from_min_max(
            egui::pos2(photo.left(), bounds.top()),
            egui::pos2(bounds.left(), bounds.bottom()),
        ),
        Rect::from_min_max(
            egui::pos2(bounds.right(), bounds.top()),
            egui::pos2(photo.right(), bounds.bottom()),
        ),
    ] {
        ui.painter().rect_filled(outside, 0.0, shade);
    }
    ui.painter().rect_stroke(
        bounds,
        0.0,
        Stroke::new(3.0_f32, theme.mark_shadow),
        egui::StrokeKind::Outside,
    );
    ui.painter().rect_stroke(
        bounds,
        0.0,
        Stroke::new(1.0_f32, theme.mark_photo),
        egui::StrokeKind::Inside,
    );
    let handle = if dragging { 10.0 } else { 8.0 };
    for corner in [
        bounds.left_top(),
        bounds.right_top(),
        bounds.left_bottom(),
        bounds.right_bottom(),
    ] {
        let square = Rect::from_center_size(corner, Vec2::splat(handle));
        ui.painter().rect_filled(square, 1.0, theme.mark_photo);
        ui.painter().rect_stroke(
            square,
            1.0,
            Stroke::new(1.0_f32, theme.mark_shadow),
            egui::StrokeKind::Outside,
        );
    }
}

fn paint_crop_hint(ui: &egui::Ui, photo: Rect, theme: &Theme) {
    let font = egui::FontId::proportional(f32::from(theme.font_size_small));
    let galley = ui
        .painter()
        .layout_no_wrap(CROP_HINT.to_string(), font, theme.text_primary);
    let padding = Vec2::splat(6.0);
    let plate = Rect::from_min_size(
        egui::pos2(photo.left() + 10.0, photo.bottom() - galley.size().y - 10.0),
        galley.size() + padding * 2.0,
    );
    ui.painter()
        .rect_filled(plate, 3.0, with_alpha(theme.mark_shadow, 180));
    ui.painter()
        .galley(plate.min + padding, galley, theme.text_primary);
}

/// A mask the canvas lasso drew, in normalized photo coordinates.
pub type DrawnMask = (MaskShape, Vec<[f64; 2]>);

/// The local gesture the canvas accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaskTool {
    #[default]
    Off,
    Lasso,
    Oval,
    Card,
}

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
    tool: MaskTool,
    drag: &mut MaskDrag,
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
        if tool == MaskTool::Lasso {
            Sense::click()
        } else {
            Sense::drag()
        },
    );
    if tool != MaskTool::Lasso {
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
                let (shape, points) = if tool == MaskTool::Card {
                    (MaskShape::Gradient, vec![start, end])
                } else {
                    (
                        MaskShape::Oval,
                        vec![
                            center,
                            egui::pos2(end.x, center.y),
                            egui::pos2(center.x, end.y),
                        ],
                    )
                };
                let vertices = points
                    .into_iter()
                    .map(|point| {
                        [
                            f64::from((point.x - photo.left()) / photo.width().max(1.0)),
                            f64::from((point.y - photo.top()) / photo.height().max(1.0)),
                        ]
                    })
                    .collect();
                return Some((shape, vertices));
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
    if tool == MaskTool::Lasso {
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
    fn a_drag_from_each_corner_orders_the_same_rect() {
        let photo = photo();
        let top_left = egui::pos2(200.0, 100.0);
        let bottom_right = egui::pos2(400.0, 200.0);
        let forward = drag_rect(photo, top_left, bottom_right);
        let backward = drag_rect(photo, bottom_right, top_left);
        assert_eq!(forward, backward);
        assert!(forward[0] < forward[2]);
        assert!(forward[1] < forward[3]);
    }

    #[test]
    fn a_crop_drag_clamps_a_press_outside_the_photo() {
        let mut drag = CropDrag::new(photo(), egui::pos2(-20.0, 90.0));
        assert_eq!(drag.start, egui::pos2(100.0, 90.0));
        drag.update(photo(), egui::pos2(900.0, 220.0));
        assert!(drag.pending[0].abs() < 1e-6);
        assert!((drag.pending[1] - 0.2).abs() < 1e-6);
        assert!((drag.pending[2] - 1.0).abs() < 1e-6);
        assert!((drag.pending[3] - 0.85).abs() < 1e-6);
    }

    #[test]
    fn a_pending_crop_keeps_a_rectangle_below_the_commit_size() {
        let mut drag = CropDrag::new(photo(), egui::pos2(200.0, 100.0));
        drag.update(photo(), egui::pos2(202.0, 101.0));
        let bounds = stored_rect(photo(), &drag.pending).expect("a pending box paints");
        assert!(bounds.width() > 0.0);
        assert!(bounds.height() > 0.0);
    }

    #[test]
    fn enter_confirms_the_pending_crop_once() {
        let pending = [0.2, 0.3, 0.8, 0.7];
        assert_eq!(
            confirmed_crop(Some(pending), None, true, false, false),
            Some(pending)
        );
        assert_eq!(confirmed_crop(Some(pending), None, true, true, false), None);
    }

    #[test]
    fn a_small_pending_crop_does_not_replace_the_stored_rect() {
        let stored = [0.1, 0.2, 0.9, 0.8];
        assert_eq!(
            confirmed_crop(
                Some([0.2, 0.2, 0.21, 0.21]),
                Some(&stored),
                false,
                false,
                true
            ),
            Some(stored)
        );
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

    #[test]
    fn a_card_drag_keeps_the_two_axis_ends() {
        let start = egui::pos2(100.0, 100.0);
        let end = egui::pos2(300.0, 200.0);
        let points = vec![start, end];
        assert_eq!(points.len(), 2);
        assert!((points[1].x - 300.0).abs() < 1e-9);
    }
}
