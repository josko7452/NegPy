//! Ports the retry and per-frame preview cases from `tests/scanners/test_service.py`
//! and `tests/scanners/test_per_frame_roll.py`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use negpy_backend::services::scanning::params::ScanParams;
use negpy_backend::services::scanning::roll::{
    FrameScanner, PerFrameRollSession, RollSession, effective_pitch_mm,
};
use negpy_backend::services::scanning::service::{SCAN_IO_RETRY_ATTEMPTS, ScannerService};
use negpy_backend::services::scanning::simulated::SimulatedBackend;
use negpy_backend::services::scanning::transport::{
    Cancel, Progress, ScanError, ScanResult, ScannerBackend, ScannerCapabilities, ScannerDevice,
    ScannerSession, ScannerUnavailable,
};

fn no_cancel() -> Cancel {
    Arc::new(|| false)
}

fn no_progress() -> Progress {
    Arc::new(|_, _| {})
}

/// A backend whose scan fails transiently a fixed number of times before it succeeds.
struct FlakyBackend {
    device: ScannerDevice,
    transient_failures: Arc<AtomicU32>,
    opens: Arc<AtomicU32>,
}

impl FlakyBackend {
    fn new(transient_failures: u32) -> Self {
        Self {
            device: ScannerDevice {
                id: "flaky:0".to_string(),
                vendor: "Test".to_string(),
                model: "Flaky".to_string(),
                capabilities: ScannerCapabilities::default(),
            },
            transient_failures: Arc::new(AtomicU32::new(transient_failures)),
            opens: Arc::new(AtomicU32::new(0)),
        }
    }
}

struct FlakySession {
    device: ScannerDevice,
    transient_failures: Arc<AtomicU32>,
}

impl ScannerSession for FlakySession {
    fn device_id(&self) -> &str {
        &self.device.id
    }

    fn scan(
        &mut self,
        params: &ScanParams,
        _progress: &Progress,
        _cancel: &Cancel,
    ) -> Result<ScanResult, ScanError> {
        if self.transient_failures.load(Ordering::SeqCst) > 0 {
            self.transient_failures.fetch_sub(1, Ordering::SeqCst);
            return Err(ScanError::Transient("device I/O".to_string()));
        }
        Ok(ScanResult {
            rgb: vec![0; 12],
            dpi: params.dpi,
            device_model: "Flaky".to_string(),
            ..ScanResult::default()
        })
    }

    fn eject(&mut self) -> Result<bool, String> {
        Ok(false)
    }

    fn close(&mut self) {}
}

impl ScannerBackend for FlakyBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerUnavailable> {
        Ok(vec![self.device.clone()])
    }

    fn open_session(&mut self, _device_id: &str) -> Result<Box<dyn ScannerSession>, String> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FlakySession {
            device: self.device.clone(),
            transient_failures: Arc::clone(&self.transient_failures),
        }))
    }

    fn eject(&mut self, _device_id: &str) -> Result<bool, String> {
        Ok(false)
    }
}

/// A backend that offers the optional film and metering hooks.
struct OptionalHooksBackend {
    device: ScannerDevice,
}

impl ScannerBackend for OptionalHooksBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerUnavailable> {
        Ok(vec![self.device.clone()])
    }

    fn open_session(&mut self, _device_id: &str) -> Result<Box<dyn ScannerSession>, String> {
        Err("not used".to_string())
    }

    fn eject(&mut self, _device_id: &str) -> Result<bool, String> {
        Ok(false)
    }

    fn media_loaded(&mut self, _device_id: &str) -> Option<bool> {
        Some(true)
    }

    fn detect_frames(
        &mut self,
        _device_id: &str,
        _film_format: Option<&str>,
        _film_type: &str,
    ) -> usize {
        6
    }

    fn meter(
        &mut self,
        _device_id: &str,
        _params: &ScanParams,
        _progress: &Progress,
        _cancel: &Cancel,
    ) -> Result<std::collections::BTreeMap<String, i64>, String> {
        Ok(std::collections::BTreeMap::from([("red".to_string(), 7)]))
    }
}

#[test]
fn the_optional_hooks_report_what_the_backend_can_tell() {
    let device = pitch_device(Some(3), 38.0);
    let mut service = ScannerService::new(Box::new(OptionalHooksBackend { device }));
    assert_eq!(service.media_loaded("x"), Some(true));
    assert_eq!(service.detect_frames("x", Some("135"), "negative"), 6);
    assert_eq!(
        service
            .meter("x", &ScanParams::default(), &no_progress(), &no_cancel())
            .unwrap()
            .get("red"),
        Some(&7)
    );
}

