//! Persistent data-directory selection on Windows.
//!
//! Mirrors `negpy/kernel/system/user_directory.py`. The record names one absolute folder
//! and nothing is copied or moved when it changes, so a wrong record must fail loudly
//! rather than silently start from an empty library.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const DATABASES: [&str; 2] = ["edits.db", "settings.db"];
const DIRECTORIES: [&str; 8] = [
    "presets",
    "cache",
    "icc",
    "crosstalk",
    "sensor",
    "flatfield",
    "gear",
    "export",
];

/// The folder under `LOCALAPPDATA` that holds the selection record and its data folders.
pub fn local_data_root() -> io::Result<PathBuf> {
    let base = match std::env::var_os("LOCALAPPDATA") {
        Some(value) => PathBuf::from(value),
        None => dirs::home_dir()
            .unwrap_or_default()
            .join("AppData")
            .join("Local"),
    };
    if !base.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LOCALAPPDATA must be an absolute path.",
        ));
    }
    Ok(base.join("NegPy"))
}

/// The data folder the record names, or `None` when there is no record.
pub fn saved_user_directory() -> io::Result<Option<PathBuf>> {
    let root = local_data_root()?;
    let record_path = root.join("data-location.json");
    let text = match fs::read_to_string(&record_path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid NegPy data location in {}.", record_path.display()),
        )
    };
    let record: serde_json::Value = serde_json::from_str(&text).map_err(|_| invalid())?;
    let name = record
        .get("directory")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(invalid)?;
    let version = record.get("version").and_then(serde_json::Value::as_u64);
    let named = Path::new(name);
    let target = match version {
        Some(1)
            if name.starts_with("data-")
                && named
                    .file_name()
                    .is_some_and(|file| file == named.as_os_str()) =>
        {
            root.join(name)
        }
        Some(2) if named.is_absolute() => PathBuf::from(name),
        _ => return Err(invalid()),
    };
    if !target.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "The saved NegPy data folder is missing: {}",
                target.display()
            ),
        ));
    }
    Ok(Some(target))
}

/// Create `path` if absent and prove it accepts a write.
fn probe_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    let probe = path.join(".negpy-probe");
    let mut file = fs::File::create(&probe)?;
    file.write_all(b"NegPy")?;
    file.flush()?;
    drop(file);
    let _ = fs::remove_file(&probe);
    Ok(())
}

/// Probe the data folder and its subdirectories, and the databases that already exist,
/// before the app configuration loads.
pub fn ensure_writable(path: &Path) -> io::Result<()> {
    probe_directory(path)?;
    for name in DIRECTORIES {
        probe_directory(&path.join(name))?;
    }
    for name in DATABASES {
        let database = path.join(name);
        if database.exists() {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&database)?;
        }
    }
    Ok(())
}

/// Validate and remember a folder without moving or copying user data.
pub fn select_user_directory(target: &Path) -> io::Result<()> {
    if !target.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Select an absolute data-folder path.",
        ));
    }
    ensure_writable(target)?;
    let root = local_data_root()?;
    probe_directory(&root)?;
    let record = root.join("data-location.json");
    let body = serde_json::json!({"version": 2, "directory": target.to_string_lossy()});
    // Publish the complete record without replacing another instance's selection.
    if record.exists() {
        if saved_user_directory()?.as_deref() != Some(target) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Another NegPy instance selected a data folder. Close NegPy and start it again.",
            ));
        }
        return Ok(());
    }
    let temporary = root.join(format!(".data-location-{}.tmp", std::process::id()));
    fs::write(&temporary, serde_json::to_vec(&body).unwrap_or_default())?;
    match fs::hard_link(&temporary, &record) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            if saved_user_directory()?.as_deref() != Some(target) {
                let _ = fs::remove_file(&temporary);
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Another NegPy instance selected a data folder. Close NegPy and start it again.",
                ));
            }
        }
        Err(err) => {
            let _ = fs::remove_file(&temporary);
            return Err(err);
        }
    }
    let _ = fs::remove_file(&temporary);
    Ok(())
}
