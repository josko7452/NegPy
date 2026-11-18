//! The fixture frame the capture shows.
//!
//! The capture binary decodes the frame once, before the event loop, and hands the app
//! an 8-bit display image. The app never touches the filesystem or the pipeline.

use std::path::{Path, PathBuf};

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::features::retouch::processor::downsample_to_long_edge;
use negpy_backend::infrastructure::loaders::factory::{self, LoadOptions};
use negpy_backend::infrastructure::storage::local_asset_store::file_hash;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::render_engine::DarkroomEngine;
use negpy_backend::services::rendering::display_frame::{DisplayFrame, render_display};
use negpy_backend::services::rendering::import::oriented;

/// The decoded, display-encoded reference frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameImage {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl FrameImage {
    /// A frame ready for a texture upload.
    #[must_use]
    pub fn from_display(name: impl Into<String>, frame: DisplayFrame) -> Self {
        Self {
            name: name.into(),
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels,
        }
    }

    /// Whether the buffer holds `width * height * 3` samples.
    #[must_use]
    pub fn is_rgb(&self) -> bool {
        self.pixels.len() == self.width * self.height * 3
    }
}

/// The edit stored for a frame, else the defaults.
#[must_use]
pub fn stored_config(profile: Option<&Path>, path: &Path) -> WorkspaceConfig {
    let Some(profile) = profile else {
        return WorkspaceConfig::default();
    };
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
    let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
    let Ok(hash) = file_hash(path) else {
        return WorkspaceConfig::default();
    };
    repo.load_file_settings(&hash)
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Decode `path` at a bounded proxy size and render its display frame.
pub fn load_display_frame(
    path: &Path,
    profile: Option<&Path>,
    max_edge: usize,
) -> Result<FrameImage, String> {
    let config = stored_config(profile, path);
    let loaded = factory::load(
        path,
        LoadOptions {
            linear_raw: config.process.linear_raw,
            positive_decode: None,
            load_ir: false,
        },
    )?;
    let decoded = loaded
        .image
        .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
    let oriented = oriented(&decoded, loaded.orientation);
    let proxy = downsample_to_long_edge(&oriented, max_edge.max(1));
    let hash = file_hash(path).map_err(|error| error.to_string())?;
    let mut engine = DarkroomEngine::new();
    let frame = render_display(&mut engine, &proxy, &config, &hash);
    if !frame.is_rgb() {
        return Err("The decoded frame is not an RGB buffer.".to_string());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("frame")
        .to_string();
    Ok(FrameImage::from_display(name, frame))
}

/// A display frame of one flat color, for a capture with no decodable source.
#[must_use]
pub fn placeholder(width: usize, height: usize) -> FrameImage {
    FrameImage {
        name: "P5080635.ORF".to_string(),
        width,
        height,
        pixels: vec![0xa8; width * height * 3],
    }
}

/// The frame path when the folder and the manifest frame name resolve to a file.
#[must_use]
pub fn frame_path(folder: Option<PathBuf>, frame: &str) -> Option<PathBuf> {
    folder
        .map(|folder| folder.join(frame))
        .filter(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placeholder_is_an_rgb_frame() {
        let frame = placeholder(4, 3);
        assert!(frame.is_rgb());
        assert_eq!(frame.pixels.len(), 36);
    }

    #[test]
    fn a_missing_folder_has_no_frame() {
        assert!(frame_path(None, "P5080635.ORF").is_none());
        assert!(frame_path(Some(PathBuf::from("/missing/roll")), "a.orf").is_none());
    }

    #[test]
    fn a_config_with_no_profile_reads_the_defaults() {
        let config = stored_config(None, Path::new("/missing/frame.orf"));
        assert_eq!(config, WorkspaceConfig::default());
    }
}