#[test]
fn a_backend_without_the_hooks_reports_nothing() {
    let backend = FlakyBackend::new(0);
    let mut service = ScannerService::new(Box::new(backend));
    assert_eq!(service.media_loaded("x"), None);
    assert_eq!(service.detect_frames("x", None, "negative"), 0);
    assert_eq!(
        service
            .meter("x", &ScanParams::default(), &no_progress(), &no_cancel())
            .unwrap_err(),
        "This scanner cannot meter a frame on its own"
    );
}

#[test]
fn a_transient_failure_is_retried_with_a_fresh_open() {
    let opens = Arc::new(AtomicU32::new(0));
    let backend = FlakyBackend {
        device: ScannerDevice {
            id: "flaky:0".to_string(),
            vendor: "Test".to_string(),
            model: "Flaky".to_string(),
            capabilities: ScannerCapabilities::default(),
        },
        transient_failures: Arc::new(AtomicU32::new(2)),
        opens: Arc::clone(&opens),
    };
    let mut service = ScannerService::new(Box::new(backend));
    let result = service.run_scan(
        "flaky:0",
        &ScanParams::default(),
        &no_progress(),
        &no_cancel(),
        Duration::ZERO,
    );
    assert!(result.is_ok());
    assert_eq!(opens.load(Ordering::SeqCst), SCAN_IO_RETRY_ATTEMPTS);
}

#[test]
fn a_fatal_failure_is_not_retried() {
    let backend = FlakyBackend::new(SCAN_IO_RETRY_ATTEMPTS + 1);
    let mut service = ScannerService::new(Box::new(backend));
    let error = service
        .run_scan(
            "flaky:0",
            &ScanParams::default(),
            &no_progress(),
            &no_cancel(),
            Duration::ZERO,
        )
        .unwrap_err();
    assert!(error.is_transient());
}

#[test]
fn the_service_lists_and_opens_the_simulated_devices() {
    let mut service = ScannerService::new(Box::new(SimulatedBackend::new()));
    let devices = service.list_devices().unwrap();
    assert_eq!(devices.len(), 3);
    let mut session = service.open_session(&devices[0].id).unwrap();
    assert_eq!(session.device_id(), devices[0].id);
    session.close();
}

fn pitch_device(capacity: Option<usize>, pitch: f64) -> ScannerDevice {
    ScannerDevice {
        id: "coolscan3:test".to_string(),
        vendor: "Nikon".to_string(),
        model: "LS-50".to_string(),
        capabilities: ScannerCapabilities {
            adapter_frame_capacity: capacity,
            frame_pitch_mm: pitch,
            max_area_mm: (25.0, 38.0),
            ..ScannerCapabilities::default()
        },
    }
}

/// A frame scanner that records what each preview asked for.
struct RecordingScanner {
    seen: Vec<ScanParams>,
    fail_on: Option<usize>,
    cancel_on: Option<usize>,
}

