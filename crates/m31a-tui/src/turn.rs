//! Canonical Turn State Machine and Lifecycle for M31A autonomous turns.

use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub struct IllegalTransitionError {
    pub from: &'static str,
    pub to: &'static str,
}

impl std::fmt::Display for IllegalTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Illegal turn transition from '{}' to '{}'",
            self.from, self.to
        )
    }
}

impl std::error::Error for IllegalTransitionError {}

/// Stable correlation identifier for all asynchronous operations in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCorrelation {
    pub session_id: String,
    pub session_epoch: u64,
    pub turn_id: String,
    pub operation_id: String,
}

impl EventCorrelation {
    pub fn new(
        session_id: impl Into<String>,
        session_epoch: u64,
        turn_id: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            session_epoch,
            turn_id: turn_id.into(),
            operation_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Canonical Turn State representing every valid phase in an autonomous turn lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnState {
    /// Ready and waiting for user input.
    Idle,
    /// Prompt received and submitted.
    Submitted {
        turn_id: String,
        started_at: Instant,
    },
    /// Awaiting admission through trust, guardian, or rate-limit gates.
    AwaitingAdmission { turn_id: String },
    /// Model is streaming reasoning/thinking tokens.
    Thinking {
        turn_id: String,
        started_at: Instant,
        preview: String,
    },
    /// Model is streaming response content chunks.
    Streaming {
        turn_id: String,
        started_at: Instant,
    },
    /// Sandbox or tool execution is active.
    ExecutingTool {
        turn_id: String,
        tool_id: String,
        tool_name: String,
        started_at: Instant,
    },
    /// Paused awaiting security approval decision.
    AwaitingApproval {
        turn_id: String,
        request_id: String,
        summary: String,
    },
    /// Paused awaiting response to an elicitation question.
    AwaitingQuestion {
        turn_id: String,
        question_id: String,
        prompt: String,
    },
    /// Compacting or summarizing conversation context.
    Compacting {
        turn_id: String,
        started_at: Instant,
    },
    /// User requested cancellation (e.g. Ctrl+C), cancellation signal sent.
    CancellationRequested { turn_id: String },
    /// Actively terminating child processes and cancelling tasks.
    Cancelling { turn_id: String },
    /// Turn completed execution.
    Completed {
        turn_id: String,
        tokens_used: usize,
        duration_ms: u64,
    },
    /// Turn halted with a failure.
    Failed { turn_id: String, error: String },
    /// Recovering from failure (e.g. self-healing or checkpoint rollback).
    Recovering { turn_id: String },
}

impl TurnState {
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            TurnState::Submitted { .. }
                | TurnState::AwaitingAdmission { .. }
                | TurnState::Thinking { .. }
                | TurnState::Streaming { .. }
                | TurnState::ExecutingTool { .. }
                | TurnState::Compacting { .. }
                | TurnState::CancellationRequested { .. }
                | TurnState::Cancelling { .. }
        )
    }

    pub fn is_cancellable(&self) -> bool {
        self.is_active() && !matches!(self, TurnState::Cancelling { .. })
    }

    pub fn turn_id(&self) -> Option<&str> {
        match self {
            TurnState::Idle => None,
            TurnState::Submitted { turn_id, .. }
            | TurnState::AwaitingAdmission { turn_id }
            | TurnState::Thinking { turn_id, .. }
            | TurnState::Streaming { turn_id, .. }
            | TurnState::ExecutingTool { turn_id, .. }
            | TurnState::AwaitingApproval { turn_id, .. }
            | TurnState::AwaitingQuestion { turn_id, .. }
            | TurnState::Compacting { turn_id, .. }
            | TurnState::CancellationRequested { turn_id }
            | TurnState::Cancelling { turn_id }
            | TurnState::Completed { turn_id, .. }
            | TurnState::Failed { turn_id, .. }
            | TurnState::Recovering { turn_id } => Some(turn_id),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TurnState::Idle => "Ready",
            TurnState::Submitted { .. } => "Submitted",
            TurnState::AwaitingAdmission { .. } => "Awaiting Admission",
            TurnState::Thinking { .. } => "Thinking",
            TurnState::Streaming { .. } => "Generating",
            TurnState::ExecutingTool { .. } => "Executing Tool",
            TurnState::AwaitingApproval { .. } => "Awaiting Approval",
            TurnState::AwaitingQuestion { .. } => "Awaiting Question",
            TurnState::Compacting { .. } => "Compacting",
            TurnState::CancellationRequested { .. } => "Cancelling...",
            TurnState::Cancelling { .. } => "Cancelling...",
            TurnState::Completed { .. } => "Completed",
            TurnState::Failed { .. } => "Failed",
            TurnState::Recovering { .. } => "Recovering",
        }
    }
}

