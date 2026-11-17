//! Linear sensor-RGB demosaic for calibration metering.
//!
//! Mirrors `negpy/infrastructure/capture/raw_demosaic.py`: the decode stays in the sensor's
//! own RGB, with no white balance and no gamma, and it scales by the camera's calibrated
//! white level, never the frame's own brightest pixel. A meter that compares frames needs
//! one fixed reference.

use std::path::Path;

use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::infrastructure::loaders::raw_decoder::unsupported_raw_reason;

/// The camera's calibrated linearity limit, with the black level the scale starts from.
///
/// A per-channel table can sit below the format's generic ADC ceiling: trusting the
/// larger number lets already non-linear photosites read as clean. A body with no table
/// falls back to the 16-bit format ceiling.
#[must_use]
pub fn linearity_limit(
    white_level: Option<&[u32]>,
    black_level: Option<&[f64]>,
    positions: Option<&[usize]>,
) -> Option<u32> {
    let generic = white_level
        .and_then(|levels| levels.first().copied())
        .filter(|value| *value > 0)
        .unwrap_or(u16::MAX as u32);
    let black = black_level
        .filter(|levels| !levels.is_empty())
        .map(|levels| levels.iter().copied().fold(f64::INFINITY, f64::min))
        .unwrap_or(0.0);
    let calibrated = positions.and_then(|positions| {
        let levels = white_level?;
        if levels.len() <= 1 {
            return None;
        }
        let values: Vec<u32> = positions
            .iter()
            .filter_map(|index| levels.get(*index).copied())
            .collect();
        (!values.is_empty()).then(|| values.iter().copied().min().unwrap_or(0))
    });
    let limit = calibrated.map_or(generic, |value| value.min(generic));
    if limit == 0 {
        return None;
    }
    Some(limit.saturating_sub(black.max(0.0) as u32).max(1))
}

/// The RAW's as-shot multipliers as `[R, G, B]`, normalized to green.
#[must_use]
pub fn as_shot_wb(coeffs: Option<[f32; 4]>) -> Option<[f64; 3]> {
    let [red, green, blue, _] = coeffs?;
    [red, green, blue]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && green > 0.0)
        .then_some([f64::from(red / green), 1.0, f64::from(blue / green)])
}

fn scale_sensor(pixels: &mut [f32], limit: Option<u32>) {
    let scale = limit.map_or(1.0, |limit| 1.0 / f64::from(limit.max(1)));
    for value in pixels {
        *value = (*value as f64 * scale).clamp(0.0, 1.0) as f32;
    }
}

/// Decode one RAW to sensor-native linear RGB in 0..1.
///
/// `half_size` bins each 2x2 Bayer quad into one pixel, which calibration meters on a
/// uniform base patch. A non-Bayer sensor keeps the full decode.
#[must_use]
pub fn linear_demosaic(path: &Path, half_size: bool) -> Option<ImageBuffer> {
    if unsupported_raw_reason(path).is_some() {
        return None;
    }
    let raw = rawler::decode_file(path).ok()?;
    let limit = linearity_limit(Some(raw.whitelevel.0.as_slice()), None, None);
    let steps: &[rawler::imgop::develop::ProcessingStep] = if half_size {
        &[
            rawler::imgop::develop::ProcessingStep::Rescale,
            rawler::imgop::develop::ProcessingStep::Demosaic,
            rawler::imgop::develop::ProcessingStep::CropActiveArea,
            rawler::imgop::develop::ProcessingStep::CropDefault,
        ]
    } else {
        &[
            rawler::imgop::develop::ProcessingStep::Rescale,
            rawler::imgop::develop::ProcessingStep::Demosaic,
            rawler::imgop::develop::ProcessingStep::FujiRotate,
            rawler::imgop::develop::ProcessingStep::CropActiveArea,
            rawler::imgop::develop::ProcessingStep::CropDefault,
        ]
    };
    let developed = rawler::imgop::develop::RawDevelop::new_with(steps)
        .develop_intermediate(&raw)
        .ok()?;
    let (width, height, mut pixels) = match developed {
        rawler::imgop::develop::Intermediate::ThreeColor(image) => {
            (image.width, image.height, image.into_flatten())
        }
        rawler::imgop::develop::Intermediate::FourColor(image) => {
            let width = image.width;
            let height = image.height;
            let pixels = image
                .into_inner()
                .into_iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect();
            (width, height, pixels)
        }
        rawler::imgop::develop::Intermediate::Monochrome(image) => {
            let width = image.width;
            let height = image.height;
            let pixels = image
                .data
                .into_iter()
                .flat_map(|value| [value, value, value])
                .collect();
            (width, height, pixels)
        }
    };
    for pixel in pixels.chunks_exact_mut(3) {
        pixel[0] = pixel[0].max(0.0);
        pixel[1] = pixel[1].max(0.0);
        pixel[2] = pixel[2].max(0.0);
    }
    scale_sensor(&mut pixels, limit);
    Some(ImageBuffer::from_rgb(width, height, pixels))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_per_channel_table_lowers_the_limit_to_its_minimum() {
        assert_eq!(
            linearity_limit(Some(&[16383, 15311, 16383]), None, Some(&[0, 1, 2])),
            Some(15311)
        );
        assert_eq!(
            linearity_limit(Some(&[16383, 15311, 16383]), None, Some(&[0, 2])),
            Some(16383)
        );
    }

    #[test]
    fn a_calibrated_limit_never_exceeds_the_format_ceiling() {
        assert_eq!(
            linearity_limit(Some(&[15000, 20000]), None, Some(&[1])),
            Some(15000)
        );
    }

    #[test]
    fn the_black_level_reduces_the_scale() {
        assert_eq!(
            linearity_limit(Some(&[16383]), Some(&[512.0]), None),
            Some(15871)
        );
    }

    #[test]
    fn a_missing_or_zero_white_level_has_no_limit() {
        assert_eq!(linearity_limit(Some(&[0]), None, None), Some(65535));
        assert_eq!(linearity_limit(None, None, None), Some(65535));
    }

    #[test]
    fn a_white_balanced_as_shot_reading_is_normalized_to_green() {
        assert_eq!(
            as_shot_wb(Some([2.0, 1.0, 0.5, 1.0])),
            Some([2.0, 1.0, 0.5])
        );
        assert_eq!(as_shot_wb(None), None);
        assert_eq!(as_shot_wb(Some([0.0, 1.0, 1.0, 1.0])), None);
    }

    #[test]
    fn a_sensor_scale_pins_every_frame_to_one_reference() {
        let mut pixels = vec![8192.0f32, 4096.0, 16384.0];
        scale_sensor(&mut pixels, Some(16384));
        assert_eq!(pixels, vec![0.5, 0.25, 1.0]);
    }

    #[test]
    fn a_missing_file_decodes_to_nothing() {
        assert!(linear_demosaic(Path::new("/missing/frame.dng"), false).is_none());
    }
}
