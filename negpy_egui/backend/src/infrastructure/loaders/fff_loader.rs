//! Flextight FFF detection.
//!
//! Mirrors `negpy/infrastructure/loaders/fff_loader.py`: the largest directory
//! is the full-resolution image, either 16-bit RGB or SGI LogLuv.

use std::path::Path;

use crate::infrastructure::loaders::tiff_walker::{IfdInfo, read_ifds};

const PHOTOMETRIC_RGB: u16 = 2;
const PHOTOMETRIC_LOGLUV: u16 = 32845;

/// The SGI LogLuv compression codes a Flextight raw uses.
#[must_use]
pub fn is_sgilog_compression(compression: u16) -> bool {
    matches!(compression, 34676 | 34677)
}

fn is_full_res(info: &IfdInfo) -> bool {
    if info.photometric == PHOTOMETRIC_LOGLUV {
        return true;
    }
    info.samples_per_pixel >= 3 && info.photometric == PHOTOMETRIC_RGB && info.bits_per_sample >= 16
}

/// The largest image directory by pixel count, matching the Python FFF rule.
#[must_use]
pub fn find_full_res_ifd(ifds: &[IfdInfo]) -> Option<&IfdInfo> {
    ifds.iter()
        .filter(|info| is_full_res(info))
        .max_by_key(|info| info.pixels())
}

/// True if any directory uses an SGI LogLuv compression.
#[must_use]
pub fn has_sgilog_ifd(ifds: &[IfdInfo]) -> bool {
    ifds.iter()
        .any(|info| is_sgilog_compression(info.compression))
}

/// True if this `.fff` is an Imacon/Hasselblad Flextight scanner file.
#[must_use]
pub fn is_flextight_fff(path: &Path) -> bool {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("fff"))
    {
        return false;
    }
    read_ifds(path)
        .as_deref()
        .and_then(find_full_res_ifd)
        .is_some()
}
