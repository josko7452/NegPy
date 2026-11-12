//! Loader families and the scanner default for a Slide.
//!
//! Mirrors `negpy/infrastructure/loaders/factory.py`'s format choice and
//! `source_kind.py`'s Positive default, without a RAW decoder.

use std::path::Path;

use crate::domain::models::workspace::WorkspaceConfig;
use crate::domain::models::{
    ProcessMode, hdr_active, is_rgb_triplet, stitch_active, with_positive_source,
};
use crate::infrastructure::loaders::constants::{
    SUPPORTED_JPEG_EXTENSIONS, SUPPORTED_JXL_EXTENSIONS, SUPPORTED_TIFF_EXTENSIONS, extension,
};

/// The loader family a path selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoaderKind {
    Tiff,
    Jpeg,
    Jxl,
    Pakon,
    Noritsu,
    CameraRaw,
}

const PAKON_SPECS: &[(u64, u64, u64)] = &[
    (36_000_000, 2000, 3000),
    (9_000_000, 1000, 1500),
    (24_000_000, 2000, 2000),
    (48_000_000, 2000, 4000),
    (72_000_000, 4000, 3000),
];
const PAKON_HEADER_SIZE: u64 = 16;
const PAKON_TRIMMED_HEIGHT: u64 = 2000;

/// `(height, width)` from a TLX header, when its fields and file size agree.
#[must_use]
pub fn pakon_header(path: &Path) -> Option<(u64, u64)> {
    let size = std::fs::metadata(path).ok()?.len();
    let mut raw = [0u8; 16];
    let mut file = std::fs::File::open(path).ok()?;
    std::io::Read::read_exact(&mut file, &mut raw).ok()?;
    let header_size = u32::from_le_bytes(raw[0..4].try_into().ok()?);
    let width = u32::from_le_bytes(raw[4..8].try_into().ok()?);
    let height = u32::from_le_bytes(raw[8..12].try_into().ok()?);
    let bits = u32::from_le_bytes(raw[12..16].try_into().ok()?);
    if u64::from(header_size) != PAKON_HEADER_SIZE
        || bits != 48
        || width == 0
        || height == 0
        || PAKON_HEADER_SIZE + u64::from(width) * u64::from(height) * 6 != size
    {
        return None;
    }
    Some((u64::from(height), u64::from(width)))
}

/// `(height, width)` for a Pakon raw: its header, a known file size, or a trimmed frame.
#[must_use]
pub fn pakon_layout(path: &Path) -> Option<(u64, u64)> {
    if let Some(layout) = pakon_header(path) {
        return Some(layout);
    }
    let size = std::fs::metadata(path).ok()?.len();
    if let Some((_, height, width)) = PAKON_SPECS
        .iter()
        .find(|(spec, _, _)| size.abs_diff(*spec) < 1024)
    {
        return Some((*height, *width));
    }
    let width = size / (PAKON_TRIMMED_HEIGHT * 6);
    (size % (PAKON_TRIMMED_HEIGHT * 6) == 0 && (1000..=6000).contains(&width))
        .then_some((PAKON_TRIMMED_HEIGHT, width))
}

/// Whether `path` selects the Pakon loader. A headerless file needs one of its extensions.
#[must_use]
pub fn is_pakon(path: &Path) -> bool {
    let extensionless = path.extension().is_none();
    let named =
        extension(path).is_some_and(|value| [".raw", ".dat", ".bin"].contains(&value.as_str()));
    (extensionless || named) && pakon_layout(path).is_some()
}

const NORITSU_DIMS: &[(u64, u64)] = &[
    (3711, 5028),
    (4937, 5028),
    (6079, 5028),
    (7317, 5028),
    (5010, 5028),
    (4036, 5028),
    (5185, 5028),
    (5256, 5028),
    (9972, 5028),
    (10379, 5028),
    (3859, 5028),
    (6787, 5028),
    (6476, 5028),
    (7274, 5028),
    (7682, 5028),
    (7158, 4502),
    (3551, 4502),
    (12681, 4502),
    (11348, 4502),
    (4042, 6391),
];

/// `(width, height)` for a headerless Noritsu raw, or None when ambiguous.
#[must_use]
pub fn detect_noritsu_dims(path: &Path) -> Option<(u64, u64)> {
    let size = std::fs::metadata(path).ok()?.len();
    if let Some((width, height)) = NORITSU_DIMS
        .iter()
        .find(|(width, height)| width * height * 6 == size)
    {
        return Some((*width, *height));
    }
    let mut heights: Vec<u64> = NORITSU_DIMS.iter().map(|(_, height)| *height).collect();
    heights.sort_unstable();
    heights.dedup();
    let height_matches: Vec<(u64, u64)> = heights
        .into_iter()
        .filter(|height| size % (height * 6) == 0)
        .map(|height| (size / (height * 6), height))
        .collect();
    if height_matches.len() == 1 {
        return height_matches.first().copied();
    }
    let total_pixels = size / 6;
    if total_pixels * 6 != size {
        return None;
    }
    let mut candidates = Vec::new();
    for width in 2000..=15000 {
        if total_pixels % width != 0 {
            continue;
        }
        let height = total_pixels / width;
        if height < 2000 {
            continue;
        }
        let aspect = width.max(height) as f64 / width.min(height) as f64;
        if (1.0..=4.5).contains(&aspect) {
            candidates.push((width, height));
        }
    }
    (candidates.len() == 1).then(|| candidates[0])
}

/// Whether `path` is a headerless Noritsu EZController raw, not a Pakon file.
#[must_use]
pub fn is_noritsu(path: &Path) -> bool {
    extension(path).as_deref() == Some(".raw")
        && !is_pakon(path)
        && detect_noritsu_dims(path).is_some()
}

/// The loader family selected by an extension and its scanner headers.
#[must_use]
pub fn select_loader(path: &Path) -> LoaderKind {
    if crate::infrastructure::loaders::constants::has_extension(path, SUPPORTED_TIFF_EXTENSIONS) {
        return LoaderKind::Tiff;
    }
    if crate::infrastructure::loaders::constants::has_extension(path, SUPPORTED_JPEG_EXTENSIONS) {
        return LoaderKind::Jpeg;
    }
    if crate::infrastructure::loaders::constants::has_extension(path, SUPPORTED_JXL_EXTENSIONS) {
        return LoaderKind::Jxl;
    }
    if is_pakon(path) {
        return LoaderKind::Pakon;
    }
    if is_noritsu(path) {
        return LoaderKind::Noritsu;
    }
    LoaderKind::CameraRaw
}

/// A finished image or a film scanner raw: a Slide of it starts Positive.
#[must_use]
pub fn is_scanned_positive_source(path: &Path) -> bool {
    !matches!(select_loader(path), LoaderKind::CameraRaw)
}

fn is_composite(config: &WorkspaceConfig) -> bool {
    hdr_active(&config.hdr) || stitch_active(&config.stitch) || is_rgb_triplet(&config.rgbscan)
}

/// A Slide's scanner default: Positive on for a finished image or scanner raw.
#[must_use]
pub fn with_scanner_positive_default(mut config: WorkspaceConfig, path: &Path) -> WorkspaceConfig {
    if config.process.process_mode != ProcessMode::E6
        || config.process.positive_source
        || is_composite(&config)
        || !is_scanned_positive_source(path)
    {
        return config;
    }
    config = with_positive_source(config, true);
    config
}
