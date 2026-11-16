//! A scanner backend with no device behind it, for tests and the simulated rig.
//!
//! Mirrors the three panel shapes of `negpy/infrastructure/simulated/scanner.py`: a feeder
//! addressed by frame, a prescan device and a roll-discovery device.

use crate::services::scanning::params::{FilmType, ScanMode, ScanParams};
use crate::services::scanning::transport::{
    Cancel, Progress, ScanError, ScanResult, ScannerBackend, ScannerCapabilities, ScannerDevice,
    ScannerSession, ScannerUnavailable,
};

const AREA_MM: (f64, f64) = (36.0, 24.0);
const MAX_EDGE_PX: f64 = 2000.0;
const PROGRESS_STEPS: usize = 10;
const ROLL_FRAMES: usize = 6;

fn caps() -> ScannerCapabilities {
    ScannerCapabilities {
        ir_channel: true,
        supported_dpi: vec![1000, 2000, 4000],
        supported_depths: vec![8, 16],
        sources: vec![ScanMode::Negative, ScanMode::Positive],
        max_area_mm: AREA_MM,
        ..ScannerCapabilities::default()
    }
}

/// The devices the simulated backend offers.
#[must_use]
pub fn devices() -> Vec<ScannerDevice> {
    vec![
        ScannerDevice {
            id: "sim:feeder".to_string(),
            vendor: "Simulated".to_string(),
            model: "Coolscan (feeder)".to_string(),
            capabilities: ScannerCapabilities {
                auto_exposure: true,
                autofocus: true,
                multi_exposure: true,
                max_n_passes: 4,
                adapter_frame_capacity: Some(6),
                adapter_frame_control: true,
                can_eject: true,
                frame_pitch_mm: 38.0,
                exposure_time_us: Some((100, 100_000)),
                max_samples: 16,
                ..caps()
            },
        },
        ScannerDevice {
            id: "sim:prescan".to_string(),
            vendor: "Simulated".to_string(),
            model: "OpticFilm (prescan)".to_string(),
            capabilities: ScannerCapabilities {
                supported_dpi: vec![900, 1800, 3600, 7200],
                sources: vec![ScanMode::Transparency],
                prescan: true,
                prescan_dpi: 300,
                ..caps()
            },
        },
        ScannerDevice {
            id: "sim:roll".to_string(),
            vendor: "Simulated".to_string(),
            model: "LS-50 (roll)".to_string(),
            capabilities: ScannerCapabilities {
                supported_depths: vec![16],
                can_eject: true,
                hw_clean: true,
                roll_discovery: true,
                film_formats: vec!["135".to_string(), "120".to_string(), "110".to_string()],
                film_types: vec![
                    FilmType::Negative.as_str().to_string(),
                    FilmType::Mono.as_str().to_string(),
                    FilmType::Positive.as_str().to_string(),
                    FilmType::Kodachrome.as_str().to_string(),
                ],
                max_samples: 16,
                superfine: true,
                exposure_lock: true,
                ..caps()
            },
        },
    ]
}

fn device(device_id: &str) -> Option<ScannerDevice> {
    devices().into_iter().find(|device| device.id == device_id)
}

/// The frame size a request asks for, capped to the simulator's largest edge.
fn frame_dims(params: &ScanParams) -> (usize, usize) {
    let (mut width_mm, mut height_mm) = AREA_MM;
    if let Some([x1, y1, x2, y2]) = params.window {
        width_mm *= x2 - x1;
        height_mm *= y2 - y1;
    }
    let width = width_mm / 25.4 * params.dpi as f64;
    let height = height_mm / 25.4 * params.dpi as f64;
    let scale = (MAX_EDGE_PX / width.max(height).max(1.0)).min(1.0);
    (
        ((height * scale) as usize).max(1),
        ((width * scale) as usize).max(1),
    )
}

