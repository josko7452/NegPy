//! The geometry, crop and auto-crop processors in `negpy/features/geometry/processor.py` order.

use crate::domain::models::{AspectRatio, AutocropMode, GeometryConfig};
use crate::features::geometry::logic::{
    Roi, apply_fine_rotation, apply_geometry_orientation, apply_keystone, apply_margin_to_roi,
    apply_radial_distortion, compute_geometry_crop_rect, crop, get_manual_rect_coords,
    ratio_aspect, ratio_dims,
};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// The frame inputs the geometry stage reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct GeometryContext {
    pub scale_factor: f64,
}

/// The transforms that coordinate mappers replay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeometryParams {
    pub rotation: i64,
    pub fine_rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub converge_v: f64,
    pub converge_h: f64,
}

/// The geometry output and the downstream coordinate correction.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryResult {
    pub image: ImageBuffer,
    pub active_roi: Option<Roi>,
    pub distortion_k1: f64,
    pub geometry_params: GeometryParams,
}

/// Orientation, fine rotation, distortion, keystone and the active ROI.
#[must_use]
pub fn process(
    img: &ImageBuffer,
    config: &GeometryConfig,
    context: GeometryContext,
) -> GeometryResult {
    let mut image = apply_geometry_orientation(
        img,
        config.rotation,
        config.flip_horizontal,
        config.flip_vertical,
    );
    if config.fine_rotation != 0.0 {
        image = apply_fine_rotation(&image, config.fine_rotation);
    }
    if config.distortion_k1 != 0.0 {
        image = apply_radial_distortion(&image, config.distortion_k1);
    }
    image = apply_keystone(&image, config.converge_v, config.converge_h);

    let active_roi = if let Some(rect) = config.crop_rect.as_deref().filter(|rect| rect.len() == 4)
    {
        Some(get_manual_rect_coords(
            (image.height, image.width),
            rect,
            config.autocrop_offset,
            context.scale_factor,
        ))
    } else if config.crop_to_valid && !config.crop_from_auto {
        let rect = compute_geometry_crop_rect(
            config.fine_rotation,
            config.converge_v,
            config.converge_h,
            image.width,
            image.height,
        );
        Some(get_manual_rect_coords(
            (image.height, image.width),
            &[rect.0, rect.1, rect.2, rect.3],
            config.autocrop_offset,
            context.scale_factor,
        ))
    } else if config.autocrop_offset > 0 {
        let margin = config.autocrop_offset as f64 * context.scale_factor;
        Some(apply_margin_to_roi(
            (0, image.height as i64, 0, image.width as i64),
            image.height,
            image.width,
            margin,
        ))
    } else {
        None
    };

    GeometryResult {
        image,
        active_roi,
        distortion_k1: config.distortion_k1,
        geometry_params: GeometryParams {
            rotation: config.rotation,
            fine_rotation: config.fine_rotation,
            flip_horizontal: config.flip_horizontal,
            flip_vertical: config.flip_vertical,
            converge_v: config.converge_v,
            converge_h: config.converge_h,
        },
    }
}

/// `CropProcessor`: apply the resolved window.
#[must_use]
pub fn crop_image(image: &ImageBuffer, active_roi: Option<Roi>) -> ImageBuffer {
    crop(image, active_roi)
}

/// `AutocropDetection`: outer-film evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct AutocropDetection {
    pub roi: Option<Roi>,
    pub correction_angle: f64,
    pub confidence: f64,
    pub supported_sides: Vec<&'static str>,
    pub supported_corners: Vec<&'static str>,
    pub evidence_sources: Vec<&'static str>,
    pub geometry_score: f64,
    pub vertical_edge_contrast: f64,
    pub vertical_edge_profile: Vec<f32>,
}

impl Default for AutocropDetection {
    fn default() -> Self {
        Self {
            roi: None,
            correction_angle: 0.0,
            confidence: 0.0,
            supported_sides: Vec::new(),
            supported_corners: Vec::new(),
            evidence_sources: Vec::new(),
            geometry_score: 0.0,
            vertical_edge_contrast: 0.0,
            vertical_edge_profile: Vec::new(),
        }
    }
}