impl FrameScanner for RecordingScanner {
    fn scan(
        &mut self,
        _device_id: &str,
        params: &ScanParams,
        _progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError> {
        self.seen.push(params.clone());
        if let Some(slot) = self.cancel_on {
            if params.frame == Some(slot as i64) {
                let _ = cancel;
                return Err(ScanError::Fatal("cancelled mid-scan".to_string()));
            }
        }
        if self.fail_on == params.frame.map(|frame| frame as usize) {
            return Err(ScanError::Fatal("jammed".to_string()));
        }
        Ok(ScanResult {
            rgb: vec![0; 4 * 4 * 3],
            dpi: params.dpi,
            ..ScanResult::default()
        })
    }
}

#[test]
fn previews_yield_one_slot_each_in_order() {
    let device = pitch_device(Some(3), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    let previews = session.preview(&[0, 1, 2], &no_cancel(), None);
    assert_eq!(
        previews
            .iter()
            .map(|preview| preview.slot)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(
        previews
            .iter()
            .all(|preview| preview.rgb.is_some() && preview.error.is_none())
    );
    assert_eq!(
        scanner
            .seen
            .iter()
            .map(|params| params.frame.unwrap())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn previews_are_cheap_reads() {
    let device = pitch_device(Some(3), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 400);
    session.preview(&[1], &no_cancel(), None);
    let params = &scanner.seen[0];
    assert_eq!((params.dpi, params.depth), (400, 8));
    assert!(!params.capture_ir);
    assert!(!params.autofocus);
    assert!(!params.auto_exposure);
    assert!(params.window.is_none());
}

#[test]
fn a_failed_slot_is_yielded_and_the_strip_continues() {
    let device = pitch_device(Some(3), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: Some(2),
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    let previews = session.preview(&[1, 2, 3], &no_cancel(), None);
    assert_eq!(previews.len(), 3);
    assert!(previews[1].rgb.is_none());
    assert!(previews[2].rgb.is_some());
}

#[test]
fn the_service_writes_a_scan_under_a_free_name() {
    let dir = tempfile::tempdir().unwrap();
    let service = ScannerService::new(Box::new(FlakyBackend::new(0)));
    let result = ScanResult {
        rgb: vec![0u8; 4 * 3 * 3],
        dpi: 300,
        ..ScanResult::default()
    };
    let first = service
        .write_result(&result, dir.path(), "s_{{ seq }}", "TIFF", None, 4, 3)
        .unwrap();
    let second = service
        .write_result(&result, dir.path(), "s_{{ seq }}", "TIFF", None, 4, 3)
        .unwrap();
    assert_ne!(first, second);
    assert!(first.exists() && second.exists());
}

#[test]
fn a_batch_reuses_the_frame_number_as_the_first_name() {
    let dir = tempfile::tempdir().unwrap();
    let service = ScannerService::new(Box::new(FlakyBackend::new(0)));
    let result = ScanResult {
        rgb: vec![0u8; 4 * 3 * 3],
        dpi: 300,
        ..ScanResult::default()
    };
    let path = service
        .write_result(&result, dir.path(), "s_{{ seq }}", "TIFF", Some(7), 4, 3)
        .unwrap();
    assert_eq!(path.file_name().unwrap(), "s_7.tif");
}

#[test]
fn offsets_convert_from_pitch_fractions_to_mm() {
    let device = pitch_device(Some(3), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    session.set_offset(1, 0.25);
    let previews = session.preview(&[1], &no_cancel(), None);
    assert!((scanner.seen[0].frame_offset_mm - 9.5).abs() < 1e-9);
    assert!((previews[0].offset - 0.25).abs() < 1e-9);
}

#[test]
fn an_unknown_pitch_disables_offsets_rather_than_guessing() {
    let mut device = pitch_device(Some(2), 0.0);
    device.capabilities.max_area_mm = (0.0, 0.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    assert_eq!(effective_pitch_mm(&device.capabilities), 0.0);
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    session.set_offset(1, 0.5);
    session.preview(&[1], &no_cancel(), None);
    assert_eq!(scanner.seen[0].frame_offset_mm, 0.0);
}

#[test]
fn an_unknown_pitch_with_no_area_reports_zero() {
    let device = ScannerDevice {
        id: "x".to_string(),
        vendor: "Test".to_string(),
        model: "X".to_string(),
        capabilities: ScannerCapabilities::default(),
    };
    assert_eq!(effective_pitch_mm(&device.capabilities), 0.0);
}

#[test]
fn the_slot_count_follows_the_adapter_capacity() {
    let device = pitch_device(Some(4), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let session = PerFrameRollSession::new(&mut scanner, device, 1000);
    assert_eq!(session.slot_count(), 4);
    let bare = pitch_device(None, 38.0);
    let session = PerFrameRollSession::new(&mut scanner, bare, 1000);
    assert_eq!(session.slot_count(), 1);
}

#[test]
fn a_frame_less_device_omits_frame_rather_than_requesting_frame_one() {
    let device = pitch_device(None, 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    session.preview(&[1], &no_cancel(), None);
    assert!(scanner.seen[0].frame.is_none());
}

#[test]
fn approve_and_close_are_no_ops() {
    let device = pitch_device(Some(3), 38.0);
    let mut scanner = RecordingScanner {
        seen: Vec::new(),
        fail_on: None,
        cancel_on: None,
    };
    let mut session = PerFrameRollSession::new(&mut scanner, device, 1000);
    session.approve(1);
    session.close();
    assert_eq!((session.offset_range()), (0.0, 1.0));
    assert!(session.supports_single_slot_preview());
}
