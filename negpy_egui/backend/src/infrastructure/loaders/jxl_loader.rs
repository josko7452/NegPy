//! JPEG XL scan loading.
//!
//! Mirrors `negpy/infrastructure/loaders/jxl_loader.py`: sensor samples stay in
//! their own encoding, a gray image is promoted to RGB, and the fourth channel
//! is dropped.

use std::path::Path;

use jxl_oxide::JxlImage;
use jxl_oxide::image::BitDepth;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::{ImageBuffer, LoadedImage};

/// Load a JPEG XL image.
#[must_use]
pub fn load(path: &Path) -> Option<LoadedImage> {
    let image = JxlImage::open_with_defaults(path).ok()?;
    let render = image.render_frame(0).ok()?;
    let width = render.stream().width() as usize;
    let height = render.stream().height() as usize;
    let channels = image.pixel_format().channels();
    let depth = image.image_header().metadata.bit_depth;
    let mut samples = vec![0f32; width * height * channels];
    let mut stream = render.stream_no_alpha();
    let written = stream.write_to_buffer(&mut samples);
    if written != samples.len() {
        return None;
    }

    let pixels: Vec<f32> = samples
        .chunks_exact(channels)
        .flat_map(|sample| {
            [
                sample[0],
                sample[1.min(channels - 1)],
                sample[2.min(channels - 1)],
            ]
        })
        .collect();
    let mut image = ImageBuffer::from_rgb(width, height, pixels);
    if is_sixteen_bit(depth) {
        let source_u16: Vec<u16> = image
            .pixels
            .iter()
            .map(|value| (value.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
            .collect();
        image = ImageBuffer::from_rgb16(width, height, source_u16);
    }

    Some(LoadedImage {
        image: Some(image),
        orientation: read_orientation(path),
        color_space: None,
        ..LoadedImage::default()
    })
}

fn is_sixteen_bit(depth: BitDepth) -> bool {
    matches!(
        depth,
        BitDepth::IntegerSample {
            bits_per_sample: 16
        }
    )
}
