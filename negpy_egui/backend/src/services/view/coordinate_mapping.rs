//! Mirrors `negpy/services/view/coordinate_mapping.py`: viewport to raw and back.

use negpy_core::warp::{self, Border, Image, Interp};

use crate::features::geometry::logic::{
    compute_distortion_scale, keystone_matrix, radial_center_public,
};

/// A viewport-to-raw grid, `(u, v)` per output pixel, row major.
#[derive(Clone, Debug, PartialEq)]
pub struct UvGrid {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<f32>,
}

impl UvGrid {
    /// The `(u, v)` at pixel `(x, y)`.
    #[must_use]
    pub fn at(&self, x: usize, y: usize) -> (f64, f64) {
        let index = (y * self.width + x) * 2;
        (self.samples[index] as f64, self.samples[index + 1] as f64)
    }
}

/// The geometric state one grid replays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridState {
    pub rotation: i64,
    pub fine_rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub distortion_k1: f64,
    pub converge_v: f64,
    pub converge_h: f64,
    /// The autocrop ROI in `(y1, y2, x1, x2)`, when the frame is cropped.
    pub autocrop: Option<(usize, usize, usize, usize)>,
}

impl Default for GridState {
    fn default() -> Self {
        Self {
            rotation: 0,
            fine_rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
            distortion_k1: 0.0,
            converge_v: 0.0,
            converge_h: 0.0,
            autocrop: None,
        }
    }
}

const GEOMETRY_EXIF: [u8; 16] = [1, 4, 2, 3, 8, 5, 7, 6, 3, 2, 4, 1, 6, 7, 5, 8];

fn orientation_code(rotation: i64, flip_h: bool, flip_v: bool) -> u8 {
    let index = (rotation.rem_euclid(4) * 4 + i64::from(flip_h) * 2 + i64::from(flip_v)) as usize;
    GEOMETRY_EXIF[index]
}

/// `apply_geometry_orientation` on a two-channel float grid.
fn orient_grid(samples: &[f32], height: usize, width: usize, code: u8) -> UvGrid {
    let (oh, ow) = negpy_core::orient::oriented_dims(height, width, code);
    let src: Vec<u8> = samples
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect();
    let mut dst = vec![0u8; oh * ow * 8];
    negpy_core::orient::orient(&src, height, width, 8, code, &mut dst);
    let samples = dst
        .chunks_exact(4)
        .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    UvGrid {
        width: ow,
        height: oh,
        samples,
    }
}

fn invert_affine(m: [f64; 6]) -> [f64; 6] {
    let d = m[0] * m[4] - m[1] * m[3];
    let d = if d != 0.0 { 1.0 / d } else { 0.0 };
    let (a11, a22, a12, a21) = (m[4] * d, m[0] * d, -m[1] * d, -m[3] * d);
    [
        a11,
        a12,
        -a11 * m[2] - a12 * m[5],
        a21,
        a22,
        -a21 * m[2] - a22 * m[5],
    ]
}

