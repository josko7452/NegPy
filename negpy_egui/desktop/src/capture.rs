//! Screenshot capture for the golden comparison harness.
//!
//! The screenshot arrives as an [`egui::Event::Screenshot`] on a later frame. The caller
//! requests it, then hands the event image to [`write_png`] after checking its size.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{Color32, ColorImage, Event, UserData, ViewportCommand};

/// One requested capture and the size the manifest expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureSize {
    pub width: usize,
    pub height: usize,
}

/// The integer factor from a physical capture to the logical size, when exact.
#[must_use]
pub fn proportional_factor(image: &ColorImage, expected: CaptureSize) -> Option<usize> {
    if expected.width == 0 || expected.height == 0 {
        return None;
    }
    if image.size[0] % expected.width != 0 || image.size[1] % expected.height != 0 {
        return None;
    }
    let x = image.size[0] / expected.width;
    let y = image.size[1] / expected.height;
    (x == y && x >= 1).then_some(x)
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

/// Scale a screenshot down by an integer factor, row by row.
#[must_use]
pub fn downscale(image: &ColorImage, factor: usize) -> ColorImage {
    if factor <= 1 {
        return image.clone();
    }
    let width = image.size[0] / factor;
    let height = image.size[1] / factor;
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            pixels.push(image.pixels[(y * factor) * image.size[0] + x * factor]);
        }
    }
    ColorImage::new([width, height], pixels)
}

/// Scale a screenshot to an exact size by averaging each source block.
#[must_use]
pub fn resize(image: &ColorImage, width: usize, height: usize) -> ColorImage {
    if image.size == [width, height] || width == 0 || height == 0 {
        return image.clone();
    }
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        let y0 = y * image.size[1] / height;
        let y1 = ((y + 1) * image.size[1] / height).max(y0 + 1);
        for x in 0..width {
            let x0 = x * image.size[0] / width;
            let x1 = ((x + 1) * image.size[0] / width).max(x0 + 1);
            let mut sums = [0u64; 4];
            let mut count = 0u64;
            for sy in y0..y1.min(image.size[1]) {
                for sx in x0..x1.min(image.size[0]) {
                    let pixel = image.pixels[sy * image.size[0] + sx];
                    sums[0] += u64::from(pixel.r());
                    sums[1] += u64::from(pixel.g());
                    sums[2] += u64::from(pixel.b());
                    sums[3] += u64::from(pixel.a());
                    count += 1;
                }
            }
            let divisor = count.max(1);
            pixels.push(Color32::from_rgba_unmultiplied(
                (sums[0] / divisor) as u8,
                (sums[1] / divisor) as u8,
                (sums[2] / divisor) as u8,
                (sums[3] / divisor) as u8,
            ));
        }
    }
    ColorImage::new([width, height], pixels)
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

    #[test]
    fn a_physical_capture_must_scale_by_one_integer() {
        let expected = CaptureSize {
            width: 4,
            height: 3,
        };
        let exact = ColorImage::new([4, 3], vec![Color32::BLACK; 12]);
        assert_eq!(proportional_factor(&exact, expected), Some(1));
        let hidpi = ColorImage::new([8, 6], vec![Color32::BLACK; 48]);
        assert_eq!(proportional_factor(&hidpi, expected), Some(2));
        let squashed = ColorImage::new([8, 4], vec![Color32::BLACK; 32]);
        assert_eq!(proportional_factor(&squashed, expected), None);
    }

    #[test]
    fn a_hidpi_capture_scales_to_its_logical_size() {
        let pixels = (0..16).map(Color32::from_gray).collect();
        let image = ColorImage::new([4, 4], pixels);
        let scaled = downscale(&image, 2);
        assert_eq!(scaled.size, [2, 2]);
        assert_eq!(
            scaled.pixels,
            vec![
                Color32::from_gray(0),
                Color32::from_gray(2),
                Color32::from_gray(8),
                Color32::from_gray(10)
            ]
        );
    }
}
