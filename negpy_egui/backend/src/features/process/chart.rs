//! Chart patch sampling and grid reading.

use crate::domain::color::{WORKING_GAMMA, working_lab};
use crate::features::process::chart_ref::{ChartSpec, delta_e2000, reference_lab};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const ROWS: usize = 4;
pub const COLS: usize = 6;
pub const SAMPLE_FRACTION: f64 = 0.4;
pub const FIT_KL: f64 = 2.0;
const EXPOSURE_WEIGHTS: [f32; 3] = [0.2973769, 0.6273491, 0.0752741];

/// The four ways a 6x4 grid can be read from its corners.
pub const ORIENTATIONS: [[usize; 24]; 4] = [
    [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    ],
    [
        5, 4, 3, 2, 1, 0, 11, 10, 9, 8, 7, 6, 17, 16, 15, 14, 13, 12, 23, 22, 21, 20, 19, 18,
    ],
    [
        18, 19, 20, 21, 22, 23, 12, 13, 14, 15, 16, 17, 6, 7, 8, 9, 10, 11, 0, 1, 2, 3, 4, 5,
    ],
    [
        23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    ],
];

/// The four corner-patch centers in clockwise order, starting on a long side.
#[must_use]
pub fn grid_corners(corners: [[f64; 2]; 4]) -> [[f64; 2]; 4] {
    let center = corners.iter().fold([0.0, 0.0], |sum, point| {
        [sum[0] + point[0], sum[1] + point[1]]
    });
    let center = [center[0] / 4.0, center[1] / 4.0];
    let mut c = corners;
    c.sort_by(|a, b| {
        let angle = |p: &[f64; 2]| (p[1] - center[1]).atan2(p[0] - center[0]);
        angle(a).total_cmp(&angle(b))
    });
    let d = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    if d(c[1], c[0]) < d(c[2], c[1]) {
        c.rotate_left(1);
    }
    c
}

/// The patch centers in pixels, from the four corner-patch centers, bilinear.
#[must_use]
pub fn patch_centers(corners_px: [[f64; 2]; 4]) -> Vec<[f64; 2]> {
    let [top_left, top_right, bottom_right, bottom_left] = grid_corners(corners_px);
    (0..ROWS)
        .flat_map(|row| {
            (0..COLS).map(move |column| {
                let u = column as f64 / (COLS - 1) as f64;
                let v = row as f64 / (ROWS - 1) as f64;
                let top = [
                    top_left[0] + (top_right[0] - top_left[0]) * u,
                    top_left[1] + (top_right[1] - top_left[1]) * u,
                ];
                let bottom = [
                    bottom_left[0] + (bottom_right[0] - bottom_left[0]) * u,
                    bottom_left[1] + (bottom_right[1] - bottom_left[1]) * u,
                ];
                [
                    top[0] + (bottom[0] - top[0]) * v,
                    top[1] + (bottom[1] - top[1]) * v,
                ]
            })
        })
        .collect()
}

/// The patch sample radius in pixels.
#[must_use]
pub fn sample_radius(corners_px: [[f64; 2]; 4]) -> f64 {
    let [top_left, top_right, bottom_right, bottom_left] = grid_corners(corners_px);
    let d = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    let pitch_x =
        (d(top_right, top_left) + d(bottom_right, bottom_left)) / (2.0 * (COLS - 1) as f64);
    let pitch_y =
        (d(bottom_left, top_left) + d(bottom_right, top_right)) / (2.0 * (ROWS - 1) as f64);
    0.5 * SAMPLE_FRACTION * pitch_x.min(pitch_y)
}

