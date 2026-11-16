//! Ports the detection-plane cases from `tests/test_image_processor.py` and
//! `tests/test_retouch_logic.py`.

use negpy_backend::features::retouch::processor::{
    detection_downsample, downsample_to_long_edge, resize_mask, without_ir,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn flat(width: usize, height: usize, value: f32) -> ImageBuffer {
    ImageBuffer::from_rgb(width, height, vec![value; width * height * 3])
}

#[test]
fn downsampling_to_the_long_edge_never_upscales() {
    let image = flat(2000, 1000, 0.1);
    let shrunk = downsample_to_long_edge(&image, 500);
    assert_eq!((shrunk.width, shrunk.height), (500, 250));
    assert_eq!(downsample_to_long_edge(&image, 4000), image);
    assert_eq!(downsample_to_long_edge(&image, 0), image);
}

#[test]
fn a_speck_survives_the_detection_downsample() {
    let mut pixels = vec![0.18f32; 1200 * 800 * 3];
    for y in 400..402 {
        for x in 600..602 {
            let index = (y * 1200 + x) * 3;
            pixels[index..index + 3].fill(0.005);
        }
    }
    let image = ImageBuffer::from_rgb(1200, 800, pixels);
    let small = detection_downsample(&image, 400);
    assert_eq!(small.len(), 800 * 533);
    assert!(small.iter().any(|value| *value < 0.02));
}

#[test]
fn the_detection_plane_shrinks_to_the_ir_target() {
    let image = flat(2400, 1600, 0.2);
    let small = detection_downsample(&image, 400);
    assert_eq!(small.len(), 1600 * 1067);
    let large = flat(4800, 3200, 0.2);
    let small = detection_downsample(&large, 400);
    assert!(small.len() < 4800 * 3200);
}

#[test]
fn a_mask_resizes_nearest_and_keeps_its_shape() {
    let mask = vec![true, false, false, true];
    assert_eq!(resize_mask(&mask, 2, 2, (2, 2)), mask);
    let grown = resize_mask(&mask, 2, 2, (4, 4));
    assert_eq!(grown.len(), 16);
    assert_eq!(grown[0], true);
    assert_eq!(grown[15], true);
}

#[test]
fn ir_corrected_pixels_are_released_to_clean() {
    let score = vec![0.5f32, 0.5, 0.5, 0.5];
    let ir_mask = vec![true, false, false, false];
    let (released, _) = without_ir(Some(&score), &[], 2, 2, &ir_mask, (2, 2));
    assert_eq!(released, Some(vec![1.0, 0.5, 0.5, 0.5]));
}

#[test]
fn a_score_fully_released_becomes_none() {
    let score = vec![0.5f32; 4];
    let all = vec![true; 4];
    let (released, _) = without_ir(Some(&score), &[], 2, 2, &all, (2, 2));
    assert_eq!(released, None);
}

#[test]
fn a_hair_fully_released_is_dropped() {
    let hairs = vec![Some(vec![1u8, 1, 1, 1])];
    let all = vec![true; 4];
    let (_, kept) = without_ir(None, &hairs, 2, 2, &all, (2, 2));
    assert!(kept.is_empty());
}

#[test]
fn a_hair_partly_covered_keeps_its_uncovered_pixels() {
    let hairs = vec![Some(vec![1u8, 1, 0, 0])];
    let half = vec![true, false, false, false];
    let (_, kept) = without_ir(None, &hairs, 2, 2, &half, (2, 2));
    assert_eq!(kept, vec![Some(vec![0u8, 1, 0, 0])]);
}
