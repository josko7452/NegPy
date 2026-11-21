//! Roll-aware crop calibration built on the single-frame film detector.
//!
//! Mirrors `negpy/features/geometry/batch_autocrop.py` without Qt, storage or file I/O.

use crate::features::geometry::logic::Roi;
use crate::features::geometry::logic::ratio_aspect;
use crate::features::geometry::processor::{closest_standard_ratio, enforce_roi_aspect_ratio};

pub const BORDER_SIDES: [&str; 4] = ["top", "bottom", "left", "right"];
pub const TRUSTED_CONFIDENCE: f64 = 0.58;
pub const MAX_AUTOMATIC_DESKEW: f64 = 8.0;
pub const DEFAULT_SAFETY_BORDER: f64 = 0.01;
const MIN_BORDER_SAMPLES: usize = 5;
const FALLBACK_CONFIDENCE: f64 = 0.30;
const MIN_FALLBACK_COVERAGE: f64 = 0.9;
const MIN_TRUSTED_FRAMES: usize = 2;
const CONFIRMED_ANGLE_TOL: f64 = 2.5;
const MAX_EDGE_FIT_DELTA: f64 = 0.5;

/// Single-frame evidence in the final, deskewed display coordinate space.
#[derive(Clone, Debug, PartialEq)]
pub struct CropEvidence {
    pub key: String,
    /// `(height, width)`.
    pub canvas_shape: (usize, usize),
    pub roi: Option<Roi>,
    pub correction_angle: f64,
    pub confidence: f64,
    pub target_ratio: String,
    pub rebate_trim: f64,
    pub angle_confident: bool,
    pub supported_sides: Vec<&'static str>,
    pub supported_corners: Vec<&'static str>,
    pub evidence_sources: Vec<&'static str>,
    pub geometry_score: f64,
    pub vertical_edge_contrast: f64,
    pub vertical_edge_profile: Vec<f32>,
    /// Per-side thickness in [`BORDER_SIDES`] order; NaN has no clean transition.
    pub border: Vec<f64>,
    pub bright_border: Vec<f64>,
    pub fallback_roi: Option<Roi>,
    pub reason: String,
}

impl Default for CropEvidence {
    fn default() -> Self {
        Self {
            key: String::new(),
            canvas_shape: (0, 0),
            roi: None,
            correction_angle: 0.0,
            confidence: 0.0,
            target_ratio: String::new(),
            rebate_trim: 1.0,
            angle_confident: false,
            supported_sides: Vec::new(),
            supported_corners: Vec::new(),
            evidence_sources: Vec::new(),
            geometry_score: 0.0,
            vertical_edge_contrast: 0.0,
            vertical_edge_profile: Vec::new(),
            border: Vec::new(),
            bright_border: Vec::new(),
            fallback_roi: None,
            reason: String::new(),
        }
    }
}

/// Robust normalized geometry shared by trustworthy frames in a roll.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RollCropTemplate {
    pub width: f64,
    pub fallback_width: f64,
    pub height: f64,
    pub center_x: f64,
    pub top: f64,
    pub correction_angle: f64,
    pub width_mad: f64,
    pub center_mad: f64,
    pub top_mad: f64,
    pub angle_mad: f64,
    pub confidence: f64,
    pub sample_count: usize,
    pub border: Vec<f64>,
    pub border_sample_count: usize,
    pub from_fallback: bool,
}

/// Explicit crop payload ready for controller-side conflict checks.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCrop {
    pub key: String,
    pub crop_rect: [f64; 4],
    pub correction_angle: f64,
    pub confidence: f64,
    pub calibrated: bool,
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn mad(values: &[f64], center: Option<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let pivot = center.unwrap_or_else(|| median(values));
    median(
        &values
            .iter()
            .map(|value| (value - pivot).abs())
            .collect::<Vec<_>>(),
    )
}

fn percentile75(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = 0.75 * (sorted.len() - 1) as f64;
    let lo = position.floor() as usize;
    let hi = position.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (position - lo as f64)
}

fn normalized_roi(roi: Roi, (height, width): (usize, usize)) -> [f64; 4] {
    [
        roi.2 as f64 / width as f64,
        roi.0 as f64 / height as f64,
        roi.3 as f64 / width as f64,
        roi.1 as f64 / height as f64,
    ]
}

