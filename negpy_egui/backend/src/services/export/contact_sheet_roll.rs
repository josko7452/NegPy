//! Mirrors the pure half of `negpy/services/export/contact_sheet_roll.py`: order and format.

use std::collections::HashMap;

use crate::domain::models::{MetadataConfig, ProcessMode, WorkspaceConfig};
use crate::features::metadata::gear::{FilmStock, GearLibrary};
use crate::services::film_edge::geometry::{
    DEFAULT_FRAME_120, FRAME_SIZES_120, FilmGeometry, SheetFormat,
};
use crate::services::film_edge::layout::frame_turns;

/// What the sheet reads from a frame's file headers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameFacts {
    pub capture_time: String,
    pub birth_time: f64,
    pub scan_size: Option<(i64, i64)>,
    pub scan_ev: Option<f64>,
}

/// One asset and the edit the sheet reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetFrame {
    pub name: String,
    pub path: String,
    pub half: i64,
    pub config: WorkspaceConfig,
    pub facts: FrameFacts,
}

impl SheetFrame {
    /// A frame with no file facts, for callers that only have the edit.
    #[must_use]
    pub fn new(name: &str, path: &str, config: WorkspaceConfig) -> Self {
        Self {
            name: name.to_string(),
            path: path.to_string(),
            half: 0,
            config,
            facts: FrameFacts::default(),
        }
    }

    /// The display name, falling back to the path's file name.
    #[must_use]
    pub fn display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        std::path::Path::new(&self.path)
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }
}

/// The parts of a composite, or the primary path.
#[must_use]
pub fn asset_paths(path: &str, hdr_paths: &[String], stitch_paths: &[String]) -> Vec<String> {
    let parts = if !hdr_paths.is_empty() {
        hdr_paths
    } else {
        stitch_paths
    };
    if parts.is_empty() {
        vec![path.to_string()]
    } else {
        parts.to_vec()
    }
}

/// Indices by lower-case name, then half.
#[must_use]
pub fn name_order(frames: &[SheetFrame]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..frames.len()).collect();
    order.sort_by_key(|index| {
        (
            frames[*index].display_name().to_lowercase(),
            frames[*index].half,
        )
    });
    order
}

/// Indices by EXIF time only when every frame has one, else by creation time.
#[must_use]
pub fn creation_order(frames: &[SheetFrame]) -> Vec<usize> {
    let by_exif = !frames.is_empty()
        && frames
            .iter()
            .all(|frame| !frame.facts.capture_time.is_empty());
    let mut order: Vec<usize> = (0..frames.len()).collect();
    order.sort_by(|a, b| {
        let frame = |index: usize| &frames[index];
        let when = |frame: &SheetFrame| {
            if by_exif {
                frame.facts.capture_time.clone()
            } else {
                format!("{:020}", frame.facts.birth_time)
            }
        };
        when(frame(*a))
            .cmp(&when(frame(*b)))
            .then_with(|| {
                frame(*a)
                    .display_name()
                    .to_lowercase()
                    .cmp(&frame(*b).display_name().to_lowercase())
            })
            .then_with(|| frame(*a).half.cmp(&frame(*b).half))
    });
    order
}

/// Indices by the Roll and Frame No. in metadata, the name a tie.
#[must_use]
pub fn frame_order(frames: &[SheetFrame]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..frames.len()).collect();
    order.sort_by(|a, b| {
        let left = &frames[*a].config.metadata;
        let right = &frames[*b].config.metadata;
        left.capture_roll
            .to_lowercase()
            .cmp(&right.capture_roll.to_lowercase())
            .then_with(|| {
                left.capture_frame
                    .unwrap_or(0)
                    .cmp(&right.capture_frame.unwrap_or(0))
            })
            .then_with(|| {
                frames[*a]
                    .display_name()
                    .to_lowercase()
                    .cmp(&frames[*b].display_name().to_lowercase())
            })
    });
    order
}

fn crop_fractions(config: &WorkspaceConfig) -> (f64, f64) {
    let Some(rect) = &config.geometry.crop_rect else {
        return (1.0, 1.0);
    };
    if rect.len() < 4 {
        return (1.0, 1.0);
    }
    ((rect[2] - rect[0]).max(1e-3), (rect[3] - rect[1]).max(1e-3))
}

/// Width over height of the whole picture after rotation, uncropped.
#[must_use]
pub fn upright_aspect(frame: &SheetFrame, tile_shape: Option<(i64, i64)>) -> Option<f64> {
    if let Some((width, height)) = frame.facts.scan_size
        && width > 0
        && height > 0
    {
        let (w, h) = if frame.config.geometry.rotation % 2 != 0 {
            (height, width)
        } else {
            (width, height)
        };
        return Some(w as f64 / h as f64);
    }
    let (width, height) = tile_shape?;
    if width <= 0 || height <= 0 {
        return None;
    }
    let (fx, fy) = crop_fractions(&frame.config);
    Some((height as f64 / width as f64) * (fy / fx))
}

