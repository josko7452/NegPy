//! Ports of the transform and crop helpers in `negpy/features/geometry/logic.py`.

use negpy_core::warp::{self, Border, Image, Interp};

use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const FINE_ROTATION_LIMIT: f64 = 45.0;
const DISTORT_EPS: f64 = 1e-6;
const KEYSTONE_EPS: f64 = 1e-4;
const CROP_TO_VALID_ITERS: usize = 30;
const CROP_TO_VALID_TERNARY_ITERS: usize = 25;
const GEOMETRY_EXIF: [u8; 16] = [1, 4, 2, 3, 8, 5, 7, 6, 3, 2, 4, 1, 6, 7, 5, 8];

/// A half-open pixel window, `(y1, y2, x1, x2)`.
pub type Roi = (i64, i64, i64, i64);

/// `_resolve_ratio_dims`: the orientation-corrected target width and height.
#[must_use]
pub fn ratio_dims(cw: usize, ch: usize, ratio: crate::domain::models::AspectRatio) -> (f64, f64) {
    let target_aspect = ratio_aspect(ratio).unwrap_or(1.5);
    let target_aspect = if ch > cw {
        if target_aspect > 1.0 {
            1.0 / target_aspect
        } else {
            target_aspect
        }
    } else if target_aspect < 1.0 {
        1.0 / target_aspect
    } else {
        target_aspect
    };
    let current = cw as f64 / ch as f64;
    if current > target_aspect {
        (ch as f64 * target_aspect, ch as f64)
    } else {
        (cw as f64, cw as f64 / target_aspect)
    }
}

/// The canonical width-over-height ratio without orientation correction.
#[must_use]
pub fn ratio_aspect(ratio: crate::domain::models::AspectRatio) -> Option<f64> {
    let text = ratio.as_str();
    let (w, h) = text.split_once(':')?;
    Some(w.parse::<f64>().ok()? / h.parse::<f64>().ok()?.max(1e-9))
}

