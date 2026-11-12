//! Ports of `negpy/features/local/logic.py` onto the `negpy_core` mask kernels.

use negpy_core::shapes::{self, LocalMask as CoreMask, MaskAlpha};

use crate::domain::models::local::MAX_KEYED_MASKS;
use crate::domain::models::{LocalAdjustmentsConfig, LocalMask, MaskKey, MaskShape};
use crate::features::geometry::mapping::{map_coords_to_geometry, smooth_polyline};

pub const OVAL_SAMPLES: usize = 64;
pub const OVERLAP_GRID: usize = 96;
pub const OVERLAP_ALPHA: f32 = 0.25;

/// The minimum vertices a shape needs.
#[must_use]
pub fn min_points(shape: MaskShape) -> usize {
    if shape == MaskShape::Gradient { 2 } else { 3 }
}

/// `outline_points`: the closed outline in the same space as the control points.
#[must_use]
pub fn outline_points(shape: MaskShape, points: &[[f64; 2]]) -> Vec<(f64, f64)> {
    match shape {
        MaskShape::Gradient => Vec::new(),
        MaskShape::Oval => {
            let (center, px, qx) = (points[0], points[1], points[2]);
            let (ux, uy) = (px[0] - center[0], px[1] - center[1]);
            let (vx, vy) = (qx[0] - center[0], qx[1] - center[1]);
            (0..OVAL_SAMPLES)
                .map(|i| {
                    let t = 2.0 * std::f64::consts::PI * i as f64 / OVAL_SAMPLES as f64;
                    (
                        center[0] + ux * t.cos() + vx * t.sin(),
                        center[1] + uy * t.cos() + vy * t.sin(),
                    )
                })
                .collect()
        }
        MaskShape::Polygon => smooth_polyline(
            &points.iter().map(|p| (p[0], p[1])).collect::<Vec<_>>(),
            true,
            16,
        ),
    }
}

/// `_mask_spec`: the pixel-space alpha definition.
#[must_use]
pub fn mask_alpha(
    shape: MaskShape,
    points: &[[f64; 2]],
    height: usize,
    width: usize,
    feather_sigma: f64,
) -> MaskAlpha {
    if shape == MaskShape::Gradient {
        let (a, b) = (points[0], points[1]);
        let (ax, ay) = (a[0] * width as f64, a[1] * height as f64);
        let (dx, dy) = (b[0] * width as f64 - ax, b[1] * height as f64 - ay);
        let denom = dx * dx + dy * dy;
        if denom <= 1e-9 {
            return MaskAlpha::Empty;
        }
        return MaskAlpha::Ramp {
            ax: ax as f32,
            ay: ay as f32,
            kx: (dx / denom) as f32,
            ky: (dy / denom) as f32,
        };
    }
    let outline = outline_points(shape, points)
        .into_iter()
        .map(|(x, y)| ((x * width as f64) as i64, (y * height as f64) as i64))
        .collect();
    MaskAlpha::Fill {
        outline,
        sigma: feather_sigma,
    }
}

/// `rasterise`: the control points, normalized, to a float32 alpha plane.
#[must_use]
pub fn rasterise(
    shape: MaskShape,
    points: &[[f64; 2]],
    height: usize,
    width: usize,
    feather_sigma: f64,
    invert: bool,
) -> Vec<f32> {
    let mut alpha = vec![0.0; height * width];
    shapes::rasterise(
        &mask_alpha(shape, points, height, width, feather_sigma),
        (height, width),
        invert,
        &mut alpha,
    )
    .expect("the mask covers the frame");
    alpha
}

fn tinted_area(mask: &LocalMask, grid: usize) -> Option<Vec<f32>> {
    if mask.vertices.len() < min_points(mask.shape) {
        return None;
    }
    Some(rasterise(
        mask.shape,
        &mask.vertices,
        grid,
        grid,
        mask.feather * grid as f64,
        mask.invert,
    ))
}

/// `overlapping_masks`: the indices whose tinted area meets mask `index`.
#[must_use]
pub fn overlapping_masks(config: &LocalAdjustmentsConfig, index: usize) -> Vec<usize> {
    if index >= config.masks.len() {
        return Vec::new();
    }
    let Some(target) = tinted_area(&config.masks[index], OVERLAP_GRID) else {
        return Vec::new();
    };
    config
        .masks
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .filter_map(|(i, mask)| {
            let other = tinted_area(mask, OVERLAP_GRID)?;
            shapes::alphas_overlap(&target, &other, OVERLAP_ALPHA).then_some(i)
        })
        .collect()
}

/// The geometry values the map builder replays.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalGeometry {
    pub rotation: i64,
    pub fine_rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub distortion_k1: f64,
    pub converge_v: f64,
    pub converge_h: f64,
}

/// `limited_indices`: the tone-limited masks in plane order.
#[must_use]
pub fn limited_indices(config: &LocalAdjustmentsConfig) -> Vec<usize> {
    config
        .masks
        .iter()
        .enumerate()
        .filter(|(_, mask)| {
            mask.enabled
                && mask.key != MaskKey::Off
                && mask.vertices.len() >= min_points(mask.shape)
        })
        .map(|(i, _)| i)
        .take(MAX_KEYED_MASKS as usize)
        .collect()
}

/// `limited_masks`: the masks behind planes 3 onward.
#[must_use]
pub fn limited_masks(config: &LocalAdjustmentsConfig) -> Vec<&LocalMask> {
    limited_indices(config)
        .into_iter()
        .map(|i| &config.masks[i])
        .collect()
}

/// `compute_local_maps`: HxWx(3 + K) planes, in mask order.
#[must_use]
pub fn compute_local_maps(
    config: &LocalAdjustmentsConfig,
    height: usize,
    width: usize,
    orig_shape: (usize, usize),
    geometry: LocalGeometry,
) -> Vec<f32> {
    let limited = limited_indices(config);
    let planes = 3 + limited.len();
    let mut maps = vec![0.0; height * width * planes];
    if config.masks.is_empty() {
        return maps;
    }
    let short_side = height.min(width) as f64;
    let mut specs = Vec::new();
    for (i, mask) in config.masks.iter().enumerate() {
        if !mask.enabled || mask.vertices.len() < min_points(mask.shape) {
            continue;
        }
        let transformed: Vec<[f64; 2]> = mask
            .vertices
            .iter()
            .map(|point| {
                let (x, y) = map_coords_to_geometry(
                    point[0],
                    point[1],
                    orig_shape,
                    geometry.rotation,
                    geometry.fine_rotation,
                    geometry.flip_horizontal,
                    geometry.flip_vertical,
                    None,
                    geometry.distortion_k1,
                    geometry.converge_v,
                    geometry.converge_h,
                );
                [x, y]
            })
            .collect();
        specs.push(CoreMask {
            alpha: mask_alpha(
                mask.shape,
                &transformed,
                height,
                width,
                mask.feather * short_side,
            ),
            invert: mask.invert,
            plane: limited.iter().position(|&index| index == i),
            stops: mask.stops as f32,
            grade: mask.grade as f32,
            flash: mask.flash as f32,
        });
    }
    shapes::local_maps(&specs, (height, width), planes, &mut maps)
        .expect("the masks cover the frame");
    maps
}
