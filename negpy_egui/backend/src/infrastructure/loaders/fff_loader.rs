//! Flextight FFF detection.
//!
//! Mirrors `negpy/infrastructure/loaders/fff_loader.py`: the largest directory
//! is the full-resolution image, either 16-bit RGB or SGI LogLuv.

use std::path::Path;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};
use crate::infrastructure::loaders::logluv::{
    COMPRESSION_SGILOG, COMPRESSION_SGILOG24, decode_strip_logluv32, logluv24_to_rgb,
    logluv32_to_rgb,
};
use crate::infrastructure::loaders::tiff_loader::decode_ifd;
use crate::infrastructure::loaders::tiff_walker::{IfdInfo, read_ifds};

const PHOTOMETRIC_RGB: u16 = 2;
const PHOTOMETRIC_LOGLUV: u16 = 32845;

/// The SGI LogLuv compression codes a Flextight raw uses.
#[must_use]
pub fn is_sgilog_compression(compression: u16) -> bool {
    matches!(compression, 34676 | 34677)
}

fn is_full_res(info: &IfdInfo) -> bool {
    if info.photometric == PHOTOMETRIC_LOGLUV {
        return true;
    }
    info.samples_per_pixel >= 3 && info.photometric == PHOTOMETRIC_RGB && info.bits_per_sample >= 16
}

/// The largest image directory by pixel count, matching the Python FFF rule.
#[must_use]
pub fn find_full_res_ifd(ifds: &[IfdInfo]) -> Option<&IfdInfo> {
    ifds.iter()
        .filter(|info| is_full_res(info))
        .max_by_key(|info| info.pixels())
}

/// True if any directory uses an SGI LogLuv compression.
#[must_use]
pub fn has_sgilog_ifd(ifds: &[IfdInfo]) -> bool {
    ifds.iter()
        .any(|info| is_sgilog_compression(info.compression))
}

/// True if this `.fff` is an Imacon/Hasselblad Flextight scanner file.
#[must_use]
pub fn is_flextight_fff(path: &Path) -> bool {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("fff"))
    {
        return false;
    }
    read_ifds(path)
        .as_deref()
        .and_then(find_full_res_ifd)
        .is_some()
}

/// Load the full-resolution image from a Flextight FFF.
#[must_use]
pub fn load(path: &Path) -> Option<LoadedImage> {
    let ifds = read_ifds(path)?;
    let selected = find_full_res_ifd(&ifds)?;
    if is_sgilog_compression(selected.compression) {
        let pixels = decode_logluv(path, selected)?;
        return Some(LoadedImage {
            image: Some(pixels),
            orientation: read_orientation(path),
            ..LoadedImage::default()
        });
    }
    let (image, _, _) = decode_ifd(path, selected.offset)?;
    Some(LoadedImage {
        image: Some(image),
        orientation: read_orientation(path),
        ..LoadedImage::default()
    })
}

fn decode_logluv(path: &Path, info: &IfdInfo) -> Option<ImageBuffer> {
    let data = std::fs::read(path).ok()?;
    let width = usize::try_from(info.width).ok()?;
    let height = usize::try_from(info.height).ok()?;
    let (bpp, rle) = match info.compression {
        COMPRESSION_SGILOG => (4, true),
        COMPRESSION_SGILOG24 => (3, false),
        _ => return None,
    };
    let rows = info.rows_per_strip.max(info.height) as usize;
    let mut packed = Vec::with_capacity(width * height);
    for (index, (&offset, &count)) in info
        .strip_offsets
        .iter()
        .zip(&info.strip_byte_counts)
        .enumerate()
    {
        let start = usize::try_from(offset).ok()?;
        let end = start.checked_add(usize::try_from(count).ok()?)?;
        let strip = data.get(start..end)?;
        let strip_rows = if index + 1 == info.strip_offsets.len() {
            height.saturating_sub(index * rows)
        } else {
            rows
        };
        let pixels = strip_rows * width;
        let codes = if rle {
            decode_strip_logluv32(strip, pixels, width)
        } else {
            decode_logluv24(strip, pixels, bpp)?
        };
        packed.extend(codes);
    }
    if packed.len() < width * height {
        return None;
    }
    let rgb = if rle {
        logluv32_to_rgb(&packed)
    } else {
        logluv24_to_rgb(&packed)
    };
    let mut normalized = vec![0f32; rgb.len()];
    negpy_core::loaders::normalize_linear(&rgb, 3, &mut normalized);
    Some(ImageBuffer::from_rgb(width, height, normalized))
}

fn decode_logluv24(strip: &[u8], pixels: usize, bpp: usize) -> Option<Vec<u32>> {
    (bpp == 3 && strip.len() >= pixels * 3).then(|| {
        strip
            .chunks_exact(3)
            .take(pixels)
            .map(|pixel| {
                (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2])
            })
            .collect()
    })
}
