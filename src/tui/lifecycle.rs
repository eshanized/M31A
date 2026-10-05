//! Governed lifecycle projection for the conversation-first TUI.
//!
//! Pure presentation projection over the canonical M31A runtime lifecycle.
//! This module never executes tools, never mutates workspace state, never
//! authorizes execution, and never writes to the database. It only derives
//! renderable lifecycle state from trusted runtime events.
//!
//! Authority order: persisted `LifecycleStage` > runtime `EventType` >
//! `InteractionEvent` summaries. UI-only guesses are never invented here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;

/// Renderable lifecycle stage derived from canonical runtime truth.
///
/// Only includes stages that exist in `LifecycleStage` or are deterministically
/// derivable from authoritative runtime events. No UI-only convenience states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TuiLifecycleStage {
    /// No governed lifecycle active for this session yet.
    Idle,
    /// A fresh intent was recorded; discovery has not converged yet.
    IntentActive,
    /// The runtime is drafting a candidate plan (not yet reviewable).
    PlanDraft,
    /// The runtime is drafting the candidate task set (not yet reviewable).
    TasksDraft,
    /// Runtime is waiting for the operator to answer discovery questions.
    DiscoveryRequired,
    /// A candidate plan revision is waiting for explicit operator review.
    PlanReviewRequired,
    /// A newer plan revision superseded the one under review.
    PlanRevisionAvailable,
    /// The candidate plan was explicitly accepted by the operator.
    PlanAccepted,
    /// A candidate task set is waiting for explicit operator review.
    TasksReviewRequired,
    /// A newer task revision superseded the one under review.
    TaskRevisionAvailable,
    /// The candidate task set was explicitly accepted by the operator.
    TasksAccepted,
    /// Execution was requested; explicit workspace-modification authorization needed.
    ExecutionAuthorizationRequired,
    /// Execution was authorized for exact artifact revisions — not yet executing.
    ExecutionAuthorized,
    /// The runtime crossed the StartExecution boundary and is executing.
    Executing,
    /// Verification evidence is being collected or evaluated.
    Verifying,
    /// Terminal: lifecycle completed with verification evidence.
    Completed,
    /// Terminal or actionable: execution or verification failed with evidence.
    Failed,
    /// Terminal: operator rejected a governance decision.
    Rejected,
    /// Terminal: operator cancelled the lifecycle.
    Cancelled,
    /// Blocked on an external condition; may resume via unblock.
    Blocked,
}

impl TuiLifecycleStage {
    /// Explicit textual state indicator; never color-only.
    /// Quiet human-readable labels — no bracketed shouting.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::IntentActive => "Working",
            Self::PlanDraft => "Planning",
            Self::TasksDraft => "Planning",
            Self::DiscoveryRequired => "Waiting for input",
            Self::PlanReviewRequired => "Waiting for approval",
            Self::PlanRevisionAvailable => "Review update",
            Self::PlanAccepted => "Accepted",
            Self::TasksReviewRequired => "Waiting for approval",
            Self::TaskRevisionAvailable => "Review update",
            Self::TasksAccepted => "Accepted",
            Self::ExecutionAuthorizationRequired => "Waiting for approval",
            Self::ExecutionAuthorized => "Authorized",
            Self::Executing => "Working",
            Self::Verifying => "Verifying",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Rejected => "Rejected",
            Self::Cancelled => "Cancelled",
            Self::Blocked => "Blocked",
        }
    }

    /// Legacy bracketed label for compatibility with older snapshots.
    /// New rendering must use `label()`.
    pub fn bracket_label(&self) -> String {
        format!("[{}]", self.label().to_uppercase())
    }

    /// Terminal stages reject further governed transitions in the projection.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Rejected | Self::Cancelled
        )
    }

    /// Stages where the TUI must surface an explicit governance decision.
    pub fn is_governance_gate(&self) -> bool {
        matches!(
            self,
            Self::DiscoveryRequired
                | Self::PlanReviewRequired
                | Self::TasksReviewRequired
                | Self::ExecutionAuthorizationRequired
        )
    }
}