/// `detect_film_bounds_with_confidence`.
#[must_use]
pub fn detect_film_bounds_with_confidence(img: &ImageBuffer) -> AutocropDetection {
    if img.width < 2 || img.height < 2 {
        return AutocropDetection::default();
    }
    let detection = negpy_core::detect::detect_film_bounds(&img.pixels, img.height, img.width, 3);
    let mut out = AutocropDetection {
        vertical_edge_profile: detection.profile,
        vertical_edge_contrast: detection.contrast.max(0.0),
        ..AutocropDetection::default()
    };
    let Some(c) = detection.consensus else {
        return out;
    };
    out.roi = Some(c.roi);
    out.correction_angle = c.correction_angle.clamp(-45.0, 45.0);
    out.confidence = c.confidence.clamp(0.0, 1.0);
    out.supported_sides = c.sides;
    out.supported_corners = c.corners;
    out.evidence_sources = c.sources;
    out.geometry_score = c.geometry_score.clamp(0.0, 1.0);
    out
}

/// `_detect_film_bounds`.
#[must_use]
pub fn detect_film_bounds(img: &ImageBuffer) -> Option<Roi> {
    detect_film_bounds_with_confidence(img).roi
}

/// `_normalize_detection_input`: area-downsample to `detect_res`, never upscale.
#[must_use]
pub fn normalize_detection_input(img: &ImageBuffer, detect_res: u32) -> (ImageBuffer, f64) {
    let scale = (detect_res as f64 / img.width.max(img.height) as f64).min(1.0);
    if scale >= 1.0 {
        return (img.clone(), 1.0);
    }
    let width = ((img.width as f64 * scale).round() as usize).max(1);
    let height = ((img.height as f64 * scale).round() as usize).max(1);
    let pixels =
        negpy_core::resize::resize_area(&img.pixels, img.height, img.width, 3, height, width)
            .expect("a positive source and destination size");
    (ImageBuffer::from_rgb(width, height, pixels), scale)
}

/// `_scale_roi`.
#[must_use]
pub fn scale_roi(roi: Roi, det_scale: f64, height: usize, width: usize) -> Roi {
    if det_scale >= 1.0 {
        return roi;
    }
    (
        ((roi.0 as f64 / det_scale).round() as i64).max(0),
        ((roi.1 as f64 / det_scale).round() as i64).min(height as i64),
        ((roi.2 as f64 / det_scale).round() as i64).max(0),
        ((roi.3 as f64 / det_scale).round() as i64).min(width as i64),
    )
}

/// `_detection_luma`: the P99.5-anchored plane.
#[must_use]
pub fn detection_luma(img: &ImageBuffer) -> Vec<f32> {
    let mut lum = vec![0.0; img.width * img.height];
    negpy_core::analysis::luminance(&img.pixels, &mut lum);
    let mut det = vec![0.0; lum.len()];
    negpy_core::analysis::detection_luma(&lum, &mut det).expect("a non-empty plane");
    det
}

/// `_trim_opaque_border`.
#[must_use]
pub fn trim_opaque_border(
    lum: &[f32],
    roi: Roi,
    height: usize,
    width: usize,
    black: f32,
    frac: f32,
    max_trim: f32,
) -> Roi {
    let (y1, y2, x1, x2) = roi;
    let (y1u, y2u) = (y1.max(0) as usize, y2.max(0).min(height as i64) as usize);
    let (x1u, x2u) = (x1.max(0) as usize, x2.max(0).min(width as i64) as usize);
    let (bh, bw) = (y2u.saturating_sub(y1u), x2u.saturating_sub(x1u));
    if bh < 4 || bw < 4 {
        return roi;
    }
    let row_black: Vec<f32> = (0..bh)
        .map(|y| {
            (0..bw)
                .filter(|&x| lum[(y1u + y) * width + x1u + x] < black)
                .count() as f32
                / bw as f32
        })
        .collect();
    let col_black: Vec<f32> = (0..bw)
        .map(|x| {
            (0..bh)
                .filter(|&y| lum[(y1u + y) * width + x1u + x] < black)
                .count() as f32
                / bh as f32
        })
        .collect();
    let run = |profile: &[f32], limit: usize, from_start: bool| {
        let n = profile.len();
        let mut i = 0;
        while i < limit {
            let v = profile[if from_start { i } else { n - 1 - i }];
            if v < frac {
                break;
            }
            i += 1;
        }
        if i >= limit { 0 } else { i as i64 }
    };
    let ly = (max_trim * bh as f32).round() as usize;
    let lx = (max_trim * bw as f32).round() as usize;
    let top = run(&row_black, ly, true);
    let bottom = run(&row_black, ly, false);
    let left = run(&col_black, lx, true);
    let right = run(&col_black, lx, false);
    let out = (y1 + top, y2 - bottom, x1 + left, x2 - right);
    if out.1 <= out.0 || out.3 <= out.2 {
        roi
    } else {
        out
    }
}

