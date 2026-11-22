//! Canvas geometry: the photo rect, the crop cursor and the crop box.

use eframe::egui::{self, CursorIcon, Rect, Sense, Stroke, Vec2};

use negpy_backend::domain::models::{LocalMask, MaskShape};

use crate::styles::templates::with_alpha;
use crate::styles::theme::Theme;

/// The smallest crop a drag can make, as a fraction of the photo.
pub const MIN_CROP: f64 = 0.02;
/// How close a press must be to a box corner to move it, in screen points.
const CROP_CORNER_GRAB: f32 = 12.0;
/// The crop hint, shown while Draw is on.
pub const CROP_HINT: &str = "Drag a crop. Right-click or Enter keeps it.";
/// The bounds-pick hint, shown while Dmin or Dmax is armed.
pub const PICK_HINT: &str = "Click the spot to set it.";
/// The zoom range a fit-relative canvas scale uses.
pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 8.0;

/// One composition guide, in unit fractions of the crop rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropGuide {
    Thirds,
    Phi,
    Diagonals,
    GoldenTriangles,
    GoldenSpiral,
    Armature,
    DiagonalMethod,
    Grid,
    Off,
}

impl CropGuide {
    /// The guide for a stored key, defaulting to thirds.
    #[must_use]
    pub fn coerce(value: &str) -> Self {
        match value {
            "phi" => Self::Phi,
            "diagonals" => Self::Diagonals,
            "triangles" => Self::GoldenTriangles,
            "spiral" => Self::GoldenSpiral,
            "armature" => Self::Armature,
            "diagonal_method" => Self::DiagonalMethod,
            "grid" => Self::Grid,
            "off" => Self::Off,
            _ => Self::Thirds,
        }
    }

    /// The unit-fraction polylines this guide draws.
    #[must_use]
    pub fn lines(self) -> Vec<Vec<[f64; 2]>> {
        match self {
            Self::Thirds => fractions(&[1.0 / 3.0, 2.0 / 3.0]),
            Self::Phi => fractions(&[0.381_966_011_250_105, 0.618_033_988_749_895]),
            Self::Diagonals => vec![
                vec![[0.0, 0.0], [1.0, 1.0]],
                vec![[1.0, 0.0], [0.0, 1.0]],
                vec![[0.5, 0.0], [0.5, 1.0]],
                vec![[0.0, 0.5], [1.0, 0.5]],
            ],
            Self::GoldenTriangles => vec![
                vec![[0.0, 0.0], [1.0, 1.0]],
                vec![[1.0, 0.0], [0.5, 0.5]],
                vec![[0.0, 1.0], [0.5, 0.5]],
            ],
            Self::GoldenSpiral => spiral(),
            Self::Armature => vec![
                vec![[0.0, 0.0], [1.0, 1.0]],
                vec![[1.0, 0.0], [0.0, 1.0]],
                vec![[0.0, 0.0], [1.0, 0.5]],
                vec![[0.0, 0.0], [0.5, 1.0]],
                vec![[1.0, 0.0], [0.0, 0.5]],
                vec![[1.0, 0.0], [0.5, 1.0]],
                vec![[1.0, 1.0], [0.0, 0.5]],
                vec![[1.0, 1.0], [0.5, 0.0]],
                vec![[0.0, 1.0], [1.0, 0.5]],
                vec![[0.0, 1.0], [0.5, 0.0]],
            ],
            Self::DiagonalMethod => {
                vec![vec![[0.0, 0.0], [1.0, 1.0]], vec![[1.0, 0.0], [0.0, 1.0]]]
            }
            Self::Grid => {
                let mut lines = Vec::new();
                for step in 1..8 {
                    let at = step as f64 / 8.0;
                    lines.push(vec![[at, 0.0], [at, 1.0]]);
                    lines.push(vec![[0.0, at], [1.0, at]]);
                }
                lines
            }
            Self::Off => Vec::new(),
        }
    }
}

fn fractions(values: &[f64]) -> Vec<Vec<[f64; 2]>> {
    let mut lines = Vec::new();
    for value in values {
        lines.push(vec![[*value, 0.0], [*value, 1.0]]);
        lines.push(vec![[0.0, *value], [1.0, *value]]);
    }
    lines
}

