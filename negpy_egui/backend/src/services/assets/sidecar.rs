//! JSON sidecars next to a source file.
//!
//! Mirrors `negpy/services/assets/sidecar.py`. The database is the primary store; a sidecar
//! is loaded on a miss and promoted into the database.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::domain::models::workspace::{BOUNDS_VERSION, WorkspaceConfig};
use crate::infrastructure::storage::repository::StorageRepository;
use crate::kernel::system::paths::{apply_umask_mode, replace_file, safe_unlink};

pub const SIDECAR_EXT: &str = ".negpy";

/// The sidecar path next to `source_path`: `<basename>.negpy`, or `<basename>.<half>.negpy`.
#[must_use]
pub fn sidecar_path_for(source_path: &Path, half: u32) -> PathBuf {
    let file_name = source_path.file_name().map_or_else(
        || "frame".to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let base = Path::new(&file_name).file_stem().map_or_else(
        || file_name.clone(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let suffix = if half == 0 {
        String::new()
    } else {
        format!(".{half}")
    };
    source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{base}{suffix}{SIDECAR_EXT}"))
}

/// Write the full edit as JSON next to the source. Returns the path written.
pub fn write_sidecar(
    source_path: &Path,
    config: &WorkspaceConfig,
    half: u32,
) -> std::io::Result<PathBuf> {
    let path = sidecar_path_for(source_path, half);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut payload = config
        .to_dict()
        .into_iter()
        .collect::<serde_json::Map<String, Value>>();
    payload.insert("bounds_version".to_string(), Value::from(BOUNDS_VERSION));
    let text = serde_json::to_string_pretty(&Value::Object(payload))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

    let temporary = path.with_extension("part");
    let result = fs::write(&temporary, text)
        .and_then(|()| apply_umask_mode(&temporary))
        .and_then(|()| replace_file(&temporary, &path));
    if result.is_err() {
        safe_unlink(&temporary);
    }
    result.map(|()| path)
}

/// Load edits from the sidecar at `path`. `None` when absent or malformed.
#[must_use]
pub fn read_sidecar(path: &Path) -> Option<WorkspaceConfig> {
    let text = fs::read_to_string(path).ok()?;
    let Value::Object(mut data) = serde_json::from_str::<Value>(&text).ok()? else {
        return None;
    };
    let bounds_version = data
        .get("bounds_version")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if bounds_version < BOUNDS_VERSION
        && !data
            .get("lock_bounds")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        data.remove("local_floors");
        data.remove("local_ceils");
    }
    let mut flat = Map::new();
    for (key, value) in data {
        if key != "bounds_version" {
            flat.insert(key, value);
        }
    }
    Some(WorkspaceConfig::from_flat_dict(&flat))
}

/// Load the sidecar next to `source_path`, or `None`.
#[must_use]
pub fn load_sidecar(source_path: &Path, half: u32) -> Option<WorkspaceConfig> {
    let path = sidecar_path_for(source_path, half);
    path.exists().then(|| read_sidecar(&path)).flatten()
}

/// Load the edit for one asset, preferring the database, then a path fallback, then the
/// sidecar. A path fallback is only valid for a whole, non-composite, non-forked frame.
pub fn load_or_promote(
    repo: &StorageRepository,
    file_hash: &str,
    source_path: &Path,
    half: u32,
    composite: bool,
    forked: bool,
) -> rusqlite::Result<Option<WorkspaceConfig>> {
    if let Some(config) = repo.load_file_settings(file_hash)? {
        return Ok(Some(config));
    }
    if half == 0
        && !composite
        && !forked
        && let Some((old_hash, config)) =
            repo.load_file_settings_by_path(&source_path.to_string_lossy())?
    {
        repo.rehome_file_settings(&old_hash, file_hash, &source_path.to_string_lossy())?;
        return Ok(Some(config));
    }
    if composite || forked {
        return Ok(None);
    }
    let Some(config) = load_sidecar(source_path, half) else {
        return Ok(None);
    };
    repo.save_file_settings(file_hash, &config, &source_path.to_string_lossy())?;
    Ok(Some(config))
}

/// Promote every sidecar among `assets` that has no saved edit.
pub fn promote_sidecars(
    repo: &StorageRepository,
    assets: &[(String, PathBuf, u32, bool, bool)],
) -> rusqlite::Result<()> {
    let hashes: Vec<String> = assets
        .iter()
        .map(|(hash, _, _, _, _)| hash.clone())
        .collect();
    let saved = repo.saved_hashes(&hashes)?;
    for (hash, path, half, composite, forked) in assets {
        if saved.contains(hash) {
            continue;
        }
        load_or_promote(repo, hash, path, *half, *composite, *forked)?;
    }
    Ok(())
}