fn pixel_roi(rect: [f64; 4], (height, width): (usize, usize)) -> Roi {
    let at = |value: f64, limit: usize| {
        (value * limit as f64)
            .round_ties_even()
            .clamp(0.0, limit as f64) as i64
    };
    (
        at(rect[1], height),
        at(rect[3], height),
        at(rect[0], width),
        at(rect[2], width),
    )
}

fn clamp_rect(rect: [f64; 4]) -> [f64; 4] {
    let clamp = |value: f64| value.clamp(0.0, 1.0);
    let [mut x1, mut y1, mut x2, mut y2] = rect.map(clamp);
    if x1 > x2 {
        std::mem::swap(&mut x1, &mut x2);
    }
    if y1 > y2 {
        std::mem::swap(&mut y1, &mut y2);
    }
    [x1, y1, x2, y2]
}

fn roi_coverage(roi: Roi, (height, width): (usize, usize)) -> f64 {
    if height == 0 || width == 0 {
        return 0.0;
    }
    let area = (roi.1 - roi.0).max(0) * (roi.3 - roi.2).max(0);
    area as f64 / (height * width) as f64
}

fn roll_border(evidence: &[CropEvidence]) -> (Vec<f64>, usize) {
    let measured: Vec<&Vec<f64>> = evidence
        .iter()
        .filter(|item| item.border.len() == BORDER_SIDES.len())
        .map(|item| &item.border)
        .collect();
    if measured.is_empty() {
        return (Vec::new(), 0);
    }
    let mut medians = Vec::with_capacity(BORDER_SIDES.len());
    let mut max_count = 0;
    for index in 0..BORDER_SIDES.len() {
        let values: Vec<f64> = measured
            .iter()
            .map(|row| row[index])
            .filter(|value| value.is_finite())
            .collect();
        max_count = max_count.max(values.len());
        medians.push(if values.len() >= MIN_BORDER_SAMPLES {
            median(&values)
        } else {
            f64::NAN
        });
    }
    if medians.iter().any(|value| value.is_finite()) {
        (medians, max_count)
    } else {
        (Vec::new(), max_count)
    }
}

fn threshold_fallback_frames(evidence: &[CropEvidence]) -> Vec<CropEvidence> {
    let boxes: Vec<&CropEvidence> = evidence
        .iter()
        .filter(|item| item.roi.is_none() && item.fallback_roi.is_some())
        .collect();
    if boxes.is_empty() {
        return Vec::new();
    }
    let coverage = median(
        &boxes
            .iter()
            .map(|item| roi_coverage(item.fallback_roi.unwrap(), item.canvas_shape))
            .collect::<Vec<_>>(),
    );
    if coverage < MIN_FALLBACK_COVERAGE {
        return Vec::new();
    }
    boxes
        .into_iter()
        .map(|item| {
            let boxed = CropEvidence {
                roi: item.fallback_roi,
                confidence: FALLBACK_CONFIDENCE,
                evidence_sources: vec!["threshold-fallback"],
                ..item.clone()
            };
            boxed
        })
        .collect()
}

fn trusted(item: &CropEvidence) -> bool {
    item.roi.is_some()
        && item.confidence >= TRUSTED_CONFIDENCE
        && item.geometry_score >= 0.35
        && (item.supported_sides.len() >= 2 || !item.supported_corners.is_empty())
}

