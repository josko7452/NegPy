//! Shallow scans for unindexed files in a watched folder.
//!
//! Mirrors `negpy/infrastructure/filesystem/watcher.py`: one directory level, no hidden
//! files, no IR sidecars, and a comparison that ignores the separator convention and,
//! on Windows, the case.

use std::path::{Path, PathBuf};

use crate::infrastructure::loaders::constants::{is_hidden_path, is_ir_sidecar_path};

/// A path key that compares two spellings of one file: absolute, normalized and, on
/// Windows, lowercased. A missing file does not resolve a symlink.
#[must_use]
pub fn comparison_key(path: &Path) -> String {
    let normalized = crate::kernel::system::paths::absolute(path);
    let text = normalized.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('\\', "/").to_lowercase()
    } else {
        text
    }
}

/// Whether the extension is one the Library shows, compared without case.
#[must_use]
pub fn is_raw_extension(path: &Path) -> bool {
    crate::infrastructure::loaders::constants::is_image_path(path)
}

/// Files in `folder` that are not in `existing_paths`.
///
/// The scan is shallow and stops at the first unreadable entry, as the Python caller
/// does. A missing folder reads as no new files.
#[must_use]
pub fn scan_for_new_files(folder: &Path, existing_paths: &[PathBuf]) -> Vec<PathBuf> {
    if !folder.exists() {
        return Vec::new();
    }
    let known: Vec<String> = existing_paths
        .iter()
        .map(|path| comparison_key(path))
        .collect();
    let mut new_files = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return new_files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        if is_hidden_path(&path) || is_ir_sidecar_path(&path) || !is_raw_extension(&path) {
            continue;
        }
        let key = comparison_key(&path);
        if !known.contains(&key) {
            new_files.push(crate::kernel::system::paths::absolute(&path));
        }
    }
    new_files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(tag: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("negpy-watch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("folder");
        folder
    }

    #[test]
    fn a_new_raw_is_listed_once() {
        let folder = temp_folder("new");
        let raw = folder.join("frame.NEF");
        let main = folder.join("frame.tif");
        let sidecar = folder.join("frame_ir.tif");
        let other = folder.join("notes.txt");
        for path in [&raw, &main, &sidecar, &other] {
            std::fs::write(path, b"x").expect("write");
        }
        let mut found = scan_for_new_files(&folder, &[]);
        found.sort();
        let mut expected = [
            crate::kernel::system::paths::absolute(&main),
            crate::kernel::system::paths::absolute(&raw),
        ];
        expected.sort();
        assert_eq!(found, expected);

        let known = [
            crate::kernel::system::paths::absolute(&raw),
            crate::kernel::system::paths::absolute(&main),
        ];
        assert!(scan_for_new_files(&folder, &known).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_hidden_file_is_skipped() {
        let folder = temp_folder("hidden");
        let hidden = folder.join(".frame.nef");
        std::fs::write(&hidden, b"x").expect("write");
        assert!(scan_for_new_files(&folder, &[]).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_missing_folder_reads_as_empty() {
        let missing = std::env::temp_dir().join("negpy-watch-missing-folder");
        assert!(scan_for_new_files(&missing, &[]).is_empty());
    }
}
