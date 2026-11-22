//! Golden fixture state: the capture matrix and its fixed reference frame.
//!
//! The Python harness writes `golden/tall/manifest.json`. This reader keeps the same
//! fields so the Rust capture writes a manifest the comparator already understands.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::capture::CaptureSize;

/// A capture matrix entry, as the golden manifest records it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CaptureEntry {
    pub file: String,
    pub width: usize,
    pub height: usize,
    pub group: String,
    #[serde(default)]
    pub tab: String,
    #[serde(default)]
    pub subtab: String,
    #[serde(default)]
    pub scroll: f64,
    pub view: String,
}

impl CaptureEntry {
    /// The expected pixel size.
    #[must_use]
    pub fn size(&self) -> CaptureSize {
        CaptureSize {
            width: self.width,
            height: self.height,
        }
    }
}

/// The manifest both applications write beside their captures.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GoldenManifest {
    pub profile: String,
    pub folder: String,
    pub frame: String,
    pub width: usize,
    pub height: usize,
    pub captures: Vec<CaptureEntry>,
    /// The folder the capture reads the frame from, when the manifest names one.
    #[serde(default)]
    pub source_folder: String,
    /// The copied user directory the capture reads edits from.
    #[serde(default)]
    pub source_profile: String,
    #[serde(default)]
    pub source: Option<ManifestSource>,
}

/// The paths the golden capture was produced from.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ManifestSource {
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub frame: String,
    #[serde(default)]
    pub rig: String,
}

/// The state the reference roll asks the window to show.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ReferenceFixture {
    pub name: String,
    pub window: ReferenceWindow,
    pub fixture: ReferenceState,
}

/// The logical window size of the reference capture.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ReferenceWindow {
    pub width: usize,
    pub height: usize,
    pub scale: f32,
}

/// The frame, tab and panel state a capture sets before the screenshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ReferenceState {
    pub frame: String,
    pub tab: String,
    pub analysis: ReferenceAnalysis,
    pub strip: ReferenceStrip,
    pub canvas: ReferenceCanvas,
}

/// The Analysis card's open state and log axis.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ReferenceAnalysis {
    pub open: bool,
    pub log: bool,
}

/// The Film Strip's open state and height.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ReferenceStrip {
    pub open: bool,
    pub height: usize,
}

/// The canvas view state.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ReferenceCanvas {
    pub fit: bool,
}

impl GoldenManifest {
    /// Read a manifest from `path`.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("Could not parse {}: {error}", path.display()))
    }

    /// Parse a manifest from `text`.
    pub fn read_str(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| format!("Could not parse the manifest: {error}"))
    }

    /// Check the entries against the manifest's own window and each other.
    pub fn validate(&self) -> Result<(), String> {
        if self.captures.is_empty() {
            return Err("The manifest holds no captures.".to_string());
        }
        let mut seen = BTreeSet::new();
        for entry in &self.captures {
            if entry.file.trim().is_empty() {
                return Err("A manifest capture has no file name.".to_string());
            }
            if entry.width == 0 || entry.height == 0 {
                return Err(format!("{} has an empty size.", entry.file));
            }
            if !seen.insert(entry.file.clone()) {
                return Err(format!("The manifest names {} twice.", entry.file));
            }
            if entry.width > self.width || entry.height > self.height {
                return Err(format!(
                    "{} is larger than the manifest window.",
                    entry.file
                ));
            }
        }
        Ok(())
    }

    /// Resolve each capture against `directory`.
    #[must_use]
    pub fn paths(&self, directory: &Path) -> Vec<PathBuf> {
        self.captures
            .iter()
            .map(|entry| directory.join(&entry.file))
            .collect()
    }

    /// The folder a capture reads its frame from: the explicit source folder, then the
    /// recorded source block, then the manifest's own folder when it is a path.
    #[must_use]
    pub fn frame_folder(&self) -> Option<PathBuf> {
        let explicit = std::env::var_os("NEGPY_GUI_FOLDER").map(PathBuf::from);
        explicit
            .or_else(|| {
                self.source
                    .as_ref()
                    .map(|source| PathBuf::from(&source.folder))
                    .filter(|path| path.is_dir())
            })
            .or_else(|| {
                (!self.source_folder.is_empty())
                    .then(|| PathBuf::from(&self.source_folder))
                    .filter(|path| path.is_dir())
            })
            .or_else(|| {
                let path = PathBuf::from(&self.folder);
                path.is_dir().then_some(path)
            })
    }

    /// The user directory a capture copies and reads edits from.
    #[must_use]
    pub fn profile_dir(&self) -> Option<PathBuf> {
        std::env::var_os("NEGPY_GUI_PROFILE")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .or_else(|| {
                self.source
                    .as_ref()
                    .map(|source| PathBuf::from(&source.profile))
                    .filter(|path| path.is_dir())
            })
            .or_else(|| {
                (!self.source_profile.is_empty())
                    .then(|| PathBuf::from(&self.source_profile))
                    .filter(|path| path.is_dir())
            })
            .or_else(|| {
                let path = PathBuf::from(&self.profile);
                path.is_dir().then_some(path)
            })
    }

    /// Write the manifest to `path` with a trailing newline.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("Could not serialize the manifest: {error}"))?;
        std::fs::write(path, format!("{text}\n"))
            .map_err(|error| format!("Could not write {}: {error}", path.display()))
    }
}