/// Build a median/MAD roll template from multi-side, trustworthy detections.
#[must_use]
pub fn build_roll_template(evidence: &[CropEvidence]) -> Option<RollCropTemplate> {
    let mut trusted_items: Vec<&CropEvidence> =
        evidence.iter().filter(|item| trusted(item)).collect();
    if trusted_items.len() < MIN_TRUSTED_FRAMES && !evidence.is_empty() {
        trusted_items.clear();
    }
    let (border, border_sample_count) = roll_border(evidence);
    let mut from_fallback = false;
    let fallback = threshold_fallback_frames(evidence);
    if trusted_items.is_empty() {
        trusted_items = fallback.iter().collect();
        from_fallback = true;
    }
    if trusted_items.is_empty() {
        return None;
    }
    let rects: Vec<[f64; 4]> = trusted_items
        .iter()
        .map(|item| normalized_roi(item.roi.unwrap(), item.canvas_shape))
        .collect();
    let widths: Vec<f64> = rects.iter().map(|rect| rect[2] - rect[0]).collect();
    let heights: Vec<f64> = rects.iter().map(|rect| rect[3] - rect[1]).collect();
    let centers: Vec<f64> = rects.iter().map(|rect| (rect[0] + rect[2]) * 0.5).collect();
    let tops: Vec<f64> = rects.iter().map(|rect| rect[1]).collect();
    let angles: Vec<f64> = trusted_items
        .iter()
        .map(|item| item.correction_angle)
        .collect();

    let width_med = median(&widths);
    let angle_med = median(&angles);
    let width_tol = 0.025f64.max(3.5 * mad(&widths, Some(width_med)));
    let angle_tol = 0.45f64.max(3.5 * mad(&angles, Some(angle_med)));
    let keep: Vec<bool> = widths
        .iter()
        .zip(&angles)
        .map(|(width, angle)| {
            (width - width_med).abs() <= width_tol && (angle - angle_med).abs() <= angle_tol
        })
        .collect();
    let keep = if keep.iter().any(|value| *value) {
        keep
    } else {
        vec![true; widths.len()]
    };
    let kept: Vec<&CropEvidence> = trusted_items
        .iter()
        .zip(&keep)
        .filter_map(|(item, kept)| kept.then_some(*item))
        .collect();
    let widths: Vec<f64> = widths
        .iter()
        .zip(&keep)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    let heights: Vec<f64> = heights
        .iter()
        .zip(&keep)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    let centers: Vec<f64> = centers
        .iter()
        .zip(&keep)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    let tops: Vec<f64> = tops
        .iter()
        .zip(&keep)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    let mut kept_angles: Vec<f64> = angles
        .iter()
        .zip(&keep)
        .filter_map(|(value, kept)| kept.then_some(*value))
        .collect();
    let fitted: Vec<f64> = kept
        .iter()
        .filter(|item| item.angle_confident)
        .map(|item| item.correction_angle)
        .collect();
    if fitted.len() >= 3 {
        kept_angles = fitted;
    }

    Some(RollCropTemplate {
        width: median(&widths),
        fallback_width: percentile75(&widths),
        height: median(&heights),
        center_x: median(&centers),
        top: median(&tops),
        correction_angle: median(&kept_angles),
        width_mad: mad(&widths, None),
        center_mad: mad(&centers, None),
        top_mad: mad(&tops, None),
        angle_mad: mad(&kept_angles, None),
        confidence: median(&kept.iter().map(|item| item.confidence).collect::<Vec<_>>()),
        sample_count: kept.len(),
        border,
        border_sample_count,
        from_fallback,
    })
}

fn map_rect_between_rotations(
    rect: [f64; 4],
    (height, width): (usize, usize),
    source_angle: f64,
    target_angle: f64,
) -> [f64; 4] {
    let delta = target_angle - source_angle;
    if delta.abs() <= 1e-7 {
        return rect;
    }
    let (h, w) = (height as f64, width as f64);
    let points = [
        (rect[0] * w, rect[1] * h),
        (rect[2] * w, rect[1] * h),
        (rect[2] * w, rect[3] * h),
        (rect[0] * w, rect[3] * h),
    ];
    let m = negpy_core::warp::rotation_matrix((w as f32 / 2.0, h as f32 / 2.0), delta, 1.0);
    let mapped: Vec<(f64, f64)> = points
        .iter()
        .map(|(x, y)| (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]))
        .collect();
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for (x, y) in mapped {
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    let left = left.floor().clamp(0.0, w) as usize;
    let top = top.floor().clamp(0.0, h) as usize;
    let right = right.ceil().clamp(0.0, w) as usize;
    let bottom = bottom.ceil().clamp(0.0, h) as usize;
    clamp_rect([
        left as f64 / w,
        top as f64 / h,
        right as f64 / w,
        bottom as f64 / h,
    ])
}

