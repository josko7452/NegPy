//! Grid reading and orientation from a synthetic card.

use negpy_backend::features::process::chart::{
    COLS, FitCancelled, ORIENTATIONS, ROWS, detect_orientation, fit_chart_matrix,
    fit_chart_matrix_with, grid_corners, measure_lab, patch_centers, sample_radius,
};
use negpy_backend::features::process::chart_ref::{
    COLORCHECKER_24, SPYDERCHECKR_24, delta_e2000, reference_lab,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

const CELL: usize = 20;
const GAP: usize = 6;

fn corners_px() -> [[f64; 2]; 4] {
    let pitch = (CELL + GAP) as f64;
    let c0 = GAP as f64 + CELL as f64 / 2.0;
    [
        [c0, c0],
        [c0 + 5.0 * pitch, c0],
        [c0 + 5.0 * pitch, c0 + 3.0 * pitch],
        [c0, c0 + 3.0 * pitch],
    ]
}

fn reference_rgb() -> Vec<[f32; 3]> {
    let lab = reference_lab(&COLORCHECKER_24);
    let flat: Vec<f32> = lab.iter().flat_map(|p| p.map(|v| v as f32)).collect();
    let mut rgb = vec![0.0f32; flat.len()];
    let (_, to_rgb) = negpy_backend::domain::color::working_lab();
    negpy_core::color::lab_to_rgb(&flat, &mut rgb, to_rgb);
    rgb.as_chunks::<3>()
        .0
        .iter()
        .map(|p| p.map(|v| v.clamp(1e-4, 1.0)))
        .collect()
}

fn raw_chart_image(rgb: &[[f32; 3]], order: &[usize; 24]) -> (usize, usize, Vec<f32>) {
    let pitch = CELL + GAP;
    let width = GAP + COLS * pitch;
    let height = GAP + ROWS * pitch;
    let mut image = vec![0.0f32; width * height * 3];
    for (cell, &patch) in order.iter().enumerate() {
        let r = cell / COLS;
        let c = cell % COLS;
        for y in GAP + r * pitch..GAP + r * pitch + CELL {
            for x in GAP + c * pitch..GAP + c * pitch + CELL {
                let at = (y * width + x) * 3;
                image[at..at + 3].copy_from_slice(&rgb[patch]);
            }
        }
    }
    (width, height, image)
}

/// A chart whose pixels are working-encoded, as the pipeline hands them to the fit.
fn chart_image(rgb: &[[f32; 3]], order: &[usize; 24]) -> ImageBuffer {
    let (width, height, linear) = raw_chart_image(rgb, order);
    let mut encoded = vec![0.0f32; linear.len()];
    negpy_core::color::oetf_encode(
        &linear,
        &mut encoded,
        negpy_backend::domain::color::WORKING_INV_GAMMA,
    );
    ImageBuffer::from_rgb(width, height, encoded)
}

#[test]
fn grid_corners_accept_any_permutation_and_a_sideways_grid() {
    let straight = [[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [0.0, 3.0]];
    let sideways = [[0.0, 0.0], [3.0, 0.0], [3.0, 5.0], [0.0, 5.0]];
    for perm in [[0, 1, 2, 3], [2, 0, 3, 1], [3, 2, 1, 0]] {
        let corners = [
            straight[perm[0]],
            straight[perm[1]],
            straight[perm[2]],
            straight[perm[3]],
        ];
        let centers = patch_centers(corners);
        for (index, center) in centers.iter().enumerate() {
            let expected = [(index % COLS) as f64, (index / COLS) as f64];
            assert!((center[0] - expected[0]).abs() < 1e-9);
            assert!((center[1] - expected[1]).abs() < 1e-9);
        }
    }
    let corners = [sideways[0], sideways[2], sideways[1], sideways[3]];
    let centers: std::collections::BTreeSet<(i64, i64)> = patch_centers(corners)
        .iter()
        .map(|p| (p[0].round() as i64, p[1].round() as i64))
        .collect();
    assert_eq!(centers.len(), 24);
    assert!(centers.contains(&(0, 0)) && centers.contains(&(3, 5)));
}

#[test]
fn patch_centers_span_the_corners() {
    let centers = patch_centers([[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [0.0, 3.0]]);
    assert_eq!(centers[0], [0.0, 0.0]);
    assert_eq!(centers[5], [5.0, 0.0]);
    assert_eq!(centers[23], [5.0, 3.0]);
    assert_eq!(centers[7], [1.0, 1.0]);
}

#[test]
fn the_sample_radius_uses_the_smaller_patch_pitch() {
    let radius = sample_radius([[0.0, 0.0], [60.0, 0.0], [60.0, 40.0], [0.0, 40.0]]);
    assert!((radius - 2.4).abs() < 1e-9, "{radius}");
}

#[test]
fn orientation_is_found_from_the_colors() {
    let rgb = reference_rgb();
    let reference = reference_lab(&COLORCHECKER_24);
    for (which, order) in ORIENTATIONS.iter().enumerate() {
        let image = chart_image(&rgb, order);
        assert_eq!(
            detect_orientation(&image, corners_px(), &COLORCHECKER_24),
            *order,
            "{which}"
        );
        let lab = measure_lab(&image, corners_px(), &COLORCHECKER_24, order);
        let error = delta_e2000(&reference, &lab);
        assert!(error.iter().sum::<f64>() / 24.0 < 1.0);
    }
}

#[test]
fn grid_corners_are_clockwise_from_a_long_side() {
    let corners = grid_corners([[5.0, 3.0], [0.0, 0.0], [0.0, 3.0], [5.0, 0.0]]);
    assert_eq!(corners, [[0.0, 0.0], [5.0, 0.0], [5.0, 3.0], [0.0, 3.0]]);
}

use negpy_backend::features::process::chart::matrix_from_offdiagonal;

#[test]
fn the_fit_undoes_a_known_density_mix() {
    let chart = &SPYDERCHECKR_24;
    let rgb: Vec<[f64; 3]> = reference_lab(chart)
        .iter()
        .map(|lab| {
            let flat = lab.map(|v| v as f32);
            let (_, to_rgb) = negpy_backend::domain::color::working_lab();
            let mut out = [0.0f32; 3];
            negpy_core::color::lab_to_rgb(&flat, &mut out, to_rgb);
            out.map(f64::from)
        })
        .collect();
    let density: Vec<[f64; 3]> = rgb.iter().map(|p| p.map(|v| -v.log10())).collect();
    let mix = [[0.9, 0.15, -0.05], [-0.1, 0.85, 0.25], [0.05, 0.2, 0.75]];
    let lab_for = |matrix: [f64; 9]| {
        let mut normalized = matrix;
        for row in 0..3 {
            let sum: f64 = normalized[row * 3..row * 3 + 3].iter().sum();
            for column in 0..3 {
                normalized[row * 3 + column] /= sum;
            }
        }
        let patch: Vec<[f64; 3]> = density
            .iter()
            .map(|p| {
                let mixed: [f64; 3] =
                    std::array::from_fn(|r| (0..3).map(|k| mix[r][k] * p[k]).sum());
                std::array::from_fn(|r| {
                    let value: f64 = (0..3).map(|k| normalized[r * 3 + k] * mixed[k]).sum();
                    10.0f64.powf(-value)
                })
            })
            .collect();
        let flat: Vec<f32> = patch.iter().flat_map(|p| p.map(|v| v as f32)).collect();
        let mut out = vec![0.0f32; flat.len()];
        let (space, _) = negpy_backend::domain::color::working_lab();
        negpy_core::color::rgb_to_lab(&flat, &mut out, space);
        Ok(out
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| p.map(f64::from))
            .collect())
    };
    let fit = fit_chart_matrix_with(lab_for, chart, None, &|| false).unwrap();
    let before = fit.before.iter().sum::<f64>() / 24.0;
    let after = fit.after.iter().sum::<f64>() / 24.0;
    assert!(before > 4.0, "{before}");
    assert!(after < 0.4 * before, "{before} -> {after}");
    let color_before = fit.before_color.iter().sum::<f64>() / 24.0;
    let color_after = fit.after_color.iter().sum::<f64>() / 24.0;
    assert!(
        color_after < 0.4 * color_before,
        "{color_before} -> {color_after}"
    );
    assert!(fit.matrix[0] == 1.0 && fit.matrix[4] == 1.0 && fit.matrix[8] == 1.0);
}

#[test]
fn a_cancelled_fit_stops() {
    let image = chart_image(&reference_rgb(), &ORIENTATIONS[0]);
    let result = fit_chart_matrix(
        |_| image.clone(),
        corners_px(),
        &SPYDERCHECKR_24,
        None,
        &|| true,
    );
    assert_eq!(result, Err(FitCancelled));
}

#[test]
fn the_matrix_parametrization_keeps_a_unit_diagonal() {
    let matrix = matrix_from_offdiagonal([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(matrix, [1.0, 1.0, 2.0, 3.0, 1.0, 4.0, 5.0, 6.0, 1.0]);
}
