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
use negpy_backend::services::export::render::render_export_file;
use negpy_backend::services::export::service::write_export;

/// One roll export: every source path with the edit it prints with.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportRequest {
    pub paths: Vec<PathBuf>,
    pub configs: Vec<WorkspaceConfig>,
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
    for (path, config) in request.paths.iter().zip(&request.configs) {
        match export_one(
            path,
            config,
            &request.color_space,
            &request.user_icc_dir,
            engine,
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
    color_space: &str,
    user_icc_dir: &Path,
    engine: &mut DarkroomEngine,
) -> Result<PathBuf, String> {
    let render = render_export_file(path, config, Some(color_space), engine)?;
    write_export(
        &ImageBuffer::from_rgb(render.image.width, render.image.height, render.image.pixels),
        path,
        &config.export,
        config,
        &render.color_space,
        user_icc_dir,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_request_writes_nothing() {
        let request = ExportRequest {
            paths: Vec::new(),
            configs: Vec::new(),
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
            color_space: "sRGB".to_string(),
            user_icc_dir: PathBuf::new(),
        };
        let mut engine = DarkroomEngine::new();
        let report = run_export(&request, &mut engine);
        assert!(report.written.is_empty());
        assert_eq!(report.failed.len(), 1);
    }
}
