//! Pakon planar raw loading.
//!
//! Mirrors `negpy/infrastructure/loaders/pakon_loader.py`: a TLX header or a
//! known file size gives the layout, and an unknown layout is guessed from the
//! first samples.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};
use crate::infrastructure::loaders::source_kind::{pakon_header, pakon_layout};

/// Load a Pakon raw.
#[must_use]
pub fn load(path: &Path) -> Option<LoadedImage> {
    let (height, width) = pakon_layout(path)?;
    let (height, width) = (usize::try_from(height).ok()?, usize::try_from(width).ok()?);
    let pixels = height.checked_mul(width)?.checked_mul(3)?;
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let offset = size.saturating_sub(u64::try_from(pixels).ok()?.saturating_mul(2));
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = Vec::with_capacity(pixels * 2);
    file.read_to_end(&mut bytes).ok()?;
    if bytes.len() < pixels * 2 {
        return None;
    }
    let samples: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take(pixels)
        .collect();

    let planar = pakon_header(path).is_some() || !looks_interleaved(&samples);
    let rgb = if planar {
        let plane = height * width;
        let mut rgb = Vec::with_capacity(pixels);
        for index in 0..plane {
            rgb.push(samples[index]);
            rgb.push(samples[plane + index]);
            rgb.push(samples[2 * plane + index]);
        }
        rgb
    } else {
        samples
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0]])
            .collect()
    };

    Some(LoadedImage {
        image: Some(ImageBuffer::from_rgb16(width, height, rgb)),
        orientation: 0,
        ..LoadedImage::default()
    })
}

fn looks_interleaved(samples: &[u16]) -> bool {
    let sample = &samples[..samples.len().min(6000)];
    if sample.len() < 4 {
        return false;
    }
    let adjacent = sample
        .windows(2)
        .map(|pair| f32::from(pair[1].abs_diff(pair[0])))
        .sum::<f32>()
        / (sample.len() - 1) as f32;
    let step3 = sample
        .windows(4)
        .map(|block| f32::from(block[3].abs_diff(block[0])))
        .sum::<f32>()
        / (sample.len() - 3) as f32;
    adjacent > step3 * 1.5
}