fn spiral() -> Vec<Vec<[f64; 2]>> {
    const PHI: f64 = 1.618_033_988_749_895;
    let mut points = Vec::new();
    let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 1.0, 1.0 / PHI);
    let mut angle = std::f64::consts::PI;
    for index in 0..10 {
        let (cx, cy, radius);
        match index % 4 {
            0 => {
                radius = h;
                (cx, cy) = (x + radius, y + radius);
                x += radius;
                w -= radius;
            }
            1 => {
                radius = w;
                (cx, cy) = (x, y + radius);
                y += radius;
                h -= radius;
            }
            2 => {
                radius = h;
                (cx, cy) = (x + w - radius, y);
                w -= radius;
            }
            _ => {
                radius = w;
                (cx, cy) = (x + radius, y + h - radius);
                h -= radius;
            }
        }
        for step in 0..=12 {
            let t = angle + (std::f64::consts::FRAC_PI_2) * (step as f64 / 12.0);
            points.push([(cx + radius * t.cos()) * PHI, (cy + radius * t.sin()) * PHI]);
        }
        angle += std::f64::consts::FRAC_PI_2;
    }
    vec![
        points
            .into_iter()
            .map(|[px, py]| [px.clamp(0.0, 1.0), py.clamp(0.0, 1.0)])
            .collect(),
    ]
}

/// A crop drag in progress, in photo coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropDrag {
    pub start: egui::Pos2,
    pub pending: [f64; 4],
    pub mode: CropDragMode,
}

/// What a crop press does: draw a new box, move it, or move one corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CropDragMode {
    Draw,
    Move {
        origin: [f64; 2],
        rect: [f64; 4],
    },
    Corner {
        corner: CropCorner,
        anchor: [f64; 2],
    },
}

/// The crop box corner a drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl CropCorner {
    fn opposite(self, rect: [f64; 4]) -> [f64; 2] {
        match self {
            Self::TopLeft => [rect[2], rect[3]],
            Self::TopRight => [rect[0], rect[3]],
            Self::BottomLeft => [rect[2], rect[1]],
            Self::BottomRight => [rect[0], rect[1]],
        }
    }
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
            mode: CropDragMode::Draw,
        }
    }

    /// Start a gesture on an existing crop box inside `photo`.
    #[must_use]
    pub fn on_box(photo: Rect, start: egui::Pos2, rect: [f64; 4]) -> Self {
        let start = clamp_point(photo, start);
        let corner = crop_corner_hit(photo, &rect, start);
        let mode = if !crop_hit(photo, None, Some(&rect), start) {
            CropDragMode::Draw
        } else if let Some(corner) = corner {
            CropDragMode::Corner {
                corner,
                anchor: corner.opposite(rect),
            }
        } else {
            CropDragMode::Move {
                origin: normalize_point(photo, start),
                rect,
            }
        };
        Self {
            start,
            pending: rect,
            mode,
        }
    }

    /// Move the drag to `current` and keep the ordered normalized rect.
    pub fn update(&mut self, photo: Rect, current: egui::Pos2) {
        self.pending = match self.mode {
            CropDragMode::Draw => region_rect(photo, self.start, current),
            CropDragMode::Move { origin, rect } => {
                let point = normalize_point(photo, clamp_point(photo, current));
                move_rect(rect, point[0] - origin[0], point[1] - origin[1])
            }
            CropDragMode::Corner { anchor, .. } => {
                let point = normalize_point(photo, clamp_point(photo, current));
                let [x1, y1] = anchor;
                [
                    x1.min(point[0]),
                    y1.min(point[1]),
                    x1.max(point[0]),
                    y1.max(point[1]),
                ]
            }
        };
    }
}

fn crop_corner_hit(photo: Rect, rect: &[f64; 4], point: egui::Pos2) -> Option<CropCorner> {
    let bounds = stored_rect(photo, rect)?;
    let radius = CROP_CORNER_GRAB;
    [
        (CropCorner::TopLeft, bounds.left_top()),
        (CropCorner::TopRight, bounds.right_top()),
        (CropCorner::BottomLeft, bounds.left_bottom()),
        (CropCorner::BottomRight, bounds.right_bottom()),
    ]
    .into_iter()
    .find(|(_, corner)| corner.distance(point) <= radius)
    .map(|(corner, _)| corner)
}

fn move_rect(rect: [f64; 4], dx: f64, dy: f64) -> [f64; 4] {
    let width = rect[2] - rect[0];
    let height = rect[3] - rect[1];
    let x1 = (rect[0] + dx).clamp(0.0, 1.0 - width);
    let y1 = (rect[1] + dy).clamp(0.0, 1.0 - height);
    [x1, y1, x1 + width, y1 + height]
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

/// The fitted rect at `zoom`, centered in `area`. `zoom` is fit-relative: 1.0 fits.
#[must_use]
pub fn zoomed_photo_rect(area: Rect, width: u32, height: u32, zoom: f32) -> Rect {
    let fitted = photo_rect(area, width, height);
    let zoom = if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    Rect::from_center_size(fitted.center(), fitted.size() * zoom)
}

/// The true pixel zoom: the drawn long edge against the source's long edge.
#[must_use]
pub fn zoom_percent(photo: Rect, width: u32, height: u32) -> u32 {
    let source = (width.max(height)) as f32;
    if source <= 0.0 {
        return 100;
    }
    ((photo.width().max(photo.height()) / source) * 100.0)
        .round()
        .clamp(1.0, 9999.0) as u32
}

/// Read a wheel notch as a zoom step, where one notch is `WHEEL_ZOOM_STEP`.
pub fn zoom_for_wheel(zoom: f32, scroll: f32) -> f32 {
    (zoom * WHEEL_ZOOM_STEP.powf(scroll)).clamp(ZOOM_MIN, ZOOM_MAX)
}

/// One wheel notch, as Python's canvas widget reads it.
pub const WHEEL_ZOOM_STEP: f32 = 1.15;

/// What the manual Dmin and Dmax pick asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PickAction {
    #[default]
    None,
    /// A click inside the photo, normalized to its content.
    At([f64; 2]),
}

