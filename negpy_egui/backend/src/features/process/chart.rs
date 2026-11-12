//! Chart patch sampling over the shared core median.

use crate::infrastructure::loaders::image_buffer::ImageBuffer;

pub const ROWS: usize = 4;
pub const COLS: usize = 6;
pub const SAMPLE_FRACTION: f64 = 0.4;

/// The patch centers in pixels from the four detected card corners.
#[must_use]
pub fn patch_centers(corners_px: [[f64; 2]; 4]) -> Vec<[f64; 2]> {
    let [top_left, top_right, bottom_right, bottom_left] = corners_px;
    (0..ROWS)
        .flat_map(|row| {
            (0..COLS).map(move |column| {
                let u = (column as f64 + 0.5) / COLS as f64;
                let v = (row as f64 + 0.5) / ROWS as f64;
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
    let width = ((corners_px[1][0] - corners_px[0][0]).powi(2)
        + (corners_px[1][1] - corners_px[0][1]).powi(2))
    .sqrt()
        / COLS as f64;
    let height = ((corners_px[3][0] - corners_px[0][0]).powi(2)
        + (corners_px[3][1] - corners_px[0][1]).powi(2))
    .sqrt()
        / ROWS as f64;
    0.5 * SAMPLE_FRACTION * width.min(height)
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
        Some(crate::domain::color::WORKING_GAMMA as f32),
    )
}