/// `_get_threshold_autocrop_coords`.
#[must_use]
pub fn threshold_autocrop_coords(img: &ImageBuffer, assist_luma: Option<f64>) -> Roi {
    let threshold = assist_luma.map_or(0.96, |v| (v - 0.02).clamp(0.5, 0.98)) as f32;
    let lum = detection_luma(img);
    negpy_core::analysis::threshold_box(&lum, img.height, img.width, threshold)
        .map(|(y1, y2, x1, x2)| (y1 as i64, y2 as i64, x1 as i64, x2 as i64))
        .unwrap_or((0, img.height as i64, 0, img.width as i64))
}

/// `_refine_roi_to_image`.
#[must_use]
pub fn refine_roi_to_image(
    img: &ImageBuffer,
    film_roi: Roi,
) -> Option<(Roi, Option<Vec<f32>>, Option<Vec<f32>>)> {
    let (roi, tiers) =
        negpy_core::analysis::refine_roi_to_image(&img.pixels, img.height, img.width, 3, film_roi)?;
    let (rows, cols) = tiers.map_or((None, None), |t| (Some(t.rows), Some(t.cols)));
    Some((roi, rows, cols))
}

/// `_trim_film_edges`.
#[must_use]
pub fn trim_film_edges(lum: &[f32], film_roi: Roi, height: usize, width: usize) -> Roi {
    negpy_core::analysis::trim_film_edges(lum, height, width, film_roi)
}

/// `scale_roi_inset`.
#[must_use]
pub fn scale_roi_inset(film_roi: Roi, roi: Roi, factor: f64) -> Roi {
    if factor == 1.0 {
        return roi;
    }
    let scaled = (
        film_roi.0 + ((roi.0 - film_roi.0) as f64 * factor).round() as i64,
        film_roi.1 - ((film_roi.1 - roi.1) as f64 * factor).round() as i64,
        film_roi.2 + ((roi.2 - film_roi.2) as f64 * factor).round() as i64,
        film_roi.3 - ((film_roi.3 - roi.3) as f64 * factor).round() as i64,
    );
    if scaled.1 <= scaled.0 || scaled.3 <= scaled.2 {
        roi
    } else {
        scaled
    }
}

/// `enforce_roi_aspect_ratio`.
#[must_use]
pub fn enforce_roi_aspect_ratio(roi: Roi, height: usize, width: usize, ratio: AspectRatio) -> Roi {
    let (mut y1, mut y2, mut x1, mut x2) = roi;
    let (cw, ch) = (x2 - x1, y2 - y1);
    if cw <= 0 || ch <= 0 {
        return (0, height as i64, 0, width as i64);
    }
    if ratio == AspectRatio::Free {
        return (
            y1.max(0),
            y2.min(height as i64),
            x1.max(0),
            x2.min(width as i64),
        );
    }
    let (target_w, target_h) = ratio_dims(cw as usize, ch as usize, ratio);
    if target_w < cw as f64 {
        let nx1 = x1 as f64 + (cw as f64 - target_w) / 2.0;
        x1 = nx1 as i64;
        x2 = (nx1 + target_w) as i64;
    } else if target_h < ch as f64 {
        let ny1 = y1 as f64 + (ch as f64 - target_h) / 2.0;
        y1 = ny1 as i64;
        y2 = (ny1 + target_h) as i64;
    }
    (
        y1.max(0),
        y2.min(height as i64),
        x1.max(0),
        x2.min(width as i64),
    )
}