impl ReferenceFixture {
    /// Read the fixture state from `path`.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("Could not parse {}: {error}", path.display()))
    }

    /// The fixture state shipped with the golden capture set.
    pub fn bundled() -> Result<Self, String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/gui/reference_roll.json");
        Self::read(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "profile": "profile",
        "folder": "roll",
        "frame": "P5080635.ORF",
        "width": 1920,
        "height": 1600,
        "captures": [
            {"file": "01.png", "width": 1920, "height": 1600, "group": "roll", "tab": "", "subtab": "", "scroll": 0.0, "view": "canvas"}
        ]
    }"#;

    #[test]
    fn a_manifest_round_trips_and_resolves_its_files() {
        let manifest: GoldenManifest = serde_json::from_str(MANIFEST).unwrap();
        manifest.validate().unwrap();
        assert_eq!(
            manifest.paths(Path::new("actual"))[0],
            PathBuf::from("actual/01.png")
        );
        assert_eq!(manifest.captures[0].size(), CaptureSize::GOLDEN);
    }

    #[test]
    fn a_duplicate_file_is_rejected() {
        let mut manifest: GoldenManifest = serde_json::from_str(MANIFEST).unwrap();
        manifest.captures.push(manifest.captures[0].clone());
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn a_capture_larger_than_the_window_is_rejected() {
        let mut manifest: GoldenManifest = serde_json::from_str(MANIFEST).unwrap();
        manifest.captures[0].width = 1921;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn a_manifest_names_its_source_paths() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let roll = dir.path().join("roll");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::create_dir_all(&roll).unwrap();
        let text = r#"{
            "profile": "copied profile",
            "folder": "roll",
            "frame": "P5080635.ORF",
            "width": 1920,
            "height": 1600,
            "captures": [],
            "source": {"profile": "__PROFILE__", "folder": "__ROLL__", "frame": "P5080635.ORF", "rig": ""}
        }"#
        .replace("__PROFILE__", &profile.display().to_string())
        .replace("__ROLL__", &roll.display().to_string());
        let manifest: GoldenManifest = serde_json::from_str(&text).unwrap();
        assert_eq!(manifest.frame_folder().unwrap(), roll);
        assert_eq!(manifest.profile_dir().unwrap(), profile);
    }

    #[test]
    fn the_reference_fixture_reads_its_panel_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reference.json");
        let text = r#"{
            "name": "reference_roll",
            "window": {"width": 1920, "height": 1600, "scale": 1.0},
            "fixture": {
                "frame": "P5080635.ORF",
                "tab": "roll",
                "analysis": {"open": true, "log": true},
                "strip": {"open": true, "height": 248},
                "canvas": {"fit": true}
            }
        }"#;
        std::fs::write(&path, text).unwrap();
        let fixture = ReferenceFixture::read(&path).unwrap();
        assert_eq!(fixture.fixture.tab, "roll");
        assert_eq!(fixture.fixture.strip.height, 248);
        assert!(fixture.fixture.analysis.log);
    }

    #[test]
    fn the_narrow_manifest_covers_a_smaller_window() {
        let text = include_str!("../../tests/gui/golden/narrow/manifest.json");
        let manifest = GoldenManifest::read_str(text).unwrap();
        manifest.validate().unwrap();
        assert_eq!(manifest.width, 1400);
        assert_eq!(manifest.height, 900);
        assert!(manifest.captures.iter().any(|entry| entry.width == 1400));
    }

    #[test]
    fn the_bundled_fixture_reads() {
        let fixture = ReferenceFixture::bundled().unwrap();
        assert_eq!(fixture.fixture.frame, "P5080635.ORF");
        assert_eq!(fixture.window.width, 1920);
    }
}
