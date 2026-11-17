//! Screenshot capture for the golden comparison harness.
//!
//! The screenshot arrives as an [`egui::Event::Screenshot`] on a later frame. The caller
//! requests it, then hands the event image to [`write_png`] after checking its size.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{ColorImage, Event, UserData, ViewportCommand};

/// One requested capture and the size the manifest expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureSize {
    pub width: usize,
    pub height: usize,
}

impl CaptureSize {
    /// The golden window size.
    pub const GOLDEN: Self = Self {
        width: 1920,
        height: 1600,
    };

    /// Whether a screenshot matches this size.
    #[must_use]
    pub fn accepts(self, image: &ColorImage) -> bool {
        image.size == [self.width, self.height]
    }
}

/// The output path and expected size of one golden capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRequest {
    pub path: String,
    pub size: CaptureSize,
}

/// Ask egui for a screenshot on the next frame.
pub fn request(ctx: &eframe::egui::Context) {
    ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
}

/// Take the first screenshot event in this frame.
#[must_use]
pub fn take_screenshot(ctx: &eframe::egui::Context) -> Option<Arc<ColorImage>> {
    ctx.input(|input| {
        input.events.iter().find_map(|event| {
            if let Event::Screenshot { image, .. } = event {
                Some(Arc::clone(image))
            } else {
                None
            }
        })
    })
}

/// Write a screenshot as an 8-bit RGB PNG. The caller rejects a size mismatch first.
pub fn write_png(image: &ColorImage, path: &Path) -> Result<(), String> {
    if image.size[0] == 0 || image.size[1] == 0 {
        return Err("The screenshot is empty.".to_string());
    }
    if image.pixels.len() != image.size[0] * image.size[1] {
        return Err("The screenshot pixel count does not match its size.".to_string());
    }
    let mut rgb = Vec::with_capacity(image.pixels.len() * 3);
    for pixel in &image.pixels {
        rgb.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
    }
    image::save_buffer(
        path,
        &rgb,
        image.size[0] as u32,
        image.size[1] as u32,
        image::ColorType::Rgb8,
    )
    .map_err(|error| format!("Could not write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    #[test]
    fn a_capture_size_matches_only_its_own_pixels() {
        let image = ColorImage::new([4, 3], vec![Color32::BLACK; 12]);
        assert!(
            CaptureSize {
                width: 4,
                height: 3
            }
            .accepts(&image)
        );
        assert!(
            !CaptureSize {
                width: 3,
                height: 4
            }
            .accepts(&image)
        );
    }

    #[test]
    fn a_png_round_trips_its_rgb_pixels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.png");
        let pixels = vec![
            Color32::from_rgb(12, 34, 56),
            Color32::from_rgb(210, 200, 190),
        ];
        let image = ColorImage::new([2, 1], pixels);
        write_png(&image, &path).unwrap();
        let decoded = image::open(&path).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.as_raw(), &[12, 34, 56, 210, 200, 190]);
    }
}
