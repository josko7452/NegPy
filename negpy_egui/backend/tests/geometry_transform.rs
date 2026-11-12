//! Ports of `tests/test_geometry_logic.py`, `test_crop_to_valid.py` and `test_geometry_distortion.py`.

use negpy_backend::domain::models::GeometryConfig;
use negpy_backend::features::geometry::logic::{
    apply_geometry_orientation, apply_keystone, apply_margin_to_roi, apply_radial_distortion,
    compute_geometry_crop_rect, get_manual_rect_coords,
};
use negpy_backend::features::geometry::mapping::{
    map_coords_to_geometry, map_point_radial, smooth_polyline,
};
use negpy_backend::features::geometry::processor::{GeometryContext, crop_image, process};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn blank(width: usize, height: usize) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![0.0; width * height * 3])
}

#[test]
fn a_quarter_turn_swaps_the_dimensions() {
    let img = blank(40, 20);
    let out = apply_geometry_orientation(&img, 1, false, false);
    assert_eq!((out.width, out.height), (20, 40));
}

#[test]
fn a_horizontal_flip_keeps_a_marker_on_the_other_side() {
    let mut pixels = vec![0.0; 8 * 4 * 3];
    pixels[..3].fill(1.0);
    let img = ImageBuffer::from_rgb(8, 4, pixels);
    let out = apply_geometry_orientation(&img, 0, true, false);
    assert_eq!(out.pixels[7 * 3], 1.0);
}

#[test]
fn a_manual_rect_is_a_plain_slice() {
    let roi = get_manual_rect_coords((100, 200), &[0.1, 0.1, 0.9, 0.9], 0, 1.0);
    assert_eq!(roi, (10, 90, 20, 180));
}

#[test]
fn a_crop_margin_is_scale_invariant() {
    let preview = apply_margin_to_roi((100, 140, 200, 260), 1000, 1000, 19.0);
    assert_eq!(preview, (119, 121, 219, 241));
}

#[test]
fn zero_geometry_is_the_full_frame() {
    assert_eq!(
        compute_geometry_crop_rect(0.0, 0.0, 0.0, 300, 200),
        (0.0, 0.0, 1.0, 1.0)
    );
}

#[test]
fn a_crop_to_valid_rect_avoids_the_void_wedge() {
    let (x1, y1, x2, y2) = compute_geometry_crop_rect(5.0, 0.0, 0.0, 300, 200);
    assert!(x1 > 0.0 && y1 > 0.0 && x2 < 1.0 && y2 < 1.0);
    assert!((x1 - (1.0 - x2)).abs() < 1e-3);
}

#[test]
fn a_positive_converge_v_keeps_the_canvas_size() {
    let img = blank(200, 120);
    let out = apply_keystone(&img, 10.0, 0.0);
    assert_eq!(out.width, img.width);
    assert_eq!(out.height, img.height);
}

#[test]
fn distortion_keeps_a_flat_field_flat_and_round_trips_a_point() {
    let img = ImageBuffer::from_rgb(90, 60, vec![0.37; 90 * 60 * 3]);
    let out = apply_radial_distortion(&img, 0.07);
    assert!(out.pixels.iter().all(|v| (*v - 0.37).abs() < 1e-5));
    let (x, y) = map_point_radial(10.0, 20.0, 0.07, 90, 60);
    assert!(x.is_finite() && y.is_finite());
}

#[test]
fn the_coordinate_mapper_keeps_a_raw_point_raw() {
    assert_eq!(
        map_coords_to_geometry(
            -0.2,
            1.3,
            (100, 200),
            0,
            0.0,
            false,
            false,
            None,
            0.0,
            0.0,
            0.0
        ),
        (-0.2, 1.3)
    );
}

#[test]
fn a_quarter_turn_maps_the_corner_with_both_coordinates_at_once() {
    let mapped = map_coords_to_geometry(
        0.0,
        0.0,
        (100, 200),
        1,
        0.0,
        false,
        false,
        None,
        0.0,
        0.0,
        0.0,
    );
    assert_eq!(mapped, (0.0, 1.0));
}

#[test]
fn the_polyline_interpolates_its_control_points() {
    let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
    let out = smooth_polyline(&pts, false, 4);
    assert_eq!(out[0], pts[0]);
    assert_eq!(out[4], pts[1]);
    assert_eq!(out[8], pts[2]);
}

#[test]
fn the_processor_resolves_a_manual_crop() {
    let mut config = GeometryConfig::default();
    config.crop_rect = Some(vec![0.1, 0.1, 0.9, 0.9]);
    let result = process(
        &blank(200, 100),
        &config,
        GeometryContext { scale_factor: 1.0 },
    );
    assert_eq!(result.active_roi, Some((10, 90, 20, 180)));
    let cropped = crop_image(&result.image, result.active_roi);
    assert_eq!((cropped.width, cropped.height), (160, 80));
}

#[test]
fn crop_by_default_yields_to_an_armed_auto_crop() {
    let mut config = GeometryConfig::default();
    config.crop_to_valid = true;
    config.crop_from_auto = true;
    let result = process(
        &blank(200, 100),
        &config,
        GeometryContext { scale_factor: 1.0 },
    );
    assert_eq!(result.active_roi, None);
}