/// (24, 3) median of each patch box.
#[must_use]
pub fn sample_patches(img: &ImageBuffer, corners_px: [[f64; 2]; 4]) -> Vec<[f64; 3]> {
    let radius = sample_radius(corners_px).round().max(1.0) as usize;
    let boxes: Vec<(usize, usize, usize, usize)> = patch_centers(corners_px)
        .into_iter()
        .map(|[x, y]| {
            let cx = (x.round() as usize).min(img.width - 1);
            let cy = (y.round() as usize).min(img.height - 1);
            (
                cx.saturating_sub(radius),
                (cx + radius + 1).max(1).min(img.width),
                cy.saturating_sub(radius),
                (cy + radius + 1).max(1).min(img.height),
            )
        })
        .collect();
    negpy_core::process::chart_patches(&img.pixels, img.width, 3, &boxes, None)
}

/// (24, 3) patch medians decoded from the working encoding.
#[must_use]
pub fn measure_patches(img: &ImageBuffer, corners_px: [[f64; 2]; 4]) -> Vec<[f64; 3]> {
    let radius = sample_radius(corners_px).round().max(1.0) as usize;
    let boxes: Vec<(usize, usize, usize, usize)> = patch_centers(corners_px)
        .into_iter()
        .map(|[x, y]| {
            let cx = (x.round() as usize).min(img.width - 1);
            let cy = (y.round() as usize).min(img.height - 1);
            (
                cx.saturating_sub(radius),
                (cx + radius + 1).max(1).min(img.width),
                cy.saturating_sub(radius),
                (cy + radius + 1).max(1).min(img.height),
            )
        })
        .collect();
    negpy_core::process::chart_patches(
        &img.pixels,
        img.width,
        3,
        &boxes,
        Some(WORKING_GAMMA as f32),
    )
}

fn decode_patches(
    encoded: &ImageBuffer,
    corners_px: [[f64; 2]; 4],
    order: &[usize; 24],
) -> Vec<[f64; 3]> {
    let linear = measure_patches(encoded, corners_px);
    let mut rgb = vec![[0.0f64; 3]; 24];
    for (source, &slot) in linear.iter().zip(order.iter()) {
        rgb[slot] = *source;
    }
    rgb
}

/// The chart's patches in printed order, decoded and exposure-matched, as Lab.
#[must_use]
pub fn measure_lab(
    encoded: &ImageBuffer,
    corners_px: [[f64; 2]; 4],
    chart: &ChartSpec,
    order: &[usize; 24],
) -> Vec<[f64; 3]> {
    let rgb = decode_patches(encoded, corners_px, order);
    let reference = reference_lab(chart);
    let mut log_sum = 0.0;
    for &index in chart.exposure_patches {
        let l = reference[index][0];
        let y_ref = ((l + 16.0) / 116.0).powi(3);
        let y = (rgb[index][0] * f64::from(EXPOSURE_WEIGHTS[0])
            + rgb[index][1] * f64::from(EXPOSURE_WEIGHTS[1])
            + rgb[index][2] * f64::from(EXPOSURE_WEIGHTS[2]))
        .max(1e-6);
        log_sum += (y_ref / y).ln();
    }
    let gain = (log_sum / chart.exposure_patches.len() as f64).exp() as f32;
    let flat: Vec<f32> = rgb
        .iter()
        .flat_map(|p| p.map(|v| (v as f32) * gain))
        .collect();
    let mut lab = vec![0.0f32; flat.len()];
    let (space, _) = working_lab();
    negpy_core::color::rgb_to_lab(&flat, &mut lab, space);
    lab.as_chunks::<3>()
        .0
        .iter()
        .map(|p| p.map(f64::from))
        .collect()
}

/// The reading of the grid whose colors are nearest the chart, from the four candidate Lab sets.
#[must_use]
pub fn detect_orientation_from_lab(
    reference: &[[f64; 3]],
    lab_sets: &[Vec<[f64; 3]>; 4],
) -> [usize; 24] {
    let mut best = 0;
    let mut best_error = f64::INFINITY;
    for (index, lab) in lab_sets.iter().enumerate() {
        let error: f64 = delta_e2000(reference, lab).iter().sum();
        if error < best_error {
            best_error = error;
            best = index;
        }
    }
    ORIENTATIONS[best]
}

