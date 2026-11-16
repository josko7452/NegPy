//! The edge print's fixed face and its measured text runs.

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont, point};

/// The bundled face, the same on every platform.
pub const FONT_BYTES: &[u8] = include_bytes!("../../../assets/LiberationSans-Regular.ttf");
/// Aileron's capitals are 0.70 em tall; the edge layouts size text by cap height.
pub const EDGE_CAP_EM: f64 = 0.70;

/// A measured line of text at one size.
#[derive(Clone, Debug)]
pub struct TextRun {
    pub font: FontArc,
    pub size: f32,
    pub text: String,
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    /// The glyphs with their placement relative to the line origin.
    pub glyphs: Vec<(Glyph, f32)>,
}

impl TextRun {
    /// A line sized so its capitals are `cap` pixels tall.
    #[must_use]
    pub fn capped(text: &str, cap: f64) -> Self {
        let font = FontArc::try_from_slice(FONT_BYTES).expect("the bundled face parses");
        let size = (cap / EDGE_CAP_EM).max(1.0) as f32;
        Self::new(font, text, size)
    }

    /// A line at an explicit pixel size.
    #[must_use]
    pub fn new(font: FontArc, text: &str, size: f32) -> Self {
        let scaled = font.as_scaled(PxScale::from(size));
        let mut glyphs = Vec::new();
        let mut caret = 0.0f32;
        let mut previous = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(last) = previous {
                caret += scaled.kern(last, id);
            }
            let glyph = id.with_scale_and_position(size, point(caret, 0.0));
            glyphs.push((glyph, caret));
            caret += scaled.h_advance(id);
            previous = Some(id);
        }
        Self {
            ascent: scaled.ascent(),
            descent: scaled.descent(),
            width: caret,
            font,
            size,
            text: text.to_string(),
            glyphs,
        }
    }

    /// The line resized to a narrower width, keeping its height.
    #[must_use]
    pub fn narrowed(&self, width: f32) -> Self {
        if self.width <= 0.0 || width <= 0.0 || width >= self.width {
            return self.clone();
        }
        let scale = width / self.width;
        let mut glyphs = Vec::new();
        for (glyph, x) in &self.glyphs {
            let mut placed = glyph.clone();
            placed.position.x = x * scale;
            glyphs.push((placed, x * scale));
        }
        Self {
            width,
            glyphs,
            ..self.clone()
        }
    }

    /// Each character in its own `cell`-wide slot, centered; `0` leaves the run unchanged.
    #[must_use]
    pub fn monospaced(&self, cell: f32) -> Self {
        if cell <= 0.0 || self.text.is_empty() {
            return self.clone();
        }
        let mut glyphs = Vec::new();
        let mut caret = 0.0f32;
        for (index, ch) in self.text.chars().enumerate() {
            if ch.is_whitespace() {
                caret = (index + 1) as f32 * cell;
                continue;
            }
            let id = self.font.as_scaled(PxScale::from(self.size)).glyph_id(ch);
            let advance = self.font.as_scaled(PxScale::from(self.size)).h_advance(id);
            let left = (index as f32 + 0.5) * cell - advance / 2.0;
            let glyph = id.with_scale_and_position(self.size, point(left, 0.0));
            glyphs.push((glyph, left));
            caret = left + advance;
        }
        Self {
            width: caret.max(self.text.chars().count() as f32 * cell),
            glyphs,
            ..self.clone()
        }
    }
}

/// A uint8 RGB canvas the strip painter writes into.
#[derive(Clone, Debug, PartialEq)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Canvas {
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 3],
        }
    }

    pub fn set(&mut self, x: usize, y: usize, color: [u8; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let at = (y * self.width + x) * 3;
        self.pixels[at..at + 3].copy_from_slice(&color);
    }

    /// Blend `color` over `(x0, y0, x1, y1)` by `coverage`, where coverage is 0..1.
    pub fn blend(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, coverage: &[f32], color: [u8; 3]) {
        let w = (x1 - x0) as usize;
        let h = (y1 - y0) as usize;
        if w == 0 || h == 0 || coverage.len() != w * h {
            return;
        }
        for row in 0..h {
            for column in 0..w {
                let x = x0 + column as i64;
                let y = y0 + row as i64;
                if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
                    continue;
                }
                let alpha = coverage[row * w + column].clamp(0.0, 1.0);
                let at = (y as usize * self.width + x as usize) * 3;
                for (channel, value) in color.iter().enumerate() {
                    self.pixels[at + channel] = (self.pixels[at + channel] as f32 * (1.0 - alpha)
                        + *value as f32 * alpha
                        + 0.5)
                        .clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_measures_its_width_and_cap_height() {
        let run = TextRun::capped("KODAK 400", 1.4);
        assert!(run.width > 0.0);
        assert!(run.ascent >= run.descent);
        assert_eq!(run.text, "KODAK 400");
    }

    #[test]
    fn a_monospaced_line_gives_each_character_one_cell() {
        let run = TextRun::capped("ABC", 1.0).monospaced(10.0);
        assert!((run.width - 30.0).abs() < 1.0);
        assert_eq!(run.glyphs.len(), 3);
    }
}
