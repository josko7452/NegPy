//! Solve tilt and swing from four marked rebate edges.

use crate::features::geometry::logic::FINE_ROTATION_LIMIT;
use crate::features::geometry::mapping::{map_point_keystone, pre_keystone_point};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

const KEYSTONE_SOLVE_MAX_ITER: usize = 20;
const KEYSTONE_SOLVE_TOL: f64 = 1e-7;
const KEYSTONE_RANGE: f64 = 15.0;

/// One marked edge, each endpoint normalized on the source image.
pub type Edge = ((f64, f64), (f64, f64));
type MappedEdge = ((f64, f64, usize, usize), (f64, f64, usize, usize));

/// The four named rebate edges a keystone solve needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeSet {
    pub left: Option<Edge>,
    pub right: Option<Edge>,
    pub top: Option<Edge>,
    pub bottom: Option<Edge>,
}

/// The geometry state a keystone solve maps through before its own correction.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeystoneSolveShape {
    pub orig_shape: (usize, usize),
    pub rotation_k: i64,
    pub fine_rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub distortion_k1: f64,
}

fn pre_point(shape: KeystoneSolveShape, point: (f64, f64)) -> (f64, f64, usize, usize) {
    pre_keystone_point(
        point.0,
        point.1,
        shape.orig_shape,
        shape.rotation_k,
        shape.fine_rotation,
        shape.flip_horizontal,
        shape.flip_vertical,
        shape.distortion_k1,
    )
}

fn slope_v(edge: MappedEdge, v: f64, h: f64) -> Option<f64> {
    let (first, second) = edge;
    let mapped1 = map_point_keystone(first.0, first.1, v, h, first.2, first.3);
    let mapped2 = map_point_keystone(second.0, second.1, v, h, second.2, second.3);
    let dy = mapped2.1 - mapped1.1;
    (dy.abs() >= 1e-9).then_some((mapped2.0 - mapped1.0) / dy)
}

fn slope_h(edge: MappedEdge, v: f64, h: f64) -> Option<f64> {
    let (first, second) = edge;
    let mapped1 = map_point_keystone(first.0, first.1, v, h, first.2, first.3);
    let mapped2 = map_point_keystone(second.0, second.1, v, h, second.2, second.3);
    let dx = mapped2.0 - mapped1.0;
    (dx.abs() >= 1e-9).then_some((mapped2.1 - mapped1.1) / dx)
}

fn residuals(
    left: MappedEdge,
    right: MappedEdge,
    top: MappedEdge,
    bottom: MappedEdge,
    v: f64,
    h: f64,
) -> Option<[f64; 2]> {
    Some([
        slope_v(left, v, h)? - slope_v(right, v, h)?,
        slope_h(top, v, h)? - slope_h(bottom, v, h)?,
    ])
}

/// Raise `ValueError` for an edge that is horizontal or vertical after correction.
pub const EDGE_HORIZONTAL: &str =
    "the edge is horizontal after correction; mark it along its length";
/// Raise `ValueError` for an edge that is horizontal or vertical after correction.
pub const EDGE_VERTICAL: &str = "the edge is vertical after correction; mark it along its length";
/// A solve needs all four marked rebate edges.
pub const EDGES_MISSING: &str =
    "solve_keystone_from_edges needs all four edges: left, right, top and bottom";