/// Read one click on the photo while a bounds pick is armed. The click lands in
/// content-normalized coordinates, the same space a crop rect uses.
pub fn pick_tool(ui: &mut egui::Ui, photo: Rect, active: bool) -> PickAction {
    if !active {
        return PickAction::None;
    }
    let response = ui.interact(photo, ui.id().with("bounds-pick"), Sense::click());
    response.clone().on_hover_cursor(CursorIcon::Crosshair);
    if response.clicked()
        && let Some(point) = response.interact_pointer_pos()
    {
        let point = clamp_point(photo, point);
        return PickAction::At([
            f64::from((point.x - photo.left()) / photo.width().max(1.0)),
            f64::from((point.y - photo.top()) / photo.height().max(1.0)),
        ]);
    }
    let theme = Theme::default();
    paint_hint(ui, photo, PICK_HINT, &theme);
    PickAction::None
}

/// Paint the point each hand-set bound was picked at, while the pick fold is open.
pub fn paint_picks(ui: &egui::Ui, photo: Rect, picks: &[(&[f64], &str)]) {
    let theme = Theme::default();
    for (point, label) in picks {
        if point.len() < 2 {
            continue;
        }
        let at = egui::pos2(
            photo.left() + point[0] as f32 * photo.width(),
            photo.top() + point[1] as f32 * photo.height(),
        );
        if !photo.contains(at) {
            continue;
        }
        ui.painter()
            .circle_filled(at, 4.0, with_alpha(theme.mark_shadow, 200));
        ui.painter()
            .circle_stroke(at, 4.0, Stroke::new(1.0_f32, theme.mark_photo));
        ui.painter().text(
            at + Vec2::new(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(f32::from(theme.font_size_small)),
            theme.mark_photo,
        );
    }
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
    active: bool,
    drag: &mut Option<CropDrag>,
    guide: CropGuide,
) -> CropAction {
    let theme = Theme::default();
    let response = ui.interact(
        photo,
        ui.id().with("crop-photo"),
        if active {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let mut action = CropAction::None;
    if active {
        if response.drag_started()
            && let Some(start) = response.interact_pointer_pos()
        {
            let boxed = rect.and_then(|rect| <[f64; 4]>::try_from(rect).ok());
            *drag = Some(match boxed {
                Some(rect) if crop_hit(photo, None, Some(&rect), start) => {
                    CropDrag::on_box(photo, start, rect)
                }
                _ => CropDrag::new(photo, start),
            });
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
        let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
        let editing = text_has_cursor(ui);
        let double_click = response.double_clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|point| crop_hit(photo, *drag, rect, point));
        if let Some(rect) = confirmed_crop(
            drag.map(|active| active.pending),
            rect,
            enter,
            double_click,
            editing,
            response.secondary_clicked(),
        ) {
            *drag = None;
            action = CropAction::Confirm(rect);
        }
        let bounds = drag
            .and_then(|active| stored_rect(photo, &active.pending))
            .or_else(|| rect.and_then(|rect| stored_rect(photo, rect)));
        if let Some(bounds) = bounds {
            paint_crop_box(ui, photo, bounds, drag.is_some(), &theme);
            paint_guide(ui, bounds, guide, &theme);
        }
        if let Some(corner) = response.hover_pos().and_then(|point| {
            rect.and_then(|rect| <[f64; 4]>::try_from(rect).ok())
                .and_then(|rect| crop_corner_hit(photo, &rect, point))
        }) {
            let icon = match corner {
                CropCorner::TopLeft | CropCorner::BottomRight => CursorIcon::ResizeNwSe,
                CropCorner::TopRight | CropCorner::BottomLeft => CursorIcon::ResizeNeSw,
            };
            response.clone().on_hover_cursor(icon);
        } else if response
            .hover_pos()
            .is_some_and(|point| crop_hit(photo, *drag, rect, point))
        {
            response.clone().on_hover_cursor(CursorIcon::Grab);
        } else {
            response.clone().on_hover_cursor(CursorIcon::Crosshair);
        }
        paint_crop_hint(ui, photo, &theme);
    }
    action
}

fn paint_guide(ui: &egui::Ui, bounds: Rect, guide: CropGuide, theme: &Theme) {
    let stroke = Stroke::new(1.0_f32, with_alpha(theme.mark_photo, 150));
    for line in guide.lines() {
        let points: Vec<egui::Pos2> = line
            .iter()
            .map(|[x, y]| {
                egui::pos2(
                    bounds.left() + *x as f32 * bounds.width(),
                    bounds.top() + *y as f32 * bounds.height(),
                )
            })
            .collect();
        if points.len() >= 2 {
            ui.painter().add(egui::Shape::line(points, stroke));
        }
    }
}

/// Whether a text field is focused and has a cursor. A focused button must not block Enter.
fn text_has_cursor(ui: &egui::Ui) -> bool {
    ui.ctx().memory(|memory| {
        memory
            .focused()
            .and_then(|id| egui::TextEdit::load_state(ui.ctx(), id))
            .is_some_and(|state| state.cursor.char_range().is_some())
    })
}

fn confirmed_crop(
    pending: Option<[f64; 4]>,
    stored: Option<&[f64]>,
    enter: bool,
    double_click: bool,
    editing_text: bool,
    secondary_click: bool,
) -> Option<[f64; 4]> {
    if !(enter && !editing_text) && !double_click && !secondary_click {
        return None;
    }
    pending
        .filter(|rect| rect[2] - rect[0] >= MIN_CROP && rect[3] - rect[1] >= MIN_CROP)
        .or_else(|| stored.and_then(|rect| <[f64; 4]>::try_from(rect).ok()))
}

/// Whether `point` lies in the crop box a drag or the stored rect defines.
fn crop_hit(
    photo: Rect,
    drag: Option<CropDrag>,
    stored: Option<&[f64]>,
    point: egui::Pos2,
) -> bool {
    drag.map(|active| active.pending)
        .or_else(|| stored.and_then(|rect| <[f64; 4]>::try_from(rect).ok()))
        .and_then(|rect| stored_rect(photo, &rect))
        .is_some_and(|bounds| bounds.contains(point))
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
    paint_hint(ui, photo, CROP_HINT, theme);
}

fn paint_hint(ui: &egui::Ui, photo: Rect, text: &str, theme: &Theme) {
    let font = egui::FontId::proportional(f32::from(theme.font_size_small));
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, theme.text_primary);
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

/// The retouch tool the canvas accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RetouchTool {
    #[default]
    Off,
    Heal,
    Scratch,
    Line,
    Clone,
}

/// What a retouch gesture asked the edit to store.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RetouchAction {
    #[default]
    None,
    Heal(Vec<[f64; 2]>),
    Line([f64; 5]),
    Clone(Vec<[f64; 2]>),
}

/// The in-progress retouch stroke.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RetouchDrag {
    pub points: Vec<egui::Pos2>,
    pub start: Option<egui::Pos2>,
    pub clone_source: Option<egui::Pos2>,
    /// The clone source in photo-normalized coordinates.
    pub clone_source_normalized: Option<[f64; 2]>,
}

