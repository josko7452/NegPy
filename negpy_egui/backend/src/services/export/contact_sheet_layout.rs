//! Mirrors `negpy/services/export/contact_sheet_layout.py`: sheet geometry in millimeters.

use serde_json::{Map, Value};

use crate::services::film_edge::geometry::FilmGeometry;

pub const MM_PER_INCH: f64 = 25.4;
pub const SHEET_MARGIN: f64 = 5.0;
pub const STRIP_GAP: f64 = 3.0;
pub const LABEL_BAND: f64 = 8.0;
pub const ROW_BORDER: f64 = 0.8;
pub const ROW_GAP: f64 = 0.5;
pub const MAX_STRETCH: f64 = 1.5;
pub const PAPER_MIN: f64 = 50.0;
pub const PAPER_MAX: f64 = 610.0;
pub const DPI_CHOICES: [u32; 3] = [150, 300, 600];
pub const DEFAULT_DPI: u32 = 300;
pub const MAX_SHEET_PIXELS: u64 = 89_000_000;

/// One Ilford sheet, portrait, at its true inch size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperSize {
    pub label: &'static str,
    pub width: f64,
    pub height: f64,
}

pub const ILFORD_PAPERS: [PaperSize; 11] = [
    PaperSize {
        label: "12.7 × 17.8 cm",
        width: 127.0,
        height: 177.8,
    },
    PaperSize {
        label: "16.5 × 21.6 cm",
        width: 165.1,
        height: 215.9,
    },
    PaperSize {
        label: "17.8 × 24 cm",
        width: 177.8,
        height: 241.3,
    },
    PaperSize {
        label: "20.3 × 25.4 cm",
        width: 203.2,
        height: 254.0,
    },
    PaperSize {
        label: "21 × 29.7 cm",
        width: 210.0,
        height: 297.0,
    },
    PaperSize {
        label: "21.6 × 27.9 cm",
        width: 215.9,
        height: 279.4,
    },
    PaperSize {
        label: "24 × 30.5 cm",
        width: 241.3,
        height: 304.8,
    },
    PaperSize {
        label: "27.9 × 35.6 cm",
        width: 279.4,
        height: 355.6,
    },
    PaperSize {
        label: "30.5 × 40.6 cm",
        width: 304.8,
        height: 406.4,
    },
    PaperSize {
        label: "40.6 × 50.8 cm",
        width: 406.4,
        height: 508.0,
    },
    PaperSize {
        label: "50.8 × 61 cm",
        width: 508.0,
        height: 609.6,
    },
];

/// The smallest sheet that holds a whole 36-exposure roll.
pub const DEFAULT_PAPER: PaperSize = ILFORD_PAPERS[6];

/// An Ilford preset that matches the size in either orientation.
#[must_use]
pub fn paper_preset(width: f64, height: f64, tolerance: f64) -> Option<PaperSize> {
    ILFORD_PAPERS.iter().copied().find(|paper| {
        ((width - paper.width).abs() <= tolerance && (height - paper.height).abs() <= tolerance)
            || ((width - paper.height).abs() <= tolerance
                && (height - paper.width).abs() <= tolerance)
    })
}

/// The nearest Ilford size in either orientation, else the rounded size.
#[must_use]
pub fn snap_paper(width: f64, height: f64, tolerance: f64) -> (f64, f64) {
    for paper in ILFORD_PAPERS {
        if (width - paper.width).abs() <= tolerance && (height - paper.height).abs() <= tolerance {
            return (paper.width, paper.height);
        }
        if (width - paper.height).abs() <= tolerance && (height - paper.width).abs() <= tolerance {
            return (paper.height, paper.width);
        }
    }
    (width.round(), height.round())
}

/// The smallest paper that holds one frame of the strip.
#[must_use]
pub fn min_paper_size(geometry: &FilmGeometry, label: bool) -> (f64, f64) {
    let band = if label { LABEL_BAND } else { 0.0 };
    (
        geometry.pitch + 2.0 * SHEET_MARGIN,
        geometry.width + 2.0 * SHEET_MARGIN + band,
    )
}

