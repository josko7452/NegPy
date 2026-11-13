//! Ports the deterministic abstain cases of tests/test_frame_skew.py.

use negpy_backend::features::geometry::skew::{measure_frame_skew, trusted_frame_skew};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;

fn textured(x: usize, y: usize) -> f32 {
    let coarse = ((x / 9) as f32 * 0.37 + (y / 7) as f32 * 0.61).sin();
    let fine = ((x / 3) as f32 * 0.91 + (y / 5) as f32 * 0.43).cos();
    0.18 + 0.06 * (coarse + fine + 2.0)
}

#[test]
fn a_blank_frame_abstains() {
    let blank = ImageBuffer::from_rgb(600, 400, vec![0.5; 600 * 400 * 3]);
    assert!(measure_frame_skew(&blank, true).is_none());
}

#[test]
fn a_single_textured_band_is_not_trusted() {
    let mut image = ImageBuffer::from_rgb(180, 120, vec![0.4; 180 * 120 * 3]);
    for y in 45..55 {
        for x in 10..170 {
            let at = (y * 180 + x) * 3;
            image.pixels[at..at + 3].fill(textured(x, y));
        }
    }
    assert!(trusted_frame_skew(&image, true).is_none());
}
