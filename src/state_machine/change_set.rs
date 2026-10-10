//! Change set state machine and validated lifecycle transitions (KRN-02).
//!
//! Enforces: The model proposes. The runtime decides.
//! State transitions govern the lifecycle of a code modification change proposal:
//! Proposed -> Validating -> Authorized -> Applying -> Applied -> Observing -> Verifying -> Accepted.
//! Terminal states: Accepted, Rejected, Conflicted, RolledBack, Failed.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::error::TransitionError;

/// Validated states of a change set lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSetState {
    /// Initial state: model has submitted a structured change proposal.
    Proposed,
    /// Runtime is reconciling preconditions against repository baseline and active graph.
    Validating,
    /// Preconditions verified, policy and permissions approved; authorized to mutate.
    Authorized,
    /// Controlled mutation is actively applying edits to workspace files.
    Applying,
    /// All mutations successfully written to workspace files.
    Applied,
    /// Runtime is re-observing the repository: computing fresh hashes, diff, affected symbols.
    Observing,
    /// Verification checks (compiler, tests, static analysis) are executing.
    Verifying,
    /// Final success: verification passed and diff self-review approved.
    Accepted,
    /// Precondition or policy check failed prior to mutation.
    Rejected,
    /// Stale hash, symbol move, or competing task conflict detected.
    Conflicted,
    /// Mutation or verification failed, and workspace files were restored to pre-proposal state.
    RolledBack,
    /// Unrecoverable failure occurred during execution.
    Failed,
    /// Rollback failed or partially restored workspace files; manual/escalated reconciliation required.
    PartialRecovery,
}

impl ChangeSetState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Validating => "validating",
            Self::Authorized => "authorized",
            Self::Applying => "applying",
            Self::Applied => "applied",
            Self::Observing => "observing",
            Self::Verifying => "verifying",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Conflicted => "conflicted",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
            Self::PartialRecovery => "partial_recovery",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Accepted
                | Self::Rejected
                | Self::Conflicted
                | Self::RolledBack
                | Self::Failed
                | Self::PartialRecovery
        )
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

impl fmt::Display for ChangeSetState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ChangeSetState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "proposed" => Ok(Self::Proposed),
            "validating" => Ok(Self::Validating),
            "authorized" => Ok(Self::Authorized),
            "applying" => Ok(Self::Applying),
            "applied" => Ok(Self::Applied),
            "observing" => Ok(Self::Observing),
            "verifying" => Ok(Self::Verifying),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "conflicted" => Ok(Self::Conflicted),
            "rolled_back" | "rolledback" => Ok(Self::RolledBack),
            "failed" => Ok(Self::Failed),
            "partial_recovery" | "partialrecovery" => Ok(Self::PartialRecovery),
            other => Err(format!("Unknown ChangeSetState: '{other}'")),
        }
    }
}

/// Events driving change set state machine transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeSetEvent {
    StartValidation,
    Authorize,
    ReportConflict,
    Reject,
    StartApplying,
    MutationApplied,
    Rollback,
    RecoveryFailed,
    StartObserving,
    StartVerifying,
    Accept,
    Fail,
}