/// Clamp a custom paper to the frame minimum and the largest sheet.
#[must_use]
pub fn clamp_paper(width: f64, height: f64, geometry: &FilmGeometry, label: bool) -> (f64, f64) {
    let (min_w, min_h) = min_paper_size(geometry, label);
    (
        PAPER_MAX.min(PAPER_MIN.max(min_w).max(width)),
        PAPER_MAX.min(PAPER_MIN.max(min_h).max(height)),
    )
}

/// The raster size of a sheet at `dpi`.
#[must_use]
pub fn sheet_pixels(width: f64, height: f64, dpi: u32) -> (u32, u32) {
    (
        (width / MM_PER_INCH * f64::from(dpi)).round() as u32,
        (height / MM_PER_INCH * f64::from(dpi)).round() as u32,
    )
}

/// Whether a sheet stays under the decompression-bomb pixel limit.
#[must_use]
pub fn dpi_allowed(width: f64, height: f64, dpi: u32) -> bool {
    let (px_w, px_h) = sheet_pixels(width, height, dpi);
    u64::from(px_w) * u64::from(px_h) <= MAX_SHEET_PIXELS
}

/// The highest allowed DPI at or below `wanted`.
#[must_use]
pub fn best_dpi(width: f64, height: f64, wanted: u32) -> u32 {
    for dpi in DPI_CHOICES.into_iter().rev() {
        if dpi <= wanted && dpi_allowed(width, height, dpi) {
            return dpi;
        }
    }
    DPI_CHOICES[0]
}

/// One film strip's top-left corner and frame range on the paper.
#[derive(Clone, Debug, PartialEq)]
pub struct StripPlacement {
    pub x: f64,
    pub y: f64,
    pub first: usize,
    pub count: usize,
    pub roll_start: f64,
    pub length: f64,
}

/// One upright frame in a row; `slot` indexes the plan's frames.
#[derive(Clone, Debug, PartialEq)]
pub struct FramePlacement {
    pub slot: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One row's black band, frames included.
#[derive(Clone, Debug, PartialEq)]
pub struct RowPlacement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub frames: Vec<FramePlacement>,
}

/// One sheet's placements and content block.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetPage {
    pub strips: Vec<StripPlacement>,
    pub block: (f64, f64, f64, f64),
    pub rows: Vec<RowPlacement>,
}

impl SheetPage {
    /// The frame slots this page prints, in order.
    #[must_use]
    pub fn slots(&self) -> Vec<usize> {
        if !self.rows.is_empty() {
            return self
                .rows
                .iter()
                .flat_map(|row| row.frames.iter().map(|frame| frame.slot))
                .collect();
        }
        self.strips
            .iter()
            .flat_map(|strip| strip.first..strip.first + strip.count)
            .collect()
    }
}

/// The paginated placement of one roll.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetPlan {
    pub paper_width: f64,
    pub paper_height: f64,
    pub geometry: FilmGeometry,
    pub frame_count: usize,
    pub frames_per_strip: usize,
    pub strips_per_sheet: usize,
    pub pages: Vec<SheetPage>,
    pub label: bool,
    pub reason: String,
}

impl SheetPlan {
    /// Frames one page holds when the strips fit.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.frames_per_strip * self.strips_per_sheet
    }
}

/// `(frames per strip, strips per sheet)` for this paper.
#[must_use]
pub fn plan_capacity(
    width: f64,
    height: f64,
    geometry: &FilmGeometry,
    label: bool,
) -> (usize, usize) {
    let band = if label { LABEL_BAND } else { 0.0 };
    let per_strip = ((width - 2.0 * SHEET_MARGIN) / geometry.pitch + 1e-9).floor();
    let strips = ((height - 2.0 * SHEET_MARGIN - band + STRIP_GAP) / (geometry.width + STRIP_GAP)
        + 1e-9)
        .floor();
    (per_strip.max(0.0) as usize, strips.max(0.0) as usize)
}

/// `(first frame, count)` per strip; a frame in `breaks` starts a new strip.
#[must_use]
pub fn cut_strips(frame_count: usize, per_strip: usize, breaks: &[usize]) -> Vec<(usize, usize)> {
    let mut stops: Vec<usize> = breaks
        .iter()
        .copied()
        .filter(|value| *value > 0 && *value < frame_count)
        .collect();
    stops.sort_unstable();
    stops.dedup();
    stops.push(frame_count);
    let mut strips = Vec::new();
    let mut first = 0;
    for stop in stops {
        while first < stop {
            let count = per_strip.min(stop - first);
            strips.push((first, count));
            first += count;
        }
    }
    strips
}

