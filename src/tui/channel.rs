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

/// Maximum number of critical events retained in the bounded outbox during channel saturation.
pub const MAX_CRITICAL_OUTBOX_CAPACITY: usize = 256;

fn is_terminal_or_approval(event: &InteractionEvent) -> bool {
    matches!(
        event,
        InteractionEvent::ApprovalRequested { .. }
            | InteractionEvent::ApprovalResolved { .. }
            | InteractionEvent::TaskCompleted { .. }
            | InteractionEvent::TaskFailed { .. }
            | InteractionEvent::TaskCancelled { .. }
            | InteractionEvent::MissionStateChanged { .. }
            | InteractionEvent::Completion { .. }
            | InteractionEvent::Error { .. }
            | InteractionEvent::AgentCompleted { .. }
            | InteractionEvent::AgentFailed { .. }
            | InteractionEvent::AgentCancelled { .. }
            | InteractionEvent::JobCompleted { .. }
            | InteractionEvent::JobFailed { .. }
            | InteractionEvent::VerificationPassed { .. }
            | InteractionEvent::VerificationFailed { .. }
    )
}

#[derive(Debug, Default)]
pub struct OutboxState {
    pub critical_queue: std::collections::VecDeque<InteractionEvent>,
    pub latest_budget: Option<Box<crate::tui::model::TuiBudgetSnapshot>>,
    pub latest_workflow: Option<Box<crate::workflow::engine::WorkflowExecutionSnapshot>>,
    pub needs_reconciliation: bool,
    pub dropped_critical_count: u64,
    pub last_log_time: Option<std::time::Instant>,
}

impl OutboxState {
    pub fn push_critical(&mut self, event: InteractionEvent) -> Result<(), ActionSendError> {
        if self.critical_queue.len() < MAX_CRITICAL_OUTBOX_CAPACITY {
            self.critical_queue.push_back(event);
            Ok(())
        } else if is_terminal_or_approval(&event) {
            // Find oldest non-terminal event to evict
            if let Some(idx) = self
                .critical_queue
                .iter()
                .position(|e| !is_terminal_or_approval(e))
            {
                self.critical_queue.remove(idx);
                self.critical_queue.push_back(event);
                self.needs_reconciliation = true;
                self.dropped_critical_count += 1;
                self.log_saturation_warning();
                Ok(())
            } else {
                // All items are terminal/approval; cannot evict, record gap and fail
                self.needs_reconciliation = true;
                self.dropped_critical_count += 1;
                self.log_saturation_warning();
                Err(ActionSendError::Full)
            }
        } else {
            // Non-terminal event under critical saturation
            self.needs_reconciliation = true;
            self.dropped_critical_count += 1;
            self.log_saturation_warning();
            Err(ActionSendError::Full)
        }
    }

    fn log_saturation_warning(&mut self) {
        let now = std::time::Instant::now();
        let should_log = match self.last_log_time {
            Some(t) => now.duration_since(t).as_secs() >= 2,
            None => true,
        };
        if should_log {
            self.last_log_time = Some(now);
            tracing::warn!(
                "Critical interaction outbox saturated (capacity {}). Backpressure active; reconciliation flag set. Total dropped: {}",
                MAX_CRITICAL_OUTBOX_CAPACITY,
                self.dropped_critical_count
            );
        }
    }
}

/// Sender of `InteractionEvent` from runtime bridge to TUI.
#[derive(Debug, Clone)]
pub enum TuiInteractionSender {
    Bounded {
        tx: mpsc::Sender<InteractionEvent>,
        outbox: std::sync::Arc<std::sync::Mutex<OutboxState>>,
    },
    Unbounded(mpsc::UnboundedSender<InteractionEvent>),
}