/// A deterministic color negative in a black holder, as 8-bit RGB and IR.
///
/// The orange mask leaves blue densest at the frame centre. Dust lowers RGB and IR together.
#[must_use]
pub fn negative(height: usize, width: usize, seed: i64) -> (Vec<u8>, Vec<u8>) {
    let mut rgb = vec![0u8; height * width * 3];
    let mut ir = vec![0.9f32; height * width];
    let border_y = (height / 20).max(1);
    let border_x = (width / 20).max(1);
    let seed = seed as f32;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let inside = y >= border_y
                && y < height.saturating_sub(border_y)
                && x >= border_x
                && x < width.saturating_sub(border_x);
            let (red, green, blue) = if inside {
                let y_norm = y as f32 / height.max(1) as f32;
                let x_norm = x as f32 / width.max(1) as f32;
                let scene =
                    (0.25 + 0.5 * x_norm + 0.1 * (8.0 * y_norm + seed).sin()).clamp(0.0, 1.0);
                let density = [0.25 + 1.6 * scene, 0.55 + 1.6 * scene, 0.85 + 1.6 * scene];
                let transmittance = |value: f32| (10f32.powf(-value) * 255.0).round() as u8;
                (
                    transmittance(density[0]),
                    transmittance(density[1]),
                    transmittance(density[2]),
                )
            } else {
                (10, 10, 10)
            };
            rgb[index * 3..index * 3 + 3].copy_from_slice(&[red, green, blue]);
            if !inside {
                ir[index] = 0.02;
            }
        }
    }
    for speck in 0..12 {
        let cx = (seed as usize * 17 + speck * 97) % width.max(1);
        let cy = (seed as usize * 29 + speck * 53) % height.max(1);
        let radius = 2usize;
        for y in cy.saturating_sub(radius)..(cy + radius).min(height) {
            for x in cx.saturating_sub(radius)..(cx + radius).min(width) {
                let index = y * width + x;
                if (x as isize - cx as isize).pow(2) + (y as isize - cy as isize).pow(2)
                    <= radius as isize * radius as isize
                {
                    for channel in 0..3 {
                        rgb[index * 3 + channel] /= 8;
                    }
                    ir[index] *= 0.05;
                }
            }
        }
    }
    let ir = ir
        .into_iter()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    (rgb, ir)
}

/// The simulated scanner.
#[derive(Clone, Debug, Default)]
pub struct SimulatedBackend;

impl SimulatedBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// The number of frames the roll device finds on a strip.
    #[must_use]
    pub fn detect_frames(&self, _device_id: &str) -> usize {
        ROLL_FRAMES
    }

    /// The per-channel exposures a metering run reports.
    #[must_use]
    pub fn meter(&self) -> std::collections::BTreeMap<String, i64> {
        std::collections::BTreeMap::from([
            ("red".to_string(), 1200),
            ("green".to_string(), 1800),
            ("blue".to_string(), 2600),
        ])
    }

    /// Preview slots of a roll. The last slot asks for confirmation because its boundary
    /// is inferred.
    pub fn preview_roll(
        &self,
        slots: &[usize],
        approved: &[usize],
        cancel: &Cancel,
    ) -> Vec<RollPreview> {
        slots
            .iter()
            .filter(|_| !cancel())
            .map(|slot| {
                let (rgb, _) = negative(160, 240, *slot as i64);
                RollPreview {
                    slot: *slot,
                    rgb,
                    needs_approval: *slot == ROLL_FRAMES && !approved.contains(slot),
                }
            })
            .collect()
    }

    fn scan_once(
        device: &ScannerDevice,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError> {
        for step in 0..PROGRESS_STEPS {
            if cancel() {
                return Err(ScanError::Fatal("Scan cancelled".to_string()));
            }
            progress(step as f64 / PROGRESS_STEPS as f64, None);
        }
        progress(1.0, None);
        let (height, width) = frame_dims(params);
        let (rgb, ir) = negative(height, width, params.frame.unwrap_or(0));
        Ok(ScanResult {
            rgb,
            ir: params.capture_ir.then_some(ir),
            dpi: params.dpi,
            device_model: device.model.clone(),
            ir_valid_mask: None,
            exposures: params.exposures.clone(),
            icc_profile: None,
        })
    }
}

/// One roll preview: slot, its image and whether the boundary needs confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollPreview {
    pub slot: usize,
    pub rgb: Vec<u8>,
    pub needs_approval: bool,
}

