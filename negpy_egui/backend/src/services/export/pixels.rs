//! Export pixel conversion: float buffer to integer samples, then to the target profile.
//!
//! Mirrors `_export_pixels`, the greyscale re-encode and the color-management helpers in
//! `negpy/services/rendering/image_processor.py`. Pixel quantisation happens once at the
//! export bit depth; the profile pair is applied afterwards.

use std::path::{Path, PathBuf};

use crate::domain::models::ExportConfig;
use crate::infrastructure::display::color_mgmt::{icc_enabled_file, profile_bytes};
use crate::infrastructure::display::matrix_trc::{apply_profile_pair_u8, apply_profile_pair_u16};
use crate::kernel::image::logic::{float_to_uint8, float_to_uint16, float_to_uint16_luma};

/// The working space the render ends in.
pub const WORKING_COLOR_SPACE: &str =
    crate::infrastructure::display::color_mgmt::WORKING_COLOR_SPACE;
/// The greyscale target space.
pub const GREYSCALE_COLOR_SPACE: &str = "Greyscale";

/// One exported raster: its samples and the profile that describes them.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPixels {
    /// Row-major samples: RGB at `channels == 3`, luma at 1.
    pub samples: Vec<u8>,
    /// The same samples, at 16 bits, when the depth is 16.
    pub samples16: Vec<u16>,
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub depth: u8,
    /// The profile bytes the encoder tags the file with.
    pub icc: Option<Vec<u8>>,
}

impl ExportPixels {
    /// The 16-bit codes when this is a 16-bit export, otherwise the 8-bit codes.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        if self.depth == 16 {
            self.samples16
                .iter()
                .flat_map(|value| value.to_ne_bytes())
                .collect()
        } else {
            self.samples.clone()
        }
    }
}

/// Resolve a custom profile path, or a named space's bundled profile.
#[must_use]
pub fn profile_for(name: &str, custom: Option<&Path>, user_icc_dir: &Path) -> Option<Vec<u8>> {
    if let Some(path) = custom.filter(|path| icc_enabled_file(path)) {
        return std::fs::read(path).ok();
    }
    profile_bytes(name, user_icc_dir).map(|profile| profile.bytes)
}

/// The target profile an export tags, with the same source and destination fallback as
/// the Python helpers: an unreadable destination leaves the pixels in the working space.
#[must_use]
pub fn target_profile(
    color_space: &str,
    icc_output: Option<&Path>,
    user_icc_dir: &Path,
) -> (String, Option<Vec<u8>>) {
    let (resolved, bytes) = if let Some(path) = icc_output.filter(|path| icc_enabled_file(path)) {
        (color_space.to_string(), std::fs::read(path).ok())
    } else {
        (
            color_space.to_string(),
            profile_bytes(color_space, user_icc_dir).map(|profile| profile.bytes),
        )
    };
    if bytes.is_none() && profile_bytes(color_space, user_icc_dir).is_none() {
        return (
            WORKING_COLOR_SPACE.to_string(),
            profile_bytes(WORKING_COLOR_SPACE, user_icc_dir).map(|profile| profile.bytes),
        );
    }
    (resolved, bytes)
}

/// The export depth after the format's own limit: JPEG and WebP stay at 8 bits.
#[must_use]
pub fn export_depth(config: &ExportConfig) -> u8 {
    match config.export_fmt {
        crate::domain::models::ExportFormat::Jpeg | crate::domain::models::ExportFormat::Webp => 8,
        _ => {
            if config.export_bit_depth >= 16 {
                16
            } else {
                8
            }
        }
    }
}

