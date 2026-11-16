//! Pixel conversions and float-array helpers the kernels share.
//!
//! Mirrors `negpy/kernel/image/logic.py` for the primitives that do not belong to one
//! feature: the bit-depth conversions, channel widening, luminance and the percentile
//! read off a sorted grid. Every kernel itself lives in `negpy_core`.

use crate::domain::types::{LUMA_B, LUMA_G, LUMA_R};
use crate::infrastructure::loaders::image_buffer::ImageBuffer;

/// Float samples in 0..1 to 16-bit codes, rounded half up and clipped.
#[must_use]
pub fn float_to_uint16(values: &[f32]) -> Vec<u16> {
    let mut out = vec![0u16; values.len()];
    negpy_core::convert::to_uint(values, &mut out);
    out
}

/// Float samples in 0..1 to 8-bit codes, rounded half up and clipped.
#[must_use]
pub fn float_to_uint8(values: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; values.len()];
    negpy_core::convert::to_uint(values, &mut out);
    out
}

/// The luminance of each pixel to 8-bit codes.
#[must_use]
pub fn float_to_uint8_luma(values: &[f32], channels: usize) -> Vec<u8> {
    let mut out = vec![0u8; sample_count(values.len(), channels)];
    negpy_core::convert::luma_to_uint(values, channels.max(1), &mut out);
    out
}

/// The luminance of each pixel to 16-bit codes.
#[must_use]
pub fn float_to_uint16_luma(values: &[f32], channels: usize) -> Vec<u16> {
    let mut out = vec![0u16; sample_count(values.len(), channels)];
    negpy_core::convert::luma_to_uint(values, channels.max(1), &mut out);
    out
}

fn sample_count(len: usize, channels: usize) -> usize {
    if channels > 1 { len / channels } else { len }
}

/// 8-bit samples to floats in 0..1.
#[must_use]
pub fn uint8_to_float32(values: &[u8]) -> Vec<f32> {
    let mut out = vec![0.0f32; values.len()];
    negpy_core::convert::uint8_to_float32(values, &mut out);
    out
}

/// 16-bit samples to floats in 0..1.
#[must_use]
pub fn uint16_to_float32(values: &[u16]) -> Vec<f32> {
    let mut out = vec![0.0f32; values.len()];
    negpy_core::convert::uint16_to_float32(values, &mut out);
    out
}

/// Widen a one-channel or two-dimensional frame to three channels.
#[must_use]
pub fn ensure_rgb(image: &ImageBuffer) -> ImageBuffer {
    let pixels = if image.pixels.len() == image.width * image.height {
        let mut out = vec![0.0f32; image.width * image.height * 3];
        negpy_core::convert::gray_to_rgb(&image.pixels, &mut out);
        out
    } else {
        image.pixels.clone()
    };
    ImageBuffer::from_rgb(image.width, image.height, pixels)
}

/// The luminance of an `H x W x 3` buffer, or a weighted sum over an `N x 3` one.
#[must_use]
pub fn get_luminance(values: &[f32], channels: usize) -> Vec<f32> {
    if channels == 3 {
        let mut out = vec![0.0f32; values.len() / 3];
        negpy_core::analysis::luminance(values, &mut out);
        return out;
    }
    values
        .chunks_exact(channels.max(1))
        .map(|pixel| {
            let red = pixel.first().copied().unwrap_or(0.0);
            let green = pixel.get(1).copied().unwrap_or(0.0);
            let blue = pixel.get(2).copied().unwrap_or(0.0);
            red * LUMA_R + green * LUMA_G + blue * LUMA_B
        })
        .collect()
}

/// `numpy.percentile` of one column of a sorted grid, with numpy's arithmetic.
///
/// The virtual index is `(n - 1) * (q / 100)` in float64, and the upper half interpolates
/// back from the high sample. Both steps are bit-exact with `np.percentile`.
#[must_use]
pub fn percentile_from_sorted(sorted: &[f32], q: f64) -> f32 {
    if sorted.is_empty() {
        return f32::NAN;
    }
    let last = (sorted.len() - 1) as f64;
    let virtual_index = last * (q / 100.0);
    let low = (virtual_index.floor() as isize).clamp(0, sorted.len() as isize - 1) as usize;
    let high = (low + 1).min(sorted.len() - 1);
    let t = virtual_index - low as f64;
    let (a, b) = (sorted[low], sorted[high]);
    let diff = b - a;
    if t >= 0.5 {
        b - diff * (1.0 - t as f32)
    } else {
        a + diff * t as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bit_depth_conversions_round_half_up() {
        assert_eq!(float_to_uint8(&[-1.0, 0.0, 0.5, 1.0]), [0, 0, 128, 255]);
        assert_eq!(float_to_uint16(&[0.0, 0.5, 1.0]), [0, 32768, 65535]);
        assert_eq!(uint8_to_float32(&[0, 255]), [0.0, 1.0]);
        assert_eq!(uint16_to_float32(&[0, 65535]), [0.0, 1.0]);
    }

    #[test]
    fn a_luma_conversion_collapses_three_channels() {
        assert_eq!(
            float_to_uint8_luma(&[1.0, 1.0, 1.0, 0.0, 0.0, 0.0], 3),
            [255, 0]
        );
        assert_eq!(float_to_uint16_luma(&[1.0, 1.0, 1.0], 3), [65535]);
    }

    #[test]
    fn a_grey_frame_widens_to_three_channels() {
        let grey = ImageBuffer::from_rgb(2, 1, vec![0.25, 0.75]);
        let rgb = ensure_rgb(&grey);
        assert_eq!(rgb.pixels, [0.25, 0.25, 0.25, 0.75, 0.75, 0.75]);
    }

    #[test]
    fn luminance_weighs_the_three_channels_by_the_working_weights() {
        let luma = get_luminance(&[1.0, 0.0, 0.0], 3);
        assert!((luma[0] - LUMA_R).abs() < 1e-6);
        let neutral = get_luminance(&[0.5, 0.5, 0.5], 3);
        assert!((neutral[0] - 0.5 * (LUMA_R + LUMA_G + LUMA_B)).abs() < 1e-6);
    }

    #[test]
    fn the_percentile_matches_numpy_arithmetic() {
        let sorted = [0.0f32, 1.0, 2.0, 3.0, 4.0];
        assert!((percentile_from_sorted(&sorted, 50.0) - 2.0).abs() < 1e-6);
        assert!((percentile_from_sorted(&sorted, 90.0) - 3.6).abs() < 1e-6);
        assert!((percentile_from_sorted(&sorted, 0.0) - 0.0).abs() < 1e-6);
        assert!((percentile_from_sorted(&sorted, 100.0) - 4.0).abs() < 1e-6);
        assert!(percentile_from_sorted(&[], 50.0).is_nan());
    }
}