/// Authoritative state machine enforcing legal turn state transitions.
#[derive(Debug, Clone)]
pub struct TurnStateMachine {
    current: TurnState,
}

impl Default for TurnStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnStateMachine {
    pub fn new() -> Self {
        Self {
            current: TurnState::Idle,
        }
    }

    pub fn state(&self) -> &TurnState {
        &self.current
    }

    pub fn is_active(&self) -> bool {
        self.current.is_active()
    }

    pub fn label(&self) -> &'static str {
        self.current.label()
    }

    pub fn current_turn_id(&self) -> Option<&str> {
        self.current.turn_id()
    }

    /// Transitions to the target `TurnState` if the transition is legal under the state machine contract.
    pub fn transition_to(&mut self, next: TurnState) -> Result<(), IllegalTransitionError> {
        let legal = match (&self.current, &next) {
            // From Idle: can start a turn or stay idle
            (TurnState::Idle, TurnState::Submitted { .. })
            | (TurnState::Idle, TurnState::Idle)
            | (TurnState::Idle, TurnState::ExecutingTool { .. }) => true,

            // From Submitted: can await admission, start thinking/streaming, cancel, or fail
            (TurnState::Submitted { .. }, TurnState::AwaitingAdmission { .. })
            | (TurnState::Submitted { .. }, TurnState::Thinking { .. })
            | (TurnState::Submitted { .. }, TurnState::Streaming { .. })
            | (TurnState::Submitted { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::Submitted { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::Submitted { .. }, TurnState::Failed { .. }) => true,

            // From AwaitingAdmission
            (TurnState::AwaitingAdmission { .. }, TurnState::Thinking { .. })
            | (TurnState::AwaitingAdmission { .. }, TurnState::Streaming { .. })
            | (TurnState::AwaitingAdmission { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::AwaitingAdmission { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::AwaitingAdmission { .. }, TurnState::Failed { .. }) => true,

            // From Thinking: can stream, execute tool, compact, await approval/question, cancel, complete, fail
            (TurnState::Thinking { .. }, TurnState::Thinking { .. })
            | (TurnState::Thinking { .. }, TurnState::Streaming { .. })
            | (TurnState::Thinking { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::Thinking { .. }, TurnState::Compacting { .. })
            | (TurnState::Thinking { .. }, TurnState::AwaitingApproval { .. })
            | (TurnState::Thinking { .. }, TurnState::AwaitingQuestion { .. })
            | (TurnState::Thinking { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::Thinking { .. }, TurnState::Completed { .. })
            | (TurnState::Thinking { .. }, TurnState::Failed { .. }) => true,

            // From Streaming
            (TurnState::Streaming { .. }, TurnState::Streaming { .. })
            | (TurnState::Streaming { .. }, TurnState::Thinking { .. })
            | (TurnState::Streaming { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::Streaming { .. }, TurnState::Compacting { .. })
            | (TurnState::Streaming { .. }, TurnState::AwaitingApproval { .. })
            | (TurnState::Streaming { .. }, TurnState::AwaitingQuestion { .. })
            | (TurnState::Streaming { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::Streaming { .. }, TurnState::Completed { .. })
            | (TurnState::Streaming { .. }, TurnState::Failed { .. }) => true,

            // From ExecutingTool
            (TurnState::ExecutingTool { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Thinking { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Streaming { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Compacting { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::AwaitingApproval { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::AwaitingQuestion { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Completed { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Failed { .. })
            | (TurnState::ExecutingTool { .. }, TurnState::Idle) => true,

            // From AwaitingApproval
            (TurnState::AwaitingApproval { .. }, TurnState::ExecutingTool { .. })
            | (TurnState::AwaitingApproval { .. }, TurnState::Thinking { .. })
            | (TurnState::AwaitingApproval { .. }, TurnState::Streaming { .. })
            | (TurnState::AwaitingApproval { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::AwaitingApproval { .. }, TurnState::Failed { .. })
            | (TurnState::AwaitingApproval { .. }, TurnState::Idle) => true,

            // From AwaitingQuestion
            (TurnState::AwaitingQuestion { .. }, TurnState::Thinking { .. })
            | (TurnState::AwaitingQuestion { .. }, TurnState::Streaming { .. })
            | (TurnState::AwaitingQuestion { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::AwaitingQuestion { .. }, TurnState::Failed { .. })
            | (TurnState::AwaitingQuestion { .. }, TurnState::Idle) => true,

            // From Compacting
            (TurnState::Compacting { .. }, TurnState::Thinking { .. })
            | (TurnState::Compacting { .. }, TurnState::Streaming { .. })
            | (TurnState::Compacting { .. }, TurnState::Completed { .. })
            | (TurnState::Compacting { .. }, TurnState::CancellationRequested { .. })
            | (TurnState::Compacting { .. }, TurnState::Failed { .. }) => true,

            // From CancellationRequested
            (TurnState::CancellationRequested { .. }, TurnState::Cancelling { .. })
            | (TurnState::CancellationRequested { .. }, TurnState::Idle)
            | (TurnState::CancellationRequested { .. }, TurnState::Failed { .. }) => true,

            // From Cancelling
            (TurnState::Cancelling { .. }, TurnState::Idle)
            | (TurnState::Cancelling { .. }, TurnState::Failed { .. }) => true,

            // From Completed
            (TurnState::Completed { .. }, TurnState::Idle)
            | (TurnState::Completed { .. }, TurnState::Submitted { .. }) => true,

            // From Failed
            (TurnState::Failed { .. }, TurnState::Recovering { .. })
            | (TurnState::Failed { .. }, TurnState::Idle)
            | (TurnState::Failed { .. }, TurnState::Submitted { .. }) => true,

            // From Recovering
            (TurnState::Recovering { .. }, TurnState::Idle)
            | (TurnState::Recovering { .. }, TurnState::Failed { .. }) => true,

            // Any state can be forced to Idle on explicit reset/recovery
            (_, TurnState::Idle) => true,
            _ => false,
        };

        if legal {
            self.current = next;
            Ok(())
        } else {
            Err(IllegalTransitionError {
                from: self.current.label(),
                to: next.label(),
            })
        }
    }

    /// Resets the state machine unconditionally to Idle (e.g. on shutdown, hard reset, or session switch).
    pub fn force_idle(&mut self) {
        self.current = TurnState::Idle;
    }

    pub fn transition_idle(&mut self) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Idle)
    }

    pub fn transition_submitted(
        &mut self,
        turn_id: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Submitted {
            turn_id: turn_id.into(),
            started_at: Instant::now(),
        })
    }

    pub fn transition_thinking(
        &mut self,
        turn_id: impl Into<String>,
        preview: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Thinking {
            turn_id: turn_id.into(),
            started_at: Instant::now(),
            preview: preview.into(),
        })
    }

    pub fn transition_streaming(
        &mut self,
        turn_id: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Streaming {
            turn_id: turn_id.into(),
            started_at: Instant::now(),
        })
    }

    pub fn transition_executing_tool(
        &mut self,
        turn_id: impl Into<String>,
        tool_id: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::ExecutingTool {
            turn_id: turn_id.into(),
            tool_id: tool_id.into(),
            tool_name: tool_name.into(),
            started_at: Instant::now(),
        })
    }

    pub fn transition_awaiting_approval(
        &mut self,
        turn_id: impl Into<String>,
        request_id: impl Into<String>,
        summary: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::AwaitingApproval {
            turn_id: turn_id.into(),
            request_id: request_id.into(),
            summary: summary.into(),
        })
    }

    pub fn transition_awaiting_question(
        &mut self,
        turn_id: impl Into<String>,
        question_id: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::AwaitingQuestion {
            turn_id: turn_id.into(),
            question_id: question_id.into(),
            prompt: prompt.into(),
        })
    }

    pub fn transition_compacting(
        &mut self,
        turn_id: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Compacting {
            turn_id: turn_id.into(),
            started_at: Instant::now(),
        })
    }

    pub fn transition_cancellation_requested(
        &mut self,
        turn_id: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::CancellationRequested {
            turn_id: turn_id.into(),
        })
    }

    pub fn transition_cancelling(
        &mut self,
        turn_id: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Cancelling {
            turn_id: turn_id.into(),
        })
    }

    pub fn transition_completed(
        &mut self,
        turn_id: impl Into<String>,
        tokens_used: usize,
        duration_ms: u64,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Completed {
            turn_id: turn_id.into(),
            tokens_used,
            duration_ms,
        })
    }

    pub fn transition_failed(
        &mut self,
        turn_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Result<(), IllegalTransitionError> {
        self.transition_to(TurnState::Failed {
            turn_id: turn_id.into(),
            error: error.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_state_machine_happy_path() {
        let mut sm = TurnStateMachine::new();
        assert_eq!(*sm.state(), TurnState::Idle);
        assert!(!sm.is_active());

        // Submitted
        sm.transition_to(TurnState::Submitted {
            turn_id: "turn-1".to_string(),
            started_at: Instant::now(),
        })
        .expect("legal");
        assert!(sm.is_active());
        assert_eq!(sm.current_turn_id(), Some("turn-1"));

        // Thinking
        sm.transition_to(TurnState::Thinking {
            turn_id: "turn-1".to_string(),
            started_at: Instant::now(),
            preview: "Analyzing code...".to_string(),
        })
        .expect("legal");
        assert_eq!(sm.label(), "Thinking");

        // Streaming
        sm.transition_to(TurnState::Streaming {
            turn_id: "turn-1".to_string(),
            started_at: Instant::now(),
        })
        .expect("legal");
        assert_eq!(sm.label(), "Generating");

        // Executing tool
        sm.transition_to(TurnState::ExecutingTool {
            turn_id: "turn-1".to_string(),
            tool_id: "tool-42".to_string(),
            tool_name: "read_file".to_string(),
            started_at: Instant::now(),
        })
        .expect("legal");
        assert_eq!(sm.label(), "Executing Tool");

        // Completed
        sm.transition_to(TurnState::Completed {
            turn_id: "turn-1".to_string(),
            tokens_used: 1024,
            duration_ms: 1500,
        })
        .expect("legal");
        assert!(!sm.is_active());

        // Back to Idle
        sm.transition_to(TurnState::Idle).expect("legal");
        assert_eq!(*sm.state(), TurnState::Idle);
    }

    #[test]
    fn test_cancellation_lifecycle() {
        let mut sm = TurnStateMachine::new();
        sm.transition_to(TurnState::Submitted {
            turn_id: "turn-cancel".to_string(),
            started_at: Instant::now(),
        })
        .unwrap();

        sm.transition_to(TurnState::Streaming {
            turn_id: "turn-cancel".to_string(),
            started_at: Instant::now(),
        })
        .unwrap();

        // Cancellation requested
        sm.transition_to(TurnState::CancellationRequested {
            turn_id: "turn-cancel".to_string(),
        })
        .unwrap();
        assert_eq!(sm.label(), "Cancelling...");

        // Cancelling processes
        sm.transition_to(TurnState::Cancelling {
            turn_id: "turn-cancel".to_string(),
        })
        .unwrap();
        assert!(!sm.state().is_cancellable()); // already cancelling

        // Transition to Idle when confirmed
        sm.transition_to(TurnState::Idle).unwrap();
        assert_eq!(*sm.state(), TurnState::Idle);
    }

    #[test]
    fn test_illegal_transitions_rejected() {
        let mut sm = TurnStateMachine::new();
        // Cannot transition from Idle directly to Completed without running a turn
        let res = sm.transition_to(TurnState::Completed {
            turn_id: "fake".to_string(),
            tokens_used: 100,
            duration_ms: 50,
        });
        assert!(res.is_err());
    }
}
