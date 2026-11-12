//! C2: a command goes in, an event comes out, and delivering the event wakes the GUI.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use negpy_backend::channel::{Wake, channels};
use negpy_backend::event::{BackendCommand, BackendEvent};

#[test]
fn a_command_round_trips_and_wakes_the_gui() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let wake: Wake = {
        let wakes = Arc::clone(&wakes);
        Arc::new(move || {
            wakes.fetch_add(1, Ordering::SeqCst);
        })
    };
    let (backend, gui) = channels(wake);

    gui.commands
        .send(BackendCommand::ThumbnailCancel)
        .expect("the backend is listening");
    let command = backend.commands.recv().expect("a command arrived");
    assert_eq!(command, BackendCommand::ThumbnailCancel);
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        0,
        "a command does not wake the GUI"
    );

    backend
        .events
        .send(BackendEvent::ThumbnailCancelled)
        .expect("the GUI is listening");
    let event = gui.events.recv().expect("an event arrived");
    assert_eq!(event, BackendEvent::ThumbnailCancelled);
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        1,
        "delivering an event wakes the GUI once"
    );
}

#[test]
fn a_dropped_gui_sender_ends_the_backend_loop() {
    let (backend, gui) = channels(Arc::new(|| {}));
    drop(gui);
    assert!(backend.commands.recv().is_none());
}