fn calibrate_detected_rect(
    item: &CropEvidence,
    roi: Roi,
    template: &RollCropTemplate,
) -> ([f64; 4], bool) {
    let [mut x1, mut y1, mut x2, mut y2] = normalized_roi(roi, item.canvas_shape);
    let (width, height) = (x2 - x1, y2 - y1);
    let center = (x1 + x2) * 0.5;
    let mut calibrated = false;
    let width_tol = 0.035f64.max(4.0 * template.width_mad);
    let center_tol = 0.035f64.max(4.0 * template.center_mad);
    let top_tol = 0.03f64.max(4.0 * template.top_mad);
    let mut target_width = width;
    if (0.0 < template.width - width && template.width - width <= 0.10f64.max(width_tol))
        || (item.confidence < TRUSTED_CONFIDENCE && (width - template.width).abs() > width_tol)
    {
        target_width = template.width;
        calibrated = true;
    }
    if target_width != width {
        let left = item.supported_sides.contains(&"left");
        let right = item.supported_sides.contains(&"right");
        if left && !right {
            x2 = x1 + target_width;
        } else if right && !left {
            x1 = x2 - target_width;
        } else {
            x1 = center - target_width * 0.5;
            x2 = center + target_width * 0.5;
        }
    }
    if item.confidence < TRUSTED_CONFIDENCE {
        if (center - template.center_x).abs() > center_tol {
            x1 = template.center_x - target_width * 0.5;
            x2 = template.center_x + target_width * 0.5;
            calibrated = true;
        }
        if (y1 - template.top).abs() > top_tol
            || (height - template.height).abs() > 0.04f64.max(4.0 * template.top_mad)
        {
            y1 = template.top;
            y2 = template.top + template.height;
            calibrated = true;
        }
    }
    (clamp_rect([x1, y1, x2, y2]), calibrated)
}

fn rect_from_edge_profile(item: &CropEvidence, template: &RollCropTemplate) -> Option<[f64; 4]> {
    negpy_core::autocrop::rect_from_edge_profile(
        &item.vertical_edge_profile,
        item.vertical_edge_contrast,
        &negpy_core::autocrop::Template {
            center_x: template.center_x,
            width: template.width,
            fallback_width: template.fallback_width,
            top: template.top,
            height: template.height,
            center_mad: template.center_mad,
            width_mad: template.width_mad,
        },
    )
    .map(|rect| [rect.0, rect.1, rect.2, rect.3])
}

fn bright_only_border(bright: &[f64]) -> Vec<f64> {
    if bright.len() != BORDER_SIDES.len() {
        return Vec::new();
    }
    let finite = |value: f64| if value.is_finite() { value } else { 0.0 };
    let amounts: Vec<f64> = bright.iter().copied().map(finite).collect();
    let mut paired = vec![false; 4];
    if amounts[0] > 0.0 && amounts[1] > 0.0 {
        paired[0] = true;
        paired[1] = true;
    }
    if amounts[2] > 0.0 && amounts[3] > 0.0 {
        paired[2] = true;
        paired[3] = true;
    }
    if !paired.iter().any(|value| *value) {
        return Vec::new();
    }
    amounts
        .iter()
        .zip(&paired)
        .map(|(value, paired)| if *paired { *value } else { 0.0 })
        .collect()
}

fn resolve_border(frame: &[f64], roll: &[f64], bright: &[f64]) -> Vec<f64> {
    if roll.len() != BORDER_SIDES.len() {
        return bright_only_border(bright);
    }
    let frame = if frame.len() == BORDER_SIDES.len() {
        frame.to_vec()
    } else {
        vec![f64::NAN; BORDER_SIDES.len()]
    };
    frame
        .iter()
        .zip(roll)
        .map(|(own, fallback)| {
            if own.is_finite() {
                *own
            } else if fallback.is_finite() {
                *fallback
            } else {
                0.0
            }
        })
        .collect()
}

fn inset_rect_by_border(rect: [f64; 4], border: &[f64]) -> [f64; 4] {
    if border.len() != BORDER_SIDES.len() {
        return rect;
    }
    let [mut x1, mut y1, mut x2, mut y2] = rect;
    let (width, height) = (x2 - x1, y2 - y1);
    if width <= 0.0 || height <= 0.0 {
        return rect;
    }
    y1 += border[0] * height;
    y2 -= border[1] * height;
    x1 += border[2] * width;
    x2 -= border[3] * width;
    if y2 - y1 <= 0.0 || x2 - x1 <= 0.0 {
        return rect;
    }
    clamp_rect([x1, y1, x2, y2])
}