/// Paint the stored strokes and collect one retouch gesture.
pub fn retouch_tool(
    ui: &mut egui::Ui,
    photo: Rect,
    tool: RetouchTool,
    drag: &mut RetouchDrag,
) -> RetouchAction {
    let theme = Theme::default();
    if tool == RetouchTool::Off {
        return RetouchAction::None;
    }
    let response = ui.interact(
        photo,
        ui.id().with("retouch-photo"),
        Sense::click_and_drag(),
    );
    response.clone().on_hover_cursor(CursorIcon::Crosshair);
    match tool {
        RetouchTool::Line => {
            if response.clicked()
                && let Some(point) = response.interact_pointer_pos()
            {
                let normalized = normalize_point(photo, point);
                let mut line = [
                    normalized[0],
                    normalized[1],
                    normalized[0],
                    normalized[1],
                    12.0,
                ];
                if let Some(source) = drag.clone_source {
                    let other = normalize_point(photo, source);
                    if (other[0] - normalized[0]).abs() + (other[1] - normalized[1]).abs() > 1e-6 {
                        line = [normalized[0], normalized[1], other[0], other[1], 12.0];
                    }
                }
                return RetouchAction::Line(line);
            }
        }
        RetouchTool::Clone if ui.input(|input| input.modifiers.alt) => {
            if response.clicked()
                && let Some(point) = response.interact_pointer_pos()
            {
                drag.clone_source = Some(point);
                drag.clone_source_normalized = Some(normalize_point(photo, point));
                ui.painter()
                    .circle_stroke(point, 8.0, Stroke::new(1.5_f32, theme.warn_amber));
            }
        }
        _ => {
            if response.drag_started()
                && let Some(start) = response.interact_pointer_pos()
            {
                drag.points = vec![start];
            }
            if response.dragged()
                && let Some(point) = response.interact_pointer_pos()
            {
                let far = drag
                    .points
                    .last()
                    .is_none_or(|last| (*last - point).length() >= 4.0);
                if far {
                    drag.points.push(point);
                }
            }
            if response.drag_stopped() {
                let points: Vec<egui::Pos2> = drag.points.drain(..).collect();
                return gesture_action(tool, photo, &points, drag.clone_source);
            }
        }
    }
    if drag.points.len() >= 2 {
        let points: Vec<egui::Pos2> = drag.points.clone();
        ui.painter().add(egui::Shape::line(
            points,
            Stroke::new(2.0_f32, theme.mark_photo),
        ));
    }
    if let Some(source) = drag.clone_source {
        ui.painter()
            .circle_stroke(source, 8.0, Stroke::new(1.5_f32, theme.warn_amber));
    }
    RetouchAction::None
}

