//! Windows data-folder selection before application paths are bound.
//!
//! Mirrors `negpy/desktop/startup.py`. The folder picker and the error messages are
//! callbacks: a headless backend never creates a GUI itself.

use std::path::{Path, PathBuf};

use crate::kernel::system::paths::default_user_dir;
use crate::kernel::system::user_directory::{
    ensure_writable, local_data_root, saved_user_directory, select_user_directory,
};

/// The user-facing error sink. The first argument is the message; the second marks an
/// error rather than an informational notice.
pub type MessageFn<'a> = &'a mut dyn FnMut(&str, bool);

/// The native folder picker: source folder, suggested folder, and the choice.
pub type ChooserFn<'a> = &'a mut dyn FnMut(&Path, &Path) -> Option<PathBuf>;

/// Prepare the data folder on Windows before imports bind application paths.
///
/// A non-Windows platform needs no preflight. On Windows an explicit `NEGPY_USER_DIR` or
/// a saved data folder is never silently replaced by Documents: an unwritable one fails
/// with a message. Only a fresh install offers the picker.
pub fn prepare_user_directory(
    platform: &str,
    choose: ChooserFn<'_>,
    message: MessageFn<'_>,
) -> bool {
    prepare_user_directory_at(platform, &default_user_dir(), choose, message)
}

/// The preflight for an explicit source folder, so a caller can test or recover a
/// non-default location without changing process environment.
pub fn prepare_user_directory_at(
    platform: &str,
    selected: &Path,
    choose: ChooserFn<'_>,
    message: MessageFn<'_>,
) -> bool {
    prepare_user_directory_with_probe(platform, selected, &ensure_writable, choose, message)
}

/// The preflight with an injected writability probe. A caller that knows a folder is
/// unwritable can exercise the recovery branch without changing filesystem permissions.
pub fn prepare_user_directory_with_probe(
    platform: &str,
    selected: &Path,
    probe: &dyn Fn(&Path) -> std::io::Result<()>,
    choose: ChooserFn<'_>,
    message: MessageFn<'_>,
) -> bool {
    if platform != "win32" {
        return true;
    }
    match probe(selected) {
        Ok(()) => return true,
        Err(_) if std::env::var_os("NEGPY_USER_DIR").is_some() => {
            message(
                &format!(
                    "NegPy could not prepare its data folder:\n\nCannot write to {}.\n\nCheck folder access and available disk space, then try again.",
                    selected.display()
                ),
                true,
            );
            return false;
        }
        Err(_) => {}
    }
    match saved_user_directory() {
        Ok(Some(_)) => {
            message(
                &format!(
                    "NegPy could not prepare its data folder:\n\nCannot write to the saved data folder: {}.\n\nRestore access and start NegPy again.",
                    selected.display()
                ),
                true,
            );
            false
        }
        Ok(None) => choose_and_select(selected.to_path_buf(), choose, message),
        Err(error) => {
            message(&prepare_error(&error.to_string()), true);
            false
        }
    }
}

fn choose_and_select(source: PathBuf, choose: ChooserFn<'_>, message: MessageFn<'_>) -> bool {
    let mut suggested = match local_data_root() {
        Ok(root) => root.join("data"),
        Err(error) => {
            message(&prepare_error(&error.to_string()), true);
            return false;
        }
    };
    loop {
        let Some(choice) = choose(&source, &suggested) else {
            return false;
        };
        match select_user_directory(&choice) {
            Ok(()) => return true,
            Err(error) => {
                message(
                    &format!(
                        "NegPy could not use this data folder:\n{}\n\n{}\n\nSelect another folder or cancel.",
                        choice.display(),
                        error
                    ),
                    true,
                );
                suggested = choice;
            }
        }
    }
}

fn prepare_error(error: &str) -> String {
    format!(
        "NegPy could not prepare its data folder:\n\n{error}\n\nCheck folder access and available disk space, then try again. If NEGPY_USER_DIR is set, check that it names a writable folder."
    )
}