/// `apply_geometry_orientation`: rot90, then the two mirrors, as one copy.
#[must_use]
pub fn apply_geometry_orientation(
    img: &ImageBuffer,
    rotation: i64,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> ImageBuffer {
    let k = rotation.rem_euclid(4);
    if k == 0 && !flip_horizontal && !flip_vertical {
        return img.clone();
    }
    let index = (k * 4 + i64::from(flip_horizontal) * 2 + i64::from(flip_vertical)) as usize;
    let code = GEOMETRY_EXIF[index];
    if code == 1 {
        return img.clone();
    }
    let src: Vec<u8> = img.pixels.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let (oh, ow) = negpy_core::orient::oriented_dims(img.height, img.width, code);
    let mut dst = vec![0u8; oh * ow * 12];
    negpy_core::orient::orient(&src, img.height, img.width, 12, code, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    ImageBuffer::from_rgb(ow, oh, pixels)
}

/// `apply_fine_rotation`, bilinear and replicated at the edges.
#[must_use]
pub fn apply_fine_rotation(img: &ImageBuffer, angle: f64) -> ImageBuffer {
    if angle == 0.0 {
        return img.clone();
    }
    let center = (img.width as f32 / 2.0, img.height as f32 / 2.0);
    let matrix = warp::rotation_matrix(center, angle, 1.0);
    let source = Image {
        data: &img.pixels,
        h: img.height,
        w: img.width,
        cn: 3,
    };
    let mut out = vec![0.0; img.pixels.len()];
    warp::warp_affine(
        &source,
        (Interp::Linear, Border::Replicate),
        matrix,
        false,
        img.width,
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

fn radial_center(width: usize, height: usize) -> (f64, f64, f64) {
    (
        (width - 1) as f64 * 0.5,
        (height - 1) as f64 * 0.5,
        (0.5 * ((width * width + height * height) as f64).sqrt()),
    )
}

/// The radial center and half-diagonal, as the point mapper reads them.
#[must_use]
pub fn radial_center_public(width: usize, height: usize) -> (f64, f64, f64) {
    radial_center(width, height)
}

/// `compute_distortion_scale`: the largest fill scale that stays inside the source.
#[must_use]
pub fn compute_distortion_scale(k1: f64, width: usize, height: usize) -> f64 {
    if k1.abs() < 1e-9 {
        return 1.0;
    }
    let (cx, cy, halfdiag) = radial_center(width, height);
    let (hw, hh) = (cx, cy);
    let inv_hd2 = 1.0 / (halfdiag * halfdiag);
    let edge = 32;
    let mut points = Vec::with_capacity(edge * 4);
    for i in 0..edge {
        let t = i as f64 / edge as f64;
        points.push((-hw + 2.0 * hw * t, -hh));
        points.push((-hw + 2.0 * hw * t, hh));
        points.push((-hw, -hh + 2.0 * hh * t));
        points.push((hw, -hh + 2.0 * hh * t));
    }
    let max_ratio = |s: f64| {
        let mut worst = 0.0f64;
        for &(px, py) in &points {
            let pxs = px * s;
            let pys = py * s;
            let f = 1.0 + k1 * (pxs * pxs + pys * pys) * inv_hd2;
            if f <= 0.0 {
                return f64::INFINITY;
            }
            worst = worst.max((pxs * f).abs() / hw).max((pys * f).abs() / hh);
        }
        worst
    };
    let (mut lo, mut hi) = (1e-3, 1.0);
    while max_ratio(hi) < 1.0 && hi < 1e3 {
        hi *= 2.0;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if max_ratio(mid) < 1.0 {
            lo = mid
        } else {
            hi = mid
        }
    }
    0.5 * (lo + hi)
}

/// `apply_radial_distortion`: a purely geometric remap.
#[must_use]
pub fn apply_radial_distortion(img: &ImageBuffer, k1: f64) -> ImageBuffer {
    if k1.abs() < DISTORT_EPS {
        return img.clone();
    }
    let (cx, cy, halfdiag) = radial_center(img.width, img.height);
    let scale = compute_distortion_scale(k1, img.width, img.height);
    let source = Image {
        data: &img.pixels,
        h: img.height,
        w: img.width,
        cn: 3,
    };
    let mut out = vec![0.0; img.pixels.len()];
    warp::remap_radial(
        &source,
        (cx as f32, cy as f32),
        scale as f32,
        k1 as f32,
        (1.0 / (halfdiag * halfdiag)) as f32,
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

fn matmul3(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d.abs() < 1e-15 {
        return None;
    }
    let d = 1.0 / d;
    Some([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
        ],
    ])
}

/// `keystone_quad` in normalized coordinates.
#[must_use]
pub fn keystone_quad(converge_v: f64, converge_h: f64) -> [[f32; 2]; 4] {
    let a = (converge_v * 0.005) as f32;
    let b = (converge_h * 0.005) as f32;
    [[a, b], [1.0 - a, -b], [1.0 + a, 1.0 + b], [-a, 1.0 - b]]
}

/// `keystone_matrix_normalized`: source quad to the unit square.
#[must_use]
pub fn keystone_matrix_normalized(converge_v: f64, converge_h: f64) -> [[f64; 3]; 3] {
    let src = keystone_quad(converge_v, converge_h);
    let dst = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let m = warp::perspective_transform(&src, &dst).expect("the keystone quad is non-degenerate");
    [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]]
}

/// The keystone's inverse in normalized coordinates, which the shader consumes.
#[must_use]
pub fn keystone_inverse_normalized(converge_v: f64, converge_h: f64) -> [[f64; 3]; 3] {
    if converge_v.abs() < KEYSTONE_EPS && converge_h.abs() < KEYSTONE_EPS {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let inverse = invert3(keystone_matrix_normalized(converge_v, converge_h))
        .expect("the keystone matrix is invertible");
    let scale = inverse[2][2];
    inverse.map(|row| row.map(|value| value / scale))
}

/// `keystone_matrix`: the same map in pixel-index coordinates.
#[must_use]
pub fn keystone_matrix(
    converge_v: f64,
    converge_h: f64,
    width: usize,
    height: usize,
) -> [[f64; 3]; 3] {
    let a = [
        [width as f64, 0.0, -0.5],
        [0.0, height as f64, -0.5],
        [0.0, 0.0, 1.0],
    ];
    let inv_a = [
        [1.0 / width as f64, 0.0, 0.5 / width as f64],
        [0.0, 1.0 / height as f64, 0.5 / height as f64],
        [0.0, 0.0, 1.0],
    ];
    matmul3(
        matmul3(a, keystone_matrix_normalized(converge_v, converge_h)),
        inv_a,
    )
}

/// `apply_keystone`: same canvas size, replicated edges.
#[must_use]
pub fn apply_keystone(img: &ImageBuffer, converge_v: f64, converge_h: f64) -> ImageBuffer {
    if converge_v.abs() < KEYSTONE_EPS && converge_h.abs() < KEYSTONE_EPS {
        return img.clone();
    }
    let m = keystone_matrix(converge_v, converge_h, img.width, img.height);
    let m = [
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
    ];
    let source = Image {
        data: &img.pixels,
        h: img.height,
        w: img.width,
        cn: 3,
    };
    let mut out = vec![0.0; img.pixels.len()];
    warp::warp_perspective(
        &source,
        (Interp::Linear, Border::Replicate),
        m,
        false,
        (img.width, img.height),
        &mut out,
    );
    ImageBuffer::from_rgb(img.width, img.height, out)
}

/// `compute_geometry_crop_rect`: the largest frame-centered rect without replicated edges.
#[must_use]
pub fn compute_geometry_crop_rect(
    fine_rotation: f64,
    converge_v: f64,
    converge_h: f64,
    width: usize,
    height: usize,
) -> (f64, f64, f64, f64) {
    if fine_rotation.abs() < 1e-9
        && converge_v.abs() < KEYSTONE_EPS
        && converge_h.abs() < KEYSTONE_EPS
    {
        return (0.0, 0.0, 1.0, 1.0);
    }
    let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
    let rot = warp::rotation_matrix((cx as f32, cy as f32), fine_rotation, 1.0);
    let m_rot = [
        [rot[0], rot[1], rot[2]],
        [rot[3], rot[4], rot[5]],
        [0.0, 0.0, 1.0],
    ];
    let combined = matmul3(
        keystone_matrix(converge_v, converge_h, width, height),
        m_rot,
    );
    let inv = invert3(combined).expect("the combined transform is invertible");
    let source_valid = |x: f64, y: f64| {
        let p = [
            inv[0][0] * x + inv[0][1] * y + inv[0][2],
            inv[1][0] * x + inv[1][1] * y + inv[1][2],
            inv[2][0] * x + inv[2][1] * y + inv[2][2],
        ];
        if p[2].abs() < 1e-12 {
            return false;
        }
        let sx = p[0] / p[2];
        let sy = p[1] / p[2];
        -1e-6 <= sx
            && sx <= (width - 1) as f64 + 1e-6
            && -1e-6 <= sy
            && sy <= (height - 1) as f64 + 1e-6
    };
    let corners_valid = |hw: f64, hh: f64| {
        [(-hw, -hh), (hw, -hh), (-hw, hh), (hw, hh)]
            .iter()
            .all(|&(dx, dy)| source_valid(cx + dx, cy + dy))
    };
    let (max_hw, max_hh) = (cx, cy);
    let hh_max = |hw: f64| {
        let (mut lo, mut hi) = (0.0, max_hh);
        for _ in 0..CROP_TO_VALID_ITERS {
            let mid = 0.5 * (lo + hi);
            if corners_valid(hw, mid) {
                lo = mid
            } else {
                hi = mid
            }
        }
        lo
    };
    let (mut lo_w, mut hi_w) = (0.0, max_hw);
    for _ in 0..CROP_TO_VALID_TERNARY_ITERS {
        let m1 = lo_w + (hi_w - lo_w) / 3.0;
        let m2 = hi_w - (hi_w - lo_w) / 3.0;
        if m1 * hh_max(m1) < m2 * hh_max(m2) {
            lo_w = m1
        } else {
            hi_w = m2
        }
    }
    let hw = 0.5 * (lo_w + hi_w);
    let hh = hh_max(hw);
    (
        (cx - hw) / width as f64,
        (cy - hh) / height as f64,
        (cx + hw) / width as f64,
        (cy + hh) / height as f64,
    )
}

/// `apply_margin_to_roi`: positive insets, clamped so the ROI cannot invert.
#[must_use]
pub fn apply_margin_to_roi(roi: Roi, height: usize, width: usize, margin_px: f64) -> Roi {
    let (y1, y2, x1, x2) = roi;
    let margin_px = margin_px
        .min((y2 - y1 - 1) as f64 / 2.0)
        .min((x2 - x1 - 1) as f64 / 2.0);
    let ny1 = y1 as f64 + margin_px;
    let ny2 = y2 as f64 - margin_px;
    let nx1 = x1 as f64 + margin_px;
    let nx2 = x2 as f64 - margin_px;
    (
        ny1.max(0.0) as i64,
        ny2.min(height as f64) as i64,
        nx1.max(0.0) as i64,
        nx2.min(width as f64) as i64,
    )
}

/// `get_manual_rect_coords`: a normalized rect in transformed space to a pixel ROI.
#[must_use]
pub fn get_manual_rect_coords(
    (height, width): (usize, usize),
    rect: &[f64],
    offset_px: i64,
    scale_factor: f64,
) -> Roi {
    let (x1, y1, x2, y2) = (rect[0], rect[1], rect[2], rect[3]);
    let xs = (x1 * width as f64, x2 * width as f64);
    let ys = (y1 * height as f64, y2 * height as f64);
    let roi = (
        ys.0.min(ys.1) as i64,
        ys.0.max(ys.1) as i64,
        xs.0.min(xs.1) as i64,
        xs.0.max(xs.1) as i64,
    );
    apply_margin_to_roi(roi, height, width, offset_px as f64 * scale_factor)
}

/// `get_manual_crop_coords`: the full frame inset by the offset.
#[must_use]
pub fn get_manual_crop_coords(
    height: usize,
    width: usize,
    offset_px: i64,
    scale_factor: f64,
) -> Roi {
    apply_margin_to_roi(
        (0, height as i64, 0, width as i64),
        height,
        width,
        offset_px as f64 * scale_factor,
    )
}

/// A crop of an image, clamped to its bounds.
#[must_use]
pub fn crop(img: &ImageBuffer, roi: Option<Roi>) -> ImageBuffer {
    let Some((y1, y2, x1, x2)) = roi else {
        return img.clone();
    };
    let (y1, y2) = (
        y1.max(0) as usize,
        y2.min(img.height as i64).max(0) as usize,
    );
    let (x1, x2) = (x1.max(0) as usize, x2.min(img.width as i64).max(0) as usize);
    if y2 <= y1 || x2 <= x1 {
        return img.clone();
    }
    let mut pixels = Vec::with_capacity((y2 - y1) * (x2 - x1) * 3);
    for y in y1..y2 {
        pixels.extend_from_slice(&img.pixels[(y * img.width + x1) * 3..(y * img.width + x2) * 3]);
    }
    ImageBuffer::from_rgb(x2 - x1, y2 - y1, pixels)
}

/// Whether the crop was drawn or adjusted by hand, so Auto must not overwrite it.
#[must_use]
pub fn has_manual_crop(geometry: &crate::domain::models::GeometryConfig) -> bool {
    geometry.crop_rect.is_some() && !geometry.crop_from_auto
}

/// Everything border detection reads, as one comparable string.
#[must_use]
pub fn autocrop_detection_key(geometry: &crate::domain::models::GeometryConfig) -> String {
    let mut parts = vec![
        geometry.rotation.to_string(),
        i64::from(geometry.flip_horizontal).to_string(),
        i64::from(geometry.flip_vertical).to_string(),
        format!("{:.4}", geometry.fine_rotation),
        format!("{:.4}", geometry.converge_v),
        format!("{:.4}", geometry.converge_h),
        geometry.autocrop_ratio.as_str().to_string(),
        geometry.autocrop_mode.as_str().to_string(),
        format!("{:.4}", geometry.autocrop_rebate_trim),
    ];
    if geometry.distortion_k1 != 0.0 {
        parts.push(format!("{:.4}", geometry.distortion_k1));
    }
    if geometry.autocrop_square {
        parts.push("square".to_string());
    }
    parts.join("|")
}

/// The auto-crop key plus the source aspect.
#[must_use]
pub fn meter_detection_key(
    geometry: &crate::domain::models::GeometryConfig,
    width: usize,
    height: usize,
) -> String {
    let aspect = if height == 0 {
        0.0
    } else {
        width as f64 / height as f64
    };
    format!("{}|{aspect:.2}", autocrop_detection_key(geometry))
}

/// Translate a normalized `(x1, y1, x2, y2)` rect, clamped inside the frame.
#[must_use]
pub fn translate_normalized_rect(rect: [f64; 4], dx: f64, dy: f64) -> [f64; 4] {
    let [x1, y1, x2, y2] = rect;
    let (width, height) = (x2 - x1, y2 - y1);
    let max_x1 = (1.0 - width).max(0.0);
    let max_y1 = (1.0 - height).max(0.0);
    let nx1 = (x1 + dx).clamp(0.0, max_x1);
    let ny1 = (y1 + dy).clamp(0.0, max_y1);
    [nx1, ny1, nx1 + width, ny1 + height]
}

/// Mirror a normalized rect across the vertical or horizontal center line.
#[must_use]
pub fn mirror_normalized_rect(rect: [f64; 4], horizontal: bool) -> [f64; 4] {
    let [x1, y1, x2, y2] = rect;
    if horizontal {
        [1.0 - x2, y1, 1.0 - x1, y2]
    } else {
        [x1, 1.0 - y2, x2, 1.0 - y1]
    }
}

/// Rotate a normalized rect by whole quarter-turns CCW.
#[must_use]
pub fn rotate_normalized_rect(rect: [f64; 4], quarter_turns_ccw: i64) -> [f64; 4] {
    let mut current = rect;
    for _ in 0..quarter_turns_ccw.rem_euclid(4) {
        let [x1, y1, x2, y2] = current;
        current = [y1, 1.0 - x2, y2, 1.0 - x1];
    }
    current
}

/// Rotate a frame's geometry and its analysis rect by quarter-turns CCW.
#[must_use]
pub fn rotate_geometry_and_analysis(
    geometry: &crate::domain::models::GeometryConfig,
    analysis_rect: Option<[f64; 4]>,
    direction: i64,
) -> (crate::domain::models::GeometryConfig, Option<[f64; 4]>) {
    let mut new_geometry = geometry.clone();
    new_geometry.rotation = (geometry.rotation + direction).rem_euclid(4);
    if geometry.converge_v != 0.0 || geometry.converge_h != 0.0 {
        let (mut converge_v, mut converge_h) = (geometry.converge_v, geometry.converge_h);
        for _ in 0..direction.rem_euclid(4) {
            (converge_v, converge_h) = (-converge_h, converge_v);
        }
        new_geometry.converge_v = converge_v;
        new_geometry.converge_h = converge_h;
    }
    new_geometry.crop_rect = geometry
        .crop_rect
        .as_ref()
        .filter(|rect| rect.len() == 4)
        .map(|rect| {
            rotate_normalized_rect([rect[0], rect[1], rect[2], rect[3]], direction).to_vec()
        });
    (
        new_geometry,
        analysis_rect.map(|rect| rotate_normalized_rect(rect, direction)),
    )
}

/// Toggle a mirror so the result is an exact mirror of the current render.
#[must_use]
pub fn toggle_flip(
    geometry: &crate::domain::models::GeometryConfig,
    horizontal: bool,
) -> crate::domain::models::GeometryConfig {
    let mut new_geometry = geometry.clone();
    if horizontal {
        new_geometry.flip_horizontal = !geometry.flip_horizontal;
        new_geometry.converge_h = -geometry.converge_h;
    } else {
        new_geometry.flip_vertical = !geometry.flip_vertical;
        new_geometry.converge_v = -geometry.converge_v;
    }
    if geometry.fine_rotation != 0.0 {
        new_geometry.fine_rotation = -geometry.fine_rotation;
    }
    new_geometry.crop_rect = geometry
        .crop_rect
        .as_ref()
        .filter(|rect| rect.len() == 4)
        .map(|rect| {
            mirror_normalized_rect([rect[0], rect[1], rect[2], rect[3]], horizontal).to_vec()
        });
    new_geometry
}

/// Mirror geometry and its analysis rect.
#[must_use]
pub fn flip_geometry_and_analysis(
    geometry: &crate::domain::models::GeometryConfig,
    analysis_rect: Option<[f64; 4]>,
    horizontal: bool,
) -> (crate::domain::models::GeometryConfig, Option<[f64; 4]>) {
    (
        toggle_flip(geometry, horizontal),
        analysis_rect.map(|rect| mirror_normalized_rect(rect, horizontal)),
    )
}

/// Fine-rotation delta that levels a line drawn on the displayed image.
#[must_use]
pub fn straighten_delta_degrees(dx: f64, dy: f64) -> f64 {
    let theta = dy.atan2(dx).to_degrees().rem_euclid(180.0);
    if theta <= 45.0 {
        theta
    } else if theta < 135.0 {
        theta - 90.0
    } else {
        theta - 180.0
    }
}

/// Fine-rotation angle for a drag of the crop tool's rotation handle.
#[must_use]
pub fn rotation_drag_angle(
    start_angle_deg: f64,
    center: (f64, f64),
    press: (f64, f64),
    cursor: (f64, f64),
    sensitivity: f64,
    limit: f64,
) -> f64 {
    let a0 = (press.1 - center.1).atan2(press.0 - center.0);
    let a1 = (cursor.1 - center.1).atan2(cursor.0 - center.0);
    let delta = ((a1 - a0).sin().atan2((a1 - a0).cos())).to_degrees();
    (start_angle_deg - delta * sensitivity).clamp(-limit, limit)
}
