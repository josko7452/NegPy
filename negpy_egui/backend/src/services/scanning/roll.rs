//! Whole-strip preview, independent of how a transport delivers it.
//!
//! Mirrors `negpy/infrastructure/scanners/roll.py` and `per_frame_roll.py`: a scanner
//! that reads one frame at a time and one that reads the whole strip in one traversal
//! both yield per-slot previews through this seam.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::services::scanning::params::{ScanMode, ScanParams, clamp_frame_offset_mm};
use crate::services::scanning::transport::{
    Cancel, Progress, ScanError, ScanResult, ScannerCapabilities, ScannerDevice,
};

/// Feed-axis distance between frame positions, falling back to the frame's long side
/// when the transport does not report one. 0.0 when neither is known.
#[must_use]
pub fn effective_pitch_mm(caps: &ScannerCapabilities) -> f64 {
    if caps.frame_pitch_mm != 0.0 {
        return caps.frame_pitch_mm;
    }
    caps.max_area_mm.1
}

/// One slot's preview. It carries its own slot number, so results may arrive in any order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RollPreview {
    pub slot: usize,
    pub rgb: Option<Vec<u8>>,
    /// Set instead of `rgb` when this slot alone failed. The strip continues.
    pub error: Option<String>,
    /// The effective feed-axis offset the preview was taken at, as a fraction of one pitch.
    pub offset: f64,
    /// The transport inferred this slot's boundary rather than measuring it.
    pub needs_approval: bool,
    pub warnings: Vec<String>,
}

/// What a backend must offer for a per-frame roll session over single-frame scans.
pub trait FrameScanner: Send {
    fn scan(
        &mut self,
        device_id: &str,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError>;
}

/// An open strip: preview its slots, nudge their boundaries, release it.
///
/// A session does not own file writing, and `close` is terminal and idempotent.
pub trait RollSession: Send {
    fn slot_count(&self) -> usize;
    /// Legal `set_offset` bounds as a fraction of one frame pitch.
    fn offset_range(&self) -> (f64, f64);
    /// False when previewing costs a whole-strip traversal.
    fn supports_single_slot_preview(&self) -> bool;
    /// Yield one preview per requested slot, as each becomes available.
    fn preview(
        &mut self,
        slots: &[usize],
        cancel: &Cancel,
        progress: Option<&Progress>,
    ) -> Vec<RollPreview>;
    /// Move a slot's frame boundary, as a fraction of one pitch.
    fn set_offset(&mut self, slot: usize, offset: f64);
    /// Confirm an inferred boundary so the slot may be scanned. No-op where unneeded.
    fn approve(&mut self, slot: usize);
    fn close(&mut self);
}

/// Preview depth: previews are for looking at, not for developing.
pub const PREVIEW_DEPTH: i64 = 8;

/// Whole-strip preview built from single-frame scans.
///
/// It holds no device open between slots: each scan opens and closes, which keeps a
/// backend's re-enumeration recovery reachable mid-strip.
pub struct PerFrameRollSession<'a> {
    backend: &'a mut dyn FrameScanner,
    device: ScannerDevice,
    dpi: i64,
    pitch_mm: f64,
    offsets: BTreeMap<usize, f64>,
}

impl<'a> PerFrameRollSession<'a> {
    #[must_use]
    pub fn new(backend: &'a mut dyn FrameScanner, device: ScannerDevice, dpi: i64) -> Self {
        let pitch_mm = effective_pitch_mm(&device.capabilities);
        Self {
            backend,
            device,
            dpi,
            pitch_mm,
            offsets: BTreeMap::new(),
        }
    }

    fn offset_mm(&self, slot: usize) -> f64 {
        if self.pitch_mm == 0.0 {
            return 0.0;
        }
        clamp_frame_offset_mm(
            self.offsets.get(&slot).copied().unwrap_or(0.0) * self.pitch_mm,
            self.pitch_mm,
        )
    }

    fn as_fraction(&self, offset_mm: f64) -> f64 {
        if self.pitch_mm == 0.0 {
            0.0
        } else {
            offset_mm / self.pitch_mm
        }
    }
}

impl RollSession for PerFrameRollSession<'_> {
    fn slot_count(&self) -> usize {
        self.device
            .capabilities
            .adapter_frame_capacity
            .unwrap_or(1)
            .max(1)
    }

    fn offset_range(&self) -> (f64, f64) {
        (0.0, 1.0)
    }

    fn supports_single_slot_preview(&self) -> bool {
        true
    }

    fn preview(
        &mut self,
        slots: &[usize],
        cancel: &Cancel,
        progress: Option<&Progress>,
    ) -> Vec<RollPreview> {
        let has_adapter = self.device.capabilities.adapter_frame_capacity.is_some();
        let no_progress = Arc::new(|_fraction: f64, _phase: Option<&str>| {})
            as Arc<dyn Fn(f64, Option<&str>) + Send + Sync>;
        let progress = progress.unwrap_or(&no_progress);
        let mut out = Vec::with_capacity(slots.len());
        for &slot in slots {
            if cancel() {
                break;
            }
            let offset_mm = self.offset_mm(slot);
            let params = ScanParams {
                dpi: self.dpi,
                depth: PREVIEW_DEPTH,
                capture_ir: false,
                window: None,
                frame_offset_mm: offset_mm,
                autofocus: false,
                auto_exposure: false,
                frame: has_adapter.then_some(slot as i64),
                ..ScanParams::default()
            };
            match self
                .backend
                .scan(&self.device.id, &params, progress, cancel)
            {
                Ok(result) => out.push(RollPreview {
                    slot,
                    rgb: Some(result.rgb),
                    offset: self.as_fraction(offset_mm),
                    ..RollPreview::default()
                }),
                Err(error) => {
                    if cancel() {
                        break;
                    }
                    out.push(RollPreview {
                        slot,
                        error: Some(error.to_string()),
                        offset: self.as_fraction(offset_mm),
                        ..RollPreview::default()
                    });
                }
            }
        }
        out
    }

    fn set_offset(&mut self, slot: usize, offset: f64) {
        self.offsets.insert(slot, offset);
    }

    fn approve(&mut self, _slot: usize) {}

    fn close(&mut self) {}
}

/// The sources a transport offers, as a set.
#[must_use]
pub fn offered_modes(caps: &ScannerCapabilities) -> BTreeSet<ScanMode> {
    caps.sources.iter().copied().collect()
}

/// The slot numbers a strip preview should render.
#[must_use]
pub fn slots_for(count: usize, discovery: bool) -> Vec<usize> {
    let count = if discovery { count } else { count.max(1) };
    (0..count).collect()
}
