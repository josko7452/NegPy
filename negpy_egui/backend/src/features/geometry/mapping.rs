//! Coordinate mappers that replay the geometry transforms.

use negpy_core::warp;

use crate::features::geometry::logic::{
    Roi, compute_distortion_scale, keystone_matrix, radial_center_public,
};

/// `map_point_radial`: the inverse of the resample map.
#[must_use]
pub fn map_point_radial(px: f64, py: f64, k1: f64, width: usize, height: usize) -> (f64, f64) {
    if k1.abs() < 1e-6 {
        return (px, py);
    }
    let (cx, cy, halfdiag) = radial_center_public(width, height);
    let (ix, iy) = (px - cx, py - cy);
    let r_in = (ix * ix + iy * iy).sqrt();
    if r_in < 1e-9 {
        return (px, py);
    }
    let c = k1 / (halfdiag * halfdiag);
    let roots = positive_cubic_roots(c, r_in);
    let t = roots
        .into_iter()
        .min_by(|a, b| (a - r_in).abs().total_cmp(&(b - r_in).abs()))
        .unwrap_or(r_in);
    let scale = (t / compute_distortion_scale(k1, width, height)) / r_in;
    (cx + ix * scale, cy + iy * scale)
}

/// Positive real roots of `c t^3 + t - r = 0`.
fn positive_cubic_roots(c: f64, r: f64) -> Vec<f64> {
    if c.abs() < 1e-18 {
        return vec![r];
    }
    let (p, q) = (1.0 / c, -r / c);
    let discriminant = (q / 2.0).powi(2) + (p / 3.0).powi(3);
    let mut roots = Vec::with_capacity(3);
    if discriminant > 0.0 {
        let sqrt = discriminant.sqrt();
        roots.push((-q / 2.0 + sqrt).cbrt() + (-q / 2.0 - sqrt).cbrt());
    } else {
        let radius = 2.0 * (-p / 3.0).sqrt();
        let arg = ((3.0 * q / (2.0 * p)) * (-3.0 / p).sqrt()).clamp(-1.0, 1.0);
        let theta = arg.acos() / 3.0;
        for k in 0..3 {
            roots.push(radius * (theta - 2.0 * std::f64::consts::PI * k as f64 / 3.0).cos());
        }
    }
    roots
        .into_iter()
        .filter(|t| t.is_finite() && *t > 0.0)
        .collect()
}

/// `map_point_keystone`.
#[must_use]
pub fn map_point_keystone(
    px: f64,
    py: f64,
    converge_v: f64,
    converge_h: f64,
    width: usize,
    height: usize,
) -> (f64, f64) {
    if converge_v.abs() < 1e-4 && converge_h.abs() < 1e-4 {
        return (px, py);
    }
    let m = keystone_matrix(converge_v, converge_h, width, height);
    let den = m[2][0] * px + m[2][1] * py + m[2][2];
    if den.abs() < 1e-12 {
        return (px, py);
    }
    (
        (m[0][0] * px + m[0][1] * py + m[0][2]) / den,
        (m[1][0] * px + m[1][1] * py + m[1][2]) / den,
    )
}

/// `_pre_keystone_point`.
#[must_use]
pub fn pre_keystone_point(
    nx: f64,
    ny: f64,
    orig_shape: (usize, usize),
    rotation_k: i64,
    fine_rotation: f64,
    flip_horizontal: bool,
    flip_vertical: bool,
    distortion_k1: f64,
) -> (f64, f64, usize, usize) {
    let (h_orig, w_orig) = orig_shape;
    let (mut px, mut py) = (nx * w_orig as f64, ny * h_orig as f64);
    let (mut h, mut w) = (h_orig, w_orig);
    match rotation_k.rem_euclid(4) {
        1 => {
            (px, py) = (py, w as f64 - px);
            (h, w) = (w, h);
        }
        2 => {
            px = w as f64 - px;
            py = h as f64 - py;
        }
        3 => {
            (px, py) = (h as f64 - py, px);
            (h, w) = (w, h);
        }
        _ => {}
    }
    if flip_horizontal {
        px = w as f64 - px;
    }
    if flip_vertical {
        py = h as f64 - py;
    }
    if fine_rotation != 0.0 {
        let m = warp::rotation_matrix((w as f32 / 2.0, h as f32 / 2.0), fine_rotation, 1.0);
        let (old_px, old_py) = (px, py);
        px = m[0] * old_px + m[1] * old_py + m[2];
        py = m[3] * old_px + m[4] * old_py + m[5];
    }
    if distortion_k1 != 0.0 {
        (px, py) = map_point_radial(px, py, distortion_k1, w, h);
    }
    (px, py, w, h)
}

/// `map_coords_to_geometry`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn map_coords_to_geometry(
    nx: f64,
    ny: f64,
    orig_shape: (usize, usize),
    rotation_k: i64,
    fine_rotation: f64,
    flip_horizontal: bool,
    flip_vertical: bool,
    roi: Option<Roi>,
    distortion_k1: f64,
    converge_v: f64,
    converge_h: f64,
) -> (f64, f64) {
    let (mut px, mut py, mut w, mut h) = pre_keystone_point(
        nx,
        ny,
        orig_shape,
        rotation_k,
        fine_rotation,
        flip_horizontal,
        flip_vertical,
        distortion_k1,
    );
    if converge_v != 0.0 || converge_h != 0.0 {
        (px, py) = map_point_keystone(px, py, converge_v, converge_h, w, h);
    }
    if let Some((y1, y2, x1, x2)) = roi {
        px -= x1 as f64;
        py -= y1 as f64;
        h = (y2 - y1).max(1) as usize;
        w = (x2 - x1).max(1) as usize;
    }
    (px / w.max(1) as f64, py / h.max(1) as f64)
}

/// `smooth_polyline`: a uniform Catmull-Rom curve through the points.
#[must_use]
pub fn smooth_polyline(
    points: &[(f64, f64)],
    closed: bool,
    samples_per_seg: usize,
) -> Vec<(f64, f64)> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(n * samples_per_seg);
    for i in 0..if closed { n } else { n - 1 } {
        let p0 = if closed {
            points[(i + n - 1) % n]
        } else {
            points[i.saturating_sub(1)]
        };
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = if closed {
            points[(i + 2) % n]
        } else {
            points[(i + 2).min(n - 1)]
        };
        for s in 0..samples_per_seg {
            let t = s as f64 / samples_per_seg as f64;
            let (t2, t3) = (t * t, t * t * t);
            let x = 0.5
                * (2.0 * p1.0
                    + (p2.0 - p0.0) * t
                    + (2.0 * p0.0 - 5.0 * p1.0 + 4.0 * p2.0 - p3.0) * t2
                    + (-p0.0 + 3.0 * p1.0 - 3.0 * p2.0 + p3.0) * t3);
            let y = 0.5
                * (2.0 * p1.1
                    + (p2.1 - p0.1) * t
                    + (2.0 * p0.1 - 5.0 * p1.1 + 4.0 * p2.1 - p3.1) * t2
                    + (-p0.1 + 3.0 * p1.1 - 3.0 * p2.1 + p3.1) * t3);
            out.push((x, y));
        }
    }
    if !closed {
        out.push(*points.last().expect("non-empty"));
    }
    out
}