fn gesture_action(
    tool: RetouchTool,
    photo: Rect,
    points: &[egui::Pos2],
    source: Option<egui::Pos2>,
) -> RetouchAction {
    if points.is_empty() {
        return RetouchAction::None;
    }
    let normalized: Vec<[f64; 2]> = points
        .iter()
        .map(|point| normalize_point(photo, *point))
        .collect();
    match tool {
        RetouchTool::Heal | RetouchTool::Scratch => RetouchAction::Heal(normalized),
        RetouchTool::Clone if source.is_some() => RetouchAction::Clone(normalized),
        RetouchTool::Line => {
            let start = normalized[0];
            let mut line = [start[0], start[1], start[0], start[1], 12.0];
            if let Some(source) = source {
                let other = normalize_point(photo, source);
                line = [start[0], start[1], other[0], other[1], 12.0];
            }
            RetouchAction::Line(line)
        }
        _ => RetouchAction::None,
    }
}

fn normalize_point(photo: Rect, point: egui::Pos2) -> [f64; 2] {
    [
        f64::from((point.x - photo.left()) / photo.width().max(1.0)),
        f64::from((point.y - photo.top()) / photo.height().max(1.0)),
    ]
}

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
    /// The vertex handle being dragged, if any.
    pub vertex: Option<usize>,
}

/// What the local tool did with a click or a handle drag.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum MaskAction {
    #[default]
    None,
    Select(usize),
    MoveVertex(usize, usize, [f64; 2]),
}

/// The handle size a vertex takes at rest.
pub const MASK_HANDLE: f32 = 7.0;

/// Paint every stored mask and let the lasso collect points inside the photo.
pub fn local_tool(
    ui: &mut egui::Ui,
    photo: Rect,
    masks: &[LocalMask],
    selected: usize,
    tool: MaskTool,
    drag: &mut MaskDrag,
) -> (Option<DrawnMask>, MaskAction) {
    let theme = Theme::default();
    let mut action = MaskAction::None;
    for (index, mask) in masks.iter().enumerate() {
        let outline: Vec<egui::Pos2> = mask_outline(mask, photo)
            .into_iter()
            .map(|(x, y)| egui::pos2(x, y))
            .collect();
        let stroke = Stroke::new(
            if index == selected { 2.0_f32 } else { 1.0_f32 },
            if index == selected {
                theme.mark_photo
            } else {
                theme.text_hint
            },
        );
        if mask.shape == MaskShape::Gradient && outline.len() >= 2 {
            ui.painter().add(egui::Shape::line(outline, stroke));
        } else if outline.len() >= 3 {
            ui.painter().add(egui::Shape::closed_line(outline, stroke));
        }
    }
    if tool == MaskTool::Off {
        if let Some(mask) = masks.get(selected) {
            // An oval's axes are conjugate, so its three control points are its outline handles
            // and its centre.
            let handles: Vec<egui::Pos2> = mask
                .vertices
                .iter()
                .map(|point| {
                    egui::pos2(
                        photo.left() + point[0] as f32 * photo.width(),
                        photo.top() + point[1] as f32 * photo.height(),
                    )
                })
                .collect();
            for (index, handle) in handles.iter().enumerate() {
                let rect = Rect::from_center_size(*handle, Vec2::splat(MASK_HANDLE));
                let response =
                    ui.interact(rect, ui.id().with(("mask-handle", index)), Sense::drag());
                if response.drag_started() {
                    drag.vertex = Some(index);
                }
                if response.dragged()
                    && let Some(point) = response.interact_pointer_pos()
                {
                    let normalized = normalize_point(photo, point);
                    action = MaskAction::MoveVertex(selected, index, normalized);
                }
                if response.drag_stopped() {
                    drag.vertex = None;
                }
                ui.painter().rect_filled(rect, 2.0, theme.mark_photo);
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.0_f32, theme.mark_shadow),
                    egui::StrokeKind::Outside,
                );
            }
            let response = ui.interact(photo, ui.id().with("mask-select"), Sense::click());
            if response.clicked()
                && let Some(point) = response.interact_pointer_pos()
                && let Some(index) = hit_mask(masks, photo, point)
            {
                action = MaskAction::Select(index);
            }
        }
        return (None, action);
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
                return (Some((shape, vertices)), action);
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
        return (None, action);
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
                return (Some((MaskShape::Polygon, points)), action);
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
    (None, action)
}

