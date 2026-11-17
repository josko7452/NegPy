//! The format chooser for a rendered export buffer.
//!
//! Mirrors the container selection in `_encode_export`: TIFF, PNG and JPEG are encoded
//! here. JPEG XL and WebP need encoders this build does not carry, so they report the
//! block reason instead of writing a different format.

use crate::domain::models::{ExportConfig, ExportFormat};
use crate::services::export::encoders::{
    Resolution, TiffCompression, encode_jpeg, encode_png, encode_tiff, encode_webp_lossless,
};
use crate::services::export::pixels::ExportPixels;

/// The encoded file bytes and its extension.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedExport {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Why a format cannot be written by this build.
#[must_use]
pub fn unsupported_reason(format: ExportFormat) -> Option<&'static str> {
    match format {
        ExportFormat::Jxl => Some("JPEG XL export needs an encoder this build does not carry."),
        _ => None,
    }
}

/// The TIFF compression the encoder supports, with LZW mapped to deflate.
#[must_use]
pub fn tiff_compression(config: &ExportConfig) -> TiffCompression {
    match config.tiff_compression {
        crate::domain::models::TiffCompression::None => TiffCompression::None,
        crate::domain::models::TiffCompression::Lzw
        | crate::domain::models::TiffCompression::Zip => TiffCompression::Deflate,
    }
}

/// A resolution tag from the export's effective DPI.
#[must_use]
pub fn resolution_for(config: &ExportConfig) -> Resolution {
    Resolution {
        x_dpi: crate::services::export::print::effective_dpi(config) as f64,
        y_dpi: crate::services::export::print::effective_dpi(config) as f64,
        unit: 2,
    }
}

/// Encode converted pixels in the configured format.
pub fn encode_export(
    pixels: &ExportPixels,
    config: &ExportConfig,
) -> Result<EncodedExport, String> {
    if let Some(reason) = unsupported_reason(config.export_fmt) {
        return Err(reason.to_string());
    }
    if config.export_fmt == ExportFormat::Webp && !config.webp_lossless {
        return Err(
            "Lossy WebP needs an encoder this build does not carry. Turn on Lossless WebP, or pick PNG or TIFF."
                .to_string(),
        );
    }
    let resolution = resolution_for(config);
    let bytes = match config.export_fmt {
        ExportFormat::Tiff => encode_tiff(
            &pixels.bytes(),
            pixels.width,
            pixels.height,
            pixels.channels,
            u16::from(pixels.depth),
            tiff_compression(config),
            Some(resolution),
            pixels.icc.as_deref(),
        ),
        ExportFormat::Png => encode_png(
            &pixels.bytes(),
            pixels.width,
            pixels.height,
            pixels.channels,
            pixels.depth,
            config.png_compress_level.clamp(0, 9) as u8,
            pixels.icc.as_deref(),
            Some(resolution),
            None,
            None,
        ),
        ExportFormat::Jpeg => encode_jpeg(
            &pixels.samples,
            pixels.width,
            pixels.height,
            pixels.channels,
            config.jpeg_quality.clamp(1, 100) as i32,
            pixels.icc.as_deref(),
            Some((
                resolution.x_dpi.round().max(1.0) as u16,
                resolution.y_dpi.round().max(1.0) as u16,
            )),
        ),
        ExportFormat::Webp => encode_webp_lossless(
            &pixels.samples,
            pixels.width,
            pixels.height,
            pixels.channels,
            pixels.icc.as_deref(),
        ),
        ExportFormat::Jxl => None,
    };
    bytes
        .map(|bytes| EncodedExport {
            bytes,
            extension: extension_for(config.export_fmt),
        })
        .ok_or_else(|| format!("Could not encode a {}.", config.export_fmt.as_str()))
}

/// The file extension the format writes, without its dot.
#[must_use]
pub fn extension_for(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Jpeg => "jpg",
        ExportFormat::Tiff => "tiff",
        ExportFormat::Png => "png",
        ExportFormat::Jxl => "jxl",
        ExportFormat::Webp => "webp",
    }
}

/// Whether the format can be written by this build.
#[must_use]
pub fn can_encode(format: ExportFormat) -> bool {
    unsupported_reason(format).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels() -> ExportPixels {
        ExportPixels {
            samples: vec![10, 128, 240, 20, 100, 200],
            samples16: Vec::new(),
            width: 2,
            height: 1,
            channels: 3,
            depth: 8,
            icc: None,
        }
    }

    #[test]
    fn a_tiff_export_carries_the_extension() {
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Tiff;
        let out = encode_export(&pixels(), &config).expect("tiff");
        assert_eq!(out.extension, "tiff");
        assert_eq!(&out.bytes[..4], b"II*\0");
    }

    #[test]
    fn a_png_and_jpeg_export_start_with_their_signatures() {
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Png;
        let png = encode_export(&pixels(), &config).expect("png");
        assert_eq!(&png.bytes[..8], b"\x89PNG\r\n\x1a\n");
        config.export_fmt = ExportFormat::Jpeg;
        let jpeg = encode_export(&pixels(), &config).expect("jpeg");
        assert_eq!(&jpeg.bytes[..2], &[0xFF, 0xD8]);
        assert_eq!(jpeg.extension, "jpg");
    }

    #[test]
    fn a_lossy_webp_reports_its_reason() {
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Webp;
        assert!(encode_export(&pixels(), &config).is_err());
        config.export_fmt = ExportFormat::Jxl;
        assert!(!can_encode(ExportFormat::Jxl));
        assert!(unsupported_reason(ExportFormat::Tiff).is_none());
        assert!(unsupported_reason(ExportFormat::Webp).is_none());
    }

    #[test]
    fn a_lossless_webp_export_starts_with_its_container() {
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Webp;
        config.webp_lossless = true;
        let webp = encode_export(&pixels(), &config).expect("lossless webp");
        assert_eq!(&webp.bytes[..4], b"RIFF");
        assert_eq!(&webp.bytes[8..12], b"WEBP");
        assert_eq!(webp.extension, "webp");
    }

    #[test]
    fn every_tiff_compression_reaches_the_encoder() {
        let mut config = ExportConfig::default();
        config.export_fmt = ExportFormat::Tiff;
        config.tiff_compression = crate::domain::models::TiffCompression::None;
        assert_eq!(tiff_compression(&config), TiffCompression::None);
        config.tiff_compression = crate::domain::models::TiffCompression::Zip;
        assert_eq!(tiff_compression(&config), TiffCompression::Deflate);
        config.tiff_compression = crate::domain::models::TiffCompression::Lzw;
        assert_eq!(tiff_compression(&config), TiffCompression::Deflate);
    }
}
