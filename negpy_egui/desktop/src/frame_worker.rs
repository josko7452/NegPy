//! The frame-open worker: decode a strip frame away from the UI thread.
//!
//! A rapid frame switch submits many requests; only the newest can matter, so a
//! submit replaces the pending request and the worker takes the newest when it
//! finishes the last one.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::edit::FrameEdit;
use crate::frame_source::FrameImage;

/// One frame a strip click opened.
#[derive(Clone, Debug)]
pub struct FrameOpenRequest {
    pub generation: u64,
    pub index: usize,
    pub path: PathBuf,
    pub profile: Option<PathBuf>,
    pub roll_id: Option<String>,
    pub max_edge: usize,
}

/// The frame, its linear source and the panel state it binds.
#[derive(Debug)]
pub struct FrameOpenReport {
    pub generation: u64,
    pub index: usize,
    pub frame: FrameImage,
    pub source: crate::render_worker::SourceFrame,
    pub edit: FrameEdit,
    pub history: Vec<negpy_backend::session::workspace::HistoryRow>,
    pub prints: Vec<String>,
    pub roll_analysis: crate::view::rail::roll_analysis::RollAnalysisState,
}

/// Decode one frame and read the state its panels show.
#[must_use]
pub fn open(request: &FrameOpenRequest) -> Result<FrameOpenReport, String> {
    let (frame, source) = crate::frame_source::load_frame_with_source(
        &request.path,
        request.profile.as_deref(),
        request.max_edge,
    )?;
    let edit = FrameEdit::load_in_roll(
        request.profile.as_deref(),
        &request.path,
        request.roll_id.as_deref(),
    );
    let (history, prints) =
        crate::frame_source::stored_history(request.profile.as_deref(), &request.path);
    let roll_analysis = crate::view::rail::roll_analysis::RollAnalysisState::load(
        request.profile.as_deref(),
        &request.path,
        Some(&edit.config),
    );
    Ok(FrameOpenReport {
        generation: request.generation,
        index: request.index,
        frame,
        source,
        edit,
        history,
        prints,
        roll_analysis,
    })
}

struct Slot {
    pending: Option<FrameOpenRequest>,
    closed: bool,
}

/// The GUI side of the frame-open worker.
pub struct FrameWorker {
    slot: Arc<(Mutex<Slot>, Condvar)>,
    results: Receiver<Result<FrameOpenReport, (u64, String)>>,
    thread: Option<JoinHandle<()>>,
}

impl FrameWorker {
    /// Start the worker thread.
    #[must_use]
    pub fn start() -> Self {
        let slot = Arc::new((
            Mutex::new(Slot {
                pending: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let (sender, results) = channel();
        let worker_slot = Arc::clone(&slot);
        let thread = std::thread::spawn(move || {
            loop {
                let request = {
                    let (lock, ready) = &*worker_slot;
                    let mut slot = lock.lock().expect("frame slot poisoned");
                    while slot.pending.is_none() && !slot.closed {
                        slot = ready.wait(slot).expect("frame slot poisoned");
                    }
                    if slot.closed {
                        break;
                    }
                    slot.pending.take().expect("a pending frame request")
                };
                let generation = request.generation;
                let report = open(&request).map_err(|error| (generation, error));
                if sender.send(report).is_err() {
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

    /// Replace the pending request. The newest frame wins.
    pub fn submit(&self, request: FrameOpenRequest) {
        let (lock, ready) = &*self.slot;
        let mut slot = lock.lock().expect("frame slot poisoned");
        slot.pending = Some(request);
        ready.notify_one();
    }

    /// Take every finished frame without blocking.
    #[must_use]
    pub fn drain(&self) -> Vec<Result<FrameOpenReport, (u64, String)>> {
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

impl Drop for FrameWorker {
    fn drop(&mut self) {
        {
            let (lock, ready) = &*self.slot;
            let mut slot = lock.lock().expect("frame slot poisoned");
            slot.closed = true;
            ready.notify_all();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