/// `create_uv_grid`: output pixel to the raw uv it samples, with every transform applied.
#[must_use]
pub fn create_uv_grid(height: usize, width: usize, state: GridState) -> UvGrid {
    let mut samples = Vec::with_capacity(height * width * 2);
    let step_u = 1.0 / (width.max(2) - 1) as f64;
    let step_v = 1.0 / (height.max(2) - 1) as f64;
    for y in 0..height {
        let v = if y + 1 == height {
            1.0
        } else {
            y as f64 * step_v
        };
        for x in 0..width {
            let u = if x + 1 == width {
                1.0
            } else {
                x as f64 * step_u
            };
            samples.push(u as f32);
            samples.push(v as f32);
        }
    }
    let code = orientation_code(state.rotation, state.flip_horizontal, state.flip_vertical);
    let mut grid = if code == 1 {
        UvGrid {
            width,
            height,
            samples,
        }
    } else {
        orient_grid(&samples, height, width, code)
    };

    if state.fine_rotation != 0.0 {
        let (h, w) = (grid.height, grid.width);
        let m = invert_affine(warp::rotation_matrix(
            (w as f32 / 2.0, h as f32 / 2.0),
            state.fine_rotation,
            1.0,
        ));
        let o = [grid.samples[0] as f64, grid.samples[1] as f64];
        let last_x = ((h * w - 1) * 2) as usize;
        let first_below = (w - 1) * 2;
        let du = [
            (grid.samples[first_below] as f64 - o[0]) / (w.max(2) - 1) as f64,
            (grid.samples[first_below + 1] as f64 - o[1]) / (w.max(2) - 1) as f64,
        ];
        let dv = [
            (grid.samples[last_x] as f64 - o[0]) / (h.max(2) - 1) as f64,
            (grid.samples[last_x + 1] as f64 - o[1]) / (h.max(2) - 1) as f64,
        ];
        let mut out = vec![0.0; grid.samples.len()];
        warp::affine_uv_grid(m, o, du, dv, w, &mut out);
        grid.samples = out;
    }

    if state.distortion_k1 != 0.0 {
        let (cx, cy, halfdiag) = radial_center_public(grid.width, grid.height);
        let scale = compute_distortion_scale(state.distortion_k1, grid.width, grid.height);
        let source = Image {
            data: &grid.samples,
            h: grid.height,
            w: grid.width,
            cn: 2,
        };
        let mut out = vec![0.0; grid.samples.len()];
        warp::remap_radial(
            &source,
            (cx as f32, cy as f32),
            scale as f32,
            state.distortion_k1 as f32,
            (1.0 / (halfdiag * halfdiag)) as f32,
            &mut out,
        );
        grid.samples = out;
    }

    if state.converge_v != 0.0 || state.converge_h != 0.0 {
        let m = keystone_matrix(state.converge_v, state.converge_h, grid.width, grid.height);
        let m = [
            m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
        ];
        let source = Image {
            data: &grid.samples,
            h: grid.height,
            w: grid.width,
            cn: 2,
        };
        let mut out = vec![0.0; grid.samples.len()];
        warp::warp_perspective(
            &source,
            (Interp::Linear, Border::Replicate),
            m,
            false,
            (grid.width, grid.height),
            &mut out,
        );
        grid.samples = out;
    }

    if let Some((y1, y2, x1, x2)) = state.autocrop {
        let (y1, y2) = (y1.min(grid.height), y2.min(grid.height));
        let (x1, x2) = (x1.min(grid.width), x2.min(grid.width));
        if y2 > y1 && x2 > x1 {
            let mut out = Vec::with_capacity((y2 - y1) * (x2 - x1) * 2);
            for y in y1..y2 {
                let start = (y * grid.width + x1) * 2;
                out.extend_from_slice(&grid.samples[start..start + (x2 - x1) * 2]);
            }
            grid = UvGrid {
                width: x2 - x1,
                height: y2 - y1,
                samples: out,
            };
        }
    }
    grid
}

fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if d == 0.0 {
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

fn grid_homography(grid: &UvGrid) -> Option<[[f64; 3]; 3]> {
    let (x0, x1) = (grid.width / 4, grid.width - 1 - grid.width / 4);
    let (y0, y1) = (grid.height / 4, grid.height - 1 - grid.height / 4);
    let (w, h) = (grid.width as f32, grid.height as f32);
    let src = [
        [x0 as f32 / (w - 1.0), y0 as f32 / (h - 1.0)],
        [x1 as f32 / (w - 1.0), y0 as f32 / (h - 1.0)],
        [x1 as f32 / (w - 1.0), y1 as f32 / (h - 1.0)],
        [x0 as f32 / (w - 1.0), y1 as f32 / (h - 1.0)],
    ];
    let dst = [
        grid.at(x0, y0).into(),
        grid.at(x1, y0).into(),
        grid.at(x1, y1).into(),
        grid.at(x0, y1).into(),
    ];
    let dst = dst.map(|(u, v)| [u as f32, v as f32]);
    let m = warp::perspective_transform(&src, &dst)?;
    Some([[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]])
}

fn apply_homography(m: [[f64; 3]; 3], nx: f64, ny: f64) -> (f64, f64) {
    let den = m[2][0] * nx + m[2][1] * ny + m[2][2];
    if den.abs() < 1e-12 {
        return (nx, ny);
    }
    (
        (m[0][0] * nx + m[0][1] * ny + m[0][2]) / den,
        (m[1][0] * nx + m[1][1] * ny + m[1][2]) / den,
    )
}

/// `map_click_to_raw`: viewport `(0..1)` to raw `(0..1)`, continuing off the frame.
#[must_use]
pub fn map_click_to_raw(nx: f64, ny: f64, grid: &UvGrid) -> (f64, f64) {
    if (0.0..=1.0).contains(&nx) && (0.0..=1.0).contains(&ny) {
        let x = (nx * (grid.width - 1) as f64) as usize;
        let y = (ny * (grid.height - 1) as f64) as usize;
        return grid.at(x, y);
    }
    grid_homography(grid).map_or((nx, ny), |m| apply_homography(m, nx, ny))
}

/// `map_raw_to_viewport`: raw `(0..1)` to viewport `(0..1)`, the inverse of the click map.
#[must_use]
pub fn map_raw_to_viewport(rx: f64, ry: f64, grid: &UvGrid, buckets: usize) -> (f64, f64) {
    let step = (grid.height / buckets.max(1)).max(1);
    let mut best = f64::INFINITY;
    let mut best_xy = (0, 0);
    for y in (0..grid.height).step_by(step) {
        for x in (0..grid.width).step_by(step) {
            let (u, v) = grid.at(x, y);
            let dist = (u - rx).powi(2) + (v - ry).powi(2);
            if dist < best {
                best = dist;
                best_xy = (x, y);
            }
        }
    }
    let (px, py) = best_xy;
    let y0 = py.saturating_sub(step);
    let y1 = (py + step + 1).min(grid.height);
    let x0 = px.saturating_sub(step);
    let x1 = (px + step + 1).min(grid.width);
    let mut window_best = f64::INFINITY;
    let mut window_xy = (x0, y0);
    for y in y0..y1 {
        for x in x0..x1 {
            let (u, v) = grid.at(x, y);
            let dist = (u - rx).powi(2) + (v - ry).powi(2);
            if dist < window_best {
                window_best = dist;
                window_xy = (x, y);
            }
        }
    }
    let (wx, wy) = window_xy;
    let nx = ((wx as f64 + 0.5) / grid.width as f64).min(1.0);
    let ny = ((wy as f64 + 0.5) / grid.height as f64).min(1.0);
    if window_best > (2.0 / grid.height.max(grid.width) as f64).powi(2) {
        return grid_homography(grid)
            .and_then(invert3)
            .map_or((nx, ny), |m| apply_homography(m, rx, ry));
    }
    (nx, ny)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rotation: i64, fine_rotation: f64, flip_horizontal: bool) -> UvGrid {
        create_uv_grid(
            240,
            320,
            GridState {
                rotation,
                fine_rotation,
                flip_horizontal,
                ..GridState::default()
            },
        )
    }

    fn close(actual: (f64, f64), expected: (f64, f64), tolerance: f64) -> bool {
        (actual.0 - expected.0).abs() <= tolerance && (actual.1 - expected.1).abs() <= tolerance
    }

    #[test]
    fn an_inside_point_is_unchanged() {
        let grid = grid(0, 0.0, false);
        assert!(close(
            map_click_to_raw(0.25, 0.75, &grid),
            (0.25, 0.75),
            0.01
        ));
    }

    #[test]
    fn an_outside_point_keeps_its_distance() {
        let grid = grid(0, 0.0, false);
        assert!(close(map_click_to_raw(1.5, 0.5, &grid), (1.5, 0.5), 0.01));
        assert!(close(map_click_to_raw(-0.4, 0.5, &grid), (-0.4, 0.5), 0.01));
    }

    #[test]
    fn the_boundary_has_no_step() {
        let grid = grid(1, 0.0, false);
        let inside = map_click_to_raw(0.999, 0.4, &grid);
        let outside = map_click_to_raw(1.001, 0.4, &grid);
        assert!((outside.0 - inside.0).abs() < 0.01);
        assert!((outside.1 - inside.1).abs() < 0.01);
    }

    #[test]
    fn off_frame_points_round_trip() {
        for state in [
            (0, 0.0, false),
            (1, 0.0, false),
            (0, 0.0, true),
            (2, 6.0, false),
        ] {
            let grid = grid(state.0, state.1, state.2);
            for point in [(1.4, 0.5), (-0.3, 0.25), (0.5, -0.2), (1.2, 1.3)] {
                let raw = map_click_to_raw(point.0, point.1, &grid);
                assert!(
                    close(map_raw_to_viewport(raw.0, raw.1, &grid, 100), point, 0.02),
                    "{state:?} {point:?} mapped to {raw:?}"
                );
            }
        }
    }

    #[test]
    fn inside_points_round_trip() {
        for state in [
            (0, 0.0, false),
            (1, 0.0, false),
            (0, 0.0, true),
            (2, 6.0, false),
        ] {
            let grid = grid(state.0, state.1, state.2);
            let raw = map_click_to_raw(0.4, 0.6, &grid);
            assert!(close(
                map_raw_to_viewport(raw.0, raw.1, &grid, 100),
                (0.4, 0.6),
                0.02
            ));
        }
    }

    #[test]
    fn a_fine_rotation_wedge_maps_off_frame_not_to_the_corner() {
        let grid = grid(0, 6.0, false);
        let (rx, ry) = map_click_to_raw(0.005, 0.005, &grid);
        assert!(!close((rx, ry), (0.0, 0.0), 1e-3));
        assert!(rx < 0.05 || ry < 0.0 || rx < 0.0);
    }
}
