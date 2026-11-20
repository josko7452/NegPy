//! The scanner worker: one background thread turns a request into a written TIFF.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use negpy_backend::services::scanning::params::ScanParams;
use negpy_backend::services::scanning::registry;
use negpy_backend::services::scanning::service::ScannerService;

/// One scan: the selected backend, device, parameters and output.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanRequest {
    pub backend: String,
    pub device: String,
    pub params: ScanParams,
    pub output_folder: PathBuf,
    pub filename_pattern: String,
    pub output_format: String,
}

/// The file one scan wrote, or the error that stopped it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanReport {
    pub path: Option<PathBuf>,
    pub error: Option<String>,
}

/// The GUI side of the scanner worker.
pub struct ScanWorker {
    requests: Option<std::sync::mpsc::Sender<ScanRequest>>,
    results: Receiver<ScanReport>,
    thread: Option<JoinHandle<()>>,
}

impl ScanWorker {
    #[must_use]
    pub fn start() -> Self {
        let (requests, incoming) = channel();
        let (sender, results) = channel();
        let thread = std::thread::spawn(move || {
            while let Ok(request) = incoming.recv() {
                let report = run_scan(&request);
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

    pub fn submit(&self, request: ScanRequest) {
        if let Some(requests) = &self.requests {
            let _ = requests.send(request);
        }
    }

    #[must_use]
    pub fn drain(&self) -> Vec<ScanReport> {
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

impl Drop for ScanWorker {
    fn drop(&mut self) {
        self.requests.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Run one scan and write its capture.
#[must_use]
pub fn run_scan(request: &ScanRequest) -> ScanReport {
    let backend = match registry::create_backend(&request.backend) {
        Ok(backend) => backend,
        Err(error) => {
            return ScanReport {
                path: None,
                error: Some(error.to_string()),
            };
        }
    };
    let mut service = ScannerService::new(backend);
    let cancel: negpy_backend::services::scanning::transport::Cancel =
        std::sync::Arc::new(|| false);
    let progress: negpy_backend::services::scanning::transport::Progress =
        std::sync::Arc::new(|_: f64, _: Option<&str>| {});
    let result = match service.run_scan(
        &request.device,
        &request.params,
        &progress,
        &cancel,
        Duration::from_millis(500),
    ) {
        Ok(result) => result,
        Err(error) => {
            return ScanReport {
                path: None,
                error: Some(error.message().to_string()),
            };
        }
    };
    let (width, height) = result.size();
    match service.write_result(
        &result,
        &request.output_folder,
        &request.filename_pattern,
        &request.output_format,
        None,
        width,
        height,
    ) {
        Ok(path) => ScanReport {
            path: Some(path),
            error: None,
        },
        Err(error) => ScanReport {
            path: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static SIMULATION: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn an_unavailable_backend_reports_its_hint() {
        let request = ScanRequest {
            backend: "plustek".to_string(),
            device: "missing".to_string(),
            params: ScanParams::default(),
            output_folder: PathBuf::new(),
            filename_pattern: "scan_{{ seq }}".to_string(),
            output_format: "TIFF".to_string(),
        };
        let report = run_scan(&request);
        assert!(report.path.is_none());
        assert!(report.error.unwrap_or_default().contains("plustek"));
    }

    #[test]
    fn a_simulated_capture_is_written_with_its_size() {
        let _guard = SIMULATION.lock().unwrap();
        unsafe {
            std::env::set_var("NEGPY_SIMULATE_HARDWARE", "1");
        }
        let dir = tempfile::tempdir().unwrap();
        let request = ScanRequest {
            backend: "sim".to_string(),
            device: "sim:feeder".to_string(),
            params: ScanParams {
                dpi: 300,
                depth: 8,
                frame: Some(0),
                ..ScanParams::default()
            },
            output_folder: dir.path().to_path_buf(),
            filename_pattern: "scan_{{ seq }}".to_string(),
            output_format: "TIFF".to_string(),
        };
        let report = run_scan(&request);
        assert!(report.error.is_none(), "{:?}", report.error);
        let path = report.path.unwrap();
        assert!(path.exists());
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("tif"));
        unsafe {
            std::env::remove_var("NEGPY_SIMULATE_HARDWARE");
        }
    }
}
