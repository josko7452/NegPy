//! Global log configuration.
//!
//! Mirrors `negpy/kernel/system/logging.py`. The console log is invisible in a packaged
//! app, so it also writes a rotating file under the user data directory that a user can
//! attach to a bug report. File logging is best effort: a read-only home must never stop
//! the app from starting.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use log::LevelFilter;

use super::paths::default_user_dir;

/// The rotating file's size cap, in bytes, and how many backups are kept.
const MAX_BYTES: u64 = 2_000_000;
const BACKUPS: u32 = 2;

/// Configure the global logger once. A second call is a no-op, as the Python helper
/// returns the existing logger rather than adding a second handler.
pub fn setup_logging(level: LevelFilter) {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if INSTALLED.set(()).is_err() {
        return;
    }
    let mut dispatch = fern::Dispatch::new()
        .level(level)
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] {:<8} {}: {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .chain(io::stdout());
    if let Some(file) = log_file(default_user_dir()) {
        dispatch = dispatch.chain(Box::new(file) as Box<dyn Write + Send>);
    }
    if dispatch.apply().is_err() {
        eprintln!("negpy: could not install the logger");
    }
}

/// The log target for a module name: `get_logger("foo")` maps to target `negpy.foo`.
#[must_use]
pub fn target(name: &str) -> String {
    format!("negpy.{name}")
}

fn log_file(user_dir: PathBuf) -> Option<RotatingFile> {
    if fs::create_dir_all(&user_dir).is_err() {
        return None;
    }
    RotatingFile::open(&user_dir.join("negpy.log")).ok()
}

/// `get_logger` equivalent: the name a caller passes to a `log` macro as its target.
#[must_use]
pub fn get_logger(name: Option<&str>) -> String {
    match name {
        Some(name) => target(name),
        None => "negpy".to_string(),
    }
}

/// A file that rotates to `name.1`, `name.2` once it passes its size cap.
struct RotatingFile {
    path: PathBuf,
    file: fs::File,
    written: u64,
}

impl RotatingFile {
    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
        Ok(Self {
            path: path.to_path_buf(),
            file,
            written,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        for index in (1..=BACKUPS).rev() {
            let from = self.backup_path(index - 1);
            let to = self.backup_path(index);
            if from.exists() {
                let _ = fs::rename(&from, &to);
            }
        }
        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.written = 0;
        Ok(())
    }

    fn backup_path(&self, index: u32) -> PathBuf {
        if index == 0 {
            self.path.clone()
        } else {
            PathBuf::from(format!("{}.{index}", self.path.display()))
        }
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() as u64 > MAX_BYTES && self.rotate().is_err() {
            // A failed rotation must not lose the record: write to the current file.
        }
        let written = self.file.write(buf)?;
        self.written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_name_maps_to_its_negpy_target() {
        assert_eq!(get_logger(None), "negpy");
        assert_eq!(get_logger(Some("render")), "negpy.render");
    }

    #[test]
    fn the_file_rotates_once_it_passes_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("negpy.log");
        let mut file = RotatingFile::open(&path).unwrap();
        let chunk = vec![b'x'; (MAX_BYTES / 2 + 1) as usize];
        file.write_all(&chunk).unwrap();
        file.write_all(&chunk).unwrap();
        file.flush().unwrap();
        assert!(
            dir.path().join("negpy.log.1").exists(),
            "the first file rotated to .1"
        );
        assert!(fs::metadata(&path).unwrap().len() < MAX_BYTES);
    }

    #[test]
    fn rotation_keeps_only_the_configured_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("negpy.log");
        let mut file = RotatingFile::open(&path).unwrap();
        let chunk = vec![b'x'; (MAX_BYTES + 1) as usize];
        for _ in 0..(BACKUPS + 3) {
            file.write_all(&chunk).unwrap();
        }
        file.flush().unwrap();
        assert!(dir.path().join(format!("negpy.log.{BACKUPS}")).exists());
        assert!(
            !dir.path()
                .join(format!("negpy.log.{}", BACKUPS + 1))
                .exists()
        );
    }
}
