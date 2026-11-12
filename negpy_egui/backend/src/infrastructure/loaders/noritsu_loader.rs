//! Noritsu EZController raw loading.
//!
//! Mirrors `negpy/infrastructure/loaders/noritsu_loader.py`: a headerless,
//! chunky BGR16 stream whose dimensions come from the file size.

use std::io::Read;
use std::path::Path;

use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};
use crate::infrastructure::loaders::source_kind::detect_noritsu_dims;

/// Load a Noritsu raw.
#[must_use]
pub fn load(path: &Path) -> Option<LoadedImage> {
    let (width, height) = detect_noritsu_dims(path)?;
    let (width, height) = (usize::try_from(width).ok()?, usize::try_from(height).ok()?);
    let samples = height.checked_mul(width)?.checked_mul(3)?;
    let mut bytes = Vec::with_capacity(samples * 2);
    std::fs::File::open(path)
        .ok()?
        .take(u64::try_from(samples * 2).ok()?)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() < samples * 2 {
        return None;
    }
    let rgb: Vec<u16> = bytes
        .chunks_exact(6)
        .flat_map(|pixel| {
            [
                u16::from_le_bytes([pixel[4], pixel[5]]),
                u16::from_le_bytes([pixel[2], pixel[3]]),
                u16::from_le_bytes([pixel[0], pixel[1]]),
            ]
        })
        .collect();
    Some(LoadedImage {
        image: Some(ImageBuffer::from_rgb16(width, height, rgb)),
        orientation: 0,
        ..LoadedImage::default()
    })
}
