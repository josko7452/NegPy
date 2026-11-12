//! Preview memory estimates from file headers.
//!
//! Mirrors `negpy/infrastructure/loaders/memory.py`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use tiff::decoder::{Decoder, IfdDecoder};
use tiff::tags::{IfdPointer, Tag};

/// Preview storage and the peak loader workspace it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewMemoryEstimate {
    pub cached_bytes: u64,
    pub temporary_bytes: u64,
    pub source_dimensions: Option<(u64, u64)>,
}

fn bytes_per_pixel(profile: &str) -> u64 {
    match profile {
        "display" => 24,
        "scan" => 48,
        _ => 64,
    }
}

fn file_multiplier(profile: &str) -> u64 {
    match profile {
        "display" => 4,
        "scan" => 12,
        _ => 24,
    }
}

fn floor_bytes(profile: &str) -> u64 {
    match profile {
        "display" => 64 * 1024 * 1024,
        "scan" => 256 * 1024 * 1024,
        _ => 512 * 1024 * 1024,
    }
}

fn is_tiff_header(header: &[u8]) -> bool {
    matches!(header, b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
}

fn directory_dimensions(decoder: &mut Decoder<File>, directory: &tiff::Directory) -> Option<(u64, u64)> {
    let mut tags = decoder.read_directory_tags(directory);
    let width = tags.find_tag_unsigned::<u32>(Tag::ImageWidth).ok()??;
    let height = tags.find_tag_unsigned::<u32>(Tag::ImageLength).ok()??;
    (width > 0 && height > 0).then(|| (u64::from(width), u64::from(height)))
}

fn sub_directories(tags: &mut IfdDecoder<'_>) -> Vec<IfdPointer> {
    tags.find_tag(Tag::SubIfd)
        .ok()
        .flatten()
        .and_then(|value| value.into_ifd_vec().ok())
        .unwrap_or_default()
}

fn tiff_dimensions(path: &Path) -> Option<(u64, u64)> {
    let mut header = [0u8; 4];
    File::open(path).ok()?.read_exact(&mut header).ok()?;
    if !is_tiff_header(&header) {
        return None;
    }
    let mut decoder = Decoder::new(File::open(path).ok()?).ok()?;
    let mut best = None;
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::from([decoder.ifd_pointer()?]);
    while let Some(pointer) = queue.pop_front() {
        if !seen.insert(pointer.0) {
            continue;
        }
        let Ok(directory) = decoder.read_directory(pointer) else {
            continue;
        };
        if let Some(dimensions) = directory_dimensions(&mut decoder, &directory) {
            let pixels = dimensions.0.saturating_mul(dimensions.1);
            if best.is_none_or(|(w, h)| pixels > w * h) {
                best = Some(dimensions);
            }
        }
        let mut tags = decoder.read_directory_tags(&directory);
        queue.extend(sub_directories(&mut tags));
        queue.extend(directory.next());
    }
    best
}

fn image_dimensions(path: &Path) -> Option<(u64, u64)> {
    if let Some(dimensions) = tiff_dimensions(path) {
        return Some(dimensions);
    }
    let (width, height) = image::image_dimensions(path).ok()?;
    (width > 0 && height > 0).then(|| (u64::from(width), u64::from(height)))
}

/// Estimate preview storage and peak loader workspace from headers and file size.
#[must_use]
pub fn estimate_preview_memory(path: &Path, max_edge: u64, profile: &str) -> PreviewMemoryEstimate {
    let dimensions = image_dimensions(path);
    let edge = max_edge.max(1);
    let (cached_bytes, source_pixels, cached_width, cached_height) = match dimensions {
        None => (edge * edge * 3 * 4, 0, edge, edge),
        Some((width, height)) => {
            let scale = (edge as f64 / width.max(height) as f64).min(1.0);
            let cached_width = ((width as f64 * scale).round_ties_even() as u64).max(1);
            let cached_height = ((height as f64 * scale).round_ties_even() as u64).max(1);
            (
                cached_width * cached_height * 3 * 4,
                width.saturating_mul(height),
                cached_width,
                cached_height,
            )
        }
    };
    let file_bytes = std::fs::metadata(path).map_or(0, |meta| meta.len());
    let encoded_bytes = if dimensions.is_some() {
        file_bytes * 2
    } else {
        file_bytes * file_multiplier(profile)
    };
    let temporary_bytes = source_pixels
        .saturating_mul(bytes_per_pixel(profile))
        .max(encoded_bytes)
        .max(floor_bytes(profile));
    let mut cached_bytes = cached_bytes;
    if dimensions.is_none() || source_pixels.saturating_mul(12) > cached_bytes {
        cached_bytes *= 2;
        if profile != "display" {
            cached_bytes += if dimensions.is_some() {
                cached_width * cached_height * 4
            } else {
                edge * edge * 4
            };
        }
    }
    if profile != "display" {
        cached_bytes += if dimensions.is_some() {
            source_pixels * 4
        } else {
            temporary_bytes
        };
    }
    PreviewMemoryEstimate {
        cached_bytes,
        temporary_bytes,
        source_dimensions: dimensions,
    }
}