/// An open simulated device.
pub struct SimulatedSession {
    device: ScannerDevice,
}

impl ScannerSession for SimulatedSession {
    fn device_id(&self) -> &str {
        &self.device.id
    }

    fn scan(
        &mut self,
        params: &ScanParams,
        progress: &Progress,
        cancel: &Cancel,
    ) -> Result<ScanResult, ScanError> {
        SimulatedBackend::scan_once(&self.device, params, progress, cancel)
    }

    fn eject(&mut self) -> Result<bool, String> {
        Ok(self.device.capabilities.can_eject)
    }

    fn close(&mut self) {}
}

impl ScannerBackend for SimulatedBackend {
    fn list_devices(&self) -> Result<Vec<ScannerDevice>, ScannerUnavailable> {
        Ok(devices())
    }

    fn open_session(&mut self, device_id: &str) -> Result<Box<dyn ScannerSession>, String> {
        let device = device(device_id).ok_or_else(|| format!("No device {device_id}."))?;
        Ok(Box::new(SimulatedSession { device }))
    }

    fn eject(&mut self, device_id: &str) -> Result<bool, String> {
        device(device_id)
            .map(|device| device.capabilities.can_eject)
            .ok_or_else(|| format!("No device {device_id}."))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn no_cancel() -> Cancel {
        Arc::new(|| false)
    }

    fn no_progress() -> Progress {
        Arc::new(|_, _| {})
    }

    #[test]
    fn the_three_panel_shapes_are_listed() {
        let backend = SimulatedBackend::new();
        let devices = backend.list_devices().unwrap();
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].capabilities.adapter_frame_capacity, Some(6));
        assert!(devices[1].capabilities.prescan);
        assert!(devices[2].capabilities.roll_discovery);
    }

    #[test]
    fn a_scan_returns_a_negative_with_an_orange_mask() {
        let mut backend = SimulatedBackend::new();
        let mut session = backend.open_session("sim:feeder").unwrap();
        let params = ScanParams {
            dpi: 1000,
            depth: 8,
            capture_ir: true,
            frame: Some(2),
            ..ScanParams::default()
        };
        let result = session.scan(&params, &no_progress(), &no_cancel()).unwrap();
        assert!(result.ir.is_some());
        let (height, width) = frame_dims(&params);
        let centre = ((height / 2) * width + width / 2) * 3;
        assert!(
            result.rgb[centre] > result.rgb[centre + 1],
            "red over green"
        );
        assert!(
            result.rgb[centre + 1] > result.rgb[centre + 2],
            "green over blue"
        );
    }

    #[test]
    fn a_cancelled_scan_stops() {
        let mut backend = SimulatedBackend::new();
        let mut session = backend.open_session("sim:feeder").unwrap();
        let params = ScanParams {
            dpi: 1000,
            depth: 8,
            ..ScanParams::default()
        };
        let cancel: Cancel = Arc::new(|| true);
        assert_eq!(
            session
                .scan(&params, &no_progress(), &cancel)
                .unwrap_err()
                .message(),
            "Scan cancelled"
        );
    }

    #[test]
    fn the_roll_asks_to_confirm_the_last_slot_once() {
        let backend = SimulatedBackend::new();
        let slots: Vec<usize> = (1..=ROLL_FRAMES).collect();
        let previews = backend.preview_roll(&slots, &[], &no_cancel());
        assert_eq!(previews.len(), ROLL_FRAMES);
        assert!(
            previews[..ROLL_FRAMES - 1]
                .iter()
                .all(|preview| !preview.needs_approval)
        );
        assert!(previews[ROLL_FRAMES - 1].needs_approval);
        let approved = backend.preview_roll(&[ROLL_FRAMES], &[ROLL_FRAMES], &no_cancel());
        assert!(!approved[0].needs_approval);
    }

    #[test]
    fn eject_follows_the_capability() {
        let mut backend = SimulatedBackend::new();
        assert!(backend.eject("sim:feeder").unwrap());
        assert!(!backend.eject("sim:prescan").unwrap());
        assert!(backend.eject("missing").is_err());
    }
}
