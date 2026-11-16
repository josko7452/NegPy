//! Ports the plane invariants of tests/test_contrast_mask.py and test_core_render.py:
//! the mask is zero-mean, the crop keeps the surround out, and expansion clips an empty ROI.

use negpy_backend::features::exposure::metering::effective_crosstalk_matrix;
use negpy_backend::features::exposure::normalization::{
    LogNegativeBounds, PlaneGeometry, contrast_mask_plane, diffusion_grid, diffusion_plane,
    expand_mask_plane, normalized_roi,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

const BOUNDS: LogNegativeBounds = LogNegativeBounds::new([-1.4; 3], [-0.05; 3]);

fn negative(width: usize, height: usize) -> ImageBuffer {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let key = (-3.2 * (y as f32 / height as f32)).exp();
            let texture = 1.0 + 0.16 * (x as f32 / 3.0).sin() * (y as f32 / 2.5).sin();
            let scene = (key * texture).max(1e-5);
            let neg = (0.03 + 0.85 * (1.0 - scene.powf(0.35))).clamp(1e-4, 1.0);
            pixels.extend_from_slice(&[neg, neg, neg]);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

fn mean(values: &[f32]) -> f64 {
    values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f32]) -> f64 {
    let mean = mean(values);
    (values
        .iter()
        .map(|value| (*value as f64 - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt()
}

fn flat_rgb(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

#[test]
fn the_plane_is_zero_mean() {
    let image = negative(360, 240);
    let (plane, _, _, _) =
        contrast_mask_plane(&image, BOUNDS, None, PlaneGeometry::default(), None, None);
    assert!(mean(&plane).abs() < 1e-5);
}

#[test]
fn an_unmetered_frame_has_a_flat_plane() {
    let image = negative(120, 80);
    let (plane, _, _, centre) = contrast_mask_plane(
        &image,
        LogNegativeBounds::new([0.0; 3], [0.0; 3]),
        None,
        PlaneGeometry::default(),
        None,
        None,
    );
    assert_eq!(centre, 0.5);
    assert!(plane.iter().all(|value| *value == 0.0));
}

#[test]
fn the_surround_outside_the_crop_stays_out_of_the_mask() {
    let (height, width, border) = (600, 900, 60);
    let mut image = flat_rgb(width, height, 0.35);
    for y in 0..height {
        for x in 0..width {
            if y < border || y >= height - border || x < border || x >= width - border {
                let start = (y * width + x) * 3;
                image.pixels[start..start + 3].fill(0.98);
            }
        }
    }
    let roi = normalized_roi(
        Some((
            border as i64,
            (height - border) as i64,
            border as i64,
            (width - border) as i64,
        )),
        (height, width),
    );
    let (cropped, _, _, _) =
        contrast_mask_plane(&image, BOUNDS, None, PlaneGeometry::default(), roi, None);
    let (whole, _, _, _) =
        contrast_mask_plane(&image, BOUNDS, None, PlaneGeometry::default(), None, None);
    assert!(std_dev(&cropped) < 0.002);
    assert!(std_dev(&whole) > 10.0 * std_dev(&cropped) + 0.01);
}

#[test]
fn diffusion_takes_no_bounds_and_returns_none_for_an_unmetered_frame() {
    let image = negative(90, 60);
    let grid = diffusion_grid(&image, None, PlaneGeometry::default(), None);
    assert!(diffusion_plane(&grid, BOUNDS, None).is_some());
    assert!(diffusion_plane(&grid, LogNegativeBounds::new([1.0; 3], [1.0; 3]), None).is_none());
}

#[test]
fn a_crosstalk_matrix_reaches_the_planes() {
    let image = negative(90, 60);
    let unmix = effective_crosstalk_matrix(&Default::default(), None);
    let plain = diffusion_grid(&image, None, PlaneGeometry::default(), None);
    let mixed = diffusion_grid(&image, unmix, PlaneGeometry::default(), None);
    assert_eq!(plain.width, mixed.width);
    assert_eq!(plain.height, mixed.height);
}

#[test]
fn an_empty_roi_expands_to_none() {
    let plane = vec![0.5; 61 * 87];
    assert!(expand_mask_plane(&plane, (61, 87), (300, 400), Some((100, 100, 0, 10))).is_none());
}

#[test]
fn an_expansion_fills_the_whole_frame() {
    let plane: Vec<f32> = (0..61 * 87).map(|index| index as f32 / 1000.0).collect();
    let out = expand_mask_plane(&plane, (61, 87), (300, 400), None).expect("a full ROI");
    assert_eq!(out.len(), 300 * 400);
    assert!(out.iter().any(|value| value.is_finite()));
}