/// Solve tilt and swing so opposite marked rebate edges become parallel.
///
/// Returns an error when an edge lies along the axis its slope divides by.
pub fn solve_keystone_from_edges(
    edges: &EdgeSet,
    shape: KeystoneSolveShape,
    initial_converge_v: f64,
    initial_converge_h: f64,
) -> Result<(f64, f64), &'static str> {
    let (Some(left), Some(right), Some(top), Some(bottom)) =
        (edges.left, edges.right, edges.top, edges.bottom)
    else {
        return Err(EDGES_MISSING);
    };
    let left = (pre_point(shape, left.0), pre_point(shape, left.1));
    let right = (pre_point(shape, right.0), pre_point(shape, right.1));
    let top = (pre_point(shape, top.0), pre_point(shape, top.1));
    let bottom = (pre_point(shape, bottom.0), pre_point(shape, bottom.1));
    let mut v = initial_converge_v.clamp(-KEYSTONE_RANGE, KEYSTONE_RANGE);
    let mut h = initial_converge_h.clamp(-KEYSTONE_RANGE, KEYSTONE_RANGE);
    let epsilon = 1e-3;
    for _ in 0..KEYSTONE_SOLVE_MAX_ITER {
        let residual = residuals(left, right, top, bottom, v, h).ok_or(EDGE_HORIZONTAL)?;
        if residual[0].abs().max(residual[1].abs()) < KEYSTONE_SOLVE_TOL {
            break;
        }
        let dv = residuals(left, right, top, bottom, v + epsilon, h).ok_or(EDGE_HORIZONTAL)?;
        let dh = residuals(left, right, top, bottom, v, h + epsilon).ok_or(EDGE_VERTICAL)?;
        let jacobian = [
            (dv[0] - residual[0]) / epsilon,
            (dh[0] - residual[0]) / epsilon,
            (dv[1] - residual[1]) / epsilon,
            (dh[1] - residual[1]) / epsilon,
        ];
        let det = jacobian[0] * jacobian[3] - jacobian[1] * jacobian[2];
        let (delta_v, delta_h) = if det.abs() > 1e-12 {
            (
                (-residual[0] * jacobian[3] + residual[1] * jacobian[1]) / det,
                (-residual[1] * jacobian[0] + residual[0] * jacobian[2]) / det,
            )
        } else {
            least_squares(jacobian, residual)
        };
        v = (v + delta_v).clamp(-KEYSTONE_RANGE, KEYSTONE_RANGE);
        h = (h + delta_h).clamp(-KEYSTONE_RANGE, KEYSTONE_RANGE);
    }
    Ok((v, h))
}

fn least_squares(jacobian: [f64; 4], residual: [f64; 2]) -> (f64, f64) {
    let jjt = [
        jacobian[0] * jacobian[0] + jacobian[1] * jacobian[1],
        jacobian[0] * jacobian[2] + jacobian[1] * jacobian[3],
        jacobian[2] * jacobian[0] + jacobian[3] * jacobian[1],
        jacobian[2] * jacobian[2] + jacobian[3] * jacobian[3],
    ];
    let det = jjt[0] * jjt[3] - jjt[1] * jjt[2];
    if det.abs() < 1e-12 {
        return (0.0, 0.0);
    }
    let rhs = [
        -(jacobian[0] * residual[0] + jacobian[2] * residual[1]),
        -(jacobian[1] * residual[0] + jacobian[3] * residual[1]),
    ];
    (
        (rhs[0] * jjt[3] - rhs[1] * jjt[1]) / det,
        (rhs[1] * jjt[0] - rhs[0] * jjt[2]) / det,
    )
}

/// A fitted frame edge in detection-buffer pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeLine {
    pub p1: (f64, f64),
    pub p2: (f64, f64),
    pub horizontal: bool,
    pub source: &'static str,
    pub inlier_fraction: f64,
    pub rms: f64,
}

impl EdgeLine {
    #[must_use]
    pub fn length(&self) -> f64 {
        (self.p2.0 - self.p1.0).hypot(self.p2.1 - self.p1.1)
    }

    fn position(&self) -> f64 {
        if self.horizontal {
            0.5 * (self.p1.1 + self.p2.1)
        } else {
            0.5 * (self.p1.0 + self.p2.0)
        }
    }
}

/// A measured frame skew.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSkew {
    pub fine_rotation: f64,
    pub converge_v: Option<f64>,
    pub converge_h: Option<f64>,
    pub confidence: f64,
    pub lines: Vec<EdgeLine>,
    pub residual: f64,
    pub from_film_box: bool,
    pub has_opposite_pair: bool,
    pub at_canvas_edges: bool,
}

const MAX_SKEW_DEGREES: f64 = 10.0;
const MAX_KEYSTONE: f64 = 5.0;
const MIN_KEYSTONE: f64 = 0.3;
const KEYSTONE_GAIN: f64 = 2.5;
const KEYSTONE_MIN_RESIDUAL: f64 = 0.04;
const ANGLE_BIN: f64 = 0.05;
const MIN_SEGMENT: f64 = 0.04;
const MAX_LINE_RESIDUAL: f64 = 0.15;
const MIN_PAIR_SEPARATION: f64 = 0.3;
const SKEW_TRUSTED_CONFIDENCE: f64 = 0.5;
const AUTOCROP_DETECT_RES: usize = 1800;