/// Width over height of the frame as it prints upright.
#[must_use]
pub fn print_aspect(frame: &SheetFrame, tile_shape: Option<(i64, i64)>) -> Option<f64> {
    if let Some((width, height)) = tile_shape
        && width > 0
        && height > 0
    {
        return Some(height as f64 / width as f64);
    }
    let whole = upright_aspect(frame, None)?;
    let (fx, fy) = crop_fractions(&frame.config);
    Some(whole * fx / fy)
}

/// Width over height after the crop, whichever way is upright.
#[must_use]
pub fn cropped_aspect(frame: &SheetFrame) -> Option<f64> {
    let aspect = print_aspect(frame, None)?;
    Some(aspect.max(1.0 / aspect))
}

/// The `np.rot90` turn that places this frame on the strip.
#[must_use]
pub fn turns_for(
    frame: &SheetFrame,
    geometry: &FilmGeometry,
    tile_shape: Option<(i64, i64)>,
) -> i64 {
    let config = &frame.config.geometry;
    frame_turns(
        upright_aspect(frame, tile_shape),
        geometry,
        config.rotation,
        config.flip_horizontal,
        config.flip_vertical,
    )
}

fn effective_format(metadata: &MetadataConfig) -> String {
    if metadata.format == "Other" {
        metadata.format_other.clone()
    } else {
        metadata.format.clone()
    }
}

/// The 120 size named in free text, or `""`.
#[must_use]
pub fn parse_frame_120(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.contains("645") {
        return "6×4.5".to_string();
    }
    if lower.contains("6×4,5") || lower.contains("6x4,5") {
        return "6×4.5".to_string();
    }
    for (label, _) in FRAME_SIZES_120 {
        let side = label.trim_start_matches("6×");
        for separator in ["x", "×"] {
            let wanted = format!("6{separator}{side}").to_lowercase();
            if lower.contains(&wanted) {
                return label.to_string();
            }
        }
    }
    String::new()
}

fn nearest_frame_120(aspect: f64) -> &'static str {
    let candidates = ["6×4.5", "6×6", "6×7", "6×9", "6×12", "6×17"];
    candidates
        .into_iter()
        .min_by(|a, b| {
            let ratio = |label: &str| {
                let along = FRAME_SIZES_120
                    .iter()
                    .find(|(name, _)| *name == label)
                    .map_or(56.0, |(_, value)| *value);
                let value = along / 56.0;
                value.max(1.0 / value).ln()
            };
            (ratio(a) - aspect.ln())
                .abs()
                .total_cmp(&(ratio(b) - aspect.ln()).abs())
        })
        .unwrap_or(DEFAULT_FRAME_120)
}

fn majority<T: Clone + Eq + std::hash::Hash>(values: impl Iterator<Item = T>, default: T) -> T {
    let mut counts: HashMap<T, usize> = HashMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map_or(default, |(value, _)| value)
}

/// The roll's film format and, for 120, its frame size.
#[must_use]
pub fn infer_format(frames: &[SheetFrame], half_frame_roll: bool) -> (SheetFormat, String) {
    if half_frame_roll || frames.iter().any(|frame| frame.half != 0) {
        return (SheetFormat::HalfFrame, DEFAULT_FRAME_120.to_string());
    }
    let mut named_sizes: Vec<String> = Vec::new();
    let mut votes: Vec<&str> = Vec::new();
    for frame in frames {
        let format = effective_format(&frame.config.metadata);
        let trimmed = format.trim();
        let size = parse_frame_120(trimmed);
        if trimmed == "35mm" {
            votes.push("35mm");
        } else if trimmed == "120" || !size.is_empty() {
            votes.push("120");
            if !size.is_empty() {
                named_sizes.push(size);
            }
        }
    }
    let aspects: Vec<f64> = frames.iter().filter_map(cropped_aspect).collect();
    let median = median(&aspects);
    let format = if votes.is_empty() {
        if median.is_some_and(|value| value <= 1.3) {
            SheetFormat::Medium
        } else {
            SheetFormat::FullFrame
        }
    } else if majority(votes.into_iter(), "35mm") == "35mm" {
        SheetFormat::FullFrame
    } else {
        SheetFormat::Medium
    };
    if format != SheetFormat::Medium {
        return (format, DEFAULT_FRAME_120.to_string());
    }
    if !named_sizes.is_empty() {
        return (
            format,
            majority(named_sizes.iter(), &named_sizes[0]).clone(),
        );
    }
    let size = median.map_or(DEFAULT_FRAME_120, nearest_frame_120);
    (format, size.to_string())
}

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    })
}

