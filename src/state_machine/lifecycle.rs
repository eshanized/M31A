//! Human-governed pre-execution lifecycle state machine.
//!
//! Enforces: "The model proposes. The runtime decides."
//!
//! Ensures that between raw user intent and autonomous workspace execution,
//! there are mandatory, typed human-governed gates:
//! 1. Dynamic discovery & question resolution
//! 2. Plan review, editing, revision, and acceptance
//! 3. Task review, editing, addition, removal, and acceptance
//! 4. Explicit execution authorization
//!
//! Terminal outcomes: Cancelled, Rejected, Blocked, Failed, Completed.

use crate::state_machine::error::TransitionError;
use serde::{Deserialize, Serialize};

/// Human-governed lifecycle stages from raw user intent to autonomous execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LifecycleStage {
    /// Raw or minimal user intent is being understood and enriched.
    IntentActive,
    /// Unresolved unknowns exist; dynamic discovery is awaiting user answers.
    AwaitingInformation,
    /// Candidate plan proposal has been generated and is ready for review.
    PlanDraft,
    /// Candidate plan is under active inspection by the operator.
    PlanReview,
    /// Candidate plan is undergoing model revision or manual edit.
    PlanRevision,
    /// Candidate plan has been explicitly accepted by the operator.
    PlanAccepted,
    /// Executable task set is being generated from the accepted plan.
    TasksDraft,
    /// Executable tasks are under active inspection by the operator.
    TasksReview,
    /// Tasks are undergoing edit, add, remove, or regeneration.
    TasksRevision,
    /// Executable tasks have been explicitly accepted by the operator.
    TasksAccepted,
    /// Plan and tasks are accepted; explicit operator launch authorization is requested.
    ExecutionAwaitingAuthorization,
    /// Explicit operator launch authorization has been granted.
    ExecutionAuthorized,
    /// Handed off to runtime execution spine (TaskGraph / AutonomyController / Scheduler).
    Executing,

    // Terminal / exceptional outcomes
    Cancelled,
    Rejected,
    Blocked,
    Failed,
    Completed,
}

pub type LifecycleState = LifecycleStage;

impl std::fmt::Display for LifecycleStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IntentActive => write!(f, "IntentActive"),
            Self::AwaitingInformation => write!(f, "AwaitingInformation"),
            Self::PlanDraft => write!(f, "PlanDraft"),
            Self::PlanReview => write!(f, "PlanReview"),
            Self::PlanRevision => write!(f, "PlanRevision"),
            Self::PlanAccepted => write!(f, "PlanAccepted"),
            Self::TasksDraft => write!(f, "TasksDraft"),
            Self::TasksReview => write!(f, "TasksReview"),
            Self::TasksRevision => write!(f, "TasksRevision"),
            Self::TasksAccepted => write!(f, "TasksAccepted"),
            Self::ExecutionAwaitingAuthorization => write!(f, "ExecutionAwaitingAuthorization"),
            Self::ExecutionAuthorized => write!(f, "ExecutionAuthorized"),
            Self::Executing => write!(f, "Executing"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Rejected => write!(f, "Rejected"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Failed => write!(f, "Failed"),
            Self::Completed => write!(f, "Completed"),
        }
    }
}

impl std::str::FromStr for LifecycleStage {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "").as_str() {
            "intentactive" => Ok(Self::IntentActive),
            "awaitinginformation" => Ok(Self::AwaitingInformation),
            "plandraft" => Ok(Self::PlanDraft),
            "planreview" => Ok(Self::PlanReview),
            "planrevision" => Ok(Self::PlanRevision),
            "planaccepted" => Ok(Self::PlanAccepted),
            "tasksdraft" => Ok(Self::TasksDraft),
            "tasksreview" => Ok(Self::TasksReview),
            "tasksrevision" => Ok(Self::TasksRevision),
            "tasksaccepted" => Ok(Self::TasksAccepted),
            "executionawaitingauthorization" => Ok(Self::ExecutionAwaitingAuthorization),
            "executionauthorized" => Ok(Self::ExecutionAuthorized),
            "executing" => Ok(Self::Executing),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            _ => Err(TransitionError::invalid_transition(
                s,
                "UnknownLifecycleStage",
            )),
        }
    }
}

impl LifecycleStage {
    /// Check whether this state is terminal (no standard forward transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Rejected
        )
    }

    /// Check whether this state represents a human-governed review or authorization gate.
    pub fn is_governance_gate(&self) -> bool {
        matches!(
            self,
            Self::PlanReview | Self::TasksReview | Self::ExecutionAwaitingAuthorization
        )
    }
}