/// `_enforce_ratio_by_occupancy`.
#[must_use]
pub fn enforce_ratio_by_occupancy(
    roi: Roi,
    height: usize,
    width: usize,
    ratio: AspectRatio,
    row_occupancy: &[f32],
    col_occupancy: &[f32],
    det_scale: f64,
) -> Roi {
    let (mut y1, mut y2, mut x1, mut x2) = roi;
    let (cw, ch) = (x2 - x1, y2 - y1);
    if cw <= 0 || ch <= 0 {
        return (0, height as i64, 0, width as i64);
    }
    if ratio == AspectRatio::Free {
        return (
            y1.max(0),
            y2.min(height as i64),
            x1.max(0),
            x2.min(width as i64),
        );
    }
    let (target_w, target_h) = ratio_dims(cw as usize, ch as usize, ratio);
    if target_w < cw as f64 {
        x1 = negpy_core::autocrop::place_window_by_occupancy(
            x1,
            x2,
            target_w,
            col_occupancy,
            det_scale,
        );
        x2 = (x1 as f64 + target_w).round() as i64;
    } else if target_h < ch as f64 {
        y1 = negpy_core::autocrop::place_window_by_occupancy(
            y1,
            y2,
            target_h,
            row_occupancy,
            det_scale,
        );
        y2 = (y1 as f64 + target_h).round() as i64;
    }
    (
        y1.max(0),
        y2.min(height as i64),
        x1.max(0),
        x2.min(width as i64),
    )
}

/// `_closest_standard_ratio` for the film formats only.
#[must_use]
pub fn closest_standard_ratio(
    roi: Roi,
    img_shape: (usize, usize),
    fallback: AspectRatio,
) -> AspectRatio {
    const FILM_FORMATS: [AspectRatio; 11] = [
        AspectRatio::R1x1,
        AspectRatio::R3x2,
        AspectRatio::R2x3,
        AspectRatio::R4x3,
        AspectRatio::R3x4,
        AspectRatio::R5x4,
        AspectRatio::R4x5,
        AspectRatio::R6x7,
        AspectRatio::R7x6,
        AspectRatio::R65x24,
        AspectRatio::R24x65,
    ];
    let (cw, ch) = ((roi.3 - roi.2) as f64, (roi.1 - roi.0) as f64);
    if cw <= 0.0 || ch <= 0.0 {
        return fallback;
    }
    let detected = cw / ch;
    let landscape = cw >= ch;
    let candidates: Vec<(AspectRatio, f64)> = FILM_FORMATS
        .into_iter()
        .filter_map(|ratio| {
            let target = ratio_aspect(ratio)?;
            let target_landscape = target >= 1.0;
            (landscape == target_landscape || target == 1.0).then_some((ratio, target))
        })
        .collect();
    let best = candidates
        .iter()
        .min_by(|a, b| {
            (detected.max(1e-6).ln() - a.1.max(1e-6).ln())
                .abs()
                .total_cmp(&(detected.max(1e-6).ln() - b.1.max(1e-6).ln()).abs())
        })
        .copied()
        .unwrap_or((fallback, 1.5));
    let (ih, iw) = img_shape;
    let img_ratio = iw as f64 / ih as f64;
    let stretched = detected.max(1e-6).ln().abs() > img_ratio.max(1e-6).ln().abs();
    if stretched && (img_ratio.max(1e-6).ln() - best.1.max(1e-6).ln()).abs() > 0.3 {
        candidates
            .iter()
            .min_by(|a, b| {
                (img_ratio.max(1e-6).ln() - a.1.max(1e-6).ln())
                    .abs()
                    .total_cmp(&(img_ratio.max(1e-6).ln() - b.1.max(1e-6).ln()).abs())
            })
            .map(|c| c.0)
            .unwrap_or(fallback)
    } else {
        best.0
    }
}