/// Convert a float export buffer and color-manage it to the target space.
///
/// The buffer arrives in the working space. A custom input profile is used as the source
/// and a custom output profile as the destination; otherwise the named spaces resolve to
/// their bundled profiles.
#[must_use]
pub fn export_pixels(
    buffer: &[f32],
    width: usize,
    height: usize,
    config: &ExportConfig,
    color_space: &str,
    icc_input: Option<&Path>,
    icc_output: Option<&Path>,
    user_icc_dir: &Path,
) -> ExportPixels {
    let (color_space, target_icc) = target_profile(color_space, icc_output, user_icc_dir);
    let greyscale = color_space == GREYSCALE_COLOR_SPACE;
    let depth = export_depth(config);
    if depth == 16 {
        export_pixels_16(
            buffer,
            width,
            height,
            greyscale,
            &color_space,
            icc_input,
            target_icc,
            user_icc_dir,
        )
    } else {
        export_pixels_8(
            buffer,
            width,
            height,
            greyscale,
            &color_space,
            icc_input,
            target_icc,
            user_icc_dir,
        )
    }
}

fn source_profile(
    working_color_space: &str,
    icc_input: Option<&Path>,
    user_icc_dir: &Path,
) -> Option<Vec<u8>> {
    if let Some(path) = icc_input.filter(|path| icc_enabled_file(path)) {
        return std::fs::read(path).ok();
    }
    profile_bytes(working_color_space, user_icc_dir).map(|profile| profile.bytes)
}

fn export_pixels_16(
    buffer: &[f32],
    width: usize,
    height: usize,
    greyscale: bool,
    color_space: &str,
    icc_input: Option<&Path>,
    target_icc: Option<Vec<u8>>,
    user_icc_dir: &Path,
) -> ExportPixels {
    if greyscale {
        let luma = float_to_uint16_luma(buffer, 3);
        let samples = grey_reencode_u16(&luma, WORKING_COLOR_SPACE, color_space);
        return ExportPixels {
            samples: Vec::new(),
            samples16: samples,
            width,
            height,
            channels: 1,
            depth: 16,
            icc: target_icc,
        };
    }
    let codes = float_to_uint16(buffer);
    let (samples16, icc) = if let (Some(source), Some(dest)) = (
        source_profile(WORKING_COLOR_SPACE, icc_input, user_icc_dir),
        target_icc.clone(),
    ) {
        if color_space == WORKING_COLOR_SPACE && icc_input.is_none() {
            (codes, target_icc)
        } else if let Some(converted) = apply_profile_pair_u16(&codes, &source, &dest) {
            (converted, target_icc)
        } else {
            (
                codes,
                source_profile(WORKING_COLOR_SPACE, icc_input, user_icc_dir),
            )
        }
    } else {
        (codes, target_icc)
    };
    ExportPixels {
        samples: Vec::new(),
        samples16,
        width,
        height,
        channels: 3,
        depth: 16,
        icc,
    }
}

fn export_pixels_8(
    buffer: &[f32],
    width: usize,
    height: usize,
    greyscale: bool,
    color_space: &str,
    icc_input: Option<&Path>,
    target_icc: Option<Vec<u8>>,
    user_icc_dir: &Path,
) -> ExportPixels {
    if greyscale {
        let luma = float_to_uint16_luma(buffer, 3);
        let codes = grey_reencode_u16(&luma, WORKING_COLOR_SPACE, color_space);
        let samples = codes.iter().map(|value| (value / 257) as u8).collect();
        return ExportPixels {
            samples,
            samples16: Vec::new(),
            width,
            height,
            channels: 1,
            depth: 8,
            icc: target_icc,
        };
    }
    let codes = float_to_uint8(buffer);
    let (samples, icc) = if let (Some(source), Some(dest)) = (
        source_profile(WORKING_COLOR_SPACE, icc_input, user_icc_dir),
        target_icc.clone(),
    ) {
        if color_space == WORKING_COLOR_SPACE && icc_input.is_none() {
            (codes, target_icc)
        } else if let Some(converted) = apply_profile_pair_u8(&codes, &source, &dest) {
            (converted, target_icc)
        } else {
            (
                codes,
                source_profile(WORKING_COLOR_SPACE, icc_input, user_icc_dir),
            )
        }
    } else {
        (codes, target_icc)
    };
    ExportPixels {
        samples,
        samples16: Vec::new(),
        width,
        height,
        channels: 3,
        depth: 8,
        icc,
    }
}

