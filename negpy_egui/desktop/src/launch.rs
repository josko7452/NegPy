//! Manual launch state for the `negfe` binary.

use std::path::PathBuf;

use crate::edit::FrameEdit;
use crate::fixture::{GoldenManifest, ReferenceFixture};
use crate::frame_source::{FrameImage, frame_path, load_linear_source, load_strip, stored_history};
use negpy_backend::kernel::system::paths::default_user_dir;
use negpy_backend::session::startup::prepare_user_directory_with_probe;

/// The profile and folder a manual launch reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchOptions {
    pub profile: Option<PathBuf>,
    pub folder: Option<PathBuf>,
}

impl LaunchOptions {
    /// Read the profile and folder from `NEGPY_GUI_PROFILE` and `NEGPY_GUI_FOLDER`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            profile: std::env::var_os("NEGPY_GUI_PROFILE").map(PathBuf::from),
            folder: std::env::var_os("NEGPY_GUI_FOLDER").map(PathBuf::from),
        }
    }

    /// Read `--profile` and `--folder`, then let the environment override them.
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut cli = Self {
            profile: None,
            folder: None,
        };
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--profile" => cli.profile = args.next().map(PathBuf::from),
                "--folder" => cli.folder = args.next().map(PathBuf::from),
                other => return Err(format!("Unknown argument: {other}")),
            }
        }
        let env = Self::from_env();
        Ok(Self {
            profile: env.profile.or(cli.profile),
            folder: env.folder.or(cli.folder),
        })
    }

    /// The profile directory when it exists.
    #[must_use]
    pub fn profile_dir(&self) -> Option<PathBuf> {
        self.profile.clone().filter(|path| path.is_dir())
    }

    /// The profile the shell reads and writes, or `None` when the user declined a
    /// data folder. An explicit `--profile` wins; otherwise the default user directory
    /// resolves, as the Python startup does.
    #[must_use]
    pub fn resolve_profile(&self) -> Option<PathBuf> {
        self.profile_dir().or_else(|| {
            let dir = default_user_dir();
            dir.is_dir().then_some(dir)
        })
    }

    /// The folder when it exists.
    #[must_use]
    pub fn frame_folder(&self) -> Option<PathBuf> {
        self.folder.clone().filter(|path| path.is_dir())
    }
}

/// Prepare the data folder on Windows, then resolve the profile the shell reads.
///
/// A declined folder returns `None`: the shell then runs without a store, as it does
/// when the user gives no profile on a platform with no preflight.
#[must_use]
pub fn startup_profile(options: &LaunchOptions) -> Option<PathBuf> {
    if prepare_user_directory() {
        options.resolve_profile()
    } else {
        None
    }
}

/// Run the Windows data-folder preflight with the native picker and message box. A
/// non-Windows platform needs no preflight.
fn prepare_user_directory() -> bool {
    let platform = if cfg!(windows) {
        "win32"
    } else {
        std::env::consts::OS
    };
    let mut choose = |source: &std::path::Path, _suggested: &std::path::Path| {
        rfd::FileDialog::new()
            .set_title("NegPy data folder")
            .set_directory(source)
            .pick_folder()
    };
    let mut message = |text: &str, error: bool| {
        let dialog = rfd::MessageDialog::new()
            .set_title("NegPy data folder")
            .set_description(text)
            .set_buttons(rfd::MessageButtons::Ok);
        let dialog = if error {
            dialog.set_level(rfd::MessageLevel::Error)
        } else {
            dialog.set_level(rfd::MessageLevel::Info)
        };
        dialog.show();
    };
    prepare_user_directory_with_probe(
        platform,
        &default_user_dir(),
        &negpy_backend::kernel::system::user_directory::ensure_writable,
        &mut choose,
        &mut message,
    )
}

