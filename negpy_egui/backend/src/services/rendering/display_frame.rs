//! The display frame a preview shows: the pipeline output cropped to its active ROI
//! and quantized to 8-bit RGB.
//!
//! The GUI keeps a texture, not a float buffer. This module is the one place that
//! turns a linear source into those pixels, so the capture harness and the app share
//! the same path.

use crate::domain::models::WorkspaceConfig;
use crate::infrastructure::loaders::image_buffer::ImageBuffer;
use crate::kernel::image::logic::float_to_uint8;
use crate::render_engine::{DarkroomEngine, PipelineContext};

/// One 8-bit RGB frame ready for a texture upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayFrame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl DisplayFrame {
    /// A frame with `width * height * 3` samples.
    #[must_use]
    pub fn from_rgb(width: usize, height: usize, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Whether the sample count matches the stated size.
    #[must_use]
    pub fn is_rgb(&self) -> bool {
        self.pixels.len() == self.width * self.height * 3
    }
}

/// Run the pipeline and return the display-encoded frame.
///
/// `source_hash` identifies the decoded source; a different hash clears the engine's
/// checkpoints. The caller keeps the engine across renders so a slider only reruns the
/// stages below it.
pub fn render_display(
    engine: &mut DarkroomEngine,
    image: &ImageBuffer,
    config: &WorkspaceConfig,
    source_hash: &str,
) -> DisplayFrame {
    let mut context = PipelineContext::new();
    let output = engine.process(image, config, source_hash, &mut context);
    let pixels = float_to_uint8(&output.pixels);
    DisplayFrame::from_rgb(output.width, output.height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn negative(width: usize, height: usize) -> ImageBuffer {
        let mut pixels = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                let scene = 0.05 + 0.8 * (y as f32 / height as f32);
                let texture = 1.0 + 0.1 * (x as f32 / 4.0).sin();
                let value = (0.05 + 0.8 * (1.0 - (scene * texture).powf(0.4))).clamp(1e-4, 1.0);
                pixels.extend_from_slice(&[value, value * 0.9, value * 0.8]);
            }
        }
        ImageBuffer::from_rgb(width, height, pixels)
    }

    #[test]
    fn a_display_frame_is_rgb_and_matches_its_size() {
        let image = negative(40, 30);
        let config = WorkspaceConfig::default();
        let mut engine = DarkroomEngine::new();
        let frame = render_display(&mut engine, &image, &config, "source");
        assert!(frame.is_rgb());
        assert!(frame.width > 0 && frame.height > 0);
        assert!(frame.pixels.iter().any(|value| *value > 0));
    }

    #[test]
    fn a_crop_shortens_the_display_frame() {
        let image = negative(60, 40);
        let mut config = WorkspaceConfig::default();
        config.geometry.crop_rect = Some(vec![0.1, 0.1, 0.9, 0.9]);
        let mut engine = DarkroomEngine::new();
        let frame = render_display(&mut engine, &image, &config, "source");
        assert!(frame.width < 60);
        assert!(frame.height <= 40);
    }
}