/// The reading of the grid whose colors are nearest the chart.
#[must_use]
pub fn detect_orientation(
    encoded: &ImageBuffer,
    corners_px: [[f64; 2]; 4],
    chart: &ChartSpec,
) -> [usize; 24] {
    let reference = reference_lab(chart);
    ORIENTATIONS
        .into_iter()
        .min_by(|a, b| {
            let error = |order: &[usize; 24]| {
                let lab = measure_lab(encoded, corners_px, chart, order);
                delta_e2000(&reference, &lab).iter().sum::<f64>()
            };
            error(a).total_cmp(&error(b))
        })
        .unwrap_or(ORIENTATIONS[0])
}

const RIDGE: f64 = 0.5;
const FD_STEP: f64 = 0.01;
const MAX_ITERATIONS: usize = 48;
const MIN_STEP: f64 = 2e-3;

/// A fitted crosstalk matrix and the errors it left.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartFit {
    /// Row-major 3x3 with a unit diagonal.
    pub matrix: [f64; 9],
    pub before: Vec<f64>,
    pub after: Vec<f64>,
    pub order: [usize; 24],
    pub before_color: Vec<f64>,
    pub after_color: Vec<f64>,
}

/// The fit stopped because the caller asked it to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitCancelled;

/// A row-major 3x3 with a unit diagonal and the six mixing terms.
#[must_use]
pub fn matrix_from_offdiagonal(p: [f64; 6]) -> [f64; 9] {
    let mut matrix = [0.0; 9];
    let mut terms = p.into_iter();
    for row in 0..3 {
        for column in 0..3 {
            matrix[row * 3 + column] = if row == column {
                1.0
            } else {
                terms.next().unwrap_or_default()
            };
        }
    }
    matrix
}

fn residual(reference: &[[f64; 3]], lab: &[[f64; 3]], p: [f64; 6]) -> Vec<f64> {
    let mut r = crate::features::process::chart_ref::delta_e2000_with_kl(reference, lab, FIT_KL);
    r.extend(p.iter().map(|value| RIDGE * value));
    r
}

fn solve(mut a: [[f64; 7]; 6]) -> Option<[f64; 6]> {
    for column in 0..6 {
        let pivot =
            (column..6).max_by(|&i, &j| a[i][column].abs().total_cmp(&a[j][column].abs()))?;
        if !a[pivot][column].is_finite() || a[pivot][column].abs() < 1e-18 {
            return None;
        }
        a.swap(column, pivot);
        let scale = a[column][column];
        for value in &mut a[column][column..] {
            *value /= scale;
        }
        for row in 0..6 {
            if row == column {
                continue;
            }
            let factor = a[row][column];
            for index in column..7 {
                a[row][index] -= factor * a[column][index];
            }
        }
    }
    Some(std::array::from_fn(|row| a[row][6]))
}