/// Load the state a manual launch shows.
#[must_use]
pub fn load_manual_state(
    options: &LaunchOptions,
    profile: Option<&std::path::Path>,
    reference: &ReferenceFixture,
) -> (
    FrameImage,
    Option<crate::render_worker::SourceFrame>,
    Vec<FrameImage>,
    Vec<negpy_backend::session::workspace::HistoryRow>,
    Vec<String>,
    crate::view::rail::roll_analysis::RollAnalysisState,
) {
    let Some(folder) = options.frame_folder() else {
        return (
            FrameImage::empty(),
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            crate::view::rail::roll_analysis::RollAnalysisState::default(),
        );
    };
    let current = frame_path(Some(folder.clone()), &reference.fixture.frame);
    let source = current
        .as_deref()
        .and_then(|path| load_linear_source(path, profile, 1600).ok())
        .map(|(source, _)| source);
    let frame = FrameImage::empty();
    let strip = load_strip(&folder, current.as_deref(), profile, 320, 12);
    let history = current.as_deref().map_or_else(
        || (Vec::new(), Vec::new()),
        |path| stored_history(profile, path),
    );
    let roll_analysis = current.as_deref().map_or_else(
        crate::view::rail::roll_analysis::RollAnalysisState::default,
        |path| crate::view::rail::roll_analysis::RollAnalysisState::load(profile, path, None),
    );
    (frame, source, strip, history.0, history.1, roll_analysis)
}

/// The edit of the frame a manual launch shows.
#[must_use]
pub fn load_manual_edit(
    options: &LaunchOptions,
    profile: Option<&std::path::Path>,
    reference: &ReferenceFixture,
) -> FrameEdit {
    let Some(folder) = options.frame_folder() else {
        return FrameEdit::default();
    };
    let Some(path) = frame_path(Some(folder), &reference.fixture.frame) else {
        return FrameEdit::default();
    };
    FrameEdit::load(profile, &path)
}

/// The edit of the frame a manifest capture shows, read without saving.
#[must_use]
pub fn load_manifest_edit(manifest: &GoldenManifest, reference: &ReferenceFixture) -> FrameEdit {
    let Some(folder) = manifest.frame_folder() else {
        return FrameEdit::default();
    };
    let Some(path) = frame_path(Some(folder), &reference.fixture.frame) else {
        return FrameEdit::default();
    };
    FrameEdit::load(manifest.profile_dir().as_deref(), &path).read_only()
}

