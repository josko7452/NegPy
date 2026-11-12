//! A decoded source image: RGB float samples in 0..1, or the original 16-bit samples.

/// A source image ready for the pipeline.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageBuffer {
    pub width: usize,
    pub height: usize,
    /// Scene-linear samples. Empty when only `source_u16` is present.
    pub pixels: Vec<f32>,
    /// Original uint16 RGB samples, kept when the source is already linear.
    pub source_u16: Option<Vec<u16>>,
    /// Source-to-working primaries carried to the engines.
    pub rgb_xyz_matrix: Option<[[f32; 3]; 3]>,
}

/// The metadata a loader hands to the pipeline with its pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadedImage {
    pub image: Option<ImageBuffer>,
    pub orientation: u8,
    pub color_space: Option<String>,
    pub icc_profile: Option<Vec<u8>>,
    /// The normalized IR plane, when the file carries one.
    pub ir: Option<Vec<f32>>,
    pub ir_width: usize,
    pub ir_height: usize,
    /// What Source Encoding did on a Positive frame.
    pub source_encoding: Option<String>,
}

impl ImageBuffer {
    /// A float RGB buffer with `width * height * 3` samples.
    #[must_use]
    pub fn from_rgb(width: usize, height: usize, pixels: Vec<f32>) -> Self {
        Self {
            width,
            height,
            pixels,
            source_u16: None,
            rgb_xyz_matrix: None,
        }
    }

    /// A uint16 RGB buffer with no float samples yet.
    #[must_use]
    pub fn from_rgb16(width: usize, height: usize, source_u16: Vec<u16>) -> Self {
        Self {
            width,
            height,
            pixels: Vec::new(),
            source_u16: Some(source_u16),
            rgb_xyz_matrix: None,
        }
    }

    /// Whether the float buffer holds `width * height * 3` samples.
    #[must_use]
    pub fn is_rgb(&self, width: usize, height: usize) -> bool {
        self.width == width && self.height == height && self.pixels.len() == width * height * 3
    }
}
