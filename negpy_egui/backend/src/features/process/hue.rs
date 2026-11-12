//! Hue Trim: a working-Lab hue rotation, not a cast.

use crate::domain::color::working_lab;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// Rotate every hue by `degrees` in the working Lab a*b* plane.
#[must_use]
pub fn apply_hue_trim(img: &ImageBuffer, degrees: f64) -> ImageBuffer {
    if degrees == 0.0 {
        return img.clone();
    }
    let radians = degrees.to_radians();
    let rotation = (radians.cos() as f32, radians.sin() as f32);
    let mut out = vec![0.0; img.pixels.len()];
    negpy_core::lab::hue_trim(&img.pixels, rotation, working_lab(), &mut out);
    ImageBuffer::from_rgb(img.width, img.height, out)
}