/// Levenberg-Marquardt over the six mixing terms, from identity.
///
/// `render(matrix)` returns the working-encoded frame for that matrix. The callback
/// runs the real pipeline because normalization re-meters every candidate.
pub fn fit_chart_matrix<F>(
    mut render: F,
    corners_px: [[f64; 2]; 4],
    chart: &ChartSpec,
    progress: Option<&mut dyn FnMut(f64)>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ChartFit, FitCancelled>
where
    F: FnMut([f64; 9]) -> ImageBuffer,
{
    let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let frame = render(identity);
    let order = detect_orientation(&frame, corners_px, chart);
    fit_with_order(
        |matrix| {
            let image = render(matrix);
            Ok(measure_lab(&image, corners_px, chart, &order))
        },
        chart,
        order,
        progress,
        cancelled,
    )
}

/// Fit from a callback that returns the chart's patch Lab for each matrix.
///
/// This seam exists so a caller can sample an analysis buffer directly, without
/// encoding a whole frame per trial.
pub fn fit_chart_matrix_with<F>(
    lab_for_matrix: F,
    chart: &ChartSpec,
    progress: Option<&mut dyn FnMut(f64)>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ChartFit, FitCancelled>
where
    F: FnMut([f64; 9]) -> Result<Vec<[f64; 3]>, FitCancelled>,
{
    fit_with_order(lab_for_matrix, chart, ORIENTATIONS[0], progress, cancelled)
}

fn fit_with_order<F>(
    mut lab_for_matrix: F,
    chart: &ChartSpec,
    order: [usize; 24],
    progress: Option<&mut dyn FnMut(f64)>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ChartFit, FitCancelled>
where
    F: FnMut([f64; 9]) -> Result<Vec<[f64; 3]>, FitCancelled>,
{
    let reference = reference_lab(chart);
    let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    if cancelled() {
        return Err(FitCancelled);
    }
    let initial = lab_for_matrix(identity)?;
    let mut progress = progress;
    let mut lab_for = |p: [f64; 6]| {
        if cancelled() {
            return Err(FitCancelled);
        }
        lab_for_matrix(matrix_from_offdiagonal(p))
    };
    let mut p = [0.0; 6];
    let lab0 = initial;
    let mut r = residual(&reference, &lab0, p);
    let mut mu = 1.0;
    for iteration in 0..MAX_ITERATIONS {
        let mut accepted = false;
        let mut converged = false;
        loop {
            let mut jac = vec![vec![0.0; 6]; r.len()];
            for j in 0..6 {
                let mut q = p;
                q[j] += FD_STEP;
                let lab = lab_for(q)?;
                let shifted = residual(&reference, &lab, q);
                for (row, value) in jac.iter_mut().enumerate() {
                    value[j] = (shifted[row] - r[row]) / FD_STEP;
                }
            }
            let mut normal = [[0.0; 7]; 6];
            for j in 0..6 {
                for k in 0..6 {
                    normal[j][k] = jac.iter().map(|row| row[j] * row[k]).sum();
                }
                normal[j][6] = -(jac
                    .iter()
                    .zip(&r)
                    .map(|(row, residual)| row[j] * residual)
                    .sum::<f64>());
            }
            let diagonal = std::array::from_fn::<f64, 6, _>(|i| normal[i][i]);
            let mut trials = 0;
            while mu < 1e6 {
                trials += 1;
                let mut system = normal;
                for (i, value) in diagonal.iter().enumerate() {
                    system[i][i] += mu * (value + 1e-9);
                }
                let Some(step) = solve(system) else {
                    mu *= 4.0;
                    continue;
                };
                let candidate: [f64; 6] = std::array::from_fn(|i| p[i] + step[i]);
                let lab = lab_for(candidate)?;
                let new_r = residual(&reference, &lab, candidate);
                let old_norm = r.iter().map(|v| v * v).sum::<f64>();
                let new_norm = new_r.iter().map(|v| v * v).sum::<f64>();
                if new_norm < old_norm {
                    p = candidate;
                    r = new_r;
                    mu *= 0.3;
                    accepted = true;
                    converged = step.iter().map(|v| v.abs()).fold(0.0, f64::max) < MIN_STEP;
                    break;
                }
                mu *= 4.0;
            }
            if accepted || trials == 0 || mu >= 1e6 {
                break;
            }
        }
        if let Some(callback) = progress.as_mut() {
            callback((iteration + 1) as f64 / MAX_ITERATIONS as f64);
        }
        if !accepted || converged || mu >= 1e6 {
            break;
        }
    }
    let lab1 = lab_for(p)?;
    Ok(ChartFit {
        matrix: matrix_from_offdiagonal(p),
        before: crate::features::process::chart_ref::delta_e2000_with_kl(&reference, &lab0, FIT_KL),
        after: crate::features::process::chart_ref::delta_e2000_with_kl(&reference, &lab1, FIT_KL),
        order,
        before_color: crate::features::process::chart_ref::delta_e_color(&reference, &lab0),
        after_color: crate::features::process::chart_ref::delta_e_color(&reference, &lab1),
    })
}