/// Load the frame and strip a manifest capture shows.
#[must_use]
pub fn load_manifest_state(
    manifest: &GoldenManifest,
    reference: &ReferenceFixture,
) -> (
    FrameImage,
    Option<crate::render_worker::SourceFrame>,
    Vec<FrameImage>,
    Vec<negpy_backend::session::workspace::HistoryRow>,
    Vec<String>,
    crate::view::rail::roll_analysis::RollAnalysisState,
) {
    let profile = manifest.profile_dir();
    let current = manifest
        .frame_folder()
        .and_then(|folder| frame_path(Some(folder), &reference.fixture.frame));
    let source = current
        .as_deref()
        .and_then(|path| load_linear_source(path, profile.as_deref(), 1600).ok());
    let frame = source
        .as_ref()
        .map(|(source, _)| FrameImage::from_source(source))
        .unwrap_or_else(FrameImage::empty);
    let strip = manifest.frame_folder().map_or_else(
        || vec![frame.clone()],
        |folder| {
            let current = frame_path(Some(folder.clone()), &reference.fixture.frame);
            let strip = load_strip(&folder, current.as_deref(), profile.as_deref(), 320, 12);
            if strip.is_empty() && frame.is_rgb() {
                vec![frame.clone()]
            } else {
                strip
            }
        },
    );
    let history = current.as_deref().map_or_else(
        || (Vec::new(), Vec::new()),
        |path| stored_history(profile.as_deref(), path),
    );
    let roll_analysis = current.as_deref().map_or_else(
        crate::view::rail::roll_analysis::RollAnalysisState::default,
        |path| {
            crate::view::rail::roll_analysis::RollAnalysisState::load(
                profile.as_deref(),
                path,
                None,
            )
        },
    );
    (
        frame,
        source.map(|(source, _)| source),
        strip,
        history.0,
        history.1,
        roll_analysis,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_launch_has_no_paths() {
        let options = LaunchOptions {
            profile: None,
            folder: None,
        };
        assert!(options.profile_dir().is_none());
        assert!(options.frame_folder().is_none());
    }

    #[test]
    fn a_missing_folder_shows_the_empty_state() {
        let fixture = ReferenceFixture {
            name: "reference".to_string(),
            window: crate::fixture::ReferenceWindow {
                width: 1920,
                height: 1600,
                scale: 1.0,
            },
            fixture: crate::fixture::ReferenceState {
                frame: "P5080635.ORF".to_string(),
                tab: "roll".to_string(),
                analysis: crate::fixture::ReferenceAnalysis {
                    open: true,
                    log: true,
                },
                strip: crate::fixture::ReferenceStrip {
                    open: true,
                    height: 248,
                },
                canvas: crate::fixture::ReferenceCanvas { fit: true },
            },
        };
        let options = LaunchOptions {
            profile: None,
            folder: Some(PathBuf::from("/missing/folder")),
        };
        let (frame, source, strip, _, _, _) = load_manual_state(&options, None, &fixture);
        assert!(!frame.is_rgb());
        assert!(source.is_none());
        assert!(strip.is_empty());
    }

    #[test]
    fn a_launched_frame_keeps_its_linear_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.tif");
        let mut encoder =
            tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
        let samples: Vec<u8> = (0..8 * 6 * 3).map(|value| (value % 32) as u8 * 8).collect();
        encoder
            .write_image::<tiff::encoder::colortype::RGB8>(8, 6, &samples)
            .unwrap();
        let fixture = ReferenceFixture {
            name: "reference".to_string(),
            window: crate::fixture::ReferenceWindow {
                width: 1920,
                height: 1600,
                scale: 1.0,
            },
            fixture: crate::fixture::ReferenceState {
                frame: "frame.tif".to_string(),
                tab: "roll".to_string(),
                analysis: crate::fixture::ReferenceAnalysis {
                    open: true,
                    log: true,
                },
                strip: crate::fixture::ReferenceStrip {
                    open: true,
                    height: 248,
                },
                canvas: crate::fixture::ReferenceCanvas { fit: true },
            },
        };
        let options = LaunchOptions {
            profile: None,
            folder: Some(dir.path().to_path_buf()),
        };
        let (frame, source, ..) = load_manual_state(&options, None, &fixture);
        assert!(!frame.is_rgb(), "launch must not paint a CPU frame");
        assert!(source.is_some());
    }

    #[test]
    fn a_launched_frame_waits_for_the_render_worker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frame.tif");
        let mut encoder =
            tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap();
        let samples: Vec<u8> = (0..8 * 6 * 3).map(|value| (value % 32) as u8 * 8).collect();
        encoder
            .write_image::<tiff::encoder::colortype::RGB8>(8, 6, &samples)
            .unwrap();
        let fixture = ReferenceFixture {
            name: "reference".to_string(),
            window: crate::fixture::ReferenceWindow {
                width: 1920,
                height: 1600,
                scale: 1.0,
            },
            fixture: crate::fixture::ReferenceState {
                frame: "frame.tif".to_string(),
                tab: "roll".to_string(),
                analysis: crate::fixture::ReferenceAnalysis {
                    open: true,
                    log: true,
                },
                strip: crate::fixture::ReferenceStrip {
                    open: true,
                    height: 248,
                },
                canvas: crate::fixture::ReferenceCanvas { fit: true },
            },
        };
        let options = LaunchOptions {
            profile: None,
            folder: Some(dir.path().to_path_buf()),
        };
        let (frame, source, ..) = load_manual_state(&options, None, &fixture);
        assert!(!frame.is_rgb());
        assert!(source.is_some());
    }

    #[test]
    fn an_explicit_profile_wins_over_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let options = LaunchOptions {
            profile: Some(dir.path().to_path_buf()),
            folder: None,
        };
        assert_eq!(options.resolve_profile(), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn a_missing_profile_falls_back_to_the_user_directory() {
        let options = LaunchOptions {
            profile: Some(PathBuf::from("/missing/profile")),
            folder: None,
        };
        let resolved = options.resolve_profile().unwrap();
        assert_eq!(resolved, default_user_dir());
    }

    #[test]
    fn arguments_are_parsed() {
        let args = vec![
            "--profile".to_string(),
            "/tmp/profile".to_string(),
            "--folder".to_string(),
            "/tmp/roll".to_string(),
        ];
        let options = LaunchOptions::parse(args.into_iter()).unwrap();
        assert_eq!(options.profile, Some(PathBuf::from("/tmp/profile")));
        assert_eq!(options.folder, Some(PathBuf::from("/tmp/roll")));
    }

    #[test]
    fn an_unknown_argument_is_rejected() {
        let args = vec!["--bad".to_string()];
        assert!(LaunchOptions::parse(args.into_iter()).is_err());
    }
}