fn apply_target_ratio(roi: Roi, (height, width): (usize, usize), target_ratio: &str) -> Roi {
    let ratio = if target_ratio == "Free" {
        closest_standard_ratio(
            roi,
            (height, width),
            crate::domain::models::AspectRatio::R3x2,
        )
    } else {
        crate::domain::models::AspectRatio::coerce(target_ratio)
    };
    let [x1, y1, x2, y2] = [roi.2, roi.0, roi.3, roi.1].map(|value| value as f64);
    let current = (x2 - x1) / (y2 - y1).max(1e-12);
    let wanted = ratio_aspect(ratio).unwrap_or(current);
    if (current - wanted).abs() <= 1e-3 {
        return (
            roi.0.max(0),
            roi.1.min(height as i64),
            roi.2.max(0),
            roi.3.min(width as i64),
        );
    }
    enforce_roi_aspect_ratio(roi, height, width, ratio)
}

fn add_uniform_safety_border(roi: Roi, (height, width): (usize, usize), ratio: f64) -> Roi {
    let requested = (height.min(width) as f64 * ratio)
        .round_ties_even()
        .max(0.0) as i64;
    let available = roi
        .0
        .min(roi.2)
        .min(height as i64 - roi.1)
        .min(width as i64 - roi.3)
        .max(0);
    let pad = requested.min(available);
    (roi.0 - pad, roi.1 + pad, roi.2 - pad, roi.3 + pad)
}

fn own_crop(item: &CropEvidence) -> Vec<ResolvedCrop> {
    let Some((y1, y2, x1, x2)) = item.fallback_roi else {
        return Vec::new();
    };
    let (height, width) = item.canvas_shape;
    if y2 <= y1 || x2 <= x1 || height == 0 || width == 0 {
        return Vec::new();
    }
    vec![ResolvedCrop {
        key: item.key.clone(),
        crop_rect: [
            (x1 as f64 / width as f64).clamp(0.0, 1.0),
            (y1 as f64 / height as f64).clamp(0.0, 1.0),
            (x2 as f64 / width as f64).clamp(0.0, 1.0),
            (y2 as f64 / height as f64).clamp(0.0, 1.0),
        ],
        correction_angle: item.correction_angle,
        confidence: 0.5,
        calibrated: false,
    }]
}

