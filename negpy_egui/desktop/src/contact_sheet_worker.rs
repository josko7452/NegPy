//! Render and write contact-sheet pages on a background thread.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;

use negpy_backend::features::metadata::gear::GearLibrary;
use negpy_backend::services::export::contact_sheet_layout::ContactSheetSettings;
use negpy_backend::services::export::contact_sheet_page::{
    SheetEntry, encode_png, render_contact_sheet,
};

/// One sheet render: its frames, settings, roll name and optional output.
#[derive(Clone, Debug)]
pub struct SheetRequest {
    pub entries: Vec<SheetEntry>,
    pub settings: ContactSheetSettings,
    pub roll_name: String,
    pub library: Option<GearLibrary>,
    pub page: usize,
    pub output: Option<PathBuf>,
    pub request_id: u64,
}

/// A rendered page and the file it wrote, when the request asked for one.
#[derive(Clone, Debug)]
pub struct SheetReport {
    pub request_id: u64,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub written: Option<PathBuf>,
    pub error: Option<String>,
}

/// The GUI side of the contact-sheet worker.
pub struct SheetWorker {
    requests: std::sync::mpsc::Sender<SheetRequest>,
    results: Receiver<SheetReport>,
    thread: Option<JoinHandle<()>>,
}

impl SheetWorker {
    #[must_use]
    pub fn start() -> Self {
        let (requests, incoming) = channel();
        let (sender, results) = channel();
        let thread = std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                let report = run_sheet(&request);
                if sender.send(report).is_err() {
                    break;
                }
            }
        });
        Self {
            requests,
            results,
            thread: Some(thread),
        }
    }

    pub fn submit(&self, request: SheetRequest) {
        let _ = self.requests.send(request);
    }

    #[must_use]
    pub fn drain(&self) -> Vec<SheetReport> {
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

impl Drop for SheetWorker {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let (sender, _) = channel();
            drop(std::mem::replace(&mut self.requests, sender));
            let _ = thread.join();
        }
    }
}

/// Render one page, and write it as a PNG when `output` names a file.
#[must_use]
pub fn run_sheet(request: &SheetRequest) -> SheetReport {
    let canvas = render_contact_sheet(
        &request.entries,
        &request.settings,
        &request.roll_name,
        request.library.as_ref(),
        request.page,
        false,
    );
    let mut written = None;
    let mut error = None;
    if let Some(path) = &request.output {
        match encode_png(&canvas) {
            Some(bytes) => {
                if let Some(parent) = path.parent()
                    && let Err(write_error) = std::fs::create_dir_all(parent)
                {
                    error = Some(write_error.to_string());
                }
                if error.is_none() {
                    match std::fs::write(path, bytes) {
                        Ok(()) => written = Some(path.clone()),
                        Err(write_error) => error = Some(write_error.to_string()),
                    }
                }
            }
            None => error = Some("Could not encode the sheet.".to_string()),
        }
    }
    SheetReport {
        request_id: request.request_id,
        width: canvas.width,
        height: canvas.height,
        pixels: canvas.pixels,
        written,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use negpy_backend::domain::models::WorkspaceConfig;
    use negpy_backend::services::export::contact_sheet::Tile;

    fn request(output: Option<PathBuf>) -> SheetRequest {
        SheetRequest {
            entries: vec![SheetEntry {
                name: "a".to_string(),
                path: "/roll/a.tif".to_string(),
                config: WorkspaceConfig::default(),
                tile: Tile::new(4, 4, vec![90; 48]),
            }],
            settings: ContactSheetSettings::default(),
            roll_name: "Roll 1".to_string(),
            library: None,
            page: 0,
            output,
            request_id: 1,
        }
    }

    #[test]
    fn a_preview_returns_its_pixels() {
        let report = run_sheet(&request(None));
        assert!(report.error.is_none());
        assert_eq!(report.pixels.len(), report.width * report.height * 3);
        assert!(report.written.is_none());
    }

    #[test]
    fn an_export_writes_a_png() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sheet.png");
        let report = run_sheet(&request(Some(path.clone())));
        assert!(report.error.is_none());
        assert_eq!(report.written.as_deref(), Some(path.as_path()));
        assert_eq!(&std::fs::read(path).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
    }
}