struct EmptyPlan<'a> {
    width: f64,
    height: f64,
    geometry: &'a FilmGeometry,
    frame_count: usize,
    per_strip: usize,
    strips: usize,
    label: bool,
}

fn empty_plan(plan: EmptyPlan<'_>, reason: &str) -> SheetPlan {
    SheetPlan {
        paper_width: plan.width,
        paper_height: plan.height,
        geometry: plan.geometry.clone(),
        frame_count: plan.frame_count,
        frames_per_strip: plan.per_strip,
        strips_per_sheet: plan.strips,
        pages: Vec::new(),
        label: plan.label,
        reason: reason.to_string(),
    }
}

/// Lay whole strips on sheets, breaking at scene changes.
#[must_use]
pub fn plan_sheets(
    width: f64,
    height: f64,
    geometry: &FilmGeometry,
    frame_count: usize,
    label: bool,
    breaks: &[usize],
) -> SheetPlan {
    let (per_strip, strips) = plan_capacity(width, height, geometry, label);
    if per_strip == 0 {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count,
                per_strip: 0,
                strips,
                label,
            },
            "The paper is too narrow for one frame.",
        );
    }
    if strips == 0 {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count,
                per_strip,
                strips: 0,
                label,
            },
            "The paper is too short for one strip.",
        );
    }
    if frame_count == 0 {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count: 0,
                per_strip,
                strips,
                label,
            },
            "No frames to print.",
        );
    }
    let band = if label { LABEL_BAND } else { 0.0 };
    let cut = cut_strips(frame_count, per_strip, breaks);
    let mut pages = Vec::new();
    for page_start in (0..cut.len()).step_by(strips) {
        let rows = &cut[page_start..(page_start + strips).min(cut.len())];
        let block_w =
            rows.iter().map(|(_, count)| *count).max().unwrap_or(0) as f64 * geometry.pitch;
        let block_h = rows.len() as f64 * geometry.width + (rows.len() as f64 - 1.0) * STRIP_GAP;
        let left = (width - block_w) / 2.0;
        let top = ((height - block_h) / 2.0).max(SHEET_MARGIN + band);
        let placed = rows
            .iter()
            .enumerate()
            .map(|(row, (first, count))| StripPlacement {
                x: left,
                y: top + row as f64 * (geometry.width + STRIP_GAP),
                first: *first,
                count: *count,
                roll_start: *first as f64 * geometry.pitch,
                length: *count as f64 * geometry.pitch,
            })
            .collect();
        pages.push(SheetPage {
            strips: placed,
            block: (left, top, block_w, block_h),
            rows: Vec::new(),
        });
    }
    SheetPlan {
        paper_width: width,
        paper_height: height,
        geometry: geometry.clone(),
        frame_count,
        frames_per_strip: per_strip,
        strips_per_sheet: strips,
        pages,
        label,
        reason: String::new(),
    }
}

fn cut_rows(
    aspects: &[f64],
    row_height: f64,
    inner: f64,
    max_height: f64,
    breaks: &[usize],
) -> Vec<(Vec<usize>, f64)> {
    let stops: Vec<usize> = breaks
        .iter()
        .copied()
        .filter(|value| *value > 0 && *value < aspects.len())
        .collect();
    let mut rows = Vec::new();
    let mut slots: Vec<usize> = Vec::new();
    let mut natural = 0.0;
    let mut close = |slots: &Vec<usize>, natural: f64, fill: bool| {
        let gap = ROW_GAP * (slots.len() as f64 - 1.0);
        let mut scale = (inner - gap) / natural;
        if !fill || scale > MAX_STRETCH {
            scale = scale.min(1.0);
        }
        rows.push((
            slots.clone(),
            row_height * scale.min(max_height / row_height),
        ));
    };
    for (slot, aspect) in aspects.iter().enumerate() {
        let width = row_height * aspect;
        if !slots.is_empty()
            && (stops.contains(&slot) || natural + width + ROW_GAP * slots.len() as f64 > inner)
        {
            close(&slots, natural, !stops.contains(&slot));
            slots.clear();
            natural = 0.0;
        }
        slots.push(slot);
        natural += width;
    }
    if !slots.is_empty() {
        close(&slots, natural, false);
    }
    rows
}

