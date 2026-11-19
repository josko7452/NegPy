//! Manual launch state for the `negfe` binary.

use std::path::PathBuf;

use crate::fixture::{GoldenManifest, ReferenceFixture};
use crate::frame_source::{
    FrameImage, frame_path, load_display_frame, load_strip, placeholder, stored_history,
};

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

    /// The folder when it exists.
    #[must_use]
    pub fn frame_folder(&self) -> Option<PathBuf> {
        self.folder.clone().filter(|path| path.is_dir())
    }
}

/// Load the state a manual launch shows.
#[must_use]
pub fn load_manual_state(
    options: &LaunchOptions,
    reference: &ReferenceFixture,
) -> (
    FrameImage,
    Vec<FrameImage>,
    Vec<negpy_backend::session::workspace::HistoryRow>,
    Vec<String>,
    crate::view::rail::roll_analysis::RollAnalysisState,
) {
    let profile = options.profile_dir();
    let Some(folder) = options.frame_folder() else {
        return (
            placeholder(1600, 1200),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            crate::view::rail::roll_analysis::RollAnalysisState::default(),
        );
    };
    let current = frame_path(Some(folder.clone()), &reference.fixture.frame);
    let frame = current
        .as_deref()
        .and_then(|path| load_display_frame(path, profile.as_deref(), 1600).ok())
        .unwrap_or_else(|| placeholder(1600, 1200));
    let strip = load_strip(&folder, current.as_deref(), profile.as_deref(), 320, 12);
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
    (frame, strip, history.0, history.1, roll_analysis)
}

/// Load the frame and strip a manifest capture shows.
#[must_use]
pub fn load_manifest_state(
    manifest: &GoldenManifest,
    reference: &ReferenceFixture,
) -> (
    FrameImage,
    Vec<FrameImage>,
    Vec<negpy_backend::session::workspace::HistoryRow>,
    Vec<String>,
    crate::view::rail::roll_analysis::RollAnalysisState,
) {
    let profile = manifest.profile_dir();
    let frame = frame_path(manifest.frame_folder(), &reference.fixture.frame)
        .as_deref()
        .and_then(|path| load_display_frame(path, profile.as_deref(), 1600).ok())
        .unwrap_or_else(|| placeholder(1600, 1200));
    let strip = manifest.frame_folder().map_or_else(
        || vec![frame.clone()],
        |folder| {
            let current = frame_path(Some(folder.clone()), &reference.fixture.frame);
            let strip = load_strip(&folder, current.as_deref(), profile.as_deref(), 320, 12);
            if strip.is_empty() {
                vec![frame.clone()]
            } else {
                strip
            }
        },
    );
    let current = manifest
        .frame_folder()
        .and_then(|folder| frame_path(Some(folder), &reference.fixture.frame));
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
    (frame, strip, history.0, history.1, roll_analysis)
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
    fn a_missing_folder_shows_the_placeholder() {
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
        let (frame, strip, _, _, _) = load_manual_state(&options, &fixture);
        assert!(frame.is_rgb());
        assert!(strip.is_empty());
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
