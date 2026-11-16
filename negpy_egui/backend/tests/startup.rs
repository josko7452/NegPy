//! Ports the Windows data-folder preflight cases in
//! `tests/test_user_directory_recovery.py`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tempfile::TempDir;

use negpy_backend::kernel::system::user_directory::{local_data_root, saved_user_directory};
use negpy_backend::session::startup::{
    prepare_user_directory, prepare_user_directory_at, prepare_user_directory_with_probe,
};

struct Probe {
    _dir: TempDir,
    _serial: MutexGuard<'static, ()>,
    source: PathBuf,
    root: PathBuf,
    messages: Arc<Mutex<Vec<(String, bool)>>>,
    choices: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
}

fn probe() -> Probe {
    static SERIAL: Mutex<()> = Mutex::new(());
    let serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = TempDir::new().expect("temp dir");
    // SAFETY: these tests run in their own process and set only lookup roots.
    unsafe {
        std::env::set_var("LOCALAPPDATA", dir.path().join("local"));
        std::env::remove_var("NEGPY_USER_DIR");
    }
    let source = dir.path().join("Documents/NegPy");
    std::fs::create_dir_all(&source).expect("source");
    let root = local_data_root().expect("root");
    Probe {
        _serial: serial,
        source,
        root,
        messages: Arc::new(Mutex::new(Vec::new())),
        choices: Arc::new(Mutex::new(Vec::new())),
        _dir: dir,
    }
}

fn run(probe: &Probe, blocked: bool, choice: Option<PathBuf>) -> bool {
    let messages = Arc::clone(&probe.messages);
    let choices = Arc::clone(&probe.choices);
    let source = probe.source.clone();
    let mut choose = |old: &Path, suggested: &Path| {
        choices
            .lock()
            .unwrap()
            .push((old.to_path_buf(), suggested.to_path_buf()));
        choice.clone()
    };
    let mut message = |text: &str, error: bool| {
        messages.lock().unwrap().push((text.to_string(), error));
    };
    let selected = prepare_user_directory_with(
        if blocked { Some(&source) } else { None },
        "win32",
        &mut choose,
        &mut message,
    );
    selected
}

fn prepare_user_directory_with(
    blocked: Option<&Path>,
    platform: &str,
    choose: &mut dyn FnMut(&Path, &Path) -> Option<PathBuf>,
    message: &mut dyn FnMut(&str, bool),
) -> bool {
    let _ = platform;
    match blocked {
        None => {
            let selected = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .and_then(|root| root.parent().map(Path::to_path_buf))
                .map(|parent| parent.join("Documents/NegPy"))
                .expect("test source");
            prepare_user_directory_at("win32", &selected, choose, message)
        }
        Some(source) => blocked_preflight(source, choose, message),
    }
}

fn blocked_preflight(
    source: &Path,
    choose: &mut dyn FnMut(&Path, &Path) -> Option<PathBuf>,
    message: &mut dyn FnMut(&str, bool),
) -> bool {
    prepare_user_directory_with_probe(
        "win32",
        source,
        &|_| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        choose,
        message,
    )
}

#[test]
fn a_writable_startup_has_no_prompt_or_record() {
    let probe = probe();
    assert!(run(&probe, false, None));
    assert!(probe.messages.lock().unwrap().is_empty());
    assert!(probe.choices.lock().unwrap().is_empty());
    assert!(!probe.root.join("data-location.json").exists());
}

#[test]
fn a_non_windows_startup_is_unchanged() {
    let probe = probe();
    let mut choose = |_: &Path, _: &Path| -> Option<PathBuf> { panic!("no picker") };
    let mut message = |_: &str, _: bool| panic!("no message");
    assert!(prepare_user_directory("linux", &mut choose, &mut message));
    assert!(probe.messages.lock().unwrap().is_empty());
}

#[test]
fn a_blocked_default_suggests_the_local_data_folder() {
    let probe = probe();
    let target = probe.root.join("data");
    assert!(run(&probe, true, Some(target.clone())));
    let choices = probe.choices.lock().unwrap();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].0, probe.source);
    assert_eq!(choices[0].1, target);
    assert_eq!(saved_user_directory().unwrap(), Some(target));
    assert!(probe.messages.lock().unwrap().is_empty());
}

#[test]
fn cancelling_the_picker_changes_no_location() {
    let probe = probe();
    assert!(!run(&probe, true, None));
    assert!(!probe.root.exists());
}

#[test]
fn a_failed_choice_can_be_changed() {
    let probe = probe();
    let invalid = probe._dir.path().join("file");
    std::fs::write(&invalid, "do not overwrite").unwrap();
    let target = probe.root.join("data");
    let choices = Arc::new(Mutex::new(vec![
        Some(invalid.clone()),
        Some(target.clone()),
    ]));
    let messages = Arc::clone(&probe.messages);
    let choices_for_picker = Arc::clone(&choices);
    let mut choose = |_: &Path, _: &Path| choices_for_picker.lock().unwrap().remove(0);
    let mut message = |text: &str, error: bool| {
        messages.lock().unwrap().push((text.to_string(), error));
    };
    assert!(blocked_preflight(&probe.source, &mut choose, &mut message));
    assert_eq!(probe.messages.lock().unwrap().len(), 1);
    assert_eq!(saved_user_directory().unwrap(), Some(target));
    assert_eq!(
        std::fs::read_to_string(invalid).unwrap(),
        "do not overwrite"
    );
}

#[test]
fn a_blocked_explicit_override_is_not_replaced() {
    let probe = probe();
    // SAFETY: test-local environment; no other test reads this variable concurrently.
    unsafe { std::env::set_var("NEGPY_USER_DIR", &probe.source) };
    let mut choose = |_: &Path, _: &Path| -> Option<PathBuf> { panic!("no picker") };
    let messages = Arc::clone(&probe.messages);
    let mut message = |text: &str, error: bool| {
        messages.lock().unwrap().push((text.to_string(), error));
    };
    assert!(!blocked_preflight(&probe.source, &mut choose, &mut message));
    assert_eq!(probe.messages.lock().unwrap().len(), 1);
    assert!(probe.messages.lock().unwrap()[0].1);
    // SAFETY: remove the test-local override again.
    unsafe { std::env::remove_var("NEGPY_USER_DIR") };
}

#[test]
fn a_blocked_saved_location_is_not_replaced() {
    let probe = probe();
    let saved = probe.root.join("data-existing");
    std::fs::create_dir_all(&saved).unwrap();
    std::fs::create_dir_all(&probe.root).unwrap();
    std::fs::write(
        probe.root.join("data-location.json"),
        br#"{"version": 2, "directory": "PLACEHOLDER"}"#,
    )
    .unwrap();
    let text = std::fs::read_to_string(probe.root.join("data-location.json")).unwrap();
    let text = text.replace("PLACEHOLDER", &saved.display().to_string());
    std::fs::write(probe.root.join("data-location.json"), text).unwrap();
    let mut choose = |_: &Path, _: &Path| -> Option<PathBuf> { panic!("no picker") };
    let mut message = |_: &str, _: bool| {};
    assert!(!blocked_preflight(&probe.source, &mut choose, &mut message));
    assert_eq!(saved_user_directory().unwrap(), Some(saved));
}
