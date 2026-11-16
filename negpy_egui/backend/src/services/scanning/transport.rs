//! The transport contract every scanner backend implements.
//!
//! Mirrors `negpy/infrastructure/scanners/base.py` and `result.py`: a device advertises
//! what it can do, a session owns one device for a strip, and a scan returns pixels.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::services::scanning::params::{ScanMode, ScanParams};

/// The driver or library a backend needs is not installed. The text carries the install hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannerUnavailable(pub String);

impl std::fmt::Display for ScannerUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for ScannerUnavailable {}

/// A failed scan, with the backend's own transient classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    /// A transport glitch worth retrying.
    Transient(String),
    /// A real error, which fails fast.
    Fatal(String),
}

impl ScanError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_))
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Transient(message) | Self::Fatal(message) => message,
        }
    }
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ScanError {}

/// The unit returned a measured strip by itself, so its frame picks no longer line up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripReturned {
    pub loaded: bool,
}

impl std::fmt::Display for StripReturned {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let advice = if self.loaded {
            "Preview the strip again."
        } else {
            "Insert it again, then preview it."
        };
        write!(
            formatter,
            "Its frame selection, crops and per-frame offsets were cleared. {advice}"
        )
    }
}

impl std::error::Error for StripReturned {}

/// One capture: planar or interleaved samples, its IR plane and validity mask.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub rgb: Vec<u8>,
    pub ir: Option<Vec<u8>>,
    pub dpi: i64,
    pub device_model: String,
    pub ir_valid_mask: Option<Vec<u8>>,
    pub exposures: Option<BTreeMap<String, i64>>,
    pub icc_profile: Option<Vec<u8>>,
}

/// What a device can do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScannerCapabilities {
    pub ir_channel: bool,
    pub supported_dpi: Vec<i64>,
    pub supported_depths: Vec<i64>,
    pub sources: Vec<ScanMode>,
    /// `(width, height)` in millimetres.
    pub max_area_mm: (f64, f64),
    pub dpi_fixed: bool,
    pub auto_exposure: bool,
    pub autofocus: bool,
    pub prescan: bool,
    pub prescan_dpi: i64,
    pub prescan_mirror_x: bool,
    pub prescan_default_crop: Option<[f64; 4]>,
    pub multi_exposure: bool,
    pub multi_exposure_fixed: bool,
    pub max_n_passes: i64,
    pub multi_exposure_dpi: Vec<i64>,
    pub multi_pass_dpi: Vec<i64>,
    pub adapter_frame_capacity: Option<usize>,
    pub adapter_frame_control: bool,
    pub can_eject: bool,
    pub frame_pitch_mm: f64,
    /// `(min, max)` in microseconds.
    pub exposure_time_us: Option<(i64, i64)>,
    pub hw_clean: bool,
    pub roll_discovery: bool,
    pub strip_pass: bool,
    pub film_formats: Vec<String>,
    pub film_types: Vec<String>,
    pub max_samples: i64,
    pub superfine: bool,
    pub exposure_lock: bool,
    pub analog_gain: bool,
}

impl ScannerCapabilities {
    /// Whether a long exposure is merged at this resolution.
    #[must_use]
    pub fn multi_exposure_at(&self, dpi: i64) -> bool {
        self.multi_exposure
            && (self.multi_exposure_dpi.is_empty() || self.multi_exposure_dpi.contains(&dpi))
    }

    /// Whether the fixed long exposure is available at this resolution.
    #[must_use]
    pub fn fixed_multi_exposure_at(&self, dpi: i64) -> bool {
        self.multi_exposure_fixed && self.multi_exposure_at(dpi)
    }

    /// Whether same-exposure pass stacking is available at this resolution.
    #[must_use]
    pub fn multi_pass_at(&self, dpi: i64) -> bool {
        self.max_n_passes > 1
            && (self.multi_pass_dpi.is_empty() || self.multi_pass_dpi.contains(&dpi))
    }
}

/// One device as the backend addresses it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannerDevice {
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub capabilities: ScannerCapabilities,
}

/// Progress is a fraction within the current phase.
pub type Progress = Arc<dyn Fn(f64, Option<&str>) + Send + Sync>;
/// A scan's cancellation flag.
pub type Cancel = Arc<dyn Fn() -> bool + Send + Sync>;

/// One exclusive hold on a device.
pub trait ScannerSession: Send {
    fn device_id(&self) -> &str;

    fn scan(
        &mut self,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError>;

    /// Return the transport to its idle state. False when the device has no eject action.
    fn eject(&mut self) -> Result<bool, String>;

    fn close(&mut self);
}

/// One scanner transport.
pub trait ScannerBackend: Send {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerUnavailable>;
    fn refresh_devices(&self) -> Result<Vec<ScannerDevice>, ScannerUnavailable> {
        self.list_devices()
    }
    fn open_session(&mut self, device_id: &str) -> Result<Box<dyn ScannerSession>, String>;
    fn eject(&mut self, device_id: &str) -> Result<bool, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_exposure_and_passes_follow_the_resolution_list() {
        let caps = ScannerCapabilities {
            multi_exposure: true,
            multi_exposure_fixed: true,
            max_n_passes: 4,
            multi_exposure_dpi: vec![3600],
            multi_pass_dpi: vec![2400, 3600],
            ..ScannerCapabilities::default()
        };
        assert!(caps.multi_exposure_at(3600));
        assert!(!caps.multi_exposure_at(1200));
        assert!(caps.fixed_multi_exposure_at(3600));
        assert!(caps.multi_pass_at(2400));
        assert!(!caps.multi_pass_at(1200));
        let everywhere = ScannerCapabilities {
            multi_exposure: true,
            ..ScannerCapabilities::default()
        };
        assert!(everywhere.multi_exposure_at(75));
    }

    #[test]
    fn the_strip_message_offers_the_right_next_step() {
        assert!(
            StripReturned { loaded: true }
                .to_string()
                .contains("Preview the strip")
        );
        assert!(
            StripReturned { loaded: false }
                .to_string()
                .contains("Insert it again")
        );
    }
}
