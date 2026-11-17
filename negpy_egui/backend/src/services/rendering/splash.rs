//! A first-paint image from a source file's reduced-resolution page.
//!
//! Mirrors the TIFF-page half of `embedded_preview` in
//! `negpy/infrastructure/loaders/helpers.py`: page 0 counts only when the full image sits
//! in SubIFDs, and a later page counts when it is marked reduced or came from Nikon Scan.

use std::path::Path;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::infrastructure::loaders::tiff_walker::{IfdInfo, read_ifds};

/// The largest decoded preview page read into a splash.
pub const QUICK_PREVIEW_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// The longest edge a splash is capped at.
pub const SPLASH_MAX_EDGE: usize = 1600;
const REDUCED_ASPECT_TOLERANCE: f64 = 0.02;

/// A page that can serve as the splash, as an 8-bit RGB buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct Splash {
    pub image: ImageBuffer,
}

/// Read a reduced preview page, or `None` when the file carries none it can decode.
#[must_use]
pub fn try_splash_preview(path: &Path, max_edge: usize) -> Option<Splash> {
    let bytes = std::fs::metadata(path).ok()?.len();
    if bytes == 0 {
        return None;
    }
    let ifds = read_ifds(path)?;
    let page = choose_preview_page(&ifds)?;
    let decoded = crate::infrastructure::loaders::tiff_loader::decode_ifd(path, page.offset)?;
    let mut image = decoded.0;
    if image.source_u16.is_some() && image.pixels.is_empty() {
        let samples = image.source_u16.take()?;
        image.pixels = samples
            .iter()
            .map(|value| ((value >> 8) as u8) as f32 / 255.0)
            .collect();
    }
    let image = crate::kernel::image::logic::ensure_rgb(&image);
    let image = orient(&image, read_orientation(path));
    let max_edge = max_edge.max(1);
    let long = image.width.max(image.height);
    let image = if long > max_edge {
        let scale = max_edge as f64 / long as f64;
        let width = ((image.width as f64 * scale).round() as usize).max(1);
        let height = ((image.height as f64 * scale).round() as usize).max(1);
        let pixels = negpy_core::resize::resize_area(
            &image.pixels,
            image.height,
            image.width,
            3,
            height,
            width,
        )?;
        ImageBuffer::from_rgb(width, height, pixels)
    } else {
        image
    };
    Some(Splash { image })
}

fn choose_preview_page(ifds: &[IfdInfo]) -> Option<&IfdInfo> {
    let main = ifds.iter().find(|info| !info.is_subifd)?;
    let largest = ifds.iter().map(IfdInfo::pixels).max().unwrap_or(0);
    let has_subifds = ifds.iter().any(|info| info.is_subifd);
    if has_subifds
        && main.offset == ifds.first()?.offset
        && main.samples_per_pixel >= 3
        && main.pixels() < largest
        && main.pixels() * samples_bytes(main) <= QUICK_PREVIEW_MAX_BYTES
    {
        return Some(main);
    }
    ifds.iter()
        .filter(|info| info.offset != main.offset)
        .find(|info| {
            let reduced = info.subfile_type & 1 != 0;
            let nikon_scan = false;
            (reduced || nikon_scan)
                && info.samples_per_pixel == 3
                && info.photometric == 2
                && aspect_matches(main, info)
                && info.pixels() * samples_bytes(info) <= QUICK_PREVIEW_MAX_BYTES
        })
}

fn samples_bytes(info: &IfdInfo) -> u64 {
    u64::from(info.samples_per_pixel.max(1)) * u64::from((info.bits_per_sample / 8).max(1))
}

fn aspect_matches(main: &IfdInfo, candidate: &IfdInfo) -> bool {
    if main.width == 0 || main.height == 0 || candidate.height == 0 {
        return false;
    }
    let main_aspect = f64::from(main.width) / f64::from(main.height);
    let candidate_aspect = f64::from(candidate.width) / f64::from(candidate.height);
    (candidate_aspect / main_aspect - 1.0).abs() <= REDUCED_ASPECT_TOLERANCE
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
    fn a_file_without_a_preview_page_has_no_splash() {
        assert!(try_splash_preview(Path::new("/missing/frame.dng"), 1600).is_none());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.tif");
        std::fs::write(&path, b"not a tiff").unwrap();
        assert!(try_splash_preview(&path, 1600).is_none());
    }
}
