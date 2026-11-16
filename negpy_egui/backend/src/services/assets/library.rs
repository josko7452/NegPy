//! Library file filters: the images a folder holds, without opening any file.
//!
//! Mirrors the discovery half of `negpy/services/assets/library.py` and the extension
//! rules in `negpy/infrastructure/loaders/constants.py`.

use std::path::{Path, PathBuf};

use crate::infrastructure::loaders::constants::{
    is_hidden_path, is_image_name, is_ir_sidecar_path,
};
use crate::services::assets::search::facts_for;

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

/// A folder's display label: its basename, or the path when it has none.
#[must_use]
pub fn folder_label(path: &Path) -> String {
    let text = path.to_string_lossy();
    let trimmed = text.trim_end_matches(std::path::MAIN_SEPARATOR);
    Path::new(trimmed)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| text.into_owned())
}

/// One walked file, shaped like a session asset so `facts_for` reads both alike.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryFile {
    pub name: String,
    pub path: PathBuf,
    pub mtime: f64,
    pub size: u64,
}

impl LibraryFile {
    /// The asset-shaped JSON `facts_for` and `matches` read.
    #[must_use]
    pub fn asset(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "path": self.path.to_string_lossy(),
            "mtime": self.mtime,
            "size": self.size,
        })
    }
}

fn walk(root: &Path, out: &mut Vec<LibraryFile>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if is_hidden_path(&path) {
            continue;
        }
        if kind.is_dir() {
            walk(&path, out);
            continue;
        }
        if !is_image_name(&name) || is_ir_sidecar_path(&path) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        out.push(LibraryFile {
            name,
            path,
            mtime,
            size: metadata.len(),
        });
    }
}

/// Walk the roots, one asset per supported image. A nested or repeated root is not walked
/// twice, and a missing root is skipped rather than fatal.
#[must_use]
pub fn iter_library_files(roots: &[PathBuf]) -> Vec<LibraryFile> {
    let mut seen_roots: Vec<PathBuf> = Vec::new();
    let mut out = Vec::new();
    for root in roots {
        let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        if seen_roots.iter().any(|seen| real.starts_with(seen)) {
            continue;
        }
        seen_roots.push(real);
        walk(root, &mut out);
    }
    out
}

/// Paths of the walked files matching every term, in walk order.
///
/// An empty query returns nothing rather than the whole archive.
#[must_use]
pub fn search_library(
    files: &[LibraryFile],
    terms: &[crate::services::assets::search::Term],
    configs_by_path: &std::collections::HashMap<String, crate::domain::models::WorkspaceConfig>,
    marks_by_path: Option<&std::collections::HashMap<String, String>>,
) -> Vec<PathBuf> {
    if terms.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for file in files {
        let key = file.path.to_string_lossy().into_owned();
        let mark = marks_by_path.and_then(|marks| marks.get(&key));
        let mut asset = file.asset();
        asset["keeper"] = serde_json::Value::Bool(mark.is_some_and(|mark| mark == "keeper"));
        asset["excluded"] = serde_json::Value::Bool(mark.is_some_and(|mark| mark == "excluded"));
        if crate::services::assets::search::matches(
            terms,
            &facts_for(&asset, configs_by_path.get(&key)),
        ) {
            hits.push(file.path.clone());
        }
    }
    hits
}

/// One walk per session unless the user asks for a refresh.
#[derive(Default)]
pub struct LibraryWalkCache {
    roots: Vec<PathBuf>,
    files: Option<Vec<LibraryFile>>,
}

impl LibraryWalkCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        self.files = None;
    }

    /// The cached walk, re-walked only when the roots change.
    pub fn files(
        &mut self,
        roots: &[PathBuf],
        mut progress: Option<&mut dyn FnMut(usize)>,
    ) -> &[LibraryFile] {
        if self.files.is_none() || self.roots != roots {
            self.roots = roots.to_vec();
            self.files = Some(iter_library_files(roots));
        }
        let files = self.files.as_ref().expect("a walk just landed");
        if let Some(progress) = progress.as_mut() {
            progress(files.len());
        }
        files
    }
}
