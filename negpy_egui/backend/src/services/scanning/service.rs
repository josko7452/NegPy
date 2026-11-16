//! The scanner service: device listing, scan retries and whole-roll sessions.
//!
//! Mirrors `negpy/services/scanning/service.py`. It knows nothing about a particular
//! transport: the backend classifies its own failures, and the service retries only a
//! `TransientScanError`.

use std::thread::sleep;
use std::time::Duration;

use crate::services::scanning::params::ScanParams;
use crate::services::scanning::roll::{FrameScanner, PerFrameRollSession, effective_pitch_mm};
use crate::services::scanning::transport::{
    Cancel, Progress, ScanError, ScanResult, ScannerBackend, ScannerDevice, ScannerSession,
};
use crate::services::scanning::writer::write_scan;

pub const SCAN_IO_RETRY_ATTEMPTS: u32 = 3;
pub const SCAN_IO_RETRY_DELAY_S: f64 = 0.5;

/// List, scan and preview through one backend.
pub struct ScannerService {
    backend: Box<dyn ScannerBackend>,
}

impl ScannerService {
    #[must_use]
    pub fn new(backend: Box<dyn ScannerBackend>) -> Self {
        Self { backend }
    }

    pub fn list_devices(&self) -> Result<Vec<ScannerDevice>, String> {
        self.backend
            .list_devices()
            .map_err(|error| error.to_string())
    }

    pub fn refresh_devices(&self) -> Result<Vec<ScannerDevice>, String> {
        self.backend
            .refresh_devices()
            .map_err(|error| error.to_string())
    }

    pub fn open_session(&mut self, device_id: &str) -> Result<Box<dyn ScannerSession>, String> {
        self.backend.open_session(device_id)
    }

    pub fn eject(&mut self, device_id: &str) -> Result<bool, String> {
        self.backend.eject(device_id)
    }

    /// Whether film is loaded. `None` where the transport cannot tell.
    pub fn media_loaded(&mut self, device_id: &str) -> Option<bool> {
        self.backend.media_loaded(device_id)
    }

    /// How many frames the loaded film carries, 0 where the transport cannot measure it.
    pub fn detect_frames(
        &mut self,
        device_id: &str,
        film_format: Option<&str>,
        film_type: &str,
    ) -> usize {
        self.backend
            .detect_frames(device_id, film_format, film_type)
    }

    /// Per-channel exposures for `params.frame`, for later scans to reuse. Writes nothing.
    pub fn meter(
        &mut self,
        device_id: &str,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<std::collections::BTreeMap<String, i64>, String> {
        self.backend.meter(device_id, params, progress, cancel)
    }

    /// Write one capture to a free basename and return the RGB path.
    ///
    /// `seq` seeds the collision search: a single scan passes `None` and starts at 1, a
    /// range batch passes the frame number so masters are frame-numbered.
    pub fn write_result(
        &self,
        result: &ScanResult,
        output_folder: &std::path::Path,
        filename_pattern: &str,
        output_format: &str,
        seq: Option<i64>,
        width: usize,
        height: usize,
    ) -> Result<std::path::PathBuf, String> {
        let date = chrono::Local::now().format("%Y%m%d").to_string();
        write_scan(
            result,
            output_folder,
            filename_pattern,
            output_format,
            &date,
            seq,
            width,
            height,
        )
    }

    /// Scan, retrying a transient USB I/O glitch with a fresh open each try.
    ///
    /// The retry opens a new session, because a glitched handle is what the retry is
    /// recovering from. A fatal error and a cancelled scan fail at once.
    pub fn run_scan(
        &mut self,
        device_id: &str,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
        retry_delay: Duration,
    ) -> Result<ScanResult, ScanError> {
        let mut attempt = 1;
        loop {
            let error = {
                let mut session = self.open_session(device_id).map_err(ScanError::Fatal)?;
                match session.scan(params, progress, cancel) {
                    Ok(result) => return Ok(result),
                    Err(error) => error,
                }
            };
            if !error.is_transient() || attempt >= SCAN_IO_RETRY_ATTEMPTS || cancel() {
                return Err(error);
            }
            sleep(retry_delay);
            if cancel() {
                return Err(error);
            }
            attempt += 1;
        }
    }

    /// Open a strip for whole-roll preview.
    #[must_use]
    pub fn open_roll(&mut self, device: ScannerDevice, dpi: i64) -> PerFrameRollSession<'_> {
        PerFrameRollSession::new(self, device, dpi)
    }
}

impl FrameScanner for ScannerService {
    fn scan(
        &mut self,
        device_id: &str,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError> {
        let mut session = self.open_session(device_id).map_err(ScanError::Fatal)?;
        session.scan(params, progress, cancel)
    }
}

/// The pitch a device reports, for callers that size a strip tile.
#[must_use]
pub fn device_pitch_mm(device: &ScannerDevice) -> f64 {
    effective_pitch_mm(&device.capabilities)
}