fn fit_side_line(
    lum: &[f32],
    dims: (usize, usize),
    horizontal: bool,
    at: f64,
    start: f64,
    stop: f64,
    source: &'static str,
    outward: i32,
) -> Option<EdgeLine> {
    let line = negpy_core::skew::fit_side(lum, dims, horizontal, (at, start, stop), outward)?;
    Some(EdgeLine {
        p1: line.p1,
        p2: line.p2,
        horizontal: line.horizontal,
        source,
        inlier_fraction: line.inliers,
        rms: line.rms,
    })
}

fn off_border(lo: f64, hi: f64, extent: usize) -> bool {
    lo >= 2.0 && hi <= extent as f64 - 3.0
}

fn unrotate(line: EdgeLine, angle: f64, w: usize, h: usize) -> EdgeLine {
    let m = negpy_core::warp::rotation_matrix((w as f32 / 2.0, h as f32 / 2.0), angle, 1.0);
    let inv = invert_affine(m);
    let map = |p: (f64, f64)| {
        (
            inv[0] * p.0 + inv[1] * p.1 + inv[2],
            inv[3] * p.0 + inv[4] * p.1 + inv[5],
        )
    };
    EdgeLine {
        p1: map(line.p1),
        p2: map(line.p2),
        ..line
    }
}

fn invert_affine(m: [f64; 6]) -> [f64; 6] {
    let det = m[0] * m[4] - m[1] * m[3];
    if det.abs() < 1e-12 {
        return m;
    }
    let inv = 1.0 / det;
    let a = m[4] * inv;
    let b = -m[1] * inv;
    let d = -m[3] * inv;
    let e = m[0] * inv;
    [a, b, -(a * m[2] + b * m[5]), d, e, -(d * m[2] + e * m[5])]
}

fn frame_lines(lum: &[f32], dims: (usize, usize), rotated: &ImageBuffer) -> Vec<EdgeLine> {
    let (h, w) = dims;
    let detection =
        negpy_core::detect::detect_film_bounds(&rotated.pixels, rotated.height, rotated.width, 3);
    let mut boxes: Vec<(&'static str, (i64, i64, i64, i64))> = Vec::new();
    if let Some(consensus) = detection.consensus.as_ref() {
        boxes.push(("film", consensus.roi));
    }
    let outer = boxes
        .first()
        .map_or((0, h as i64, 0, w as i64), |(_, roi)| *roi);
    let refined = negpy_core::analysis::refine_roi_to_image(
        &rotated.pixels,
        rotated.height,
        rotated.width,
        3,
        outer,
    );
    if let Some((gate, _)) = refined
        && gate != outer
    {
        boxes.push(("gate", gate));
    }
    let mut lines = Vec::new();
    let mut seen: Vec<(bool, f64)> = Vec::new();
    for (source, (y1, y2, x1, x2)) in boxes {
        let rows = source != "film" || off_border(y1 as f64, y2 as f64, h);
        let cols = source != "film" || off_border(x1 as f64, x2 as f64, w);
        let candidates = [
            (true, y1 as f64, x1 as f64, x2 as f64, "top", -1, rows),
            (true, y2 as f64, x1 as f64, x2 as f64, "bottom", 1, rows),
            (false, x1 as f64, y1 as f64, y2 as f64, "left", -1, cols),
            (false, x2 as f64, y1 as f64, y2 as f64, "right", 1, cols),
        ];
        for (horizontal, at, start, stop, side, outward, fit) in candidates {
            if !fit
                || seen
                    .iter()
                    .any(|(o, p)| *o == horizontal && (*p - at).abs() < 4.0)
            {
                continue;
            }
            let label = if source == "film" {
                match side {
                    "top" => "film-top",
                    "bottom" => "film-bottom",
                    "left" => "film-left",
                    _ => "film-right",
                }
            } else {
                match side {
                    "top" => "gate-top",
                    "bottom" => "gate-bottom",
                    "left" => "gate-left",
                    _ => "gate-right",
                }
            };
            let Some(line) = fit_side_line(lum, dims, horizontal, at, start, stop, label, outward)
            else {
                continue;
            };
            let position = line.position();
            if seen
                .iter()
                .any(|(o, p)| *o == horizontal && (*p - position).abs() < 4.0)
            {
                continue;
            }
            lines.push(line);
            seen.push((horizontal, position));
        }
    }
    lines
}

fn profile_lines(lum: &[f32], dims: (usize, usize)) -> Vec<EdgeLine> {
    negpy_core::skew::profile_lines(lum, dims)
        .into_iter()
        .map(|line| EdgeLine {
            p1: line.p1,
            p2: line.p2,
            horizontal: line.horizontal,
            source: if line.horizontal {
                "profile-horizontal"
            } else {
                "profile-vertical"
            },
            inlier_fraction: line.inliers,
            rms: line.rms,
        })
        .collect()
}

fn pair_spread(lines: &[EdgeLine], horizontal: bool, extent: usize) -> f64 {
    let positions: Vec<f64> = lines
        .iter()
        .filter(|line| line.horizontal == horizontal)
        .map(EdgeLine::position)
        .collect();
    if positions.len() < 2 {
        return 0.0;
    }
    let lo = positions.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = positions.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (hi - lo) / extent as f64
}

fn at_canvas_edges(lines: &[EdgeLine], lum: &[f32], dims: (usize, usize)) -> bool {
    let placed: Vec<(bool, f64)> = lines
        .iter()
        .map(|line| (line.horizontal, line.position()))
        .collect();
    negpy_core::skew::at_canvas_edges(lum, dims, &placed)
}

fn map_points(
    points: &[(f64, f64)],
    angle: f64,
    cv: f64,
    ch: f64,
    w: usize,
    h: usize,
) -> Vec<(f64, f64)> {
    let m = negpy_core::warp::rotation_matrix((w as f32 / 2.0, h as f32 / 2.0), angle, 1.0);
    let rotated: Vec<(f64, f64)> = points
        .iter()
        .map(|(x, y)| (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]))
        .collect();
    if cv.abs() < 1e-6 && ch.abs() < 1e-6 {
        return rotated;
    }
    let k = crate::features::geometry::logic::keystone_matrix(cv, ch, w, h);
    rotated
        .iter()
        .map(|(x, y)| {
            let hx = k[0][0] * x + k[0][1] * y + k[0][2];
            let hy = k[1][0] * x + k[1][1] * y + k[1][2];
            let hz = k[2][0] * x + k[2][1] * y + k[2][2];
            (hx / hz, hy / hz)
        })
        .collect()
}

