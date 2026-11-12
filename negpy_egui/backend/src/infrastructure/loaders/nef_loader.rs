//! Coolscan NEF detection.
//!
//! Mirrors `negpy/infrastructure/loaders/nef_loader.py`: a Coolscan writes the
//! processed RGB image into a SubIFD chain, and its file carries no CFA data.

use std::path::Path;

use crate::infrastructure::loaders::helpers::read_orientation;
use crate::infrastructure::loaders::image_buffer::LoadedImage;
use crate::infrastructure::loaders::tiff_loader::decode_ifd;
use crate::infrastructure::loaders::tiff_walker::{IfdInfo, read_ifds};

const PHOTOMETRIC_RGB: u16 = 2;
const PHOTOMETRIC_CFA: u16 = 32803;
const COMPRESSION_NIKON: u16 = 34713;

fn is_cfa(info: &IfdInfo) -> bool {
    info.photometric == PHOTOMETRIC_CFA
        || info.compression == COMPRESSION_NIKON
        || (info.samples_per_pixel == 1 && info.pixels() > 100_000)
}

/// The largest RGB SubIFD of a scanner NEF, if one exists.
#[must_use]
pub fn find_rgb_subifd(ifds: &[IfdInfo]) -> Option<&IfdInfo> {
    ifds.iter()
        .filter(|info| info.is_subifd)
        .filter(|info| info.samples_per_pixel >= 3 && info.photometric == PHOTOMETRIC_RGB)
        .max_by_key(|info| info.pixels())
}

/// True if a SubIFD carries camera CFA data, which rules out a Coolscan file.
#[must_use]
pub fn has_cfa_subifd(ifds: &[IfdInfo]) -> bool {
    ifds.iter().any(|info| info.is_subifd && is_cfa(info))
}

/// True if this `.nef` is a Nikon Coolscan scanner file.
#[must_use]
pub fn is_coolscan_nef(path: &Path) -> bool {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("nef"))
    {
        return false;
    }
    let Some(ifds) = read_ifds(path) else {
        return false;
    };
    !has_cfa_subifd(&ifds) && find_rgb_subifd(&ifds).is_some()
}

/// Load the processed image from a Coolscan NEF.
#[must_use]
pub fn load(path: &Path) -> Option<LoadedImage> {
    let ifds = read_ifds(path)?;
    let selected = find_rgb_subifd(&ifds)?;
    let (image, _, _) = decode_ifd(path, selected.offset)?;
    Some(LoadedImage {
        image: Some(image),
        orientation: read_orientation(path),
        ..LoadedImage::default()
    })
}
