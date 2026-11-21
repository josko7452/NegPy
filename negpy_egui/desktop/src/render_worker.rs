//! The preview render worker: one background thread and one pending request.
//!
//! A slider drag submits many configs; only the newest can matter, so a submit replaces
//! the pending request and the worker takes the newest when it finishes the last one.
//! The engine stays on the worker, so a stage above the changed one reuses its cache.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use negpy_backend::domain::models::WorkspaceConfig;
use negpy_backend::infrastructure::display::color_mgmt::{WORKING_COLOR_SPACE, profile_bytes};
use negpy_backend::infrastructure::display::proof_lut::{
    PROOF_LUT_SIZE, ProofCondition, ProofIntent, apply_soft_proof,
};
use negpy_backend::infrastructure::loaders::image_buffer::ImageBuffer;
use negpy_backend::kernel::caching::logic::calculate_config_hash;
use negpy_backend::kernel::image::logic::float_to_uint8;
use negpy_backend::render_engine::{DarkroomEngine, PipelineContext};
use negpy_backend::services::rendering::auto_geometry::resolve_armed_autocrop;
use negpy_backend::services::rendering::display_frame::DisplayFrame;
use serde_json::Value;

/// The linear proxy of the open frame.
#[derive(Debug)]
pub struct SourceFrame {
    pub name: String,
    pub hash: String,
    pub image: ImageBuffer,
    /// The IR plane at the same proxy size, when the file carries one.
    pub ir: Option<Vec<f32>>,
    pub ir_width: usize,
    pub ir_height: usize,
}

/// The soft proof the preview applies after the pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofRequest {
    pub enabled: bool,
    pub icc_path: Option<String>,
    pub intent: String,
    pub black_point: bool,
    pub paper_white: bool,
    pub ink_black: bool,
    pub gamut_warning: bool,
}

impl ProofRequest {
    fn monitor_key(&self) -> u64 {
        negpy_backend::infrastructure::display::monitor_profile::detect_monitor_icc("", "").map_or(
            0,
            |bytes| {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                bytes.hash(&mut hasher);
                hasher.finish()
            },
        )
    }

    /// The proof conditions the transform reads.
    #[must_use]
    pub fn condition(&self) -> Option<ProofCondition> {
        let output_icc = self
            .icc_path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok());
        let monitor_icc =
            negpy_backend::infrastructure::display::monitor_profile::detect_monitor_icc("", "");
        let input_icc = None;
        if !self.enabled || (input_icc.is_none() && output_icc.is_none()) {
            return None;
        }
        Some(ProofCondition {
            working_color_space: WORKING_COLOR_SPACE.to_string(),
            input_icc,
            output_icc,
            monitor_icc,
            intent: ProofIntent::coerce(&self.intent),
            black_point: self.black_point,
            paper_white: self.paper_white,
            ink_black: self.ink_black,
            gamut_warning: self.gamut_warning && self.icc_path.is_some(),
        })
    }
}

/// One request: the frame stays shared, the config is the edit's snapshot.
#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub generation: u64,
    pub source: Arc<SourceFrame>,
    pub config: WorkspaceConfig,
    pub proof: ProofRequest,
    /// Whether this request may use the GPU chain.
    pub gpu_enabled: bool,
}

impl Default for ProofRequest {
    fn default() -> Self {
        Self {
            enabled: false,
            icc_path: None,
            intent: "relative_colorimetric".to_string(),
            black_point: false,
            paper_white: false,
            ink_black: false,
            gamut_warning: false,
        }
    }
}

impl RenderRequest {
    /// The config hash this request renders, so a result can be matched to it.
    #[must_use]
    pub fn config_hash(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            calculate_config_hash(&self.config),
            self.proof.enabled,
            self.proof.icc_path.as_deref().unwrap_or(""),
            self.proof.intent,
            self.proof.black_point,
            self.proof.paper_white,
            self.proof.ink_black,
            self.proof.gamut_warning,
            self.proof.monitor_key(),
            self.gpu_enabled
        )
    }
}