/// `_greyscale_to_u8` / `_apply_color_management_u16_greyscale`: luma in the working TRC
/// re-encoded through the sRGB OETF when the target is a grey profile.
#[must_use]
pub fn grey_reencode_u16(luma: &[u16], working_color_space: &str, color_space: &str) -> Vec<u16> {
    if working_color_space == color_space {
        return luma.to_vec();
    }
    let lin = working_oetf_decode_u16(luma);
    let gray = encode_srgb(&lin);
    gray.iter()
        .map(|value| (value * 65535.0 + 0.5).clamp(0.0, 65535.0) as u16)
        .collect()
}

fn working_oetf_decode_u16(codes: &[u16]) -> Vec<f32> {
    let mut out = Vec::with_capacity(codes.len());
    for value in codes {
        out.push((f64::from(*value) / 65535.0).powf(563.0 / 256.0) as f32);
    }
    out
}

fn encode_srgb(linear: &[f32]) -> Vec<f32> {
    linear
        .iter()
        .map(|value| {
            let value = value.clamp(0.0, 1.0) as f64;
            if value <= 0.003_130_8 {
                (value * 12.92) as f32
            } else {
                (1.055 * value.powf(1.0 / 2.4) - 0.055) as f32
            }
        })
        .collect()
}

/// The `_u16_to_u8_lut`: every uint16 code rounded to its nearest uint8 code.
#[must_use]
pub fn u16_to_u8_lut() -> Vec<u8> {
    (0..=65535u16)
        .map(|value| ((f32::from(value) / 257.0).round().clamp(0.0, 255.0)) as u8)
        .collect()
}

/// A custom profile path that exists and is readable.
#[must_use]
pub fn readable_profile(path: &Path) -> Option<PathBuf> {
    icc_enabled_file(path).then(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grey_reencode_matches_the_srgb_oetf() {
        let luma = [0u16, 32768, 65535];
        let out = grey_reencode_u16(&luma, WORKING_COLOR_SPACE, GREYSCALE_COLOR_SPACE);
        assert_eq!(out[0], 0);
        assert_eq!(out[2], 65535);
        assert!(out[1] > 30000 && out[1] < 36000, "{out:?}");
    }

    #[test]
    fn a_same_space_grey_reencode_is_identity() {
        assert_eq!(
            grey_reencode_u16(&[1, 2, 3], "Greyscale", "Greyscale"),
            [1, 2, 3]
        );
    }

    #[test]
    fn the_u16_to_u8_table_rounds_to_the_nearest_code() {
        let lut = u16_to_u8_lut();
        assert_eq!(lut.len(), 65536);
        assert_eq!(lut[0], 0);
        assert_eq!(lut[65535], 255);
        assert_eq!(lut[32768], 128);
    }

    #[test]
    fn a_float_buffer_quantises_at_the_export_depth() {
        let mut config = ExportConfig::default();
        config.export_bit_depth = 16;
        config.export_fmt = crate::domain::models::ExportFormat::Tiff;
        let pixels = export_pixels(
            &[0.0, 0.5, 1.0],
            1,
            1,
            &config,
            WORKING_COLOR_SPACE,
            None,
            None,
            Path::new("/nonexistent"),
        );
        assert_eq!(pixels.depth, 16);
        assert_eq!(pixels.channels, 3);
        assert_eq!(pixels.samples16, [0, 32768, 65535]);
    }

    #[test]
    fn jpeg_ignores_a_sixteen_bit_setting() {
        let mut config = ExportConfig::default();
        config.export_bit_depth = 16;
        config.export_fmt = crate::domain::models::ExportFormat::Jpeg;
        let pixels = export_pixels(
            &[0.0, 0.5, 1.0],
            1,
            1,
            &config,
            "sRGB",
            None,
            None,
            Path::new("/nonexistent"),
        );
        assert_eq!(pixels.depth, 8);
        assert_eq!(pixels.channels, 3);
        assert_eq!(pixels.samples[0], 0);
        assert_eq!(pixels.samples[2], 255);
    }
}
