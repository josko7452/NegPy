//! Source extensions and sidecar naming rules.
//!
//! Mirrors `negpy/infrastructure/loaders/constants.py`.

use std::path::Path;

/// TIFF extensions, the only ones that can carry an IR sidecar.
pub const SUPPORTED_TIFF_EXTENSIONS: &[&str] = &[".tif", ".tiff"];

/// JPEG extensions.
pub const SUPPORTED_JPEG_EXTENSIONS: &[&str] = &[".jpg", ".jpeg"];

/// JPEG XL extensions.
pub const SUPPORTED_JXL_EXTENSIONS: &[&str] = &[".jxl"];

/// Extensions the Library shows: camera raw formats, TIFF, JPEG and JPEG XL.
pub const SUPPORTED_RAW_EXTENSIONS: &[&str] = &[
    ".3fr", ".ari", ".arw", ".bay", ".braw", ".crw", ".cr2", ".cr3", ".cap", ".data", ".dcs",
    ".dcr", ".dng", ".drf", ".eip", ".erf", ".fff", ".gpr", ".iiq", ".k25", ".kdc", ".mdc", ".mef",
    ".mos", ".mrw", ".nef", ".nrw", ".obm", ".orf", ".pef", ".ptx", ".pxn", ".r3d", ".raf", ".raw",
    ".rwl", ".rw2", ".rwz", ".sr2", ".srf", ".srw", ".x3f", ".tif", ".tiff", ".jpg", ".jpeg",
    ".jxl",
];

/// Extensions the Library shows. The raw list already includes the finished formats.
pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = SUPPORTED_RAW_EXTENSIONS;

/// Longest first: `_ir_valid` must win before `_ir` is tested against the same stem.
pub const IR_SIDECAR_SUFFIXES: &[&str] = &["_ir_valid", "_ir"];

/// The lowercase extension of `path`, dot included.
#[must_use]
pub fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_lowercase()))
}

/// Whether `path` has one of `extensions`.
#[must_use]
pub fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    extension(path).is_some_and(|extension| extensions.contains(&extension.as_str()))
}

/// Whether `path` carries an extension the Library shows.
#[must_use]
pub fn is_image_path(path: &Path) -> bool {
    has_extension(path, SUPPORTED_IMAGE_EXTENSIONS)
}

/// Whether `name` carries an extension the Library shows.
#[must_use]
pub fn is_image_name(name: &str) -> bool {
    is_image_path(Path::new(name))
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
    if !has_extension(path, SUPPORTED_TIFF_EXTENSIONS) {
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

/// Wildcards for a file dialog, upper- and lowercase per extension.
#[must_use]
pub fn supported_raw_wildcards() -> String {
    SUPPORTED_RAW_EXTENSIONS
        .iter()
        .flat_map(|extension| {
            let base = extension.trim_start_matches('.');
            [format!("*.{base}"), format!("*.{}", base.to_uppercase())]
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_finished_formats_are_raw_extensions_too() {
        for extension in [".tif", ".tiff", ".jpg", ".jpeg", ".jxl"] {
            assert!(SUPPORTED_RAW_EXTENSIONS.contains(&extension));
        }
    }

    #[test]
    fn wildcards_list_both_cases() {
        let wildcards = supported_raw_wildcards();
        assert!(wildcards.contains("*.nef"));
        assert!(wildcards.contains("*.NEF"));
    }
}