/// The output histogram of a display-encoded frame.
fn histogram_of(pixels: &[u8], width: usize, height: usize) -> Option<Vec<f64>> {
    let encoded: Vec<f32> = pixels
        .iter()
        .map(|value| f32::from(*value) / 255.0)
        .collect();
    negpy_backend::features::exposure::analysis::output_histogram(&encoded, width, height)
}

/// The display-encoded frame as float, for the scope bins.
fn encoded_of(pixels: &[u8]) -> Vec<f32> {
    pixels
        .iter()
        .map(|value| f32::from(*value) / 255.0)
        .collect()
}

/// The waveform, parade and vectorscope bins of one display-encoded frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScopeBins {
    /// Luma counts per frame column and tone.
    pub waveform: Option<Vec<f64>>,
    /// Mean color sums per waveform cell.
    pub waveform_color: Option<Vec<f64>>,
    /// R, G and B counts side by side.
    pub parade: Option<Vec<f64>>,
    /// a*b* counts on the scope plane.
    pub vectorscope: Option<Vec<f64>>,
}

impl ScopeBins {
    fn of(pixels: &[u8], width: usize, height: usize) -> Self {
        let encoded = encoded_of(pixels);
        Self {
            waveform: negpy_backend::features::exposure::analysis::waveform(
                &encoded, width, height,
            ),
            waveform_color: negpy_backend::features::exposure::analysis::waveform_color(
                &encoded, width, height,
            ),
            parade: negpy_backend::features::exposure::analysis::waveform_parade(
                &encoded, width, height,
            ),
            vectorscope: negpy_backend::features::exposure::analysis::vectorscope_histogram(
                &encoded, width, height,
            ),
        }
    }
}

/// One finished preview.
#[derive(Clone, Debug)]
pub struct RenderOutput {
    pub generation: u64,
    /// The hash of the request that started this render.
    pub request_hash: String,
    /// The hash of the config that painted this frame, after Auto Crop resolved it.
    pub config_hash: String,
    pub frame: DisplayFrame,
    /// The display histogram `(4, 256)` of the frame on the canvas, when it has one.
    pub histogram: Option<Vec<f64>>,
    /// The scope bins the Analysis view paints.
    pub scopes: ScopeBins,
    /// The `(4, 120)` density histogram of the normalized frame, when it has one.
    pub density_histogram: Option<Vec<f64>>,
    pub metrics: BTreeMap<String, Value>,
}

#[derive(Default)]
struct Slot {
    pending: Option<RenderRequest>,
    closed: bool,
}

/// The GUI side of the worker: it submits and collects results.
pub struct RenderWorker {
    slot: Arc<(Mutex<Slot>, Condvar)>,
    results: Receiver<RenderOutput>,
    thread: Option<JoinHandle<()>>,
}