fn line_angles(lines: &[EdgeLine], params: [f64; 3], w: usize, h: usize) -> Vec<f64> {
    let points: Vec<(f64, f64)> = lines.iter().flat_map(|line| [line.p1, line.p2]).collect();
    let mapped = map_points(&points, params[0], params[1], params[2], w, h);
    mapped
        .chunks_exact(2)
        .zip(lines)
        .map(|(pair, line)| {
            let dx = pair[1].0 - pair[0].0;
            let dy = pair[1].1 - pair[0].1;
            if line.horizontal {
                dy.atan2(dx).to_degrees()
            } else {
                (-dx.atan2(dy)).to_degrees()
            }
        })
        .collect()
}

fn solve_small(matrix: &mut [[f64; 4]; 4], rhs: &mut [f64; 4], n: usize) -> Option<Vec<f64>> {
    for column in 0..n {
        let pivot = (column..n).max_by(|a, b| {
            matrix[*a][column]
                .abs()
                .total_cmp(&matrix[*b][column].abs())
        })?;
        if matrix[pivot][column].abs() < 1e-14 {
            return None;
        }
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        for row in column + 1..n {
            let factor = matrix[row][column] / matrix[column][column];
            for col in column..n {
                matrix[row][col] -= factor * matrix[column][col];
            }
            rhs[row] -= factor * rhs[column];
        }
    }
    let mut out = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|col| matrix[row][col] * out[col]).sum();
        out[row] = (rhs[row] - sum) / matrix[row][row];
    }
    Some(out)
}