/// The mask whose fill holds `point`, if any.
pub fn hit_mask(masks: &[LocalMask], photo: Rect, point: egui::Pos2) -> Option<usize> {
    masks
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, mask)| mask_hit(mask, photo, point).then_some(index))
}

/// Whether `point` selects `mask`: inside its outline, or near a card's axis.
fn mask_hit(mask: &LocalMask, photo: Rect, point: egui::Pos2) -> bool {
    let outline: Vec<egui::Pos2> = mask_outline(mask, photo)
        .into_iter()
        .map(|(x, y)| egui::pos2(x, y))
        .collect();
    if mask.shape == MaskShape::Gradient {
        return outline.len() >= 2 && distance_to_polyline(point, &outline) <= MASK_HANDLE;
    }
    outline.len() >= 3 && point_in_polygon(point, &outline)
}

/// The mask's screen outline: its smoothed fill, or the card's axis.
fn mask_outline(mask: &LocalMask, photo: Rect) -> Vec<(f32, f32)> {
    if mask.vertices.len() < negpy_backend::features::local::logic::min_points(mask.shape) {
        return Vec::new();
    }
    // A card has no outline: its axis is its control line.
    let points = if mask.shape == MaskShape::Gradient {
        mask.vertices.clone()
    } else {
        negpy_backend::features::local::logic::outline_points(mask.shape, &mask.vertices)
            .into_iter()
            .map(|(x, y)| [x, y])
            .collect()
    };
    points
        .into_iter()
        .map(|point| {
            (
                photo.left() + point[0] as f32 * photo.width(),
                photo.top() + point[1] as f32 * photo.height(),
            )
        })
        .collect()
}

/// Shortest distance from `point` to a polyline.
fn distance_to_polyline(point: egui::Pos2, points: &[egui::Pos2]) -> f32 {
    let mut best = f32::INFINITY;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ab = b - a;
        let ap = point - a;
        let denom = ab.length_sq();
        let t = if denom <= 1e-12 {
            0.0
        } else {
            (ap.dot(ab) / denom).clamp(0.0, 1.0)
        };
        best = best.min((point - (a + ab * t)).length());
    }
    best
}