/// Events that trigger lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleEvent {
    /// Start or re-engage intent understanding.
    StartIntent,
    /// Unresolved unknowns require dynamic questions from the operator.
    QuestionsRequired,
    /// User provided answers to dynamic questions.
    InformationProvided,
    /// Discovery has converged; sufficient facts exist to synthesize a plan proposal.
    DiscoveryConverged,
    /// Model has proposed a candidate plan draft.
    PlanDrafted,
    /// Transition into human plan review gate.
    EnterPlanReview,
    /// Operator submitted a manual edit to the plan.
    PlanEditSubmitted,
    /// Operator requested a model revision of the plan.
    PlanRevisionRequested,
    /// Plan revision (manual or model) has completed validation.
    PlanRevisionValidated,
    /// Operator explicitly accepted the current plan revision.
    PlanAccepted,
    /// Operator explicitly rejected the current plan revision.
    PlanRejected,
    /// Candidate tasks have been generated from the accepted plan.
    TasksDrafted,
    /// Transition into human task review gate.
    EnterTasksReview,
    /// Operator submitted an edit to an individual task.
    TaskEditSubmitted,
    /// Operator added a task to the candidate task set.
    TaskAdded,
    /// Operator removed a task from the candidate task set.
    TaskRemoved,
    /// Operator requested task regeneration.
    TaskRegenerateRequested,
    /// Task revision (edit, add, remove, regen) has completed validation.
    TaskRevisionValidated,
    /// Operator explicitly accepted the current task set.
    TasksAccepted,
    /// Request execution authorization from operator.
    RequestExecutionAuthorization,
    /// Operator explicitly authorized workspace execution.
    AuthorizeExecution,
    /// Operator rejected execution authorization.
    RejectExecutionAuthorization,
    /// Hand off accepted tasks to execution runtime.
    StartExecution,
    /// Invalidate downstream approvals due to upstream plan modification.
    InvalidateToPlanReview,
    /// Invalidate downstream authorization due to task modification.
    InvalidateToTasksReview,
    /// Block progress due to unresolvable conflict or policy violation.
    Block,
    /// Unblock and return to active governance or execution.
    Unblock,
    /// Operation or verification failed unrecoverably.
    Fail,
    /// Cancel operation.
    Cancel,
    /// Execution completed with verified evidence.
    Complete,
}

