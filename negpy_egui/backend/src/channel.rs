//! The transport between the GUI and the backend.
//!
//! One thread-safe pair of channels carries the [`BackendCommand`] stream in and the
//! [`BackendEvent`] stream out. The backend has no GUI dependency, so it cannot call
//! `egui::Context::request_repaint` itself: it calls the [`Wake`] the GUI installs. The
//! GUI wakes its context in that callback, which repaints only when the backend delivers
//! a result (AGENTS "Repaint on Demand").

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SendError, Sender, TryRecvError};

use crate::event::{BackendCommand, BackendEvent};

/// The GUI's repaint callback. The desktop side captures an `egui::Context` and calls
/// `request_repaint`; a test counts calls.
pub type Wake = Arc<dyn Fn() + Send + Sync + 'static>;

/// The backend's end of the pair: it reads commands and writes events.
pub struct BackendSide {
    pub commands: CommandReceiver,
    pub events: EventSender,
}

/// The GUI's end of the pair: it writes commands and reads events.
pub struct GuiSide {
    pub commands: CommandSender,
    pub events: EventReceiver,
}

/// Build the channel pair. `wake` runs on the thread that sends an event.
pub fn channels(wake: Wake) -> (BackendSide, GuiSide) {
    let (command_tx, command_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    (
        BackendSide {
            commands: CommandReceiver(command_rx),
            events: EventSender { tx: event_tx, wake },
        },
        GuiSide {
            commands: CommandSender(command_tx),
            events: EventReceiver(event_rx),
        },
    )
}

/// A GUI -> backend command sender.
#[derive(Clone)]
pub struct CommandSender(Sender<BackendCommand>);

impl CommandSender {
    pub fn send(&self, command: BackendCommand) -> Result<(), SendError<BackendCommand>> {
        self.0.send(command)
    }
}

/// The backend's command reader.
pub struct CommandReceiver(Receiver<BackendCommand>);

impl CommandReceiver {
    /// Block until the next command arrives, or `None` when the GUI dropped its sender.
    pub fn recv(&self) -> Option<BackendCommand> {
        self.0.recv().ok()
    }

    /// Take the next command without blocking.
    pub fn try_recv(&self) -> Result<BackendCommand, TryRecvError> {
        self.0.try_recv()
    }
}

/// A backend -> GUI event sender. Sending an event wakes the GUI.
#[derive(Clone)]
pub struct EventSender {
    tx: Sender<BackendEvent>,
    wake: Wake,
}

impl EventSender {
    /// Deliver an event and wake the GUI. The wake runs only when the send succeeds.
    pub fn send(&self, event: BackendEvent) -> Result<(), SendError<BackendEvent>> {
        self.tx.send(event)?;
        (self.wake)();
        Ok(())
    }
}

/// The GUI's event reader.
pub struct EventReceiver(Receiver<BackendEvent>);

impl EventReceiver {
    /// Block until the next event arrives, or `None` when the backend dropped its sender.
    pub fn recv(&self) -> Option<BackendEvent> {
        self.0.recv().ok()
    }

    /// Take the next event without blocking.
    pub fn try_recv(&self) -> Result<BackendEvent, TryRecvError> {
        self.0.try_recv()
    }
}
