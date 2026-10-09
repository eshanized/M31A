//! Bounded channel abstractions and backpressure support for TUI (Problem 7).
//!
//! Provides `TuiActionSender` and `TuiInteractionReceiver` supporting both
//! bounded channels (production default) and unbounded channels (for test compatibility).

use tokio::sync::mpsc::{self, error::TryRecvError, error::TrySendError};

use crate::interaction::action::ApplicationAction;
use crate::interaction::events::InteractionEvent;

/// Result of sending an action across the TUI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionSendError {
    /// Channel buffer is full (backpressure active).
    Full,
    /// Channel receiver has been closed or dropped.
    Closed,
}

impl std::fmt::Display for ActionSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => write!(f, "action channel queue is full (backpressure)"),
            Self::Closed => write!(f, "action channel receiver disconnected"),
        }
    }
}

impl std::error::Error for ActionSendError {}

/// Sender of `ApplicationAction` to the governed runtime bridge.
#[derive(Debug, Clone)]
pub enum TuiActionSender {
    Bounded(mpsc::Sender<ApplicationAction>),
    Unbounded(mpsc::UnboundedSender<ApplicationAction>),
}

impl TuiActionSender {
    /// Non-blocking attempt to submit an action to the bridge.
    pub fn try_send(&self, action: ApplicationAction) -> Result<(), ActionSendError> {
        match self {
            Self::Bounded(tx) => match tx.try_send(action) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => Err(ActionSendError::Full),
                Err(TrySendError::Closed(_)) => Err(ActionSendError::Closed),
            },
            Self::Unbounded(tx) => tx.send(action).map_err(|_| ActionSendError::Closed),
        }
    }

    /// Send an action to the bridge.
    pub fn send(&self, action: ApplicationAction) -> Result<(), ActionSendError> {
        self.try_send(action)
    }

    /// Check if the sender is still open.
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Bounded(tx) => tx.is_closed(),
            Self::Unbounded(tx) => tx.is_closed(),
        }
    }
}

impl From<mpsc::Sender<ApplicationAction>> for TuiActionSender {
    fn from(s: mpsc::Sender<ApplicationAction>) -> Self {
        Self::Bounded(s)
    }
}

impl From<mpsc::UnboundedSender<ApplicationAction>> for TuiActionSender {
    fn from(s: mpsc::UnboundedSender<ApplicationAction>) -> Self {
        Self::Unbounded(s)
    }
}

/// Receiver of `InteractionEvent` from the governed runtime bridge.
pub enum TuiInteractionReceiver {
    Bounded(mpsc::Receiver<InteractionEvent>),
    Unbounded(mpsc::UnboundedReceiver<InteractionEvent>),
}

impl TuiInteractionReceiver {
    /// Non-blocking receive of next interaction event.
    pub fn try_recv(&mut self) -> Result<InteractionEvent, TryRecvError> {
        match self {
            Self::Bounded(rx) => rx.try_recv(),
            Self::Unbounded(rx) => rx.try_recv(),
        }
    }

    /// Asynchronously await next interaction event.
    pub async fn recv(&mut self) -> Option<InteractionEvent> {
        match self {
            Self::Bounded(rx) => rx.recv().await,
            Self::Unbounded(rx) => rx.recv().await,
        }
    }
}

impl From<mpsc::Receiver<InteractionEvent>> for TuiInteractionReceiver {
    fn from(r: mpsc::Receiver<InteractionEvent>) -> Self {
        Self::Bounded(r)
    }
}

impl From<mpsc::UnboundedReceiver<InteractionEvent>> for TuiInteractionReceiver {
    fn from(r: mpsc::UnboundedReceiver<InteractionEvent>) -> Self {
        Self::Unbounded(r)
    }
}

/// Sender of `InteractionEvent` from runtime bridge to TUI.
#[derive(Debug, Clone)]
pub enum TuiInteractionSender {
    Bounded(mpsc::Sender<InteractionEvent>),
    Unbounded(mpsc::UnboundedSender<InteractionEvent>),
}

impl TuiInteractionSender {
    pub fn try_send(&self, event: InteractionEvent) -> Result<(), ActionSendError> {
        match self {
            Self::Bounded(tx) => match tx.try_send(event) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => Err(ActionSendError::Full),
                Err(TrySendError::Closed(_)) => Err(ActionSendError::Closed),
            },
            Self::Unbounded(tx) => tx.send(event).map_err(|_| ActionSendError::Closed),
        }
    }

    pub fn send_or_log(&self, event: InteractionEvent) {
        if let Err(err) = self.try_send(event) {
            tracing::warn!("Failed to deliver interaction event: {err}");
        }
    }
}

impl From<mpsc::Sender<InteractionEvent>> for TuiInteractionSender {
    fn from(s: mpsc::Sender<InteractionEvent>) -> Self {
        Self::Bounded(s)
    }
}

impl From<mpsc::UnboundedSender<InteractionEvent>> for TuiInteractionSender {
    fn from(s: mpsc::UnboundedSender<InteractionEvent>) -> Self {
        Self::Unbounded(s)
    }
}