impl TuiInteractionSender {
    pub fn try_send(&self, event: InteractionEvent) -> Result<(), ActionSendError> {
        match self {
            Self::Bounded { tx, outbox } => {
                if tx.is_closed() {
                    return Err(ActionSendError::Closed);
                }

                let mut ob = outbox.lock().unwrap();

                // 1. Drain pending outbox queue first if possible
                while let Some(front) = ob.critical_queue.front() {
                    match tx.try_send(front.clone()) {
                        Ok(()) => {
                            ob.critical_queue.pop_front();
                        }
                        Err(TrySendError::Full(_)) => {
                            break;
                        }
                        Err(TrySendError::Closed(_)) => {
                            return Err(ActionSendError::Closed);
                        }
                    }
                }

                // If queue is empty, flush coalesced telemetry
                if ob.critical_queue.is_empty() {
                    if let Some(budget) = ob.latest_budget.take() {
                        let ev = InteractionEvent::BudgetSnapshotUpdated { snapshot: budget };
                        if let Err(TrySendError::Full(InteractionEvent::BudgetSnapshotUpdated {
                            snapshot,
                        })) = tx.try_send(ev)
                        {
                            ob.latest_budget = Some(snapshot);
                        }
                    }
                    if let Some(wf) = ob.latest_workflow.take() {
                        let ev = InteractionEvent::WorkflowSnapshotUpdated { snapshot: wf };
                        if let Err(TrySendError::Full(
                            InteractionEvent::WorkflowSnapshotUpdated { snapshot },
                        )) = tx.try_send(ev)
                        {
                            ob.latest_workflow = Some(snapshot);
                        }
                    }
                }

                // 2. If outbox has items, we cannot send the new event directly ahead of queued critical events
                if !ob.critical_queue.is_empty() {
                    return match event.delivery_class() {
                        crate::interaction::events::EventDeliveryClass::Critical => {
                            ob.push_critical(event)
                        }
                        crate::interaction::events::EventDeliveryClass::ReplaceableTelemetry => {
                            match event {
                                InteractionEvent::BudgetSnapshotUpdated { snapshot } => {
                                    ob.latest_budget = Some(snapshot);
                                }
                                InteractionEvent::WorkflowSnapshotUpdated { snapshot } => {
                                    ob.latest_workflow = Some(snapshot);
                                }
                                _ => {}
                            }
                            Ok(())
                        }
                        crate::interaction::events::EventDeliveryClass::Informational => {
                            Err(ActionSendError::Full)
                        }
                    };
                }

                // 3. Outbox is empty, try sending new event directly
                match tx.try_send(event) {
                    Ok(()) => Ok(()),
                    Err(TrySendError::Closed(_)) => Err(ActionSendError::Closed),
                    Err(TrySendError::Full(ev)) => match ev.delivery_class() {
                        crate::interaction::events::EventDeliveryClass::Critical => {
                            ob.push_critical(ev)
                        }
                        crate::interaction::events::EventDeliveryClass::ReplaceableTelemetry => {
                            match ev {
                                InteractionEvent::BudgetSnapshotUpdated { snapshot } => {
                                    ob.latest_budget = Some(snapshot);
                                }
                                InteractionEvent::WorkflowSnapshotUpdated { snapshot } => {
                                    ob.latest_workflow = Some(snapshot);
                                }
                                _ => {}
                            }
                            Ok(())
                        }
                        crate::interaction::events::EventDeliveryClass::Informational => {
                            Err(ActionSendError::Full)
                        }
                    },
                }
            }
            Self::Unbounded(tx) => tx.send(event).map_err(|_| ActionSendError::Closed),
        }
    }

    /// Flush any remaining outbox items when capacity opens.
    pub fn flush_outbox(&self) {
        if let Self::Bounded { tx, outbox } = self {
            if tx.is_closed() {
                return;
            }
            let mut ob = outbox.lock().unwrap();
            while let Some(front) = ob.critical_queue.front() {
                match tx.try_send(front.clone()) {
                    Ok(()) => {
                        ob.critical_queue.pop_front();
                    }
                    Err(_) => break,
                }
            }
            if ob.critical_queue.is_empty() {
                if let Some(budget) = ob.latest_budget.take() {
                    let ev = InteractionEvent::BudgetSnapshotUpdated { snapshot: budget };
                    if let Err(TrySendError::Full(InteractionEvent::BudgetSnapshotUpdated {
                        snapshot,
                    })) = tx.try_send(ev)
                    {
                        ob.latest_budget = Some(snapshot);
                    }
                }
                if let Some(wf) = ob.latest_workflow.take() {
                    let ev = InteractionEvent::WorkflowSnapshotUpdated { snapshot: wf };
                    if let Err(TrySendError::Full(InteractionEvent::WorkflowSnapshotUpdated {
                        snapshot,
                    })) = tx.try_send(ev)
                    {
                        ob.latest_workflow = Some(snapshot);
                    }
                }
            }
        }
    }

    /// Number of critical events currently held in the bounded outbox.
    pub fn outbox_len(&self) -> usize {
        match self {
            Self::Bounded { outbox, .. } => outbox.lock().unwrap().critical_queue.len(),
            Self::Unbounded(_) => 0,
        }
    }

    /// Check and reset the needs_reconciliation flag.
    pub fn take_needs_reconciliation(&self) -> bool {
        match self {
            Self::Bounded { outbox, .. } => {
                let mut ob = outbox.lock().unwrap();
                let needed = ob.needs_reconciliation;
                ob.needs_reconciliation = false;
                needed
            }
            Self::Unbounded(_) => false,
        }
    }

    /// Total count of dropped critical events.
    pub fn dropped_critical_count(&self) -> u64 {
        match self {
            Self::Bounded { outbox, .. } => outbox.lock().unwrap().dropped_critical_count,
            Self::Unbounded(_) => 0,
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
        Self::Bounded {
            tx: s,
            outbox: std::sync::Arc::new(std::sync::Mutex::new(OutboxState::default())),
        }
    }
}

impl From<mpsc::UnboundedSender<InteractionEvent>> for TuiInteractionSender {
    fn from(s: mpsc::UnboundedSender<InteractionEvent>) -> Self {
        Self::Unbounded(s)
    }
}
