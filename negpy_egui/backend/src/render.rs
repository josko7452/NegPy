//! The render service: the one place a render runs, on its own thread.
//!
//! A slider drag submits many [`RenderParams`]; only the newest matters, so the queue
//! keeps one pending item and drops the rest (AGENTS "Latest-Wins Request Coalescing").
//! The caller supplies the render function, which the Layer 2 engine fills in.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crate::channel::EventSender;
use crate::event::{BackendEvent, RenderParams, RenderResult};

/// The preview budget of a render that fits the screen. A frame longer than this in
/// pixels renders at this size while a slider moves.
pub const PREVIEW_RENDER_SIZE: u32 = 1600;

/// The render work itself, run on the worker thread. The Layer 2 engine supplies one.
pub type RenderFn = Arc<dyn Fn(&RenderParams) -> RenderResult + Send + Sync>;

/// A one-slot queue: a submit replaces the pending item, so the consumer sees only the
/// newest. A consumer that waits is woken on the next submit or on close.
#[derive(Default)]
struct LatestSlot {
    slot: Mutex<Slot>,
    ready: Condvar,
}

#[derive(Default)]
struct Slot {
    pending: Option<RenderParams>,
    closed: bool,
}

/// The producer half of a [`LatestSlot`]. Cloning shares the same slot.
#[derive(Clone)]
pub struct LatestSender(Arc<LatestSlot>);

impl LatestSender {
    /// Replace the pending item with `item`. The old pending item is dropped.
    pub fn submit(&self, item: RenderParams) {
        let mut slot = self.0.slot.lock().expect("render slot poisoned");
        slot.pending = Some(item);
        self.0.ready.notify_one();
    }

    /// Stop the consumer once the pending item, if any, is taken.
    pub fn close(&self) {
        let mut slot = self.0.slot.lock().expect("render slot poisoned");
        slot.closed = true;
        self.0.ready.notify_all();
    }
}

/// The consumer half of a [`LatestSlot`].
pub struct LatestReceiver(Arc<LatestSlot>);

impl LatestReceiver {
    /// Block until an item is pending, then take it. `None` once closed and drained.
    pub fn recv(&self) -> Option<RenderParams> {
        let mut slot = self.0.slot.lock().expect("render slot poisoned");
        loop {
            if let Some(item) = slot.pending.take() {
                return Some(item);
            }
            if slot.closed {
                return None;
            }
            slot = self.0.ready.wait(slot).expect("render slot poisoned");
        }
    }
}

/// Build a latest-wins slot, its producer and its consumer. Every producer shares the
/// consumer's slot.
pub fn latest_slot() -> (LatestSender, LatestReceiver) {
    let shared = Arc::new(LatestSlot::default());
    (LatestSender(Arc::clone(&shared)), LatestReceiver(shared))
}

/// The render thread: it takes the newest [`RenderParams`], renders it, and emits the
/// result. The handle closes the slot and joins the thread.
pub struct RenderService {
    params: LatestSender,
    thread: Option<JoinHandle<()>>,
}

impl RenderService {
    /// Start the render thread. `render` runs on it; `events` delivers the result.
    pub fn start(render: RenderFn, events: EventSender) -> Self {
        let (params, receiver) = latest_slot();
        let thread = std::thread::spawn(move || {
            while let Some(p) = receiver.recv() {
                let result = render(&p);
                if events.send(BackendEvent::RenderFinished(result)).is_err() {
                    break;
                }
            }
        });
        Self {
            params,
            thread: Some(thread),
        }
    }

    /// Submit a render request. The newest request wins.
    pub fn submit(&self, params: RenderParams) {
        self.params.submit(params);
    }

    /// Stop the thread and join it. Idempotent.
    pub fn shutdown(&mut self) {
        self.params.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for RenderService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The render size for a frame `long_edge` pixels on its long side. While the user drags
/// a control, the frame renders at the preview budget; otherwise at full resolution.
pub fn render_size(long_edge: u32, interactive: bool) -> u32 {
    if interactive {
        long_edge.min(PREVIEW_RENDER_SIZE)
    } else {
        long_edge
    }
}

/// Build the parameters of one render from a frame size and the interaction state.
pub fn params_for(
    source_hash: &str,
    generation: u64,
    long_edge: u32,
    interactive: bool,
) -> RenderParams {
    RenderParams {
        source_hash: source_hash.to_string(),
        generation,
        preview_size: render_size(long_edge, interactive) as f32,
        interactive,
        full_resolution: !interactive,
        gpu_enabled: true,
        readback_metrics: true,
        compare: false,
        wants_thumbnail: false,
        ephemeral: false,
    }
}

/// A channel pair for the render service's own tests and callers that read results out
/// of band from the GUI event stream.
pub fn render_channel() -> (Sender<RenderResult>, Receiver<RenderResult>) {
    channel()
}