/// Film Edges off: upright frames at their own aspect, in rows as tall as the film's frame.
#[must_use]
pub fn plan_rows(
    width: f64,
    height: f64,
    geometry: &FilmGeometry,
    aspects: &[f64],
    label: bool,
    breaks: &[usize],
) -> SheetPlan {
    let band = if label { LABEL_BAND } else { 0.0 };
    let inner = width - 2.0 * SHEET_MARGIN - 2.0 * ROW_BORDER;
    let room = height - 2.0 * SHEET_MARGIN - band - 2.0 * ROW_BORDER;
    let row_height = geometry.frame_across;
    if inner <= 0.0 {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count: aspects.len(),
                per_strip: 0,
                strips: 0,
                label,
            },
            "The paper is too narrow for one frame.",
        );
    }
    if room < row_height {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count: aspects.len(),
                per_strip: 0,
                strips: 0,
                label,
            },
            "The paper is too short for one row.",
        );
    }
    if aspects.is_empty() {
        return empty_plan(
            EmptyPlan {
                width,
                height,
                geometry,
                frame_count: 0,
                per_strip: 0,
                strips: 0,
                label,
            },
            "No frames to print.",
        );
    }
    let cut = cut_rows(aspects, row_height, inner, room, breaks);
    let mut paged: Vec<Vec<(Vec<usize>, f64)>> = vec![Vec::new()];
    let mut used = 0.0;
    for (slots, h) in cut {
        let mut need = h + 2.0 * ROW_BORDER;
        if !paged.last().is_some_and(Vec::is_empty) {
            need += STRIP_GAP;
        }
        if paged.last().is_some_and(|page| !page.is_empty())
            && used + need > room + 2.0 * ROW_BORDER
        {
            paged.push(Vec::new());
            need = h + 2.0 * ROW_BORDER;
            used = 0.0;
        }
        paged
            .last_mut()
            .expect("at least one page")
            .push((slots, h));
        used += need;
    }
    let mut pages = Vec::new();
    for rows in paged {
        let widths: Vec<f64> = rows
            .iter()
            .map(|(slots, h)| {
                slots.iter().map(|slot| h * aspects[*slot]).sum::<f64>()
                    + ROW_GAP * (slots.len() as f64 - 1.0)
                    + 2.0 * ROW_BORDER
            })
            .collect();
        let block_w = widths.iter().copied().fold(0.0, f64::max);
        let block_h = rows.iter().map(|(_, h)| h + 2.0 * ROW_BORDER).sum::<f64>()
            + STRIP_GAP * (rows.len() as f64 - 1.0);
        let left = (width - block_w) / 2.0;
        let mut y = ((height - block_h) / 2.0).max(SHEET_MARGIN + band);
        let mut placed = Vec::new();
        for ((slots, h), row_w) in rows.iter().zip(&widths) {
            let mut x = left + ROW_BORDER;
            let mut frames = Vec::new();
            for slot in slots {
                frames.push(FramePlacement {
                    slot: *slot,
                    x,
                    y: y + ROW_BORDER,
                    width: h * aspects[*slot],
                    height: *h,
                });
                x += h * aspects[*slot] + ROW_GAP;
            }
            placed.push(RowPlacement {
                x: left,
                y,
                width: *row_w,
                height: h + 2.0 * ROW_BORDER,
                frames,
            });
            y += h + 2.0 * ROW_BORDER + STRIP_GAP;
        }
        let first_y = placed.first().map_or(0.0, |row| row.y);
        pages.push(SheetPage {
            strips: Vec::new(),
            block: (left, first_y, block_w, block_h),
            rows: placed,
        });
    }
    let per_row = pages
        .iter()
        .flat_map(|page| &page.rows)
        .map(|row| row.frames.len())
        .max()
        .unwrap_or(0);
    let rows_per_page = pages.iter().map(|page| page.rows.len()).max().unwrap_or(0);
    SheetPlan {
        paper_width: width,
        paper_height: height,
        geometry: geometry.clone(),
        frame_count: aspects.len(),
        frames_per_strip: per_row,
        strips_per_sheet: rows_per_page,
        pages,
        label,
        reason: String::new(),
    }
}