/// Presentation-only projection of the governed lifecycle.
///
/// Every field is derived from persisted lifecycle state, runtime events, or
/// authoritative artifacts. Nothing here is mutated by rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiLifecycleProjection {
    pub session_id: Option<String>,
    pub stage: TuiLifecycleStage,
    pub plan_revision: Option<u32>,
    pub task_revision: Option<u32>,
    pub plan_hash: Option<String>,
    pub task_hash: Option<String>,
    pub authorization_id: Option<String>,
    pub pending_questions: Vec<String>,
    #[serde(default)]
    pub pending_discovery_questions: Vec<crate::workflow::genesis::DynamicQuestion>,
    pub selected_option_index: Option<usize>,
    pub verification_summary: Option<String>,
    pub failure_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl Default for TuiLifecycleProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiLifecycleProjection {
    pub fn new() -> Self {
        Self {
            session_id: None,
            stage: TuiLifecycleStage::Idle,
            plan_revision: None,
            task_revision: None,
            plan_hash: None,
            task_hash: None,
            authorization_id: None,
            pending_questions: Vec::new(),
            pending_discovery_questions: Vec::new(),
            selected_option_index: None,
            verification_summary: None,
            failure_reason: None,
            updated_at: Utc::now(),
        }
    }

    pub fn select_next_option(&mut self) {
        if let Some(active_q) = self.pending_discovery_questions.first() {
            if !active_q.options.is_empty() {
                let current = self.selected_option_index.unwrap_or(0);
                self.selected_option_index = Some((current + 1) % active_q.options.len());
            }
        }
    }

    pub fn select_prev_option(&mut self) {
        if let Some(active_q) = self.pending_discovery_questions.first() {
            if !active_q.options.is_empty() {
                let count = active_q.options.len();
                let current = self.selected_option_index.unwrap_or(0);
                self.selected_option_index =
                    Some(if current == 0 { count - 1 } else { current - 1 });
            }
        }
    }

    pub fn selected_option(&self) -> Option<&str> {
        let q = self.pending_discovery_questions.first()?;
        let idx = self.selected_option_index?;
        q.options.get(idx).map(|s| s.as_str())
    }

    /// Compact hash for display; uses the authoritative runtime hash verbatim.
    pub fn short_hash(hash: &str) -> String {
        hash.chars().take(8).collect()
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Terminal projection states are sealed: only an explicit reset to a
    /// non-terminal event (new intent) may leave them.
    fn sealed(&self) -> bool {
        self.stage.is_terminal()
    }

    /// Hydrate the projection from persisted canonical lifecycle state.
    ///
    /// Used once at session resume before live events arrive: historical
    /// state + new events = current projection (§12). Deterministic mapping
    /// from `LifecycleStage`; terminal states seal exactly like event folds.
    pub fn apply_lifecycle_stage(
        &mut self,
        state: &crate::persistence::sqlite::repositories::lifecycle::PersistedLifecycleState,
    ) {
        use crate::state_machine::lifecycle::LifecycleStage as S;
        self.session_id = Some(state.session_id.clone());
        self.plan_revision = Some(state.plan_revision);
        self.task_revision = Some(state.task_revision);
        self.authorization_id = state.authorization_id.map(|id| id.to_string());
        self.stage = match state.stage {
            S::IntentActive => TuiLifecycleStage::IntentActive,
            S::AwaitingInformation => TuiLifecycleStage::DiscoveryRequired,
            S::PlanDraft => TuiLifecycleStage::PlanDraft,
            S::PlanReview => TuiLifecycleStage::PlanReviewRequired,
            S::PlanRevision => TuiLifecycleStage::PlanRevisionAvailable,
            S::PlanAccepted => TuiLifecycleStage::PlanAccepted,
            S::TasksDraft => TuiLifecycleStage::TasksDraft,
            S::TasksReview => TuiLifecycleStage::TasksReviewRequired,
            S::TasksRevision => TuiLifecycleStage::TaskRevisionAvailable,
            S::TasksAccepted => TuiLifecycleStage::TasksAccepted,
            S::ExecutionAwaitingAuthorization => TuiLifecycleStage::ExecutionAuthorizationRequired,
            S::ExecutionAuthorized => TuiLifecycleStage::ExecutionAuthorized,
            S::Executing => TuiLifecycleStage::Executing,
            S::Completed => TuiLifecycleStage::Completed,
            S::Failed => TuiLifecycleStage::Failed,
            S::Rejected => TuiLifecycleStage::Rejected,
            S::Cancelled => TuiLifecycleStage::Cancelled,
            S::Blocked => TuiLifecycleStage::Blocked,
        };
        self.touch();
    }

    /// Deterministically fold a trusted kernel event into the projection.
    pub fn apply_event(&mut self, event: &EventEnvelope) {
        match &event.event_type {
            EventType::PlanReviewRequired {
                session_id,
                revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                // A fresh review supersedes any revision notice.
                self.stage = TuiLifecycleStage::PlanReviewRequired;
                self.plan_revision = Some(*revision);
                self.failure_reason = None;
                self.touch();
            }
            EventType::PlanRevisionCreated {
                session_id,
                revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                // If a review is already open, flag the newer revision explicitly.
                if self.stage == TuiLifecycleStage::PlanReviewRequired {
                    self.stage = TuiLifecycleStage::PlanRevisionAvailable;
                } else {
                    self.stage = TuiLifecycleStage::PlanReviewRequired;
                }
                self.plan_revision = Some(*revision);
                self.touch();
            }
            EventType::PlanAccepted {
                session_id,
                revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::PlanAccepted;
                self.plan_revision = Some(*revision);
                self.touch();
            }
            EventType::PlanRejected {
                session_id, reason, ..
            } => {
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::Rejected;
                self.failure_reason = Some(reason.clone());
                self.touch();
            }
            EventType::TasksReviewRequired {
                session_id,
                plan_revision,
                task_revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::TasksReviewRequired;
                self.plan_revision = Some(*plan_revision);
                self.task_revision = Some(*task_revision);
                self.failure_reason = None;
                self.touch();
            }
            EventType::TaskRevisionCreated {
                session_id,
                task_revision,
                plan_revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                if self.stage == TuiLifecycleStage::TasksReviewRequired {
                    self.stage = TuiLifecycleStage::TaskRevisionAvailable;
                } else {
                    self.stage = TuiLifecycleStage::TasksReviewRequired;
                }
                self.plan_revision = Some(*plan_revision);
                self.task_revision = Some(*task_revision);
                self.touch();
            }
            EventType::TasksAccepted {
                session_id,
                task_revision,
                plan_revision,
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::TasksAccepted;
                self.plan_revision = Some(*plan_revision);
                self.task_revision = Some(*task_revision);
                self.touch();
            }
            EventType::ExecutionAuthorizationRequired {
                session_id,
                plan_revision,
                task_revision,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::ExecutionAuthorizationRequired;
                self.plan_revision = Some(*plan_revision);
                self.task_revision = Some(*task_revision);
                self.touch();
            }
            EventType::ExecutionAuthorized {
                session_id,
                authorization_id,
                ..
            } => {
                if self.sealed() {
                    return;
                }
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::ExecutionAuthorized;
                self.authorization_id = Some(authorization_id.to_string());
                self.failure_reason = None;
                self.touch();
            }
            EventType::ExecutionAuthorizationRejected { session_id, reason } => {
                self.session_id = Some(session_id.clone());
                self.stage = TuiLifecycleStage::Rejected;
                self.failure_reason = Some(reason.clone());
                self.touch();
            }
            EventType::TaskGraphMaterialized { .. } => {
                // Materialization commits before StartExecution; the runtime is
                // now committed to the authorized graph. Only advance from the
                // authorized state so replayed histories cannot fake execution.
                if self.stage == TuiLifecycleStage::ExecutionAuthorized {
                    self.stage = TuiLifecycleStage::Executing;
                    self.touch();
                }
            }
            EventType::TaskStarted { .. } | EventType::MissionStarted { .. } => {
                if self.stage == TuiLifecycleStage::ExecutionAuthorized
                    || self.stage == TuiLifecycleStage::Executing
                {
                    self.stage = TuiLifecycleStage::Executing;
                    self.touch();
                }
            }
            EventType::VerificationStarted { .. } => {
                if self.sealed() {
                    return;
                }
                self.stage = TuiLifecycleStage::Verifying;
                self.touch();
            }
            EventType::VerificationCompleted {
                passed, evidence, ..
            } => {
                if *passed {
                    if self.sealed() {
                        return;
                    }
                    self.stage = TuiLifecycleStage::Verifying;
                    self.verification_summary = Some(evidence.clone());
                } else {
                    self.stage = TuiLifecycleStage::Failed;
                    self.failure_reason = Some(evidence.clone());
                }
                self.touch();
            }
            EventType::MissionCompleted { .. } => {
                self.stage = TuiLifecycleStage::Completed;
                self.failure_reason = None;
                self.touch();
            }
            EventType::MissionFailed { reason, .. } => {
                self.stage = TuiLifecycleStage::Failed;
                self.failure_reason = Some(reason.clone());
                self.touch();
            }
            EventType::MissionCancelled { reason, .. } => {
                self.stage = TuiLifecycleStage::Cancelled;
                self.failure_reason = Some(reason.clone());
                self.touch();
            }
            EventType::TaskBlocked { reason, .. } => {
                if self.sealed() {
                    return;
                }
                self.stage = TuiLifecycleStage::Blocked;
                self.failure_reason = Some(reason.clone());
                self.touch();
            }
            EventType::TaskUnblocked { .. } if self.stage == TuiLifecycleStage::Blocked => {
                self.stage = TuiLifecycleStage::Executing;
                self.failure_reason = None;
                self.touch();
            }
            _ => {}
        }
    }
}
