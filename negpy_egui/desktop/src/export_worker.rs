//! The roll export worker: one background thread walks the strip.
//!
//! The GUI sends one snapshot. The worker decodes and renders each frame at full size,
//! writes it through the export service, and reports the files it wrote and failed.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::render_engine::DarkroomEngine;
use negpy_backend::services::assets::sidecar::write_sidecar;
use negpy_backend::services::export::render::render_export_file;

/// One roll export: every source path with the edit it prints with.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportRequest {
    pub paths: Vec<PathBuf>,
    pub configs: Vec<WorkspaceConfig>,
    /// The export config each path writes with, when it is not the frame's own.
    pub preset_exports: Vec<Option<negpy_backend::domain::models::ExportConfig>>,
    pub color_space: String,
    pub user_icc_dir: PathBuf,
}

/// The files one export wrote and the frames that failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub written: Vec<PathBuf>,
    pub failed: Vec<String>,
}

/// The GUI side of the export worker.
pub struct ExportWorker {
    requests: Option<std::sync::mpsc::Sender<ExportRequest>>,
    results: Receiver<ExportReport>,
    thread: Option<JoinHandle<()>>,
}

impl ExportWorker {
    /// Start the worker thread.
    #[must_use]
    pub fn start() -> Self {
        let (requests, incoming) = channel();
        let (sender, results) = channel();
        let thread = std::thread::spawn(move || {
            let mut engine = DarkroomEngine::new();
            while let Ok(request) = incoming.recv() {
                let report = run_export(&request, &mut engine);
                if sender.send(report).is_err() {
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

    /// Queue one roll export.
    pub fn submit(&self, request: ExportRequest) {
        if let Some(requests) = &self.requests {
            let _ = requests.send(request);
        }
    }

    /// Take every finished report without blocking.
    #[must_use]
    pub fn drain(&self) -> Vec<ExportReport> {
        let mut reports = Vec::new();
        loop {
            match self.results.try_recv() {
                Ok(report) => reports.push(report),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        reports
    }
}

impl Drop for ExportWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Render and write every frame in one request.
#[must_use]
pub fn run_export(request: &ExportRequest, engine: &mut DarkroomEngine) -> ExportReport {
    let mut report = ExportReport::default();
    let mut written = std::collections::HashSet::new();
    for (index, (path, config)) in request.paths.iter().zip(&request.configs).enumerate() {
        let preset = request.preset_exports.get(index).and_then(Option::as_ref);
        match export_one(
            path,
            config,
            preset,
            &request.color_space,
            &request.user_icc_dir,
            engine,
            &mut written,
        ) {
            Ok(written) => report.written.push(written),
            Err(error) => report.failed.push(error),
        }
    }
    report
}

fn export_one(
    path: &Path,
    config: &WorkspaceConfig,
    preset: Option<&negpy_backend::domain::models::ExportConfig>,
    color_space: &str,
    user_icc_dir: &Path,
    engine: &mut DarkroomEngine,
    written: &mut std::collections::HashSet<PathBuf>,
) -> Result<PathBuf, String> {
    let mut frame = config.clone();
    if let Some(preset) = preset {
        frame.export = preset.clone();
    }
    let render = render_export_file(path, &frame, Some(color_space), engine)?;
    let written = negpy_backend::services::export::service::write_export_with_names(
        &ImageBuffer::from_rgb(render.image.width, render.image.height, render.image.pixels),
        path,
        &frame.export,
        &frame,
        &render.color_space,
        user_icc_dir,
        written,
    )?;
    if config.export.export_sidecars_enabled {
        write_sidecar(path, config, 0)
            .map_err(|error| format!("Could not write the sidecar: {error}"))?;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sidecar_toggle_writes_the_edit_beside_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("frame.orf");
        std::fs::write(&source, b"frame").unwrap();
        let mut config = WorkspaceConfig::default();
        config.export.export_sidecars_enabled = true;
        write_sidecar(&source, &config, 0).unwrap();
        assert!(dir.path().join("frame.negpy").exists());
        let read = negpy_backend::services::assets::sidecar::load_sidecar(&source, 0);
        assert!(read.is_some());
    }

    #[test]
    fn an_empty_request_writes_nothing() {
        let request = ExportRequest {
            paths: Vec::new(),
            configs: Vec::new(),
            preset_exports: Vec::new(),
            color_space: "sRGB".to_string(),
            user_icc_dir: PathBuf::new(),
        };
        let mut engine = DarkroomEngine::new();
        let report = run_export(&request, &mut engine);
        assert!(report.written.is_empty());
        assert!(report.failed.is_empty());
    }

    #[test]
    fn dropping_the_worker_stops_its_thread() {
        let worker = ExportWorker::start();
        drop(worker);
    }

    #[test]
    fn a_missing_source_is_reported_once() {
        let request = ExportRequest {
            paths: vec![PathBuf::from("/missing/frame.dng")],
            configs: vec![WorkspaceConfig::default()],
            preset_exports: vec![None],
            color_space: "sRGB".to_string(),
            user_icc_dir: PathBuf::new(),
        };
        let mut engine = DarkroomEngine::new();
        let report = run_export(&request, &mut engine);
        assert!(report.written.is_empty());
        assert_eq!(report.failed.len(), 1);
    }
}
