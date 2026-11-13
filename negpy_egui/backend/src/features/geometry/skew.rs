//! Solve tilt and swing from four marked rebate edges.

use crate::features::geometry::mapping::{map_point_keystone, pre_keystone_point};

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
