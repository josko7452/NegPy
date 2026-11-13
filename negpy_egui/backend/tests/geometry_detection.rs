//! Ports of the observable rules in `tests/test_geometry_logic.py` detection and auto-crop tests.

use negpy_backend::domain::models::{AspectRatio, AutocropMode, GeometryConfig};
use negpy_backend::features::geometry::processor::{
    closest_standard_ratio, detect_closest_aspect_ratio, detect_film_bounds,
    detect_film_bounds_with_confidence, enforce_roi_aspect_ratio, film_and_picture,
    get_autocrop_coords, measure_film_border, measure_film_edges, normalize_detection_input,
    rebate_mask, refine_film_roi_by_tiers, resolve_autocrop_rect, roi_from_measured_border,
    scale_roi,
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
fn film_border_finds_known_per_side_widths() {
    let (height, width) = (400, 600);
    let mut lum = vec![0.35f32; height * width];
    for y in 0..8 {
        lum[y * width..(y + 1) * width].fill(0.95);
    }
    for y in height - 20..height {
        lum[y * width..(y + 1) * width].fill(0.95);
    }
    for y in 0..height {
        lum[y * width..y * width + 12].fill(0.95);
        lum[y * width + width - 30..(y + 1) * width].fill(0.95);
    }
    let measured = measure_film_border(&lum, (0, 400, 0, 600), height, width);
    assert!((measured[0] - 8.0 / 400.0).abs() < 1e-6);
    assert!((measured[1] - 20.0 / 400.0).abs() < 1e-6);
    assert!((measured[2] - 12.0 / 600.0).abs() < 1e-6);
    assert!((measured[3] - 30.0 / 600.0).abs() < 1e-6);
}

#[test]
fn a_camera_scan_walk_finds_the_mask_and_base_tiers() {
    let (height, width) = (400, 600);
    let mut lum = vec![0.35f32; height * width];
    let strip = [0.0, 0.0, 0.0, 0.0, 0.2, 0.6, 1.0, 1.0, 1.0];
    for y in 0..strip.len() {
        lum[y * width..(y + 1) * width].fill(strip[y]);
    }
    for y in 0..height {
        for x in 0..strip.len() {
            lum[y * width + x] = strip[x];
        }
    }
    let measured = measure_film_edges(&lum, (0, 400, 0, 600), height, width);
    assert!((measured[0] - 10.0 / 400.0).abs() < 1e-2);
    assert!((measured[2] - 10.0 / 600.0).abs() < 1e-2);
    assert!(measured[1].abs() < 1e-2 && measured[3].abs() < 1e-2);
}

#[test]
fn a_measured_border_insets_only_when_an_opposite_pair_agrees() {
    let (height, width) = (400, 600);
    let mut lum = vec![0.35f32; height * width];
    for y in 0..12 {
        lum[y * width..(y + 1) * width].fill(0.95);
    }
    for y in height - 12..height {
        lum[y * width..(y + 1) * width].fill(0.95);
    }
    let inset = roi_from_measured_border(&lum, (0, 400, 0, 600), height, width);
    assert!(inset.is_none_or(|roi| roi.0 >= 0 && roi.1 <= 400));
    assert!(measure_film_border(&lum, (0, 400, 0, 600), height, width)[1] > 0.0);
}

#[test]
fn a_rebate_mask_is_inside_the_box_and_outside_the_gate() {
    let mask = rebate_mask((8, 10), Some((1, 7, 1, 9)), (2, 6, 2, 8));
    assert!(mask[1 * 10 + 1]);
    assert!(mask[1 * 10 + 8]);
    assert!(!mask[3 * 10 + 3]);
    assert!(!mask[0]);
}

#[test]
fn a_rebate_mask_is_empty_without_a_film_box() {
    assert!(
        rebate_mask((4, 4), None, (1, 3, 1, 3))
            .iter()
            .all(|value| !*value)
    );
}

#[test]
fn film_and_picture_and_tiers_are_bounded_by_the_frame() {
    let image = light_bed(200, 120, (10, 110, 20, 180));
    if let Some((film, gate)) = film_and_picture(&image) {
        if let Some((top, bottom, left, right)) = film {
            assert!(0 <= top && top <= bottom && bottom <= 120);
            assert!(0 <= left && left <= right && right <= 200);
        }
        assert!(0 <= gate.0 && gate.0 <= gate.1 && gate.1 <= 120);
    }
    let lum = vec![0.4f32; 120 * 200];
    assert!(refine_film_roi_by_tiers(&lum, (0, 120, 0, 200), 120, 200).is_none());
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