/// Validate and execute a lifecycle state transition.
///
/// Invariants enforced:
/// - PlanReview cannot enter without a draft/revision plan proposal
/// - PlanAccepted cannot occur without a valid current plan
/// - TasksDraft cannot occur before PlanAccepted
/// - TasksAccepted cannot occur before valid tasks exist
/// - ExecutionAwaitingAuthorization cannot occur before TasksAccepted
/// - ExecutionAuthorized cannot occur without explicit operator authorization
/// - Executing cannot begin from TasksAccepted without explicit authorization
/// - Any upstream plan change invalidates downstream task acceptance and execution authorization
/// - Any task change invalidates execution authorization
/// - Terminal states cannot transition
pub fn transition_lifecycle(
    current: LifecycleStage,
    event: LifecycleEvent,
) -> Result<LifecycleStage, TransitionError> {
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(format!("{:?}", current)));
    }

    let target = match (current, event) {
        // Universal abort/terminal transitions from non-terminal states
        (_, LifecycleEvent::Cancel) => LifecycleStage::Cancelled,
        (_, LifecycleEvent::Fail) => LifecycleStage::Failed,
        (_, LifecycleEvent::Block) if current != LifecycleStage::Blocked => LifecycleStage::Blocked,

        // Unblock returns to previous active stage (defaulting to IntentActive)
        (LifecycleStage::Blocked, LifecycleEvent::Unblock) => LifecycleStage::IntentActive,

        // Upstream Invalidation cascades
        (
            LifecycleStage::TasksDraft
            | LifecycleStage::TasksReview
            | LifecycleStage::TasksRevision
            | LifecycleStage::TasksAccepted
            | LifecycleStage::ExecutionAwaitingAuthorization
            | LifecycleStage::ExecutionAuthorized,
            LifecycleEvent::InvalidateToPlanReview,
        ) => LifecycleStage::PlanReview,

        (
            LifecycleStage::ExecutionAwaitingAuthorization | LifecycleStage::ExecutionAuthorized,
            LifecycleEvent::InvalidateToTasksReview,
        ) => LifecycleStage::TasksReview,

        // Discovery & Intent Lifecycle
        (LifecycleStage::IntentActive, LifecycleEvent::QuestionsRequired) => {
            LifecycleStage::AwaitingInformation
        }
        (LifecycleStage::IntentActive, LifecycleEvent::DiscoveryConverged) => {
            LifecycleStage::PlanDraft
        }
        (LifecycleStage::AwaitingInformation, LifecycleEvent::InformationProvided) => {
            LifecycleStage::IntentActive
        }
        (LifecycleStage::AwaitingInformation, LifecycleEvent::DiscoveryConverged) => {
            LifecycleStage::PlanDraft
        }

        // Plan Lifecycle
        (LifecycleStage::PlanDraft, LifecycleEvent::EnterPlanReview) => LifecycleStage::PlanReview,
        (LifecycleStage::PlanReview, LifecycleEvent::PlanEditSubmitted) => {
            LifecycleStage::PlanRevision
        }
        (LifecycleStage::PlanReview, LifecycleEvent::PlanRevisionRequested) => {
            LifecycleStage::PlanRevision
        }
        (LifecycleStage::PlanRevision, LifecycleEvent::PlanRevisionValidated) => {
            LifecycleStage::PlanReview
        }
        (LifecycleStage::PlanReview, LifecycleEvent::PlanAccepted) => LifecycleStage::PlanAccepted,
        (LifecycleStage::PlanReview, LifecycleEvent::PlanRejected) => LifecycleStage::Rejected,

        // Task Lifecycle
        (LifecycleStage::PlanAccepted, LifecycleEvent::TasksDrafted) => LifecycleStage::TasksDraft,
        (LifecycleStage::TasksDraft, LifecycleEvent::EnterTasksReview) => {
            LifecycleStage::TasksReview
        }
        (LifecycleStage::TasksReview, LifecycleEvent::TaskEditSubmitted) => {
            LifecycleStage::TasksRevision
        }
        (LifecycleStage::TasksReview, LifecycleEvent::TaskAdded) => LifecycleStage::TasksRevision,
        (LifecycleStage::TasksReview, LifecycleEvent::TaskRemoved) => LifecycleStage::TasksRevision,
        (LifecycleStage::TasksReview, LifecycleEvent::TaskRegenerateRequested) => {
            LifecycleStage::TasksRevision
        }
        (LifecycleStage::TasksRevision, LifecycleEvent::TaskRevisionValidated) => {
            LifecycleStage::TasksReview
        }
        (LifecycleStage::TasksReview, LifecycleEvent::TasksAccepted) => {
            LifecycleStage::TasksAccepted
        }

        // Execution Authorization Gate
        (LifecycleStage::TasksAccepted, LifecycleEvent::RequestExecutionAuthorization) => {
            LifecycleStage::ExecutionAwaitingAuthorization
        }
        (LifecycleStage::ExecutionAwaitingAuthorization, LifecycleEvent::AuthorizeExecution) => {
            LifecycleStage::ExecutionAuthorized
        }
        (
            LifecycleStage::ExecutionAwaitingAuthorization,
            LifecycleEvent::RejectExecutionAuthorization,
        ) => LifecycleStage::Rejected,

        // Execution Spine Handoff
        (LifecycleStage::ExecutionAuthorized, LifecycleEvent::StartExecution) => {
            LifecycleStage::Executing
        }

        // Execution Completion
        (LifecycleStage::Executing, LifecycleEvent::Complete) => LifecycleStage::Completed,

        // Disallowed / Illegal Transitions
        (from, ev) => {
            return Err(TransitionError::invalid_transition(
                format!("{:?}", from),
                format!("{:?}", ev),
            ));
        }
    };

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_happy_path_governance_progression() {
        let mut state = LifecycleStage::IntentActive;

        // Discovery loop
        state = transition_lifecycle(state, LifecycleEvent::QuestionsRequired).unwrap();
        assert_eq!(state, LifecycleStage::AwaitingInformation);

        state = transition_lifecycle(state, LifecycleEvent::InformationProvided).unwrap();
        assert_eq!(state, LifecycleStage::IntentActive);

        state = transition_lifecycle(state, LifecycleEvent::DiscoveryConverged).unwrap();
        assert_eq!(state, LifecycleStage::PlanDraft);

        // Plan review
        state = transition_lifecycle(state, LifecycleEvent::EnterPlanReview).unwrap();
        assert_eq!(state, LifecycleStage::PlanReview);

        // Plan revision cycle
        state = transition_lifecycle(state, LifecycleEvent::PlanRevisionRequested).unwrap();
        assert_eq!(state, LifecycleStage::PlanRevision);
        state = transition_lifecycle(state, LifecycleEvent::PlanRevisionValidated).unwrap();
        assert_eq!(state, LifecycleStage::PlanReview);

        // Plan acceptance
        state = transition_lifecycle(state, LifecycleEvent::PlanAccepted).unwrap();
        assert_eq!(state, LifecycleStage::PlanAccepted);

        // Task generation & review
        state = transition_lifecycle(state, LifecycleEvent::TasksDrafted).unwrap();
        assert_eq!(state, LifecycleStage::TasksDraft);
        state = transition_lifecycle(state, LifecycleEvent::EnterTasksReview).unwrap();
        assert_eq!(state, LifecycleStage::TasksReview);

        // Task revision cycle (edit, add, remove)
        state = transition_lifecycle(state, LifecycleEvent::TaskEditSubmitted).unwrap();
        assert_eq!(state, LifecycleStage::TasksRevision);
        state = transition_lifecycle(state, LifecycleEvent::TaskRevisionValidated).unwrap();
        assert_eq!(state, LifecycleStage::TasksReview);

        state = transition_lifecycle(state, LifecycleEvent::TasksAccepted).unwrap();
        assert_eq!(state, LifecycleStage::TasksAccepted);

        // Execution authorization gate
        state = transition_lifecycle(state, LifecycleEvent::RequestExecutionAuthorization).unwrap();
        assert_eq!(state, LifecycleStage::ExecutionAwaitingAuthorization);

        state = transition_lifecycle(state, LifecycleEvent::AuthorizeExecution).unwrap();
        assert_eq!(state, LifecycleStage::ExecutionAuthorized);

        // Execution handoff
        state = transition_lifecycle(state, LifecycleEvent::StartExecution).unwrap();
        assert_eq!(state, LifecycleStage::Executing);

        // Completion
        state = transition_lifecycle(state, LifecycleEvent::Complete).unwrap();
        assert_eq!(state, LifecycleStage::Completed);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_cannot_bypass_execution_authorization() {
        // Attempting to jump directly from TasksAccepted to Executing must fail
        let res = transition_lifecycle(
            LifecycleStage::TasksAccepted,
            LifecycleEvent::StartExecution,
        );
        assert!(res.is_err());

        // Attempting to jump directly from TasksReview to AuthorizeExecution must fail
        let res = transition_lifecycle(
            LifecycleStage::TasksReview,
            LifecycleEvent::AuthorizeExecution,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_upstream_invalidation_cascades() {
        // If execution is authorized but plan changes, authorization is invalidated
        let state = transition_lifecycle(
            LifecycleStage::ExecutionAuthorized,
            LifecycleEvent::InvalidateToPlanReview,
        )
        .unwrap();
        assert_eq!(state, LifecycleStage::PlanReview);

        // If awaiting authorization but a task is modified, it returns to TasksReview
        let state = transition_lifecycle(
            LifecycleStage::ExecutionAwaitingAuthorization,
            LifecycleEvent::InvalidateToTasksReview,
        )
        .unwrap();
        assert_eq!(state, LifecycleStage::TasksReview);
    }

    #[test]
    fn test_rejection_outcomes() {
        let state =
            transition_lifecycle(LifecycleStage::PlanReview, LifecycleEvent::PlanRejected).unwrap();
        assert_eq!(state, LifecycleStage::Rejected);
        assert!(state.is_terminal());

        let state = transition_lifecycle(
            LifecycleStage::ExecutionAwaitingAuthorization,
            LifecycleEvent::RejectExecutionAuthorization,
        )
        .unwrap();
        assert_eq!(state, LifecycleStage::Rejected);
        assert!(state.is_terminal());
    }

    #[test]
    fn test_terminal_states_reject_all() {
        for term in &[
            LifecycleStage::Completed,
            LifecycleStage::Failed,
            LifecycleStage::Cancelled,
            LifecycleStage::Rejected,
        ] {
            assert!(transition_lifecycle(*term, LifecycleEvent::StartIntent).is_err());
            assert!(transition_lifecycle(*term, LifecycleEvent::Cancel).is_err());
            assert!(transition_lifecycle(*term, LifecycleEvent::AuthorizeExecution).is_err());
        }
    }
}