/// Resolve trustworthy and template-supported frames; ambiguous frames abstain.
#[must_use]
pub fn resolve_roll_crops(evidence: &[CropEvidence], safety_border: f64) -> Vec<ResolvedCrop> {
    let pooled: Vec<&CropEvidence> = evidence
        .iter()
        .filter(|item| item.reason != "unsupported_orientation")
        .collect();
    let ratios: Vec<&str> = {
        let mut seen = Vec::new();
        for item in evidence {
            if !seen.contains(&item.target_ratio.as_str()) {
                seen.push(item.target_ratio.as_str());
            }
        }
        seen
    };
    let templates: Vec<(&str, Option<RollCropTemplate>)> = ratios
        .into_iter()
        .map(|ratio| {
            let group: Vec<CropEvidence> = pooled
                .iter()
                .filter(|item| item.target_ratio == ratio)
                .map(|item| (*item).clone())
                .collect();
            (ratio, build_roll_template(&group))
        })
        .collect();
    let mut results = Vec::new();
    for item in evidence {
        if item.reason == "unsupported_orientation" {
            results.extend(own_crop(item));
        }
    }
    if templates.iter().all(|(_, template)| template.is_none()) {
        return results;
    }
    for item in evidence {
        if item.reason == "unsupported_orientation" {
            continue;
        }
        let Some(template) = templates
            .iter()
            .find(|(ratio, _)| *ratio == item.target_ratio)
            .and_then(|(_, template)| template.as_ref())
        else {
            continue;
        };
        let mut roi = item.roi;
        if roi.is_none() && template.from_fallback {
            roi = item.fallback_roi;
        }
        let (mut rect, confidence);
        let mut calibrated;
        if let Some(roi) = roi {
            let (found, changed) = calibrate_detected_rect(item, roi, template);
            rect = found;
            calibrated = changed;
            if item.roi.is_none() {
                calibrated = true;
                confidence = 0.55f64.min(template.confidence * 0.72);
            } else {
                confidence = item.confidence;
            }
        } else {
            let Some(found) = rect_from_edge_profile(item, template) else {
                continue;
            };
            rect = found;
            calibrated = true;
            confidence = 0.55f64.min(template.confidence * 0.72);
        }
        let mut angle = item.correction_angle;
        let angle_tol = 0.55f64.max(4.0 * template.angle_mad);
        let angle_trusted = item.confidence >= TRUSTED_CONFIDENCE;
        let divergence = (angle - template.correction_angle).abs();
        let own_measurement = item.angle_confident || angle_trusted;
        if item.roi.is_none()
            || !(own_measurement && divergence <= CONFIRMED_ANGLE_TOL.max(angle_tol))
        {
            let target = template.correction_angle;
            rect = map_rect_between_rotations(rect, item.canvas_shape, angle, target);
            angle = target;
            calibrated = true;
        }
        angle = angle.clamp(
            -crate::features::geometry::logic::FINE_ROTATION_LIMIT,
            crate::features::geometry::logic::FINE_ROTATION_LIMIT,
        );
        let mut border = resolve_border(&item.border, &template.border, &item.bright_border);
        if item.rebate_trim != 1.0 {
            border = border
                .iter()
                .map(|value| value * item.rebate_trim)
                .collect();
        }
        let inset = inset_rect_by_border(rect, &border);
        let trimmed = inset != rect;
        let mut pixel = pixel_roi(inset, item.canvas_shape);
        if trimmed && !template.from_fallback {
            pixel = apply_target_ratio(pixel, item.canvas_shape, &item.target_ratio);
        }
        let pad_ratio = if trimmed { 0.0 } else { safety_border };
        let (y1, y2, x1, x2) = add_uniform_safety_border(pixel, item.canvas_shape, pad_ratio);
        if y2 <= y1 || x2 <= x1 {
            continue;
        }
        let (height, width) = item.canvas_shape;
        results.push(ResolvedCrop {
            key: item.key.clone(),
            crop_rect: [
                (x1 as f64 / width as f64).clamp(0.0, 1.0),
                (y1 as f64 / height as f64).clamp(0.0, 1.0),
                (x2 as f64 / width as f64).clamp(0.0, 1.0),
                (y2 as f64 / height as f64).clamp(0.0, 1.0),
            ],
            correction_angle: angle,
            confidence: confidence.clamp(0.0, 1.0),
            calibrated,
        });
    }
    results
}

fn edge_fit(image: &crate::infrastructure::loaders::image_buffer::ImageBuffer) -> Option<f64> {
    let skew = crate::features::geometry::skew::trusted_frame_skew(image, false)?;
    (skew.fine_rotation.is_finite() && skew.fine_rotation.abs() <= MAX_AUTOMATIC_DESKEW)
        .then_some(skew.fine_rotation)
}

/// The config after one resolved roll crop, or `None` when a hand-drawn crop, a non-Image
/// mode or an out-of-range resolve must keep the frame as it is.
#[must_use]
pub fn apply_resolved_crop(
    config: &crate::domain::models::WorkspaceConfig,
    crop: &ResolvedCrop,
) -> Option<crate::domain::models::WorkspaceConfig> {
    let geometry = &config.geometry;
    if crate::features::geometry::logic::has_manual_crop(geometry)
        || geometry.autocrop_mode != crate::domain::models::AutocropMode::Image
    {
        return None;
    }
    let [x1, y1, x2, y2] = crop.crop_rect;
    if !(0.0..1.0).contains(&x1)
        || !(0.0..1.0).contains(&y1)
        || !(x1 < x2 && x2 <= 1.0 && y1 < y2 && y2 <= 1.0)
        || !crop.correction_angle.is_finite()
    {
        return None;
    }
    let fine_rotation = geometry.fine_rotation + crop.correction_angle;
    if fine_rotation.abs() > crate::features::geometry::logic::FINE_ROTATION_LIMIT {
        return None;
    }
    let mut updated = config.clone();
    updated.geometry.fine_rotation = fine_rotation;
    updated.geometry.crop_rect = Some(crop.crop_rect.to_vec());
    updated.geometry.crop_from_auto = true;
    updated.geometry.crop_detect_key =
        crate::features::geometry::logic::autocrop_detection_key(&updated.geometry);
    updated.geometry.autocrop_squared = false;
    let (invalidate, floors, ceils) =
        crate::domain::models::process::invalidate_local_bounds(&updated.process);
    if invalidate {
        updated.process.local_floors = floors;
        updated.process.local_ceils = ceils;
    }
    Some(updated)
}