/// `get_autocrop_coords`: detect, refine, scale and enforce the ratio.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn get_autocrop_coords(
    img: &ImageBuffer,
    offset_px: i64,
    scale_factor: f64,
    target_ratio: AspectRatio,
    detect_res: u32,
    assist_luma: Option<f64>,
    mode: AutocropMode,
    rebate_trim: f64,
) -> Roi {
    let (det, det_scale) = normalize_detection_input(img, detect_res);
    let mut film_roi = detect_film_bounds(&det);
    let from_contours = film_roi.is_some();
    if film_roi.is_none() {
        film_roi = Some(threshold_autocrop_coords(&det, assist_luma));
    }
    let lum = detection_luma(&det);
    let film_roi = trim_opaque_border(
        &lum,
        film_roi.expect("a fallback box"),
        det.height,
        det.width,
        0.02,
        0.7,
        0.2,
    );
    let (mut roi, mut row_occ, mut col_occ) = (film_roi, None, None);
    if mode == AutocropMode::Image {
        if from_contours {
            if let Some((refined, rows, cols)) = refine_roi_to_image(&det, film_roi) {
                roi = refined;
                row_occ = rows;
                col_occ = cols;
            }
        } else {
            roi = trim_film_edges(&lum, film_roi, det.height, det.width);
        }
    }
    roi = scale_roi_inset(film_roi, roi, rebate_trim);
    roi = scale_roi(roi, det_scale, img.height, img.width);
    let ratio = if target_ratio == AspectRatio::Free {
        closest_standard_ratio(roi, (img.height, img.width), AspectRatio::R3x2)
    } else {
        target_ratio
    };
    let margin = (2 + offset_px) as f64 * scale_factor;
    roi = apply_margin_to_roi(roi, img.height, img.width, margin);
    match (row_occ.as_deref(), col_occ.as_deref()) {
        (Some(rows), Some(cols)) => {
            enforce_ratio_by_occupancy(roi, img.height, img.width, ratio, rows, cols, det_scale)
        }
        _ => enforce_roi_aspect_ratio(roi, img.height, img.width, ratio),
    }
}

/// `resolve_autocrop_rect`: a normalized rect in transformed-image space.
#[must_use]
pub fn resolve_autocrop_rect(
    img: &ImageBuffer,
    config: &GeometryConfig,
    preview_size: f64,
    require_film_box: bool,
) -> Option<[f64; 4]> {
    let (det, _) = normalize_detection_input(img, 1800);
    let mut tmp = apply_geometry_orientation(
        &det,
        config.rotation,
        config.flip_horizontal,
        config.flip_vertical,
    );
    if config.fine_rotation != 0.0 {
        tmp = apply_fine_rotation(&tmp, config.fine_rotation);
    }
    if config.distortion_k1 != 0.0 {
        tmp = apply_radial_distortion(&tmp, config.distortion_k1);
    }
    tmp = apply_keystone(&tmp, config.converge_v, config.converge_h);
    let (rh, rw) = (tmp.height, tmp.width);
    if rh < 2 || rw < 2 {
        return None;
    }
    if require_film_box && detect_film_bounds(&normalize_detection_input(&tmp, 1800).0).is_none() {
        return None;
    }
    let (y1, y2, x1, x2) = get_autocrop_coords(
        &tmp,
        0,
        rh.max(rw) as f64 / preview_size,
        config.autocrop_ratio,
        1800,
        None,
        config.autocrop_mode,
        config.autocrop_rebate_trim,
    );
    if y2 - y1 < 2 || x2 - x1 < 2 {
        return None;
    }
    Some([
        x1 as f64 / rw as f64,
        y1 as f64 / rh as f64,
        x2 as f64 / rw as f64,
        y2 as f64 / rh as f64,
    ])
}

/// `resolve_meter_rect`.
#[must_use]
pub fn resolve_meter_rect(
    img: &ImageBuffer,
    config: &GeometryConfig,
    preview_size: f64,
) -> Option<[f64; 4]> {
    resolve_autocrop_rect(img, config, preview_size, true)
}

/// `detect_closest_aspect_ratio`.
#[must_use]
pub fn detect_closest_aspect_ratio(img: &ImageBuffer, fallback: AspectRatio) -> AspectRatio {
    let (det, det_scale) = normalize_detection_input(img, 1800);
    let roi = detect_film_bounds(&det).map_or_else(
        || threshold_autocrop_coords(&det, None),
        |film| refine_roi_to_image(&det, film).map_or(film, |(roi, _, _)| roi),
    );
    closest_standard_ratio(
        scale_roi(roi, det_scale, img.height, img.width),
        (img.height, img.width),
        fallback,
    )
}