fn solve(
    lines: &[EdgeLine],
    w: usize,
    h: usize,
    fit_v: bool,
    fit_h: bool,
) -> (f64, f64, f64, Vec<f64>) {
    let weights: Vec<f64> = lines
        .iter()
        .map(|line| (line.length() * line.inlier_fraction).sqrt())
        .collect();
    let mut params = [0.0; 3];
    let free: Vec<usize> = [Some(0), fit_v.then_some(1), fit_h.then_some(2)]
        .into_iter()
        .flatten()
        .collect();
    for _ in 0..12 {
        let angles = line_angles(lines, params, w, h);
        let residual: Vec<f64> = angles
            .iter()
            .zip(&weights)
            .map(|(angle, weight)| angle * weight)
            .collect();
        let n = free.len();
        let mut normal = [[0.0f64; 4]; 4];
        let mut rhs = [0.0f64; 4];
        for (index, weight) in weights.iter().enumerate() {
            let mut row = [0.0f64; 4];
            for (column, parameter) in free.iter().enumerate() {
                let mut step = [0.0f64; 3];
                step[*parameter] = 1e-3;
                let moved = line_angles(
                    lines,
                    [
                        params[0] + step[0],
                        params[1] + step[1],
                        params[2] + step[2],
                    ],
                    w,
                    h,
                );
                row[column] = (moved[index] * weight - residual[index]) / 1e-3;
            }
            for r in 0..n {
                for c in 0..n {
                    normal[r][c] += row[r] * row[c];
                }
                rhs[r] -= row[r] * residual[index];
            }
        }
        let Some(delta) = solve_small(&mut normal, &mut rhs, n) else {
            break;
        };
        for (column, parameter) in free.iter().enumerate() {
            params[*parameter] += delta[column];
        }
        params[0] = params[0].clamp(-FINE_ROTATION_LIMIT, FINE_ROTATION_LIMIT);
        params[1] = params[1].clamp(-2.0 * MAX_KEYSTONE, 2.0 * MAX_KEYSTONE);
        params[2] = params[2].clamp(-2.0 * MAX_KEYSTONE, 2.0 * MAX_KEYSTONE);
        if delta.iter().fold(0.0f64, |acc, value| acc.max(value.abs())) < 1e-6 {
            break;
        }
    }
    let residual = line_angles(lines, params, w, h);
    (params[0], params[1], params[2], residual)
}

fn normalize_detection(img: &ImageBuffer) -> ImageBuffer {
    let long = img.width.max(img.height);
    if long <= AUTOCROP_DETECT_RES {
        return img.clone();
    }
    let scale = AUTOCROP_DETECT_RES as f64 / long as f64;
    let dw = ((img.width as f64 * scale).round() as usize).max(1);
    let dh = ((img.height as f64 * scale).round() as usize).max(1);
    let Some(pixels) =
        negpy_core::resize::resize_area(&img.pixels, img.height, img.width, 3, dh, dw)
    else {
        return img.clone();
    };
    ImageBuffer::from_rgb(dw, dh, pixels)
}

fn detection_luma(img: &ImageBuffer) -> Vec<f32> {
    let mut lum = vec![0.0; img.width * img.height];
    negpy_core::analysis::luminance(&img.pixels, &mut lum);
    let mut out = vec![0.0; lum.len()];
    negpy_core::analysis::detection_luma(&lum, &mut out);
    out
}

