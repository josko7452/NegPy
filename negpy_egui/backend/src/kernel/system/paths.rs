//! Resource lookup, the user directory and the atomic file helpers.
//!
//! Mirrors `negpy/kernel/system/paths.py`. The user directory is the one place NegPy
//! keeps a user's data, so it is resolved before anything else runs and must survive a
//! registered Documents folder that does not exist on disk.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The repository root: `paths.py` walks three levels up from its own file, and this
/// crate sits at `<root>/negpy_egui/backend`, so two levels up is the same tree.
pub fn repo_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("NEGPY_RESOURCE_DIR") {
        return PathBuf::from(dir);
    }
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}

/// An absolute path to a bundled resource. Callers pass forward-slash relatives such as
/// `icc/sRGB-v4.icc`; the result keeps the platform's own separators.
pub fn resource_path(relative: &str) -> PathBuf {
    normalize(&repo_root().join(relative))
}

/// Collapse `.` and `..` lexically, without touching the filesystem, so a path reads the
/// way `os.path.normpath` writes it.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Make `path` absolute by joining the working directory, as `Path.absolute` does. No
/// symlink is resolved.
pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&std::env::current_dir().unwrap_or_default().join(path))
    }
}

/// Replace a leading `~` with the home directory. A bare `~/dev` would otherwise become a
/// literal `~` directory under the working directory, and the app would run out of it.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = dirs::home_dir()
    {
        let rest = rest.trim_start_matches(['/', '\\']);
        return home.join(rest);
    }
    PathBuf::from(path)
}

/// The NegPy dir under `base` if `base` is usable, else `None`.
///
/// A base that already exists is used as is, because the app creates its subdirectories
/// at startup. A missing base has to prove creatability by actually creating: a
/// registered-but-absent folder is indistinguishable from a creatable one until the
/// create runs.
pub fn usable_user_dir(base: &Path) -> Option<PathBuf> {
    let target = base.join("NegPy");
    if base.is_dir() {
        return Some(absolute(&target));
    }
    fs::create_dir_all(&target).ok().map(|()| absolute(&target))
}

/// Resolve the user directory, defaulting to `Documents/NegPy` with platform detection.
pub fn default_user_dir() -> PathBuf {
    if let Some(env_path) = std::env::var_os("NEGPY_USER_DIR") {
        return absolute(&expand_tilde(&env_path.to_string_lossy()));
    }

    let docs_dir = documents_dir();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home_docs = home.join("Documents");

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(docs) = docs_dir {
        candidates.push(docs);
    }
    candidates.push(home_docs);
    candidates.push(home.clone());
    candidates.dedup();

    for base in &candidates {
        if let Some(usable) = usable_user_dir(base) {
            return usable;
        }
    }
    absolute(&home.join("NegPy"))
}

#[cfg(target_os = "linux")]
fn documents_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DOCUMENTS_DIR")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg));
    }
    if let Some(path) = xdg_user_dir() {
        return Some(path);
    }
    user_dirs_file()
}

#[cfg(target_os = "linux")]
fn xdg_user_dir() -> Option<PathBuf> {
    let out = std::process::Command::new("xdg-user-dir")
        .arg("DOCUMENTS")
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[cfg(target_os = "linux")]
fn user_dirs_file() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))?;
    let text = fs::read_to_string(config_home.join("user-dirs.dirs")).ok()?;
    for line in text.lines() {
        let Some(value) = line.strip_prefix("XDG_DOCUMENTS_DIR=") else {
            continue;
        };
        // Line format: XDG_DOCUMENTS_DIR="$HOME/doc"
        let unquoted = value.trim().trim_matches('"');
        let expanded = match dirs::home_dir() {
            Some(home) => unquoted.replace("$HOME", &home.to_string_lossy()),
            None => unquoted.to_string(),
        };
        let path = PathBuf::from(expanded);
        if path.is_absolute() {
            return Some(path);
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn documents_dir() -> Option<PathBuf> {
    dirs::document_dir()
}

/// Read and restore the process umask, cached after the first call. On the first call it
/// is read on the calling thread: a file another thread created in between would take the
/// temporary mask.
fn current_umask() -> u32 {
    static UMASK: OnceLock<u32> = OnceLock::new();
    *UMASK.get_or_init(|| unsafe {
        let mask = libc::umask(0o022);
        libc::umask(mask);
        mask as u32
    })
}

/// Give a file the mode a newly created file gets under the umask. A `NamedTemporaryFile`
/// file is always 0600, and a rename keeps it, so a file written through one would be
/// private to its owner.
#[cfg(unix)]
pub fn apply_umask_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = 0o666 & !current_umask();
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
pub fn apply_umask_mode(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Atomically replace `dst` with `src`, retrying transient sharing violations on Windows.
pub fn replace_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    replace_file_with(src, dst, Duration::from_secs(2), Duration::from_millis(20))
}

/// [`replace_file`] with an explicit retry budget, for a caller that knows its watcher.
pub fn replace_file_with(
    src: &Path,
    dst: &Path,
    timeout: Duration,
    interval: Duration,
) -> std::io::Result<()> {
    if !cfg!(windows) {
        return fs::rename(src, dst);
    }
    let deadline = Instant::now() + timeout;
    let mut delay = interval.max(Duration::from_millis(5));
    loop {
        match fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(err) => {
                if dst.exists()
                    && let Ok(meta) = fs::metadata(dst)
                    && meta.permissions().readonly()
                {
                    clear_readonly(dst);
                    if fs::rename(src, dst).is_ok() {
                        return Ok(());
                    }
                }
                if !is_transient_lock(&err) || Instant::now() >= deadline {
                    return Err(err);
                }
                std::thread::sleep(delay);
                delay = (delay.mul_f64(1.5)).min(Duration::from_millis(200));
            }
        }
    }
}

#[cfg(windows)]
fn is_transient_lock(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(5 | 32))
}

#[cfg(not(windows))]
fn is_transient_lock(_err: &std::io::Error) -> bool {
    false
}

/// Silently remove a file, retrying briefly on Windows if locked and ignoring a missing one.
pub fn safe_unlink(path: &Path) {
    safe_unlink_with(path, Duration::from_millis(500));
}

/// [`safe_unlink`] with an explicit retry budget.
pub fn safe_unlink_with(path: &Path, timeout: Duration) {
    if !path.exists() {
        return;
    }
    if !cfg!(windows) {
        let _ = fs::remove_file(path);
        return;
    }
    let deadline = Instant::now() + timeout;
    let mut delay = Duration::from_millis(20);
    loop {
        if let Ok(meta) = fs::metadata(path)
            && meta.permissions().readonly()
        {
            clear_readonly(path);
        }
        match fs::remove_file(path) {
            Ok(()) => return,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(_) => {
                if Instant::now() >= deadline {
                    return;
                }
                std::thread::sleep(delay);
                delay = (delay.mul_f64(1.5)).min(Duration::from_millis(100));
            }
        }
    }
}

/// Drop the read-only attribute. Windows refuses to replace or unlink such a file.
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly(path: &Path) {
    if let Ok(meta) = fs::metadata(path) {
        let mut perm = meta.permissions();
        perm.set_readonly(false);
        let _ = fs::set_permissions(path, perm);
    }
}

#[cfg(not(windows))]
fn clear_readonly(_path: &Path) {}
