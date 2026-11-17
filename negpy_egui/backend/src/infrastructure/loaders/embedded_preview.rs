//! A camera RAW's embedded thumbnail, as a bounded 8-bit preview.
//!
//! Mirrors the RAW half of `load_bounded_preview` in
//! `negpy/infrastructure/loaders/rawpy_loader.py`: `rawler` reads the body's own JPEG
//! preview without demosaicing the sensor, then the result is orientation-corrected and
//! fitted to the longest edge. A missing preview falls back to the reduced TIFF page.

use std::path::Path;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::infrastructure::loaders::raw_decoder::unsupported_raw_reason;

/// The largest decoded preview read into a bounded thumbnail.
pub const EMBEDDED_PREVIEW_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// A bounded preview from the body's embedded JPEG, or the reduced page a TIFF carries.
///
/// A camera RAW that reports no usable preview, or whose payload this build rejects,
/// returns `None`. A finished scan decodes through its own loader, whose bound is the
/// caller's `max_edge`.
#[must_use]
pub fn load_bounded_thumbnail(path: &Path, max_edge: usize) -> Option<ImageBuffer> {
    let orientation = read_orientation(path);
    if crate::infrastructure::loaders::factory::is_camera_raw(path) {
        if unsupported_raw_reason(path).is_some() {
            return None;
        }
        let thumbnail = embedded_thumbnail(path)?;
        return Some(fit(&orient(&thumbnail, orientation), max_edge));
    }
    let fallback = crate::services::rendering::splash::try_splash_preview(path, max_edge)
        .map(|splash| splash.image)
        .or_else(|| {
            crate::infrastructure::loaders::factory::load(
                path,
                crate::infrastructure::loaders::factory::LoadOptions {
                    linear_raw: true,
                    positive_decode: None,
                    load_ir: false,
                },
            )
            .ok()
            .and_then(|loaded| loaded.image)
        })?;
    Some(fit(&orient(&fallback, orientation), max_edge))
}

fn embedded_thumbnail(path: &Path) -> Option<ImageBuffer> {
    let params = rawler::decoders::RawDecodeParams::default();
    let decoded = rawler::analyze::extract_thumbnail_pixels(path, &params).ok()?;
    let rgb = decoded.to_rgb8();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    if width == 0
        || height == 0
        || (width as u64) * (height as u64) * 3 > EMBEDDED_PREVIEW_MAX_BYTES
    {
        return None;
    }
    let pixels = rgb
        .as_raw()
        .iter()
        .map(|value| *value as f32 / 255.0)
        .collect();
    Some(ImageBuffer::from_rgb(width, height, pixels))
}

fn fit(image: &ImageBuffer, max_edge: usize) -> ImageBuffer {
    let long = image.width.max(image.height);
    if max_edge == 0 || long <= max_edge {
        return image.clone();
    }
    let scale = max_edge as f64 / long as f64;
    let width = ((image.width as f64 * scale).round() as usize).max(1);
    let height = ((image.height as f64 * scale).round() as usize).max(1);
    negpy_core::resize::resize_area(&image.pixels, image.height, image.width, 3, height, width)
        .map_or_else(
            || image.clone(),
            |pixels| ImageBuffer::from_rgb(width, height, pixels),
        )
}

fn orient(image: &ImageBuffer, orientation: u8) -> ImageBuffer {
    if orientation < 2 {
        return image.clone();
    }
    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect();
    let (height, width) = negpy_core::orient::oriented_dims(image.height, image.width, orientation);
    let mut dst = vec![0u8; height * width * 12];
    negpy_core::orient::orient(&bytes, image.height, image.width, 12, orientation, &mut dst);
    let pixels = dst
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    ImageBuffer::from_rgb(width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_has_no_preview() {
        assert!(load_bounded_thumbnail(Path::new("/missing/frame.dng"), 512).is_none());
    }

    #[test]
    fn a_decoded_preview_is_fitted_to_the_long_edge() {
        let image = ImageBuffer::from_rgb(40, 20, vec![0.5; 40 * 20 * 3]);
        let fitted = fit(&image, 10);
        assert_eq!((fitted.width, fitted.height), (10, 5));
        assert_eq!(fit(&image, 100).width, 40);
    }

    #[test]
    fn an_orientation_turns_the_preview() {
        let image = ImageBuffer::from_rgb(40, 20, vec![0.5; 40 * 20 * 3]);
        let turned = orient(&image, 6);
        assert_eq!((turned.width, turned.height), (20, 40));
    }
}
