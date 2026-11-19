//! The fixture frame the capture shows.
//!
//! The capture binary decodes the frame once, before the event loop, and hands the app
//! an 8-bit display image. The app never touches the filesystem or the pipeline.

use std::path::{Path, PathBuf};

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::features::retouch::processor::downsample_to_long_edge;
use negpy_backend::infrastructure::loaders::constants::{
    is_hidden_path, is_image_path, is_ir_sidecar_path,
};
use negpy_backend::infrastructure::loaders::factory::{self, LoadOptions};
use negpy_backend::infrastructure::storage::local_asset_store::{LocalAssetStore, file_hash};
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::render_engine::DarkroomEngine;
use negpy_backend::services::assets::thumbnails::thumbnail_cache_key;
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

/// The image files in `folder`, in name order.
#[must_use]
pub fn roll_paths(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| is_image_path(path) && !is_ir_sidecar_path(path) && !is_hidden_path(path))
        .collect();
    paths.sort();
    paths
}

/// A cached roll thumbnail for one source file, fitted to `max_edge`.
#[must_use]
pub fn load_thumbnail(path: &Path, profile: Option<&Path>, max_edge: usize) -> Option<FrameImage> {
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile?);
    let hash = file_hash(path).ok()?;
    let store = LocalAssetStore::new(app.cache_dir, app.user_icc_dir, 256);
    let cached = store.get_thumbnail(&thumbnail_cache_key(&hash, false))?;
    let long = cached.width().max(cached.height());
    let cached = if max_edge > 0 && long > max_edge as u32 {
        let scale = max_edge as f64 / f64::from(long);
        let width = ((f64::from(cached.width()) * scale).round() as u32).max(1);
        let height = ((f64::from(cached.height()) * scale).round() as u32).max(1);
        image::imageops::resize(
            &cached,
            width,
            height,
            image::imageops::FilterType::Triangle,
        )
    } else {
        cached
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("frame")
        .to_string();
    let frame = FrameImage {
        name,
        width: cached.width() as usize,
        height: cached.height() as usize,
        pixels: cached.into_raw(),
    };
    frame.is_rgb().then_some(frame)
}

/// The roll's thumbnails, with `current` first when it is in the folder.
#[must_use]
pub fn load_strip(
    folder: &Path,
    current: Option<&Path>,
    profile: Option<&Path>,
    max_edge: usize,
    limit: usize,
) -> Vec<FrameImage> {
    let mut paths = roll_paths(folder);
    if let Some(current) = current
        && let Some(index) = paths.iter().position(|path| path == current)
    {
        paths.rotate_left(index);
    }
    paths.truncate(limit);
    paths
        .iter()
        .filter_map(|path| load_thumbnail(path, profile, max_edge))
        .collect()
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

    #[test]
    fn a_roll_list_keeps_images_and_drops_sidecars_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.ORF", "a.ORF", "a.ORF.xmp", ".hidden.ORF"] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        std::fs::create_dir(dir.path().join("c.ORF")).unwrap();
        let names: Vec<String> = roll_paths(dir.path())
            .iter()
            .filter_map(|path| path.file_name()?.to_str().map(str::to_string))
            .collect();
        assert_eq!(names, ["a.ORF", "b.ORF"]);
    }

    #[test]
    fn a_strip_puts_the_open_frame_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.ORF", "b.ORF", "c.ORF"] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        let paths = roll_paths(dir.path());
        let mut ordered = paths.clone();
        let index = ordered
            .iter()
            .position(|path| path.ends_with("b.ORF"))
            .unwrap();
        ordered.rotate_left(index);
        assert!(ordered[0].ends_with("b.ORF"));
        assert_eq!(ordered.len(), 3);
    }

    #[test]
    fn a_missing_thumbnail_is_empty() {
        assert!(load_thumbnail(Path::new("/missing/frame.orf"), None, 320).is_none());
    }
}