fn measure_from(det: &ImageBuffer, coarse: f64, fit_keystone: bool) -> (Option<FrameSkew>, bool) {
    let (h, w) = (det.height, det.width);
    let rotated = if coarse.abs() > 1e-4 {
        crate::features::geometry::logic::apply_fine_rotation(det, coarse)
    } else {
        det.clone()
    };
    let rot_lum = detection_luma(&rotated);
    let mut found = frame_lines(&rot_lum, (h, w), &rotated);
    let boxed = !found.is_empty();
    if found.is_empty() {
        found = profile_lines(&rot_lum, (h, w));
    }
    let mut lines: Vec<EdgeLine> = found
        .into_iter()
        .map(|line| unrotate(line, coarse, w, h))
        .collect();
    if lines.is_empty() {
        return (None, false);
    }
    let mut fit_v;
    let mut fit_h;
    let (mut angle, mut cv, mut ch, mut residual);
    loop {
        fit_v = fit_keystone && pair_spread(&lines, false, w) >= MIN_PAIR_SEPARATION;
        fit_h = fit_keystone && pair_spread(&lines, true, h) >= MIN_PAIR_SEPARATION;
        let solved = solve(&lines, w, h, fit_v, fit_h);
        angle = solved.0;
        cv = solved.1;
        ch = solved.2;
        residual = solved.3;
        let worst = residual
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map_or(0, |(index, _)| index);
        if lines.len() <= 2 || residual[worst].abs() <= MAX_LINE_RESIDUAL {
            break;
        }
        lines.remove(worst);
    }
    if fit_v || fit_h {
        let (_, _, _, rotation_only) = solve(&lines, w, h, false, false);
        let rot_rms = (rotation_only.iter().map(|value| value * value).sum::<f64>()
            / rotation_only.len().max(1) as f64)
            .sqrt();
        loop {
            let full_rms = (residual.iter().map(|value| value * value).sum::<f64>()
                / residual.len().max(1) as f64)
                .sqrt();
            if rot_rms < KEYSTONE_MIN_RESIDUAL.max(KEYSTONE_GAIN * full_rms) {
                fit_v = false;
                fit_h = false;
            } else {
                let keep_v = fit_v && (MIN_KEYSTONE..=MAX_KEYSTONE).contains(&cv.abs());
                let keep_h = fit_h && (MIN_KEYSTONE..=MAX_KEYSTONE).contains(&ch.abs());
                if keep_v == fit_v && keep_h == fit_h {
                    break;
                }
                fit_v = keep_v;
                fit_h = keep_h;
            }
            let solved = solve(&lines, w, h, fit_v, fit_h);
            angle = solved.0;
            cv = solved.1;
            ch = solved.2;
            residual = solved.3;
        }
    }
    if angle.abs() > MAX_SKEW_DEGREES {
        return (None, false);
    }
    let rms = (residual.iter().map(|value| value * value).sum::<f64>()
        / residual.len().max(1) as f64)
        .sqrt();
    let support =
        (lines.iter().map(EdgeLine::length).sum::<f64>() / (2.0 * (w + h) as f64)).min(1.0);
    let agreement = (1.0 - rms / MAX_LINE_RESIDUAL).clamp(0.0, 1.0);
    let count = (lines.len() as f64 / 4.0).min(1.0);
    let confidence = (0.45 * support + 0.35 * agreement + 0.20 * count).clamp(0.0, 1.0);
    (
        Some(FrameSkew {
            fine_rotation: angle,
            converge_v: fit_v.then_some(cv),
            converge_h: fit_h.then_some(ch),
            confidence,
            lines: lines.clone(),
            residual: rms,
            from_film_box: boxed,
            has_opposite_pair: pair_spread(&lines, true, h) >= MIN_PAIR_SEPARATION
                || pair_spread(&lines, false, w) >= MIN_PAIR_SEPARATION,
            at_canvas_edges: !boxed && at_canvas_edges(&lines, &detection_luma(det), (h, w)),
        }),
        boxed,
    )
}

/// The rotation and keystone that square the frame, or `None` when no edge is found.
#[must_use]
pub fn measure_frame_skew(img: &ImageBuffer, fit_keystone: bool) -> Option<FrameSkew> {
    let det = normalize_detection(img);
    let lum = detection_luma(&det);
    let modes = negpy_core::skew::coarse_angles(
        &lum,
        (det.height, det.width),
        MIN_SEGMENT,
        MAX_SKEW_DEGREES,
        ANGLE_BIN,
    );
    let mut starts: Vec<f64> = Vec::new();
    let mut candidates = Vec::new();
    if let Some(first) = modes.first() {
        candidates.push(*first);
    }
    candidates.push(0.0);
    candidates.extend(modes.iter().skip(1).copied());
    for angle in candidates {
        if starts.iter().all(|other| (angle - other).abs() > 0.5) {
            starts.push(angle);
        }
    }
    let mut best: Option<(bool, f64)> = None;
    let mut result = None;
    for start in starts {
        let (skew, boxed) = measure_from(&det, start, fit_keystone);
        let Some(skew) = skew else {
            continue;
        };
        let rank = (boxed, skew.confidence);
        let confident = skew.confidence >= 0.8;
        if best.is_none_or(|current| rank > current) {
            best = Some(rank);
            result = Some(skew);
        }
        if boxed && confident {
            break;
        }
    }
    result
}

/// [`measure_frame_skew`] with the structural gates Auto Skew needs.
#[must_use]
pub fn trusted_frame_skew(img: &ImageBuffer, fit_keystone: bool) -> Option<FrameSkew> {
    let skew = measure_frame_skew(img, fit_keystone)?;
    if !skew.has_opposite_pair {
        return None;
    }
    if !skew.from_film_box && !skew.at_canvas_edges {
        return None;
    }
    (skew.confidence >= SKEW_TRUSTED_CONFIDENCE).then_some(skew)
}