fn side_border(lum: &[f32], roi: Roi, (height, width): (usize, usize)) -> Vec<f64> {
    let walked = crate::features::geometry::processor::measure_film_edges(lum, roi, height, width);
    let measured =
        crate::features::geometry::processor::measure_film_border(lum, roi, height, width);
    (0..4)
        .map(|index| {
            if walked[index] > 0.0 {
                walked[index]
            } else {
                measured[index]
            }
        })
        .collect()
}

fn bright_side_border(lum: &[f32], roi: Roi, (height, width): (usize, usize)) -> Vec<f64> {
    crate::features::geometry::processor::measure_film_border(lum, roi, height, width).to_vec()
}

fn no_box_evidence(
    key: &str,
    image: &crate::infrastructure::loaders::image_buffer::ImageBuffer,
    detection: &crate::features::geometry::processor::AutocropDetection,
    correction: f64,
    angle_confident: bool,
    target_ratio: &str,
    rebate_trim: f64,
) -> CropEvidence {
    let lum = crate::features::geometry::processor::detection_luma(image);
    let box_roi = crate::features::geometry::processor::threshold_autocrop_coords(image, None);
    let box_roi = crate::features::geometry::processor::trim_opaque_border(
        &lum,
        box_roi,
        image.height,
        image.width,
        0.02,
        0.7,
        0.2,
    );
    let border = side_border(&lum, box_roi, (image.height, image.width));
    let bright = bright_side_border(&lum, box_roi, (image.height, image.width));
    CropEvidence {
        key: key.to_string(),
        canvas_shape: (image.height, image.width),
        roi: None,
        correction_angle: correction,
        confidence: 0.0,
        target_ratio: target_ratio.to_string(),
        rebate_trim,
        angle_confident,
        vertical_edge_contrast: detection.vertical_edge_contrast,
        vertical_edge_profile: detection.vertical_edge_profile.clone(),
        border,
        bright_border: bright,
        fallback_roi: Some(box_roi),
        reason: "no_consensus".to_string(),
        ..CropEvidence::default()
    }
}

