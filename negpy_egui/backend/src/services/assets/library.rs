//! Library file filters: the images a folder holds, without opening any file.
//!
//! Mirrors the discovery half of `negpy/services/assets/library.py` and the extension
//! rules in `negpy/infrastructure/loaders/constants.py`.

use std::path::{Path, PathBuf};

/// TIFF extensions, the only ones that can carry an IR sidecar.
pub const SUPPORTED_TIFF_EXTENSIONS: &[&str] = &[".tif", ".tiff"];

/// Extensions the Library shows: raw formats, TIFF, JPEG and JPEG XL.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    ".3fr", ".ari", ".arw", ".bay", ".braw", ".crw", ".cr2", ".cr3", ".cap", ".data", ".dcs",
    ".dcr", ".dng", ".drf", ".eip", ".erf", ".fff", ".gpr", ".iiq", ".k25", ".kdc", ".mdc", ".mef",
    ".mos", ".mrw", ".nef", ".nrw", ".obm", ".orf", ".pef", ".ptx", ".pxn", ".r3d", ".raf", ".raw",
    ".rwl", ".rw2", ".rwz", ".sr2", ".srf", ".srw", ".x3f", ".tif", ".tiff", ".jpg", ".jpeg",
    ".jxl",
];

const IR_SIDECAR_SUFFIXES: &[&str] = &["_ir_valid", "_ir"];

/// Whether `name` carries an extension the Library shows.
#[must_use]
pub fn is_image_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// Whether `path` is hidden by its dot-prefixed name.
#[must_use]
pub fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

/// Whether `path` is an IR sidecar whose main TIFF sits next to it.
#[must_use]
pub fn is_ir_sidecar_path(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let extension = format!(".{}", extension.to_lowercase());
    if !SUPPORTED_TIFF_EXTENSIONS.contains(&extension.as_str()) {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let stem = stem.to_lowercase();
    let Some(suffix) = IR_SIDECAR_SUFFIXES
        .iter()
        .find(|suffix| stem.ends_with(**suffix))
    else {
        return false;
    };
    let main = &stem[..stem.len() - suffix.len()];
    if main.is_empty() {
        return false;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    let wanted: Vec<String> = SUPPORTED_TIFF_EXTENSIONS
        .iter()
        .map(|extension| format!("{main}{extension}"))
        .collect();
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| wanted.iter().any(|want| name.to_lowercase() == *want))
    })
}

/// The images directly inside `path`, excluding hidden names and IR sidecars.
#[must_use]
pub fn folder_image_paths(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| !is_hidden_path(&entry.path()))
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .filter(|entry| entry.file_name().to_str().is_some_and(is_image_name))
        .map(|entry| entry.path())
        .filter(|path| !is_ir_sidecar_path(path))
        .collect()
}

/// `(images, subfolders)` directly inside a folder, skipping hidden names.
#[must_use]
pub fn folder_counts(path: &Path) -> (usize, usize) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return (0, 0);
    };
    let mut images = 0;
    let mut subfolders = 0;
    for entry in entries.filter_map(Result::ok) {
        if is_hidden_path(&entry.path()) {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            subfolders += 1;
        } else if entry.file_name().to_str().is_some_and(is_image_name)
            && !is_ir_sidecar_path(&entry.path())
        {
            images += 1;
        }
    }
    (images, subfolders)
}
