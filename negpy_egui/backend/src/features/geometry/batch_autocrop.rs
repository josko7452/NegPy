//! Roll-aware crop calibration built on the single-frame film detector.
//!
//! Mirrors `negpy/features/geometry/batch_autocrop.py` without Qt, storage or file I/O.

use crate::features::geometry::logic::Roi;

pub const BORDER_SIDES: [&str; 4] = ["top", "bottom", "left", "right"];
pub const TRUSTED_CONFIDENCE: f64 = 0.58;
pub const MAX_AUTOMATIC_DESKEW: f64 = 8.0;
pub const DEFAULT_SAFETY_BORDER: f64 = 0.01;
const MIN_BORDER_SAMPLES: usize = 5;
const FALLBACK_CONFIDENCE: f64 = 0.30;
const MIN_FALLBACK_COVERAGE: f64 = 0.9;
const MIN_TRUSTED_FRAMES: usize = 2;

/// Single-frame evidence in the final, deskewed display coordinate space.
#[derive(Clone, Debug, Default, PartialEq)]
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
        .map(|item| CropEvidence {
            roi: item.fallback_roi,
            confidence: FALLBACK_CONFIDENCE,
            evidence_sources: vec!["threshold-fallback"],
            ..item.clone()
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
