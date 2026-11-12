//! One entry point over the format loaders.
//!
//! Mirrors `negpy/infrastructure/loaders/factory.py`: the extension and the
//! scanner headers select a loader, and the caller gets its decoded image.

use std::path::Path;

use crate::domain::models::SourceEncoding;
use crate::infrastructure::loaders::image_buffer::LoadedImage;
use crate::infrastructure::loaders::raw_decoder::{RawDecoder, RawlerDecoder};
use crate::infrastructure::loaders::source_kind::{LoaderKind, select_loader};
use crate::infrastructure::loaders::{
    fff_loader, jpeg_loader, jxl_loader, nef_loader, noritsu_loader, pakon_loader, tiff_loader,
};

/// The choices a caller passes to every loader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadOptions {
    /// Keep an already linear 16-bit source in its samples.
    pub linear_raw: bool,
    /// Decode a Positive source by this encoding; None opens a Negative.
    pub positive_decode: Option<SourceEncoding>,
    /// Read an IR sidecar or a fourth TIFF sample.
    pub load_ir: bool,
}

impl LoadOptions {
    /// The defaults of the Python loader call.
    #[must_use]
    pub fn new() -> Self {
        Self {
            linear_raw: false,
            positive_decode: None,
            load_ir: true,
        }
    }
}

/// Whether `path` decodes as a camera RAW rather than a finished image or scan.
#[must_use]
pub fn is_camera_raw(path: &Path) -> bool {
    select_loader(path) == LoaderKind::CameraRaw
}

/// Decode `path` with the loader its extension and contents select.
pub fn load(path: &Path, options: LoadOptions) -> Result<LoadedImage, String> {
    let decoded = match select_loader(path) {
        LoaderKind::Tiff => tiff_loader::load(
            path,
            options.linear_raw,
            options.positive_decode,
            options.load_ir,
        ),
        LoaderKind::Jpeg => jpeg_loader::load(path, options.positive_decode),
        LoaderKind::Jxl => jxl_loader::load(path),
        LoaderKind::Pakon => pakon_loader::load(path),
        LoaderKind::Noritsu => noritsu_loader::load(path),
        LoaderKind::CoolscanNef => nef_loader::load(path),
        LoaderKind::FlextightFff => fff_loader::load(path),
        LoaderKind::CameraRaw => return RawlerDecoder.decode(path),
    };
    decoded.ok_or_else(|| format!("Could not decode {}.", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_file_selects_by_its_extension() {
        for name in ["a.tif", "a.jpg", "a.jxl"] {
            assert!(!is_camera_raw(Path::new(name)), "{name}");
        }
        assert!(is_camera_raw(Path::new("a.dng")));
    }

    #[test]
    fn the_defaults_read_ir() {
        let options = LoadOptions::new();
        assert!(options.load_ir);
        assert!(!options.linear_raw);
        assert_eq!(options.positive_decode, None);
    }
}