/// The paper turned the way that needs fewer sheets; `aspects` plans rows.
#[must_use]
pub fn better_orientation(
    width: f64,
    height: f64,
    geometry: &FilmGeometry,
    frame_count: usize,
    label: bool,
    aspects: Option<&[f64]>,
) -> (f64, f64) {
    let sheets = |w: f64, h: f64| -> f64 {
        if let Some(aspects) = aspects {
            let plan = plan_rows(w, h, geometry, aspects, label, &[]);
            return if plan.pages.is_empty() {
                f64::INFINITY
            } else {
                plan.pages.len() as f64
            };
        }
        let (per_strip, strips) = plan_capacity(w, h, geometry, label);
        let capacity = per_strip * strips;
        if capacity == 0 {
            f64::INFINITY
        } else {
            (frame_count.max(1) as f64 / capacity as f64).ceil()
        }
    };
    if sheets(height, width) < sheets(width, height) {
        (height, width)
    } else {
        (width, height)
    }
}

pub const SHEET_ORDERS: [&str; 4] = ["name", "date", "scene", "frame"];

/// The contact-sheet controls, independent of the film format.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactSheetSettings {
    pub paper_width: f64,
    pub paper_height: f64,
    pub dpi: u32,
    pub roll_label: bool,
    pub edge_print: bool,
    pub order: String,
    pub white_paper: bool,
    pub film_edges: bool,
}

impl Default for ContactSheetSettings {
    fn default() -> Self {
        Self {
            paper_width: DEFAULT_PAPER.width,
            paper_height: DEFAULT_PAPER.height,
            dpi: DEFAULT_DPI,
            roll_label: true,
            edge_print: true,
            order: "name".to_string(),
            white_paper: false,
            film_edges: true,
        }
    }
}

impl ContactSheetSettings {
    /// Read a stored record, ignoring malformed or stale values.
    #[must_use]
    pub fn from_dict(data: Option<&Map<String, Value>>) -> Self {
        let Some(data) = data else {
            return Self::default();
        };
        let fallback = Self::default();
        let length = |key: &str, default: f64| {
            data.get(key)
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .map_or(default, |value| value.clamp(PAPER_MIN, PAPER_MAX))
        };
        let flag =
            |key: &str, default: bool| data.get(key).and_then(Value::as_bool).unwrap_or(default);
        Self {
            paper_width: length("paper_width", fallback.paper_width),
            paper_height: length("paper_height", fallback.paper_height),
            dpi: data
                .get("dpi")
                .and_then(Value::as_u64)
                .filter(|value| DPI_CHOICES.contains(&(*value as u32)))
                .map_or(fallback.dpi, |value| value as u32),
            roll_label: flag("roll_label", fallback.roll_label),
            edge_print: flag("edge_print", fallback.edge_print),
            order: data
                .get("order")
                .and_then(Value::as_str)
                .filter(|value| SHEET_ORDERS.contains(value))
                .unwrap_or(&fallback.order)
                .to_string(),
            white_paper: flag("white_paper", fallback.white_paper),
            film_edges: flag("film_edges", fallback.film_edges),
        }
    }

    /// The flat record a settings table stores.
    #[must_use]
    pub fn to_dict(&self) -> Map<String, Value> {
        let mut out = Map::new();
        out.insert("paper_width".to_string(), Value::from(self.paper_width));
        out.insert("paper_height".to_string(), Value::from(self.paper_height));
        out.insert("dpi".to_string(), Value::from(self.dpi));
        out.insert("roll_label".to_string(), Value::Bool(self.roll_label));
        out.insert("edge_print".to_string(), Value::Bool(self.edge_print));
        out.insert("order".to_string(), Value::String(self.order.clone()));
        out.insert("white_paper".to_string(), Value::Bool(self.white_paper));
        out.insert("film_edges".to_string(), Value::Bool(self.film_edges));
        out
    }
}
