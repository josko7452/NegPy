//! A camera RAW's as-shot white balance, read without a demosaic.
//!
//! Mirrors `camera_wb_for` and the `use_camera_wb` branch of `_load_from_open_raw` in
//! `negpy/services/rendering/{image_processor,preview_manager}.py`: the multipliers come
//! from the file's own metadata, and a buffer decoded without them keeps the key.

use std::path::Path;

use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::infrastructure::loaders::raw_decoder::unsupported_raw_reason;

/// The RAW's as-shot multipliers as `[R, G, B]`, normalized to green.
#[must_use]
pub fn camera_wb_for(path: &Path) -> Option<[f64; 3]> {
    if unsupported_raw_reason(path).is_some() {
        return None;
    }
    let raw = rawler::decode_file(path).ok()?;
    let [red, green, blue, _] = raw.wb_coeffs;
    [red, green, blue]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && green > 0.0)
        .then_some([f64::from(red / green), 1.0, f64::from(blue / green)])
}

/// Apply multipliers to a linear buffer and clip to 0..1.
#[must_use]
pub fn apply_camera_wb(image: &ImageBuffer, wb: Option<[f64; 3]>) -> ImageBuffer {
    let Some(gains) =
        wb.filter(|gains| gains.iter().all(|value| value.is_finite() && *value > 0.0))
    else {
        return image.clone();
    };
    let gains = [gains[0] as f32, gains[1] as f32, gains[2] as f32];
    let mut out = vec![0.0; image.pixels.len()];
    negpy_core::convert::scale_clip_unit(&image.pixels, &gains, &mut out);
    ImageBuffer::from_rgb(image.width, image.height, out)
}

/// Whether the cache key records a white balance for this request.
#[must_use]
pub fn decodes_with_wb(
    use_camera_wb: bool,
    bake_camera_wb: bool,
    wb_override: Option<&[f64]>,
) -> bool {
    use_camera_wb || bake_camera_wb || wb_override.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_neutral_request_keeps_the_source() {
        let image = ImageBuffer::from_rgb(2, 2, vec![0.5; 12]);
        assert_eq!(apply_camera_wb(&image, None), image);
        assert_eq!(apply_camera_wb(&image, Some([1.0, 1.0, 1.0])), image);
    }

    #[test]
    fn a_red_multiplier_scales_only_red() {
        let image = ImageBuffer::from_rgb(1, 1, vec![0.2, 0.4, 0.6]);
        let out = apply_camera_wb(&image, Some([2.0, 1.0, 0.5]));
        assert_eq!(out.pixels, [0.4, 0.4, 0.3]);
    }

    #[test]
    fn a_request_records_wb_only_when_it_decodes_with_one() {
        assert!(!decodes_with_wb(false, false, None));
        assert!(decodes_with_wb(true, false, None));
        assert!(decodes_with_wb(false, true, None));
        assert!(decodes_with_wb(false, false, Some(&[1.0, 1.0, 1.0])));
    }

    #[test]
    fn a_missing_file_has_no_wb() {
        assert!(camera_wb_for(Path::new("/missing/frame.dng")).is_none());
    }
}
