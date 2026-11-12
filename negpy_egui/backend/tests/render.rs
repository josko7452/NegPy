//! C3: the render thread coalesces rapid requests to the newest, and the render size
//! follows the interaction state.

use std::sync::{Arc, Barrier, Mutex};

use negpy_backend::channel::channels;
use negpy_backend::event::BackendEvent;
use negpy_backend::render::{
    PREVIEW_RENDER_SIZE, RenderService, latest_slot, params_for, render_size,
};

#[test]
fn rapid_requests_coalesce_to_the_newest() {
    let seen: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let first_started = Arc::new(Barrier::new(2));
    let release_first = Arc::new(Barrier::new(2));
    let (backend, gui) = channels(Arc::new(|| {}));

    let seen_fn = Arc::clone(&seen);
    let started = Arc::clone(&first_started);
    let release = Arc::clone(&release_first);
    let render = Arc::new(move |p: &negpy_backend::event::RenderParams| {
        let mut seen = seen_fn.lock().unwrap();
        seen.push(p.generation);
        // Block the first render so requests 2..4 pile up behind it.
        if seen.len() == 1 {
            drop(seen);
            started.wait();
            release.wait();
        }
        negpy_backend::event::RenderResult {
            width: 1,
            height: 1,
            metrics: Default::default(),
        }
    });

    let mut service = RenderService::start(render, backend.events);
    service.submit(params_for("a", 1, 4000, false));
    first_started.wait();
    service.submit(params_for("a", 2, 4000, true));
    service.submit(params_for("a", 3, 4000, true));
    service.submit(params_for("a", 4, 4000, true));
    release_first.wait();

    // Drain the two results: the blocked first render, then the coalesced newest.
    let mut results = 0;
    while results < 2 {
        match gui.events.recv() {
            Some(BackendEvent::RenderFinished(_)) => results += 1,
            Some(other) => panic!("unexpected event {other:?}"),
            None => break,
        }
    }
    service.shutdown();

    let generations = seen.lock().unwrap().clone();
    assert_eq!(
        generations,
        vec![1, 4],
        "generations 2 and 3 were superseded before they rendered"
    );
}

#[test]
fn the_consumer_ends_when_the_slot_closes() {
    let (sender, receiver) = latest_slot();
    sender.close();
    assert!(receiver.recv().is_none());
}

#[test]
fn a_pending_item_survives_close() {
    let (sender, receiver) = latest_slot();
    sender.submit(params_for("a", 7, 100, true));
    sender.close();
    assert_eq!(receiver.recv().map(|p| p.generation), Some(7));
    assert!(receiver.recv().is_none());
}

#[test]
fn the_render_size_follows_the_interaction_state() {
    assert_eq!(render_size(4000, true), PREVIEW_RENDER_SIZE);
    assert_eq!(render_size(4000, false), 4000);
    // A frame smaller than the budget is untouched even while dragging.
    assert_eq!(render_size(800, true), 800);
}

#[test]
fn an_idle_render_is_full_resolution() {
    let idle = params_for("a", 1, 4000, false);
    assert!(idle.full_resolution);
    assert!(!idle.interactive);
    let dragging = params_for("a", 2, 4000, true);
    assert!(dragging.interactive);
    assert!(!dragging.full_resolution);
}
