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
use negpy_backend::services::rendering::display_frame::DisplayFrame;
use serde_json::Value;

/// The linear proxy of the open frame.
#[derive(Debug)]
pub struct SourceFrame {
    pub name: String,
    pub hash: String,
    pub image: ImageBuffer,
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
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            calculate_config_hash(&self.config),
            self.proof.enabled,
            self.proof.icc_path.as_deref().unwrap_or(""),
            self.proof.intent,
            self.proof.black_point,
            self.proof.paper_white,
            self.proof.ink_black,
            self.proof.gamut_warning,
            self.proof.monitor_key()
        )
    }
}

/// One finished preview.
#[derive(Clone, Debug)]
pub struct RenderOutput {
    pub generation: u64,
    pub config_hash: String,
    pub frame: DisplayFrame,
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
                let mut context = PipelineContext::new();
                let output = engine.process(
                    &request.source.image,
                    &request.config,
                    &request.source.hash,
                    &mut context,
                );
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
                let frame = DisplayFrame::from_rgb(output.width, output.height, pixels);
                let metrics = context.metrics;
                if tx
                    .send(RenderOutput {
                        generation: request.generation,
                        config_hash: request.config_hash(),
                        frame,
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
        };
        let mut config = WorkspaceConfig::default();
        config.exposure.density = 1.4;
        let second = RenderRequest {
            generation: 2,
            source: source(),
            config,
            proof: ProofRequest::default(),
        };
        let newest = second.config_hash();
        worker.submit(first);
        worker.submit(second);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut outputs = Vec::new();
        while outputs.is_empty() {
            outputs = worker.drain();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(outputs.last().unwrap().config_hash, newest);
    }
}
