//! Ports of `tests/test_user_dir_resolution.py`, `tests/test_atomic_replace.py` and
//! `tests/test_file_modes.py`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use negpy_backend::kernel::system::paths::{
    absolute, apply_umask_mode, default_user_dir, normalize, replace_file, replace_file_with,
    safe_unlink, usable_user_dir,
};

/// Process environment is global, so the tests that set it run one at a time.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn write(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn an_existing_base_is_used_without_creating_anything() {
    let tmp = tempfile::tempdir().unwrap();
    let result = usable_user_dir(tmp.path()).unwrap();
    assert_eq!(result, absolute(&tmp.path().join("NegPy")));
    assert!(!tmp.path().join("NegPy").exists());
}

#[test]
fn a_missing_but_creatable_base_creates_the_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("Documents");
    let result = usable_user_dir(&base).unwrap();
    assert_eq!(result, absolute(&base.join("NegPy")));
    assert!(base.join("NegPy").is_dir());
}

#[test]
fn an_uncreatable_base_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("blocker");
    write(&blocker, "a file, not a directory");
    assert!(usable_user_dir(&blocker.join("Documents")).is_none());
}

#[test]
fn normalize_collapses_dots_lexically() {
    assert_eq!(
        normalize(Path::new("/a/b/../c/./d")),
        PathBuf::from("/a/c/d")
    );
    assert_eq!(normalize(Path::new("a/../../b")), PathBuf::from("../b"));
}

#[test]
fn the_env_override_wins() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let custom = tmp.path().join("custom");
    unsafe { std::env::set_var("NEGPY_USER_DIR", &custom) };
    assert_eq!(default_user_dir(), absolute(&custom));
    unsafe { std::env::remove_var("NEGPY_USER_DIR") };
}

#[test]
fn the_env_override_expands_a_leading_tilde() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    unsafe { std::env::set_var("HOME", &home) };
    unsafe { std::env::set_var("NEGPY_USER_DIR", "~/negpy-devhome") };
    assert_eq!(default_user_dir(), home.join("negpy-devhome"));
    unsafe { std::env::remove_var("NEGPY_USER_DIR") };
}

#[cfg(target_os = "linux")]
#[test]
fn a_broken_documents_folder_falls_back_to_home() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let blocker = tmp.path().join("blocker");
    write(&blocker, "");
    unsafe { std::env::set_var("HOME", &home) };
    unsafe { std::env::remove_var("NEGPY_USER_DIR") };
    unsafe { std::env::set_var("XDG_DOCUMENTS_DIR", blocker.join("OneDrive/Documents")) };
    let result = default_user_dir();
    assert_eq!(result, absolute(&home.join("Documents/NegPy")));
    assert!(result.is_dir());
    unsafe { std::env::remove_var("XDG_DOCUMENTS_DIR") };
}

#[cfg(target_os = "linux")]
#[test]
fn an_existing_documents_folder_is_used_without_side_effects() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let docs = tmp.path().join("Docs");
    fs::create_dir_all(&docs).unwrap();
    unsafe { std::env::remove_var("NEGPY_USER_DIR") };
    unsafe { std::env::set_var("XDG_DOCUMENTS_DIR", &docs) };
    let result = default_user_dir();
    assert_eq!(result, absolute(&docs.join("NegPy")));
    assert!(!docs.join("NegPy").exists());
    unsafe { std::env::remove_var("XDG_DOCUMENTS_DIR") };
}

#[test]
fn replace_file_creates_a_new_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src.txt");
    let dst = tmp.path().join("dst.txt");
    write(&src, "content");
    replace_file(&src, &dst).unwrap();
    assert!(!src.exists());
    assert_eq!(read(&dst), "content");
}

#[test]
fn replace_file_overwrites_an_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src.txt");
    let dst = tmp.path().join("dst.txt");
    write(&src, "new");
    write(&dst, "old");
    replace_file(&src, &dst).unwrap();
    assert!(!src.exists());
    assert_eq!(read(&dst), "new");
}

#[cfg(unix)]
#[test]
fn replace_file_overwrites_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src.txt");
    let dst = tmp.path().join("dst.txt");
    write(&src, "new");
    write(&dst, "old");
    fs::set_permissions(&dst, fs::Permissions::from_mode(0o444)).unwrap();
    replace_file(&src, &dst).unwrap();
    assert_eq!(read(&dst), "new");
}

#[test]
fn replace_file_raises_for_a_missing_source() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("missing.txt");
    let dst = tmp.path().join("dst.txt");
    assert!(
        replace_file_with(
            &src,
            &dst,
            Duration::from_millis(50),
            Duration::from_millis(10)
        )
        .is_err()
    );
}

#[test]
fn safe_unlink_removes_a_file_and_ignores_a_missing_one() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("temp.part");
    write(&target, "temp");
    safe_unlink(&target);
    assert!(!target.exists());
    safe_unlink(&tmp.path().join("nonexistent.part"));
}

#[cfg(unix)]
#[test]
fn safe_unlink_removes_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("readonly.part");
    write(&target, "temp");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o444)).unwrap();
    safe_unlink(&target);
    assert!(!target.exists());
}

#[cfg(unix)]
#[test]
fn a_tempfile_file_takes_the_umask_mode() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let named = tempfile::NamedTempFile::new_in(tmp.path()).unwrap();
    let path = named.path().to_path_buf();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    apply_umask_mode(&path).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o666 & !current_mask());
}

#[cfg(unix)]
fn current_mask() -> u32 {
    unsafe {
        let mask = libc::umask(0o022);
        libc::umask(mask);
        mask as u32
    }
}
