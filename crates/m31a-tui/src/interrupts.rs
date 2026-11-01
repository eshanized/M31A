//! Deferred interrupt queue buffering approval dialogs and notifications while model turns stream actively.
//!
//! Prompt overlays, tool activity toasts, and approval dialogs are queued
//! while the assistant is actively streaming (Thinking/StreamingContent/ToolExecuting).
//! They are flushed on "settle" — when the turn reaches a stable state
//! (AwaitingApproval, TurnCompleted, Failed). This prevents mid-stream flicker
//! and re-layout thrash.

use crate::chatwidget::TurnExecutionState;
use std::collections::VecDeque;
use std::time::Instant;

/// An interrupt that can be deferred during active streaming.
#[derive(Debug, Clone, PartialEq)]
pub enum DeferredInterrupt {
    /// Approval dialog for a tool call requiring human permission.
    ApprovalDialog {
        request_id: String,
        tool_name: String,
        action_summary: String,
        queued_at: Instant,
    },
    /// Tool execution activity toast (started/completed/failed).
    ToolActivity {
        tool_id: String,
        tool_name: String,
        status: ToolActivityStatus,
        queued_at: Instant,
    },
    /// Prompt overlay (e.g., question from `request_user_input` tool).
    PromptOverlay {
        question_id: String,
        prompt: String,
        options: Vec<String>,
        is_multi_select: bool,
        queued_at: Instant,
    },
    /// Generic toast notice.
    Toast {
        message: String,
        level: crate::transcript::NoticeLevel,
        queued_at: Instant,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolActivityStatus {
    Started,
    Completed,
    Failed,
}

/// State machine for deferred interrupt handling.
pub struct DeferredInterruptQueue {
    queue: VecDeque<DeferredInterrupt>,
    /// Current turn execution state.
    turn_state: TurnExecutionState,
    /// Whether we're currently inside a "settle" flush (prevents re-entrancy).
    is_flushing: bool,
}

impl Default for DeferredInterruptQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl DeferredInterruptQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            turn_state: TurnExecutionState::Idle,
            is_flushing: false,
        }
    }

    /// Returns the current turn execution state.
    pub fn turn_state(&self) -> &TurnExecutionState {
        &self.turn_state
    }

    /// Updates the turn execution state. If transitioning from an active
    /// state to a settled state, triggers a flush of deferred interrupts.
    pub fn set_turn_state(&mut self, new_state: TurnExecutionState) {
        let was_active = self.turn_state.is_active();
        let is_active = new_state.is_active();
        let is_settled = matches!(
            new_state,
            TurnExecutionState::AwaitingApproval { .. }
                | TurnExecutionState::TurnCompleted { .. }
                | TurnExecutionState::Failed { .. }
        );

        self.turn_state = new_state;

        // Flush deferred interrupts when settling from active processing
        if was_active && !is_active && is_settled && !self.is_flushing {
            self.flush_deferred();
        }
    }

    /// Enqueues a deferred interrupt. If the turn is not active, it's
    /// emitted immediately; otherwise it's queued for later flush.
    pub fn enqueue(&mut self, interrupt: DeferredInterrupt) {
        if self.turn_state.is_active() {
            self.queue.push_back(interrupt);
        } else {
            // Not streaming — emit immediately
            self.emit_interrupt(interrupt);
        }
    }

    /// Flushes all queued interrupts in FIFO order.
    fn flush_deferred(&mut self) {
        if self.is_flushing || self.queue.is_empty() {
            return;
        }

        self.is_flushing = true;
        while let Some(interrupt) = self.queue.pop_front() {
            self.emit_interrupt(interrupt);
        }
        self.is_flushing = false;
    }

    /// Emits an interrupt to the UI (to be implemented by the caller via callback).
    fn emit_interrupt(&self, interrupt: DeferredInterrupt) {
        // This would normally call a callback registered by the TUI app
        // For now, we just log; the actual UI emission is handled by the app
        // via the `on_deferred_interrupt` callback in App.
        tracing::debug!("Deferred interrupt emitted: {:?}", interrupt);
    }

    /// Returns true if there are pending deferred interrupts.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Returns the number of pending deferred interrupts.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Drains and returns all queued interrupts in FIFO order.
    pub fn drain_pending(&mut self) -> Vec<DeferredInterrupt> {
        self.queue.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interrupt_queued_during_streaming() {
        let mut queue = DeferredInterruptQueue::new();
        queue.set_turn_state(TurnExecutionState::StreamingContent {
            started_at: Instant::now(),
            tokens: 10,
        });

        queue.enqueue(DeferredInterrupt::Toast {
            message: "test".to_string(),
            level: crate::transcript::NoticeLevel::Info,
            queued_at: Instant::now(),
        });

        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_interrupt_emitted_immediately_when_idle() {
        let mut queue = DeferredInterruptQueue::new();
        queue.set_turn_state(TurnExecutionState::Idle);

        queue.enqueue(DeferredInterrupt::Toast {
            message: "test".to_string(),
            level: crate::transcript::NoticeLevel::Info,
            queued_at: Instant::now(),
        });

        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_flush_on_settle() {
        let mut queue = DeferredInterruptQueue::new();
        queue.set_turn_state(TurnExecutionState::StreamingContent {
            started_at: Instant::now(),
            tokens: 10,
        });

        queue.enqueue(DeferredInterrupt::Toast {
            message: "test".to_string(),
            level: crate::transcript::NoticeLevel::Info,
            queued_at: Instant::now(),
        });

        // Transition to settled state should flush
        queue.set_turn_state(TurnExecutionState::TurnCompleted {
            duration_ms: 1000,
            tokens_used: 50,
        });

        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_no_flush_between_active_states() {
        let mut queue = DeferredInterruptQueue::new();
        queue.set_turn_state(TurnExecutionState::Thinking {
            started_at: Instant::now(),
            tokens: 5,
            preview: "thinking...".to_string(),
        });

        queue.enqueue(DeferredInterrupt::Toast {
            message: "test1".to_string(),
            level: crate::transcript::NoticeLevel::Info,
            queued_at: Instant::now(),
        });

        // Switch to another active state - should NOT flush
        queue.set_turn_state(TurnExecutionState::StreamingContent {
            started_at: Instant::now(),
            tokens: 10,
        });

        assert_eq!(queue.pending_count(), 1);

        // Now settle - should flush
        queue.set_turn_state(TurnExecutionState::TurnCompleted {
            duration_ms: 1000,
            tokens_used: 50,
        });

        assert_eq!(queue.pending_count(), 0);
    }
}
