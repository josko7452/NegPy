//! JPEG scan loading.
//!
//! Mirrors `negpy/infrastructure/loaders/jpeg_loader.py`: an untagged JPEG is sRGB,
//! an embedded profile names its color space, and a Positive frame decodes as Auto.

use std::path::Path;

use image::{ImageDecoder, ImageReader};

use crate::domain::models::SourceEncoding;
use crate::infrastructure::loaders::helpers::{identify_color_space_from_icc, read_orientation};
use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};
use crate::infrastructure::loaders::source_encoding::{
    PROFILE, SampleDepth, decode_positive, resolve_decode,
};

/// Load a JPEG scan.
#[must_use]
pub fn load(path: &Path, positive_decode: Option<SourceEncoding>) -> Option<LoadedImage> {
    let mut decoder = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    let icc = decoder.icc_profile().ok().flatten();
    let rgb = image::DynamicImage::from_decoder(decoder).ok()?.to_rgb8();
    let (width, height) = rgb.dimensions();
    let (width, height) = (width as usize, height as usize);
    let mut metadata = LoadedImage {
        orientation: read_orientation(path),
        icc_profile: icc.clone(),
        ..LoadedImage::default()
    };
    let depth = SampleDepth::Eight;
    if positive_decode.is_some() {
        let resolved = resolve_decode(SourceEncoding::Auto, icc.as_deref(), depth);
        metadata.source_encoding = Some(resolved.ran.clone());
        metadata.color_space = if resolved.ran == PROFILE {
            icc.as_deref()
                .and_then(identify_color_space_from_icc)
                .map(str::to_string)
        } else {
            None
        };
        let pixels = rgb.as_raw().iter().map(|v| *v as f32 / 255.0).collect();
        let image = ImageBuffer::from_rgb(width, height, pixels);
        metadata.image = Some(decode_positive(&image, &resolved.decode).0);
        return Some(metadata);
    }
    metadata.color_space = icc
        .as_deref()
        .and_then(identify_color_space_from_icc)
        .map(str::to_string);
    let color = metadata
        .color_space
        .clone()
        .unwrap_or_else(|| "sRGB".to_string());
    metadata.color_space = Some(color.clone());
    let mut pixels: Vec<f32> = rgb.as_raw().iter().map(|v| *v as f32 / 255.0).collect();
    if color == "sRGB" {
        negpy_core::color::srgb_to_linear(&pixels.clone(), &mut pixels);
    }
    metadata.image = Some(ImageBuffer::from_rgb(width, height, pixels));
    Some(metadata)
}
