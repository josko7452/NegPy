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
use negpy_backend::session::workspace::{SessionFile, WorkspaceSession};

/// The decoded, display-encoded reference frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameImage {
    pub name: String,
    /// The source file, when the frame came from disk.
    pub path: Option<PathBuf>,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl FrameImage {
    /// No frame. The shell paints its empty state instead of a photograph.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            path: None,
            width: 0,
            height: 0,
            pixels: Vec::new(),
        }
    }

    /// A frame ready for a texture upload.
    #[must_use]
    pub fn from_display(name: impl Into<String>, frame: DisplayFrame) -> Self {
        Self {
            name: name.into(),
            path: None,
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels,
        }
    }

    /// The same frame bound to its source file.
    #[must_use]
    pub fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    /// Whether the buffer holds `width * height * 3` samples.
    #[must_use]
    pub fn is_rgb(&self) -> bool {
        self.width > 0 && self.height > 0 && self.pixels.len() == self.width * self.height * 3
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

/// The edit a frame prints with inside `roll_id`: its own store row with the roll
/// defaults applied.
#[must_use]
pub fn load_config_in_roll(
    profile: Option<&Path>,
    path: impl AsRef<Path>,
    roll_id: &str,
) -> WorkspaceConfig {
    let path = path.as_ref();
    let config = stored_config(profile, path);
    let Some(profile) = profile else {
        return config;
    };
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
    let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
    let Ok(hash) = file_hash(path) else {
        return config;
    };
    negpy_backend::services::assets::rolls::resolve_roll_config(
        &repo,
        Some(roll_id),
        &hash,
        &config,
    )
}

/// The frame's stored History rows and work-print names, for the panel.
#[must_use]
pub fn stored_history(
    profile: Option<&Path>,
    path: &Path,
) -> (
    Vec<negpy_backend::session::workspace::HistoryRow>,
    Vec<String>,
) {
    let Some(profile) = profile else {
        return (Vec::new(), Vec::new());
    };
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
    let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
    let Ok(hash) = file_hash(path) else {
        return (Vec::new(), Vec::new());
    };
    let mut session = WorkspaceSession::new(
        "capture",
        repo,
        LocalAssetStore::new(app.cache_dir, app.user_icc_dir, 256),
    );
    session.uploaded_files.push(SessionFile {
        name: String::new(),
        path: path.display().to_string(),
        hash: hash.clone(),
    });
    let live = session.repository.load_file_settings(&hash).ok().flatten();
    let index = session
        .repository
        .max_history_index(&hash)
        .unwrap_or_default();
    let rows = session.history_rows(live.as_ref().map(|config| (config, index)));
    let prints = session.repository.work_prints(&hash).unwrap_or_default();
    (rows, prints)
}

/// The triage mark stored for `path`, if one is.
#[must_use]
pub fn stored_mark(profile: Option<&Path>, path: &Path) -> Option<String> {
    let profile = profile?;
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
    let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
    repo.file_marks_by_path()
        .ok()?
        .get(&path.display().to_string())
        .cloned()
}

/// Write or clear the triage mark for `path`. A `None` mark clears it.
pub fn write_mark(profile: Option<&Path>, path: &Path, mark: Option<&str>) -> Result<(), String> {
    let Some(profile) = profile else {
        return Err("Open a profile first.".to_string());
    };
    let app = negpy_backend::domain::types::AppConfig::for_user_dir(profile);
    let repo = StorageRepository::new(app.edits_db_path, app.settings_db_path);
    repo.initialize().map_err(|error| error.to_string())?;
    let hash = file_hash(path).map_err(|error| error.to_string())?;
    repo.save_file_mark(&hash, mark, &path.display().to_string())
        .map_err(|error| error.to_string())
}

/// Decode `path` at a bounded proxy size and render its display frame.
pub fn load_display_frame(
    path: &Path,
    profile: Option<&Path>,
    max_edge: usize,
) -> Result<FrameImage, String> {
    let (source, config) = load_linear_source(path, profile, max_edge)?;
    let mut engine = DarkroomEngine::new();
    let frame = render_display(&mut engine, &source.image, &config, &source.hash);
    if !frame.is_rgb() {
        return Err("The decoded frame is not an RGB buffer.".to_string());
    }
    Ok(FrameImage::from_display(source.name, frame).with_path(path))
}

/// Decode `path` at a bounded proxy size and keep the linear source for a re-render.
pub fn load_linear_source(
    path: &Path,
    profile: Option<&Path>,
    max_edge: usize,
) -> Result<(crate::render_worker::SourceFrame, WorkspaceConfig), String> {
    let config = stored_config(profile, path);
    let loaded = factory::load(
        path,
        LoadOptions {
            linear_raw: config.process.linear_raw,
            positive_decode: None,
            load_ir: true,
        },
    )?;
    let decoded = loaded
        .image
        .ok_or_else(|| format!("Could not decode {}.", path.display()))?;
    let oriented = oriented(&decoded, loaded.orientation);
    let proxy = downsample_to_long_edge(&oriented, max_edge.max(1));
    let (ir, ir_width, ir_height) = loaded.ir.map_or((None, 0, 0), |ir| {
        let oriented = negpy_backend::services::rendering::import::orient_ir(
            &ir,
            loaded.ir_width,
            loaded.ir_height,
            loaded.orientation,
        );
        let (plane, width, height) = downsample_ir_plane(
            &oriented,
            loaded.ir_width,
            loaded.ir_height,
            proxy.width,
            proxy.height,
        );
        (Some(plane), width, height)
    });
    let hash = file_hash(path).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("frame")
        .to_string();
    Ok((
        crate::render_worker::SourceFrame {
            name,
            hash,
            image: proxy,
            ir,
            ir_width,
            ir_height,
        },
        config,
    ))
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
        path: Some(path.to_path_buf()),
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

/// The roll's thumbnails for explicit paths, in the order the roll lists them.
#[must_use]
pub fn load_strip_paths(
    paths: &[PathBuf],
    profile: Option<&Path>,
    max_edge: usize,
    limit: usize,
) -> Vec<FrameImage> {
    paths
        .iter()
        .take(limit)
        .filter_map(|path| load_thumbnail(path, profile, max_edge))
        .collect()
}

/// The display frame of the first path that decodes, with its strip.
#[must_use]
pub fn load_first_frame(
    paths: &[PathBuf],
    profile: Option<&Path>,
    max_edge: usize,
    strip_limit: usize,
) -> Option<(FrameImage, Vec<FrameImage>)> {
    for path in paths {
        if let Ok(frame) = load_display_frame(path, profile, max_edge) {
            let strip = load_strip_paths(paths, profile, 320, strip_limit);
            return Some((frame, strip));
        }
    }
    None
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
    fn a_missing_folder_has_no_frame() {
        assert!(frame_path(None, "P5080635.ORF").is_none());
        assert!(frame_path(Some(PathBuf::from("/missing/roll")), "a.orf").is_none());
    }

    #[test]
    fn a_mark_round_trips_through_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("frame.orf");
        std::fs::write(&photo, b"frame").unwrap();
        write_mark(Some(dir.path()), &photo, Some("excluded")).unwrap();
        assert_eq!(
            stored_mark(Some(dir.path()), &photo).as_deref(),
            Some("excluded")
        );
        write_mark(Some(dir.path()), &photo, None).unwrap();
        assert_eq!(stored_mark(Some(dir.path()), &photo), None);
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

/// Resize a one-band plane with area averaging to a target size.
fn downsample_ir_plane(
    plane: &[f32],
    width: usize,
    height: usize,
    target_width: usize,
    target_height: usize,
) -> (Vec<f32>, usize, usize) {
    if (width, height) == (target_width, target_height) || width == 0 || height == 0 {
        return (plane.to_vec(), width, height);
    }
    let dense = negpy_backend::features::retouch::processor::downsample_to_long_edge(
        &negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer::from_rgb(
            width,
            height,
            plane.iter().flat_map(|value| [*value; 3]).collect(),
        ),
        target_width.max(target_height),
    );
    let out = dense.pixels.chunks_exact(3).map(|pixel| pixel[0]).collect();
    (out, dense.width, dense.height)
}
