//! Bounded Coalescing Event Channels for TUI (D-15).
//!
//! Provides non-blocking event consumption from the runtime engine into the
//! TUI render loop. If the TUI frame rate lags, events coalesce or queue up to
//! a bounded limit without ever blocking the runtime scheduler or worker threads.

use crate::events::envelope::EventEnvelope;
use tokio::sync::mpsc::{Receiver, Sender, channel};

/// Default capacity for the bounded TUI event queue.
pub const DEFAULT_TUI_QUEUE_CAPACITY: usize = 2048;

/// Sender handle held by runtime event listener or dispatcher.
#[derive(Clone, Debug)]
pub struct TuiUpdateSender {
    tx: Sender<EventEnvelope>,
}

impl TuiUpdateSender {
    pub fn new(tx: Sender<EventEnvelope>) -> Self {
        Self { tx }
    }

    /// Send event to TUI without blocking runtime engine.
    /// If queue is full, the oldest/excess message is dropped or returns false.
    pub fn try_send(&self, event: EventEnvelope) -> bool {
        self.tx.try_send(event).is_ok()
    }
}

/// Receiver handle polled by the decoupled TUI render loop.
pub struct TuiUpdateReceiver {
    rx: Receiver<EventEnvelope>,
}

impl TuiUpdateReceiver {
    pub fn new(rx: Receiver<EventEnvelope>) -> Self {
        Self { rx }
    }

    /// Drain all available events non-blockingly to update ViewModel in one batch.
    pub fn drain_available(&mut self) -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Async wait for next event with optional timeout.
    pub async fn recv(&mut self) -> Option<EventEnvelope> {
        self.rx.recv().await
    }
}

/// Create a new bounded coalescing pair for TUI updates.
pub fn create_tui_channel(capacity: usize) -> (TuiUpdateSender, TuiUpdateReceiver) {
    let (tx, rx) = channel(capacity);
    (TuiUpdateSender::new(tx), TuiUpdateReceiver::new(rx))
}