fn gear_stock<'a>(frame: &SheetFrame, library: Option<&'a GearLibrary>) -> Option<&'a FilmStock> {
    let id = &frame.config.metadata.film_stock_id;
    if id.is_empty() {
        return None;
    }
    library?.film_stock(id)
}

/// The displayed film name, from gear when it resolves, else the metadata text.
#[must_use]
pub fn film_name(frame: &SheetFrame, library: Option<&GearLibrary>) -> String {
    if let Some(stock) = gear_stock(frame, library) {
        if !stock.display_name.trim().is_empty() {
            return stock.display_name.trim().to_string();
        }
        let (maker, name) = (stock.manufacturer.trim(), stock.stock_name.trim());
        if !maker.is_empty() && !name.to_lowercase().starts_with(&maker.to_lowercase()) {
            return format!("{maker} {name}").trim().to_string();
        }
        if !name.is_empty() {
            return name.to_string();
        }
    }
    frame.config.metadata.film.trim().to_string()
}

/// The film's color type, with process mode as the fallback.
#[must_use]
pub fn film_color_type(frame: &SheetFrame, library: Option<&GearLibrary>) -> Option<String> {
    if !frame.config.metadata.film_color_type.is_empty() {
        return Some(frame.config.metadata.film_color_type.clone());
    }
    if let Some(stock) = gear_stock(frame, library) {
        return Some(stock.color_type.clone());
    }
    match frame.config.process.process_mode {
        ProcessMode::C41 => Some("ColorNegative".to_string()),
        ProcessMode::BW => Some("B&W Negative".to_string()),
        ProcessMode::E6 if !frame.config.process.positive_source => Some(
            if frame.config.process.slide_bw {
                "B&W Slide"
            } else {
                "ColorSlide"
            }
            .to_string(),
        ),
        ProcessMode::E6 => None,
    }
}

/// The roll label: name, stock and push, developer, camera, date.
#[must_use]
pub fn roll_label_text(
    roll_name: &str,
    frames: &[SheetFrame],
    library: Option<&GearLibrary>,
) -> String {
    if frames.is_empty() {
        return roll_name.to_string();
    }
    let metas: Vec<&MetadataConfig> = frames.iter().map(|frame| &frame.config.metadata).collect();
    let mut parts = Vec::new();
    if !roll_name.trim().is_empty() {
        parts.push(roll_name.trim().to_string());
    }
    let mut stock = majority(
        frames
            .iter()
            .map(|frame| film_name(frame, library))
            .filter(|name| !name.is_empty()),
        String::new(),
    );
    if !stock.is_empty() {
        let push = majority(
            metas
                .iter()
                .map(|meta| meta.push_pull)
                .filter(|push| *push != 0),
            0,
        );
        let iso = majority(metas.iter().filter_map(|meta| meta.film_iso), 0);
        if push != 0 && iso != 0 {
            stock = format!(
                "{stock} @ EI {}",
                (iso as f64 * 2_f64.powi(push as i32)).round() as i64
            );
        }
        parts.push(stock);
    }
    let developer = majority(
        metas.iter().map(|meta| {
            [meta.developer.trim(), meta.process_dilution.trim()]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }),
        String::new(),
    );
    if !developer.is_empty() {
        parts.push(developer);
    }
    let camera = majority(
        metas.iter().map(|meta| {
            let (make, model) = (meta.camera_make.trim(), meta.camera_model.trim());
            if !make.is_empty() && !model.to_lowercase().starts_with(&make.to_lowercase()) {
                format!("{make} {model}").trim().to_string()
            } else {
                model.to_string()
            }
        }),
        String::new(),
    );
    if !camera.is_empty() {
        parts.push(camera);
    }
    let mut date = majority(
        metas.iter().map(|meta| {
            meta.capture_date
                .trim()
                .chars()
                .take(10)
                .collect::<String>()
        }),
        String::new(),
    );
    if date.is_empty() {
        date = frames
            .iter()
            .filter(|frame| !frame.facts.capture_time.is_empty())
            .map(|frame| {
                frame
                    .facts
                    .capture_time
                    .chars()
                    .take(10)
                    .collect::<String>()
            })
            .min()
            .unwrap_or_default();
    }
    if !date.is_empty() {
        parts.push(date);
    }
    parts.join(" · ")
}

/// The scan-exposure shift that brings each frame to the median.
#[must_use]
pub fn scan_exposure_offsets(frames: &[SheetFrame]) -> Vec<f64> {
    let known: Vec<f64> = frames
        .iter()
        .filter_map(|frame| frame.facts.scan_ev)
        .collect();
    let Some(reference) = median(&known) else {
        return vec![0.0; frames.len()];
    };
    frames
        .iter()
        .map(|frame| {
            frame
                .facts
                .scan_ev
                .map_or(0.0, |ev| (ev - reference) * 2.0f64.log10())
        })
        .collect()
}
