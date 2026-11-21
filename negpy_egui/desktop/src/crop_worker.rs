//! The Crop All Frames worker: decode each frame, resolve one roll crop, persist it.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;

use negpy_backend::features::geometry::batch_autocrop::{
    CropEvidence, apply_resolved_crop, detect_crop_candidate, resolve_roll_crops,
};
use negpy_backend::features::retouch::processor::downsample_to_long_edge;
use negpy_backend::infrastructure::loaders::factory::{self, LoadOptions};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::infrastructure::storage::repository::StorageRepository;
use negpy_backend::services::rendering::import::oriented;
use negpy_backend::services::rendering::lens::flatfield_source;

/// One frame the batch reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropFrame {
    pub path: PathBuf,
    pub hash: String,
    pub config_json: String,
}

/// One Crop All Frames run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropRequest {
    pub frames: Vec<CropFrame>,
    pub edits_db_path: PathBuf,
    pub settings_db_path: PathBuf,
    pub flatfield_dir: PathBuf,
}

/// What one run saved and kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CropReport {
    pub saved: usize,
    pub kept: usize,
    pub failed: usize,
}

/// The GUI side of the crop worker.
pub struct CropWorker {
    requests: Option<std::sync::mpsc::Sender<CropRequest>>,
    results: Receiver<CropReport>,
    thread: Option<JoinHandle<()>>,
}

impl CropWorker {
    /// Start the worker thread.
    #[must_use]
    pub fn start() -> Self {
        let (requests, incoming) = channel();
        let (sender, results) = channel();
        let thread = std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                if sender.send(run(&request)).is_err() {
                    break;
                }
            }
        });
        Self {
            requests: Some(requests),
            results,
            thread: Some(thread),
        }
    }

    /// Queue one Crop All Frames run.
    pub fn submit(&self, request: CropRequest) {
        if let Some(requests) = &self.requests {
            let _ = requests.send(request);
        }
    }

    /// The finished run, when the worker has one.
    #[must_use]
    pub fn drain(&self) -> Option<CropReport> {
        match self.results.try_recv() {
            Ok(report) => Some(report),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for CropWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Detect every frame, resolve the roll, then save each crop the resolve kept.
#[must_use]
pub fn run(request: &CropRequest) -> CropReport {
    let mut report = CropReport::default();
    let mut evidence = Vec::new();
    let mut evidence_index = Vec::new();
    for (index, frame) in request.frames.iter().enumerate() {
        let Some(found) = evidence_for(frame, &request.flatfield_dir) else {
            report.failed += 1;
            continue;
        };
        evidence.push(found);
        evidence_index.push(index);
    }
    let resolved = resolve_roll_crops(&evidence, 0.01);
    let repo = StorageRepository::new(
        request.edits_db_path.display().to_string(),
        request.settings_db_path.display().to_string(),
    );
    for crop in resolved {
        let Some(position) = evidence.iter().position(|item| item.key == crop.key) else {
            report.kept += 1;
            continue;
        };
        let frame = &request.frames[evidence_index[position]];
        let config = read_config(&repo, &frame.hash, &frame.config_json);
        let Some(updated) = apply_resolved_crop(&config, &crop) else {
            report.kept += 1;
            continue;
        };
        match repo.save_file_settings(&frame.hash, &updated, &frame.path.display().to_string()) {
            Ok(()) => report.saved += 1,
            Err(_) => report.failed += 1,
        }
    }
    report.kept += request.frames.len().saturating_sub(evidence.len());
    report
}

fn read_config(
    repo: &StorageRepository,
    hash: &str,
    fallback: &str,
) -> negpy_backend::domain::models::WorkspaceConfig {
    repo.load_file_settings(hash)
        .ok()
        .flatten()
        .or_else(|| {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(fallback)
                .ok()
                .map(|flat| negpy_backend::domain::models::WorkspaceConfig::from_flat_dict(&flat))
        })
        .unwrap_or_default()
}

fn evidence_for(frame: &CropFrame, flatfield_dir: &std::path::Path) -> Option<CropEvidence> {
    let config =
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&frame.config_json)
            .ok()
            .map(|flat| negpy_backend::domain::models::WorkspaceConfig::from_flat_dict(&flat))?;
    let loaded = factory::load(
        &frame.path,
        LoadOptions {
            linear_raw: config.process.linear_raw,
            positive_decode: None,
            load_ir: false,
        },
    )
    .ok()?;
    let decoded = loaded.image?;
    let oriented = oriented(&decoded, loaded.orientation);
    let entry = config
        .flatfield
        .apply
        .then(|| {
            negpy_backend::services::assets::flatfield::FlatFieldProfiles::new(flatfield_dir)
                .load_gain(&config.flatfield.profile_id)
        })
        .flatten();
    let corrected = flatfield_source(
        &oriented,
        &config,
        entry.as_ref(),
        &[],
        Some(loaded.orientation),
    );
    let proxy = downsample_to_long_edge(&corrected, 1600);
    let detection_geometry = negpy_backend::domain::models::GeometryConfig {
        crop_rect: None,
        crop_from_auto: false,
        autocrop_offset: 0,
        ..config.geometry.clone()
    };
    let detection = negpy_backend::features::geometry::processor::process(
        &proxy,
        &detection_geometry,
        negpy_backend::features::geometry::processor::GeometryContext { scale_factor: 1.0 },
    );
    Some(detect_crop_candidate(
        &frame.hash,
        &ImageBuffer::from_rgb(
            detection.image.width,
            detection.image.height,
            detection.image.pixels,
        ),
        config.geometry.autocrop_ratio.as_str(),
        config.geometry.autocrop_rebate_trim,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_jpeg(path: &std::path::Path, top: u8, bottom: u8) {
        let width = 120;
        let height = 90;
        let mut pixels = Vec::new();
        for y in 0..height {
            let value = if y < 12 || y >= height - 12 {
                top
            } else {
                bottom
            };
            for _ in 0..width {
                pixels.extend_from_slice(&[value, value, value]);
            }
        }
        let image = image::RgbImage::from_raw(width, height, pixels).unwrap();
        let mut file = std::fs::File::create(path).unwrap();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 100)
            .encode_image(&image)
            .unwrap();
    }

    #[test]
    fn a_batch_run_reads_evidence_for_each_frame() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let mut frames = Vec::new();
        for (index, name) in ["a.jpg", "b.jpg"].into_iter().enumerate() {
            let path = photos.join(name);
            write_jpeg(&path, 10 + index as u8, 200);
            let hash = negpy_backend::infrastructure::storage::local_asset_store::file_hash(&path)
                .unwrap();
            let config = negpy_backend::domain::models::WorkspaceConfig::default();
            frames.push(CropFrame {
                path,
                hash,
                config_json: serde_json::Value::Object(config.to_dict()).to_string(),
            });
        }
        let request = CropRequest {
            frames,
            edits_db_path: dir.path().join("edits.db"),
            settings_db_path: dir.path().join("settings.db"),
            flatfield_dir: dir.path().join("flatfield"),
        };
        for frame in &request.frames {
            assert!(
                evidence_for(frame, &request.flatfield_dir).is_some(),
                "{} yields evidence",
                frame.path.display()
            );
        }
        let report = run(&request);
        assert_eq!(report.saved + report.kept + report.failed, 2, "{report:?}");
    }
}