impl RenderWorker {
    /// Start the worker thread.
    #[must_use]
    pub fn start() -> Self {
        let slot = Arc::new((Mutex::new(Slot::default()), Condvar::new()));
        let (tx, results) = channel();
        let worker_slot = Arc::clone(&slot);
        let thread = std::thread::spawn(move || {
            let mut engine = DarkroomEngine::new();
            let mut gpu = negpy_backend::services::rendering::gpu_engine::GpuEngine::new();
            loop {
                let request = {
                    let (lock, ready) = &*worker_slot;
                    let mut slot = lock.lock().expect("render slot poisoned");
                    loop {
                        if let Some(request) = slot.pending.take() {
                            break Some(request);
                        }
                        if slot.closed {
                            break None;
                        }
                        slot = ready.wait(slot).expect("render slot poisoned");
                    }
                };
                let Some(request) = request else {
                    break;
                };
                let preview_size =
                    negpy_backend::kernel::system::config::app_config().preview_render_size as f64;
                let request_hash = request.config_hash();
                let (baked, retouch_tokens) =
                    negpy_backend::services::rendering::retouch::bake_with_tokens(
                        &request.source.image,
                        &request.config,
                        request.source.ir.as_deref().map(|samples| {
                            negpy_backend::services::rendering::retouch::IrPlane {
                                samples,
                                width: request.source.ir_width,
                                height: request.source.ir_height,
                            }
                        }),
                        preview_size,
                    );
                let (config, autocrop) =
                    resolve_armed_autocrop(&baked, &request.config, preview_size);
                let config_hash = if autocrop.is_some() {
                    RenderRequest {
                        config: config.clone(),
                        ..request.clone()
                    }
                    .config_hash()
                } else {
                    request_hash.clone()
                };
                let mut context = PipelineContext::new();
                let source_hash = format!("{}{}", request.source.hash, retouch_tokens);
                let use_gpu = request.gpu_enabled
                    && gpu.is_some()
                    && negpy_backend::services::rendering::gpu_render::gpu_eligible(
                        &config, &context,
                    );
                let output = if use_gpu {
                    let measured = negpy_backend::features::exposure::processor::measure_negative(
                        &baked.pixels,
                        baked.width,
                        baked.height,
                        negpy_backend::features::exposure::processor::NormalizationSettings {
                            config: &config.process,
                            mode: config.process.process_mode,
                            active_roi: None,
                            base_log: None,
                        },
                    );
                    let base = negpy_backend::services::rendering::gpu_render::GpuBase {
                        bounds: measured.final_bounds,
                        norm_density_range: measured.norm_density_range,
                        textural_range: measured.textural_range,
                        shadow_point: measured.shadow_point,
                        highlight_point: measured.highlight_point,
                        anchor: Some(measured.metered_anchor),
                        active_roi: None,
                    };
                    negpy_backend::services::rendering::gpu_render::render(
                        gpu.as_mut().expect("the GPU is open"),
                        &mut engine,
                        &baked,
                        &config,
                        &base,
                        &mut context,
                    )
                } else {
                    engine.process(&baked, &config, &source_hash, &mut context)
                };
                if autocrop.is_some() {
                    context.metrics.insert(
                        "autocrop_resolved".to_string(),
                        serde_json::to_value(&autocrop).unwrap_or(Value::Null),
                    );
                }
                let mut pixels = float_to_uint8(&output.pixels);
                if let Some(condition) = request.proof.condition() {
                    let working = profile_bytes(WORKING_COLOR_SPACE, Path::new("/nonexistent"))
                        .map(|profile| profile.bytes);
                    if let Some(working) = working {
                        if let Some(proofed) = apply_soft_proof(
                            &pixels,
                            &condition,
                            &working,
                            condition.monitor_icc.as_deref(),
                            PROOF_LUT_SIZE,
                        ) {
                            pixels = proofed;
                        }
                    }
                }
                let histogram = histogram_of(&pixels, output.width, output.height);
                let scopes = ScopeBins::of(&pixels, output.width, output.height);
                let density_histogram = context
                    .metrics
                    .get("histogram_density")
                    .and_then(|value| serde_json::from_value(value.clone()).ok());
                let frame = DisplayFrame::from_rgb(output.width, output.height, pixels);
                let metrics = context.metrics;
                if tx
                    .send(RenderOutput {
                        generation: request.generation,
                        request_hash,
                        config_hash,
                        frame,
                        histogram,
                        scopes,
                        density_histogram,
                        metrics,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            slot,
            results,
            thread: Some(thread),
        }
    }

    /// Replace the pending request. The newest request wins.
    pub fn submit(&self, request: RenderRequest) {
        let (lock, ready) = &*self.slot;
        let mut slot = lock.lock().expect("render slot poisoned");
        slot.pending = Some(request);
        ready.notify_one();
    }

    /// Take every finished preview without blocking.
    #[must_use]
    pub fn drain(&self) -> Vec<RenderOutput> {
        let mut outputs = Vec::new();
        loop {
            match self.results.try_recv() {
                Ok(output) => outputs.push(output),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        outputs
    }
}

impl Drop for RenderWorker {
    fn drop(&mut self) {
        {
            let (lock, ready) = &*self.slot;
            let mut slot = lock.lock().expect("render slot poisoned");
            slot.closed = true;
            ready.notify_all();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use negpy_backend::domain::models::WorkspaceConfig;

    fn source() -> Arc<SourceFrame> {
        let mut pixels = Vec::new();
        for y in 0..12 {
            for x in 0..16 {
                let value = 0.05 + 0.8 * (y as f32 / 12.0);
                pixels.extend_from_slice(&[value, value * 0.9, value * 0.8 + x as f32 * 1e-4]);
            }
        }
        Arc::new(SourceFrame {
            name: "frame.orf".to_string(),
            hash: "source".to_string(),
            image: ImageBuffer::from_rgb(16, 12, pixels),
            ir: None,
            ir_width: 0,
            ir_height: 0,
        })
    }

    #[test]
    fn a_request_renders_its_config_hash() {
        let worker = RenderWorker::start();
        let request = RenderRequest {
            generation: 1,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        let hash = request.config_hash();
        worker.submit(request);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let output = worker.drain().into_iter().next();
            if let Some(output) = output {
                assert_eq!(output.generation, 1);
                assert_eq!(output.config_hash, hash);
                assert!(output.frame.is_rgb());
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_render_returns_the_histogram_of_its_frame() {
        let worker = RenderWorker::start();
        let request = RenderRequest {
            generation: 1,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        worker.submit(request);
        let output = wait_for(&worker);
        let histogram = output.histogram.expect("a display histogram");
        assert_eq!(histogram.len(), 4 * 256);
        assert!(histogram.iter().sum::<f64>() > 0.0);
        let scopes = output.scopes;
        assert!(scopes.waveform.is_some());
        assert!(scopes.parade.is_some());
        assert!(scopes.vectorscope.is_some());
        assert!(output.density_histogram.is_some());
    }

    #[test]
    fn a_proofed_preview_differs_from_the_plain_preview() {
        let dir = tempfile::tempdir().unwrap();
        let icc = dir.path().join("paper.icc");
        let source_icc =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../icc/sRGB-v4.icc");
        std::fs::copy(source_icc, &icc).unwrap();
        let worker = RenderWorker::start();
        let plain = RenderRequest {
            generation: 1,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        let proofed = RenderRequest {
            generation: 2,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest {
                enabled: true,
                icc_path: Some(icc.display().to_string()),
                ..ProofRequest::default()
            },
            gpu_enabled: false,
        };
        worker.submit(plain);
        let first = wait_for(&worker);
        worker.submit(proofed);
        let second = wait_for(&worker);
        assert_ne!(first.frame.pixels, second.frame.pixels);
    }

    fn wait_for(worker: &RenderWorker) -> RenderOutput {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Some(output) = worker.drain().into_iter().next() {
                return output;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn the_proof_changes_the_config_hash() {
        let before = RenderRequest {
            generation: 1,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        let after = RenderRequest {
            generation: 1,
            source: source(),
            config: WorkspaceConfig::default(),
            proof: ProofRequest {
                enabled: true,
                icc_path: Some("/tmp/paper.icc".to_string()),
                ..ProofRequest::default()
            },
            gpu_enabled: false,
        };
        assert_ne!(before.config_hash(), after.config_hash());
    }

    #[test]
    fn only_the_newest_pending_request_runs() {
        let worker = RenderWorker::start();
        let mut config = WorkspaceConfig::default();
        config.exposure.density = 0.9;
        let first = RenderRequest {
            generation: 1,
            source: source(),
            config,
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        let mut config = WorkspaceConfig::default();
        config.exposure.density = 1.4;
        let second = RenderRequest {
            generation: 2,
            source: source(),
            config,
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        let newest = second.config_hash();
        worker.submit(first);
        worker.submit(second);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let outputs = worker.drain();
            if let Some(output) = outputs.last()
                && output.generation == 2
            {
                assert_eq!(output.config_hash, newest);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn an_armed_crop_reports_the_rect_it_froze() {
        let worker = RenderWorker::start();
        let mut config = WorkspaceConfig::default();
        config.geometry.crop_from_auto = true;
        let request = RenderRequest {
            generation: 1,
            source: source(),
            config,
            proof: ProofRequest::default(),
            gpu_enabled: false,
        };
        worker.submit(request);
        let output = wait_for(&worker);
        let resolved: Option<negpy_backend::services::rendering::auto_geometry::ArmedCrop> = output
            .metrics
            .get("autocrop_resolved")
            .and_then(|value| serde_json::from_value(value.clone()).ok());
        assert!(resolved.is_some());
        assert_ne!(output.request_hash, output.config_hash);
    }
}