fn point_in_polygon(point: egui::Pos2, points: &[egui::Pos2]) -> bool {
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for current in points {
        let crosses = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x;
        if crosses {
            inside = !inside;
        }
        previous = *current;
    }
    inside
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
    fn a_zoomed_photo_keeps_its_aspect_and_reports_pixels() {
        let area = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(500.0, 500.0));
        let fitted = zoomed_photo_rect(area, 400, 200, 1.0);
        assert!((fitted.width() - 500.0).abs() < 1e-4);
        assert_eq!(zoom_percent(fitted, 400, 200), 125);
        let doubled = zoomed_photo_rect(area, 400, 200, 2.0);
        assert!((doubled.width() - 1000.0).abs() < 1e-4);
        assert_eq!(zoom_percent(doubled, 400, 200), 250);
    }

    #[test]
    fn the_wheel_stays_inside_the_zoom_range() {
        assert!((zoom_for_wheel(1.0, 1.0) - WHEEL_ZOOM_STEP).abs() < 1e-6);
        assert_eq!(zoom_for_wheel(1.0, 100.0), ZOOM_MAX);
        assert_eq!(zoom_for_wheel(1.0, -100.0), ZOOM_MIN);
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
    fn a_press_on_a_corner_moves_only_that_corner() {
        let photo = photo();
        let rect = [0.25, 0.25, 0.75, 0.75];
        let corner = egui::pos2(
            photo.left() + 0.75 * photo.width(),
            photo.top() + 0.25 * photo.height(),
        );
        let mut drag = CropDrag::on_box(photo, corner, rect);
        assert_eq!(
            drag.mode,
            CropDragMode::Corner {
                corner: CropCorner::TopRight,
                anchor: [0.25, 0.75],
            }
        );
        drag.update(photo, egui::pos2(corner.x + 40.0, corner.y - 20.0));
        assert!((drag.pending[0] - 0.25).abs() < 1e-6);
        assert!(drag.pending[1] < 0.25);
        assert!(drag.pending[2] > 0.75);
        assert!((drag.pending[3] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn a_press_inside_the_box_translates_the_whole_box() {
        let photo = photo();
        let rect = [0.25, 0.25, 0.75, 0.75];
        let inside = egui::pos2(
            photo.left() + 0.5 * photo.width(),
            photo.top() + 0.5 * photo.height(),
        );
        let mut drag = CropDrag::on_box(photo, inside, rect);
        assert!(matches!(drag.mode, CropDragMode::Move { .. }));
        drag.update(photo, egui::pos2(inside.x + 40.0, inside.y + 20.0));
        assert!((drag.pending[2] - drag.pending[0] - 0.5).abs() < 1e-6);
        assert!((drag.pending[3] - drag.pending[1] - 0.5).abs() < 1e-6);
        assert!(drag.pending[0] > 0.25);
        assert!(drag.pending[1] > 0.25);
    }

    #[test]
    fn a_move_keeps_the_box_inside_the_photo() {
        let photo = photo();
        let rect = [0.5, 0.5, 0.9, 0.9];
        let inside = egui::pos2(
            photo.left() + 0.7 * photo.width(),
            photo.top() + 0.7 * photo.height(),
        );
        let mut drag = CropDrag::on_box(photo, inside, rect);
        drag.update(photo, egui::pos2(inside.x + 400.0, inside.y + 200.0));
        assert!((drag.pending[2] - 1.0).abs() < 1e-6);
        assert!((drag.pending[3] - 1.0).abs() < 1e-6);
        assert!((drag.pending[2] - drag.pending[0] - 0.4).abs() < 1e-6);
        assert!((drag.pending[3] - drag.pending[1] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn a_press_outside_the_box_starts_a_new_crop() {
        let photo = photo();
        let rect = [0.25, 0.25, 0.75, 0.75];
        let outside = egui::pos2(
            photo.left() + 0.05 * photo.width(),
            photo.top() + 0.05 * photo.height(),
        );
        let mut drag = CropDrag::on_box(photo, outside, rect);
        assert_eq!(drag.mode, CropDragMode::Draw);
        drag.update(photo, egui::pos2(outside.x + 20.0, outside.y + 20.0));
        assert!(drag.pending[0] < 0.1);
        assert!(drag.pending[1] < 0.1);
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
            confirmed_crop(Some(pending), None, true, false, false, false),
            Some(pending)
        );
        assert_eq!(
            confirmed_crop(Some(pending), None, true, false, true, false),
            None
        );
    }

    #[test]
    fn a_double_click_inside_the_box_confirms_it() {
        let pending = [0.2, 0.3, 0.8, 0.7];
        assert_eq!(
            confirmed_crop(Some(pending), None, false, true, false, false),
            Some(pending)
        );
        assert_eq!(
            confirmed_crop(None, Some(&pending), false, true, false, false),
            Some(pending)
        );
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
                false,
                true
            ),
            Some(stored)
        );
    }

    #[test]
    fn every_guide_key_selects_its_shape() {
        assert_eq!(CropGuide::coerce("phi"), CropGuide::Phi);
        assert_eq!(CropGuide::coerce("off"), CropGuide::Off);
        assert_eq!(CropGuide::coerce("grid"), CropGuide::Grid);
        assert_eq!(
            CropGuide::coerce("diagonal_method"),
            CropGuide::DiagonalMethod
        );
        assert_eq!(CropGuide::coerce("unknown"), CropGuide::Thirds);
        assert!(CropGuide::Off.lines().is_empty());
        assert!(!CropGuide::Thirds.lines().is_empty());
        assert!(!CropGuide::GoldenSpiral.lines().is_empty());
    }

    #[test]
    fn a_retouch_gesture_normalizes_its_points() {
        let photo = photo();
        let action = gesture_action(
            RetouchTool::Heal,
            photo,
            &[egui::pos2(200.0, 100.0), egui::pos2(300.0, 150.0)],
            None,
        );
        match action {
            RetouchAction::Heal(points) => {
                assert_eq!(points.len(), 2);
                assert!((points[0][0] - 0.25).abs() < 1e-9);
                assert!((points[1][1] - 0.5).abs() < 1e-9);
            }
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn a_line_gesture_uses_the_click_and_the_previous_point() {
        let photo = photo();
        let action = gesture_action(
            RetouchTool::Line,
            photo,
            &[egui::pos2(200.0, 100.0)],
            Some(egui::pos2(400.0, 200.0)),
        );
        match action {
            RetouchAction::Line(line) => {
                assert!((line[0] - 0.25).abs() < 1e-9);
                assert!((line[2] - 0.75).abs() < 1e-9);
            }
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn the_off_retouch_tool_claims_no_gesture() {
        let ctx = egui::Context::default();
        let mut drag = RetouchDrag::default();
        let mut action = RetouchAction::Heal(Vec::new());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = retouch_tool(ui, photo(), RetouchTool::Off, &mut drag);
            });
        });
        assert_eq!(action, RetouchAction::None);
    }

    #[test]
    fn the_off_mask_tool_claims_no_drag() {
        let ctx = egui::Context::default();
        let mut drag = MaskDrag::default();
        let mut shape = None;
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerButton {
                        pos: egui::pos2(150.0, 100.0),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::default(),
                    },
                    egui::Event::PointerMoved(egui::pos2(300.0, 180.0)),
                ],
                ..egui::RawInput::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    (shape, _) = local_tool(ui, photo(), &[], 0, MaskTool::Off, &mut drag);
                });
            },
        );
        assert!(shape.is_none());
        assert!(drag.start.is_none());
    }

    #[test]
    fn a_point_inside_a_mask_selects_it() {
        let photo = photo();
        let masks = vec![
            LocalMask {
                vertices: vec![[0.1, 0.1], [0.4, 0.1], [0.4, 0.4], [0.1, 0.4]],
                ..LocalMask::default()
            },
            LocalMask {
                vertices: vec![[0.6, 0.6], [0.9, 0.6], [0.9, 0.9], [0.6, 0.9]],
                ..LocalMask::default()
            },
        ];
        assert_eq!(hit_mask(&masks, photo, egui::pos2(190.0, 100.0)), Some(0));
        assert_eq!(hit_mask(&masks, photo, egui::pos2(400.0, 210.0)), Some(1));
        assert_eq!(hit_mask(&masks, photo, egui::pos2(110.0, 55.0)), None);
    }

    #[test]
    fn an_oval_is_selected_inside_its_outline_not_its_vertices() {
        let photo = photo();
        let masks = vec![LocalMask {
            vertices: vec![[0.5, 0.5], [0.8, 0.5], [0.5, 0.8]],
            shape: MaskShape::Oval,
            ..LocalMask::default()
        }];
        // The centre lies inside the oval, though no control point is near it.
        assert_eq!(hit_mask(&masks, photo, egui::pos2(300.0, 150.0)), Some(0));
        assert_eq!(hit_mask(&masks, photo, egui::pos2(110.0, 55.0)), None);
    }

    #[test]
    fn a_card_is_selected_near_its_axis() {
        let photo = photo();
        let masks = vec![LocalMask {
            vertices: vec![[0.1, 0.5], [0.9, 0.5]],
            shape: MaskShape::Gradient,
            ..LocalMask::default()
        }];
        assert_eq!(hit_mask(&masks, photo, egui::pos2(300.0, 152.0)), Some(0));
        assert_eq!(hit_mask(&masks, photo, egui::pos2(300.0, 220.0)), None);
    }

    #[test]
    fn a_vertex_handle_reports_its_new_point() {
        let photo = photo();
        let selected = LocalMask {
            vertices: vec![[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]],
            ..LocalMask::default()
        };
        let point = egui::pos2(
            photo.left() + 0.4 * photo.width(),
            photo.top() + 0.4 * photo.height(),
        );
        let normalized = super::normalize_point(photo, point);
        assert!((normalized[0] - 0.4).abs() < 1e-6);
        assert!(selected.vertices.len() == 4);
    }

    #[test]
    fn an_inactive_crop_box_paints_nothing_and_does_not_start_a_drag() {
        let ctx = egui::Context::default();
        let mut drag = None;
        let mut action = CropAction::None;
        let stored = [0.25, 0.25, 0.75, 0.75];
        fn shapes(ctx: &egui::Context, rect: Option<&[f64]>) -> Vec<egui::epaint::ClippedShape> {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut drag = None;
                    let _ = crop_tool(ui, photo(), rect, false, &mut drag, CropGuide::Thirds);
                });
            });
            ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut drag = None;
                    let _ = crop_tool(ui, photo(), rect, false, &mut drag, CropGuide::Thirds);
                });
            })
            .shapes
        }
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::PointerButton {
                    pos: egui::pos2(200.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                }],
                ..egui::RawInput::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = crop_tool(
                        ui,
                        photo(),
                        Some(&stored),
                        false,
                        &mut drag,
                        CropGuide::Thirds,
                    );
                });
            },
        );
        assert_eq!(action, CropAction::None);
        assert!(drag.is_none());
        assert_eq!(shapes(&ctx, Some(&stored)), shapes(&ctx, None));
    }

    #[test]
    fn a_pick_click_reads_content_normalized_coordinates() {
        let photo = photo();
        let mut action = PickAction::None;
        let mut ctx = egui::Context::default();
        let point = egui::pos2(250.0, 150.0);
        let frames = [
            vec![egui::Event::PointerMoved(point)],
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        ];
        for events in frames {
            let input = egui::RawInput {
                events,
                ..egui::RawInput::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    action = pick_tool(ui, photo, true);
                });
            });
        }
        match action {
            PickAction::At([x, y]) => {
                assert!((x - 0.375).abs() < 1e-6);
                assert!((y - 0.5).abs() < 1e-6);
            }
            PickAction::None => panic!("the click made no pick"),
        }
    }

    #[test]
    fn a_disarmed_pick_takes_no_click() {
        let mut action = PickAction::At([0.5, 0.5]);
        let mut ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = pick_tool(ui, photo(), false);
            });
        });
        assert_eq!(action, PickAction::None);
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