/// Transitions a change set from its current state based on an incoming event.
/// Fails closed if the transition is invalid or if attempting to transition from a terminal state.
pub fn transition_change_set(
    current: ChangeSetState,
    event: ChangeSetEvent,
) -> Result<ChangeSetState, TransitionError> {
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(current.to_string()));
    }

    match (current, event) {
        // From Proposed
        (ChangeSetState::Proposed, ChangeSetEvent::StartValidation) => {
            Ok(ChangeSetState::Validating)
        }
        (ChangeSetState::Proposed, ChangeSetEvent::Reject) => Ok(ChangeSetState::Rejected),
        (ChangeSetState::Proposed, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Validating
        (ChangeSetState::Validating, ChangeSetEvent::Authorize) => Ok(ChangeSetState::Authorized),
        (ChangeSetState::Validating, ChangeSetEvent::ReportConflict) => {
            Ok(ChangeSetState::Conflicted)
        }
        (ChangeSetState::Validating, ChangeSetEvent::Reject) => Ok(ChangeSetState::Rejected),
        (ChangeSetState::Validating, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Authorized
        (ChangeSetState::Authorized, ChangeSetEvent::StartApplying) => Ok(ChangeSetState::Applying),
        (ChangeSetState::Authorized, ChangeSetEvent::Reject) => Ok(ChangeSetState::Rejected),
        (ChangeSetState::Authorized, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Applying
        (ChangeSetState::Applying, ChangeSetEvent::MutationApplied) => Ok(ChangeSetState::Applied),
        (ChangeSetState::Applying, ChangeSetEvent::Rollback) => Ok(ChangeSetState::RolledBack),
        (ChangeSetState::Applying, ChangeSetEvent::RecoveryFailed) => {
            Ok(ChangeSetState::PartialRecovery)
        }
        (ChangeSetState::Applying, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Applied
        (ChangeSetState::Applied, ChangeSetEvent::StartObserving) => Ok(ChangeSetState::Observing),
        (ChangeSetState::Applied, ChangeSetEvent::Rollback) => Ok(ChangeSetState::RolledBack),
        (ChangeSetState::Applied, ChangeSetEvent::RecoveryFailed) => {
            Ok(ChangeSetState::PartialRecovery)
        }
        (ChangeSetState::Applied, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Observing
        (ChangeSetState::Observing, ChangeSetEvent::StartVerifying) => {
            Ok(ChangeSetState::Verifying)
        }
        (ChangeSetState::Observing, ChangeSetEvent::Rollback) => Ok(ChangeSetState::RolledBack),
        (ChangeSetState::Observing, ChangeSetEvent::RecoveryFailed) => {
            Ok(ChangeSetState::PartialRecovery)
        }
        (ChangeSetState::Observing, ChangeSetEvent::ReportConflict) => {
            Ok(ChangeSetState::Conflicted)
        }
        (ChangeSetState::Observing, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // From Verifying
        (ChangeSetState::Verifying, ChangeSetEvent::Accept) => Ok(ChangeSetState::Accepted),
        (ChangeSetState::Verifying, ChangeSetEvent::Rollback) => Ok(ChangeSetState::RolledBack),
        (ChangeSetState::Verifying, ChangeSetEvent::RecoveryFailed) => {
            Ok(ChangeSetState::PartialRecovery)
        }
        (ChangeSetState::Verifying, ChangeSetEvent::ReportConflict) => {
            Ok(ChangeSetState::Conflicted)
        }
        (ChangeSetState::Verifying, ChangeSetEvent::Fail) => Ok(ChangeSetState::Failed),

        // Invalid transitions
        (from, _) => Err(TransitionError::invalid_transition(
            from.to_string(),
            "unexpected_event",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_lifecycle() {
        let mut state = ChangeSetState::Proposed;
        state = transition_change_set(state, ChangeSetEvent::StartValidation).unwrap();
        assert_eq!(state, ChangeSetState::Validating);

        state = transition_change_set(state, ChangeSetEvent::Authorize).unwrap();
        assert_eq!(state, ChangeSetState::Authorized);

        state = transition_change_set(state, ChangeSetEvent::StartApplying).unwrap();
        assert_eq!(state, ChangeSetState::Applying);

        state = transition_change_set(state, ChangeSetEvent::MutationApplied).unwrap();
        assert_eq!(state, ChangeSetState::Applied);

        state = transition_change_set(state, ChangeSetEvent::StartObserving).unwrap();
        assert_eq!(state, ChangeSetState::Observing);

        state = transition_change_set(state, ChangeSetEvent::StartVerifying).unwrap();
        assert_eq!(state, ChangeSetState::Verifying);

        state = transition_change_set(state, ChangeSetEvent::Accept).unwrap();
        assert_eq!(state, ChangeSetState::Accepted);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_conflict_from_validating() {
        let state = ChangeSetState::Validating;
        let next = transition_change_set(state, ChangeSetEvent::ReportConflict).unwrap();
        assert_eq!(next, ChangeSetState::Conflicted);
        assert!(next.is_terminal());
    }

    #[test]
    fn test_rollback_from_applying() {
        let state = ChangeSetState::Applying;
        let next = transition_change_set(state, ChangeSetEvent::Rollback).unwrap();
        assert_eq!(next, ChangeSetState::RolledBack);
        assert!(next.is_terminal());
    }

    #[test]
    fn test_rollback_from_verifying() {
        let state = ChangeSetState::Verifying;
        let next = transition_change_set(state, ChangeSetEvent::Rollback).unwrap();
        assert_eq!(next, ChangeSetState::RolledBack);
        assert!(next.is_terminal());
    }

    #[test]
    fn test_terminal_state_rejects_transitions() {
        let err = transition_change_set(ChangeSetState::Accepted, ChangeSetEvent::StartValidation)
            .unwrap_err();
        assert!(matches!(err, TransitionError::TerminalState { .. }));
    }

    #[test]
    fn test_invalid_forward_transition_fails() {
        let err =
            transition_change_set(ChangeSetState::Proposed, ChangeSetEvent::Accept).unwrap_err();
        assert!(matches!(err, TransitionError::InvalidTransition { .. }));
    }
}
