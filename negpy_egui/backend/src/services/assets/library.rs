//! Library file filters: the images a folder holds, without opening any file.
//!
//! Mirrors the discovery half of `negpy/services/assets/library.py` and the extension
//! rules in `negpy/infrastructure/loaders/constants.py`.

use std::path::{Path, PathBuf};

use crate::infrastructure::loaders::constants::{is_hidden_path, is_image_name, is_ir_sidecar_path};

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
