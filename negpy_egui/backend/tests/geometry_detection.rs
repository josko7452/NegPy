//! Ports of the observable rules in `tests/test_geometry_logic.py` detection and auto-crop tests.

use negpy_backend::domain::models::{AspectRatio, AutocropMode, GeometryConfig};
use negpy_backend::features::geometry::processor::{
    closest_standard_ratio, detect_closest_aspect_ratio, detect_film_bounds,
    detect_film_bounds_with_confidence, enforce_roi_aspect_ratio, get_autocrop_coords,
    normalize_detection_input, resolve_autocrop_rect, scale_roi,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn light_bed(width: usize, height: usize, rect: (usize, usize, usize, usize)) -> ImageBuffer {
    let mut pixels = vec![1.0f32; width * height * 3];
    for y in rect.0..rect.1 {
        for x in rect.2..rect.3 {
            let i = (y * width + x) * 3;
            pixels[i..i + 3].fill(0.05);
        }
    }
    ImageBuffer::from_rgb(width, height, pixels)
}

#[test]
fn detection_input_never_upscales() {
    let img = light_bed(360, 240, (50, 190, 90, 270));
    let (same, scale) = normalize_detection_input(&img, 1800);
    assert_eq!(scale, 1.0);
    assert_eq!((same.width, same.height), (360, 240));
    let (small, scale) = normalize_detection_input(&img, 180);
    assert!(scale < 1.0);
    assert_eq!(small.width.max(small.height), 180);
}

#[test]
fn a_detection_roi_scales_back_to_the_source() {
    assert_eq!(scale_roi((10, 20, 30, 40), 0.5, 100, 100), (20, 40, 60, 80));
    assert_eq!(scale_roi((10, 20, 30, 40), 1.0, 100, 100), (10, 20, 30, 40));
}

#[test]
fn a_dark_frame_on_a_light_bed_has_an_outer_box() {
    let img = light_bed(360, 240, (50, 190, 90, 270));
    let detection = detect_film_bounds_with_confidence(&img);
    assert!(detection.roi.is_some(), "detection: {detection:?}");
}

#[test]
fn auto_crop_finds_the_dark_picture_on_a_light_bed() {
    let img = light_bed(360, 240, (50, 190, 90, 270));
    let (y1, y2, x1, x2) = get_autocrop_coords(
        &img,
        0,
        1.0,
        AspectRatio::Free,
        1800,
        None,
        AutocropMode::Image,
        1.0,
    );
    assert!(35 <= y1 && y1 <= 70, "{y1}");
    assert!(170 <= y2 && y2 <= 205, "{y2}");
    assert!(75 <= x1 && x1 <= 110, "{x1}");
    assert!(250 <= x2 && x2 <= 285, "{x2}");
}

#[test]
fn a_free_ratio_reads_a_landscape_3_to_2() {
    let img = light_bed(360, 240, (50, 190, 75, 285));
    assert_eq!(
        detect_closest_aspect_ratio(&img, AspectRatio::R3x2),
        AspectRatio::R3x2
    );
}

#[test]
fn a_forced_ratio_centers_the_crop() {
    let roi = enforce_roi_aspect_ratio((10, 110, 20, 220), 200, 300, AspectRatio::R1x1);
    let (cw, ch) = (roi.3 - roi.2, roi.1 - roi.0);
    assert_eq!(cw, ch);
    assert_eq!(roi.2, 20 + (200 - 100) / 2);
}

#[test]
fn the_closest_ratio_prefers_a_landscape_film_format() {
    let ratio = closest_standard_ratio((0, 200, 0, 300), (200, 300), AspectRatio::R3x2);
    assert_eq!(ratio, AspectRatio::R3x2);
}

#[test]
fn an_auto_crop_resolves_to_a_normalized_rect() {
    let img = light_bed(360, 240, (50, 190, 90, 270));
    let config = GeometryConfig {
        autocrop_ratio: AspectRatio::Free,
        ..GeometryConfig::default()
    };
    let rect = resolve_autocrop_rect(&img, &config, 1600.0, true);
    assert!(rect.is_some());
    let rect = rect.unwrap();
    assert!(rect[0] >= 0.0 && rect[1] >= 0.0 && rect[2] <= 1.0 && rect[3] <= 1.0);
    assert!(rect[2] > rect[0] && rect[3] > rect[1]);
}

#[test]
fn a_frame_with_no_film_edge_abstains_when_required() {
    let img = ImageBuffer::from_rgb(64, 64, vec![0.5; 64 * 64 * 3]);
    assert!(detect_film_bounds(&img).is_none());
    let config = GeometryConfig::default();
    assert!(resolve_autocrop_rect(&img, &config, 1600.0, true).is_none());
}