/// Collect crop, deskew and edge evidence for one transformed preview.
#[must_use]
pub fn detect_crop_candidate(
    key: &str,
    image: &crate::infrastructure::loaders::image_buffer::ImageBuffer,
    target_ratio: &str,
    rebate_trim: f64,
) -> CropEvidence {
    use crate::features::geometry::logic::apply_fine_rotation;
    use crate::features::geometry::processor::{
        detect_film_bounds_with_confidence, detection_luma, get_autocrop_coords, trim_opaque_border,
    };
    let (profile, profile_contrast) =
        negpy_core::analysis::batch_edge_profile(&image.pixels, image.height, image.width, 3);
    if image.width <= image.height {
        let skew = edge_fit(image);
        let own_angle = skew.unwrap_or(0.0);
        let own_image = if own_angle.abs() > 1e-4 {
            apply_fine_rotation(image, own_angle)
        } else {
            image.clone()
        };
        let ratio = crate::domain::models::AspectRatio::coerce(target_ratio);
        let own_roi = get_autocrop_coords(
            &own_image,
            0,
            1.0,
            ratio,
            1800,
            None,
            crate::domain::models::AutocropMode::Image,
            rebate_trim,
        );
        return CropEvidence {
            key: key.to_string(),
            canvas_shape: (image.height, image.width),
            roi: None,
            correction_angle: own_angle,
            confidence: 0.0,
            target_ratio: target_ratio.to_string(),
            rebate_trim,
            angle_confident: skew.is_some(),
            vertical_edge_contrast: profile_contrast,
            vertical_edge_profile: profile,
            fallback_roi: Some(own_roi),
            reason: "unsupported_orientation".to_string(),
            ..CropEvidence::default()
        };
    }
    let initial = detect_film_bounds_with_confidence(image);
    let mut correction = if initial.roi.is_some() {
        initial.correction_angle
    } else {
        0.0
    };
    if !correction.is_finite() || correction.abs() > MAX_AUTOMATIC_DESKEW {
        correction = 0.0;
    }
    let fitted = initial.roi.and_then(|roi| {
        negpy_core::autocrop::top_edge_slope(
            &detection_luma(image),
            (image.height, image.width),
            (roi.0, roi.2, roi.3),
        )
    });
    let mut angle_confident =
        fitted.is_some_and(|value| (value - correction).abs() <= MAX_EDGE_FIT_DELTA);
    if angle_confident {
        correction = fitted
            .unwrap()
            .clamp(-MAX_AUTOMATIC_DESKEW, MAX_AUTOMATIC_DESKEW);
    } else if let Some(skew) = edge_fit(image) {
        correction = skew;
        angle_confident = true;
    } else if initial.roi.is_none() {
        return no_box_evidence(key, image, &initial, 0.0, false, target_ratio, rebate_trim);
    }
    let corrected = if correction.abs() > 1e-4 {
        apply_fine_rotation(image, correction)
    } else {
        image.clone()
    };
    let final_detection = if correction.abs() > 1e-4 {
        detect_film_bounds_with_confidence(&corrected)
    } else {
        initial.clone()
    };
    if final_detection.roi.is_none() {
        if initial.roi.is_none() {
            return no_box_evidence(
                key,
                &corrected,
                &final_detection,
                correction,
                angle_confident,
                target_ratio,
                rebate_trim,
            );
        }
        return CropEvidence {
            key: key.to_string(),
            canvas_shape: (corrected.height, corrected.width),
            roi: None,
            correction_angle: correction,
            confidence: 0.0,
            target_ratio: target_ratio.to_string(),
            rebate_trim,
            angle_confident,
            vertical_edge_contrast: final_detection.vertical_edge_contrast,
            vertical_edge_profile: final_detection.vertical_edge_profile,
            reason: "deskew_no_consensus".to_string(),
            ..CropEvidence::default()
        };
    }
    let lum = detection_luma(&corrected);
    let roi = trim_opaque_border(
        &lum,
        final_detection.roi.unwrap(),
        corrected.height,
        corrected.width,
        0.02,
        0.7,
        0.2,
    );
    let border = side_border(&lum, roi, (corrected.height, corrected.width));
    let bright = bright_side_border(&lum, roi, (corrected.height, corrected.width));
    if roi.1 <= roi.0 || roi.3 <= roi.2 {
        return CropEvidence {
            key: key.to_string(),
            canvas_shape: (corrected.height, corrected.width),
            roi: None,
            correction_angle: correction,
            confidence: 0.0,
            target_ratio: target_ratio.to_string(),
            rebate_trim,
            angle_confident,
            vertical_edge_contrast: final_detection.vertical_edge_contrast,
            vertical_edge_profile: final_detection.vertical_edge_profile,
            reason: "invalid_geometry".to_string(),
            ..CropEvidence::default()
        };
    }
    CropEvidence {
        key: key.to_string(),
        canvas_shape: (corrected.height, corrected.width),
        roi: Some(roi),
        correction_angle: correction,
        confidence: final_detection.confidence.clamp(0.0, 1.0),
        target_ratio: target_ratio.to_string(),
        rebate_trim,
        angle_confident,
        supported_sides: final_detection.supported_sides,
        supported_corners: final_detection.supported_corners,
        evidence_sources: final_detection.evidence_sources,
        geometry_score: final_detection.geometry_score.clamp(0.0, 1.0),
        vertical_edge_contrast: final_detection.vertical_edge_contrast,
        vertical_edge_profile: final_detection.vertical_edge_profile,
        border,
        bright_border: bright,
        fallback_roi: None,
        reason: String::new(),
    }
}
