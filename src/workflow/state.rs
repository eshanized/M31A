//! Durable state aggregates and transition validation for workflow runs and steps.

use crate::ids::{AgentId, ArtifactId, MissionId, WorkflowRunId, WorkflowStepRunId};
use crate::workflow::error::WorkflowError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Canonical execution mode of a workflow run.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    #[default]
    Standard,
    Interactive,
    Autonomous,
}

impl fmt::Display for WorkflowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::Interactive => write!(f, "interactive"),
            Self::Autonomous => write!(f, "autonomous"),
        }
    }
}

impl FromStr for WorkflowMode {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "interactive" => Ok(Self::Interactive),
            "autonomous" => Ok(Self::Autonomous),
            other => Err(WorkflowError::InvalidState(format!(
                "unknown workflow mode: '{}'",
                other
            ))),
        }
    }
}

/// The formal lifecycle states of a workflow run.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    #[default]
    Pending,
    Running,
    AwaitingInput,
    AwaitingApproval,
    Completed,
    Failed,
    Blocked,
    Cancelled,
    Superseded,
}

impl fmt::Display for WorkflowRunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::AwaitingInput => write!(f, "awaiting_input"),
            Self::AwaitingApproval => write!(f, "awaiting_approval"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Blocked => write!(f, "blocked"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Superseded => write!(f, "superseded"),
        }
    }
}

impl FromStr for WorkflowRunState {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "awaiting_input" => Ok(Self::AwaitingInput),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            "cancelled" => Ok(Self::Cancelled),
            "superseded" => Ok(Self::Superseded),
            other => Err(WorkflowError::InvalidState(format!(
                "unknown workflow run state: '{}'",
                other
            ))),
        }
    }
}

impl WorkflowRunState {
    /// Validates whether a state transition is legal according to the formal state machine.
    pub fn can_transition_to(&self, target: WorkflowRunState) -> bool {
        if *self == target {
            return true;
        }

        match self {
            Self::Pending => matches!(target, Self::Running | Self::Cancelled),
            Self::Running => matches!(
                target,
                Self::AwaitingInput
                    | Self::AwaitingApproval
                    | Self::Completed
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
            ),
            Self::AwaitingInput => matches!(target, Self::Running | Self::Cancelled),
            Self::AwaitingApproval => {
                matches!(
                    target,
                    Self::Running | Self::Blocked | Self::Cancelled | Self::Failed
                )
            }
            Self::Blocked => matches!(target, Self::Running | Self::Failed | Self::Cancelled),
            Self::Failed => matches!(target, Self::Running | Self::Cancelled),
            Self::Completed => matches!(target, Self::Superseded),
            Self::Cancelled => matches!(target, Self::Superseded),
            Self::Superseded => false, // Terminal
        }
    }

    /// Whether this state is an unrecoverable terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Superseded)
    }
}

/// The formal lifecycle states of an individual workflow step run.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepState {
    #[default]
    Pending,
    Running,
    AwaitingApproval,
    AwaitingInput,
    Completed,
    Failed,
    Blocked,
    Cancelled,
    Skipped,
}

impl fmt::Display for WorkflowStepState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::AwaitingApproval => write!(f, "awaiting_approval"),
            Self::AwaitingInput => write!(f, "awaiting_input"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Blocked => write!(f, "blocked"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl FromStr for WorkflowStepState {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "awaiting_input" => Ok(Self::AwaitingInput),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            other => Err(WorkflowError::InvalidState(format!(
                "unknown workflow step state: '{}'",
                other
            ))),
        }
    }
}

impl WorkflowStepState {
    /// Validates whether a step state transition is legal.
    pub fn can_transition_to(&self, target: WorkflowStepState) -> bool {
        if *self == target {
            return true;
        }

        match self {
            Self::Pending => {
                matches!(
                    target,
                    Self::Running | Self::Blocked | Self::Skipped | Self::Cancelled
                )
            }
            Self::Running => matches!(
                target,
                Self::AwaitingApproval
                    | Self::AwaitingInput
                    | Self::Completed
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
            ),
            Self::AwaitingApproval => matches!(
                target,
                Self::Completed | Self::Running | Self::Blocked | Self::Cancelled | Self::Failed
            ),
            Self::AwaitingInput => matches!(target, Self::Running | Self::Cancelled),
            Self::Blocked => {
                matches!(
                    target,
                    Self::Pending | Self::Running | Self::Cancelled | Self::Skipped | Self::Failed
                )
            }
            Self::Failed => matches!(target, Self::Running | Self::Cancelled),
            Self::Completed => false, // Terminal
            Self::Skipped => false,   // Terminal
            Self::Cancelled => false, // Terminal
        }
    }

    /// Whether this step state is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Skipped | Self::Cancelled)
    }
}

/// Durable aggregate representing an execution instance of a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: WorkflowRunId,
    pub definition_id: String,
    pub definition_version: u32,
    pub workspace_root: PathBuf,
    pub status: WorkflowRunState,
    pub mode: WorkflowMode,
    pub current_step_key: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_summary: Option<String>,
}

impl WorkflowRun {
    /// Creates a new workflow run in Pending status.
    pub fn new(
        definition_id: impl Into<String>,
        definition_version: u32,
        workspace_root: PathBuf,
        mode: WorkflowMode,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: WorkflowRunId::new(),
            definition_id: definition_id.into(),
            definition_version,
            workspace_root,
            status: WorkflowRunState::Pending,
            mode,
            current_step_key: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
            error_summary: None,
        }
    }

    /// Transitions the workflow run to a new state after validating the legal state machine.
    pub fn transition_to(
        &mut self,
        new_state: WorkflowRunState,
        error_summary: Option<String>,
    ) -> Result<(), WorkflowError> {
        if !self.status.can_transition_to(new_state) {
            return Err(WorkflowError::InvalidTransition {
                from: self.status.to_string(),
                to: new_state.to_string(),
                reason: format!("illegal transition from {} to {}", self.status, new_state),
            });
        }

        self.status = new_state;
        self.updated_at = Utc::now();

        if matches!(
            new_state,
            WorkflowRunState::Completed | WorkflowRunState::Cancelled | WorkflowRunState::Failed
        ) {
            self.completed_at = Some(self.updated_at);
        }

        if error_summary.is_some() {
            self.error_summary = error_summary;
        }

        Ok(())
    }
}

/// Durable execution instance of a single step within a workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStepRun {
    pub id: WorkflowStepRunId,
    pub workflow_run_id: WorkflowRunId,
    pub step_key: String,
    pub status: WorkflowStepState,
    pub assigned_agent_id: Option<AgentId>,
    pub mission_id: Option<MissionId>,
    pub attempt_count: u32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub halt_reason: Option<String>,
}

impl WorkflowStepRun {
    /// Creates a new step run in Pending status.
    pub fn new(workflow_run_id: WorkflowRunId, step_key: impl Into<String>) -> Self {
        Self {
            id: WorkflowStepRunId::new(),
            workflow_run_id,
            step_key: step_key.into(),
            status: WorkflowStepState::Pending,
            assigned_agent_id: None,
            mission_id: None,
            attempt_count: 1,
            started_at: Utc::now(),
            completed_at: None,
            halt_reason: None,
        }
    }

    /// Transitions the step run to a new state after validating the legal transition graph.
    pub fn transition_to(
        &mut self,
        new_state: WorkflowStepState,
        halt_reason: Option<String>,
    ) -> Result<(), WorkflowError> {
        if !self.status.can_transition_to(new_state) {
            return Err(WorkflowError::InvalidTransition {
                from: self.status.to_string(),
                to: new_state.to_string(),
                reason: format!(
                    "illegal step transition from {} to {}",
                    self.status, new_state
                ),
            });
        }

        // If retrying from Failed to Running, increment attempt count
        if self.status == WorkflowStepState::Failed && new_state == WorkflowStepState::Running {
            self.attempt_count += 1;
        }

        self.status = new_state;

        if new_state.is_terminal() {
            self.completed_at = Some(Utc::now());
        }

        if halt_reason.is_some() {
            self.halt_reason = halt_reason;
        }

        Ok(())
    }
}

/// Status of an artifact produced during workflow execution.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowArtifactStatus {
    #[default]
    Valid,
    Superseded,
}

impl fmt::Display for WorkflowArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "valid"),
            Self::Superseded => write!(f, "superseded"),
        }
    }
}

impl FromStr for WorkflowArtifactStatus {
    type Err = WorkflowError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "valid" => Ok(Self::Valid),
            "superseded" => Ok(Self::Superseded),
            other => Err(WorkflowError::InvalidState(format!(
                "unknown artifact status: '{}'",
                other
            ))),
        }
    }
}

/// Durable record of an artifact produced by a workflow step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowArtifact {
    pub id: ArtifactId,
    pub workflow_run_id: WorkflowRunId,
    pub step_run_id: WorkflowStepRunId,
    pub name: String,
    pub path: PathBuf,
    pub content_hash: String,
    pub version: u32,
    pub status: WorkflowArtifactStatus,
    pub created_at: DateTime<Utc>,
}

impl WorkflowArtifact {
    /// Creates a new valid workflow artifact record.
    pub fn new(
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        name: impl Into<String>,
        path: PathBuf,
        content_hash: impl Into<String>,
        version: u32,
    ) -> Self {
        Self {
            id: ArtifactId::new(),
            workflow_run_id,
            step_run_id,
            name: name.into(),
            path,
            content_hash: content_hash.into(),
            version,
            status: WorkflowArtifactStatus::Valid,
            created_at: Utc::now(),
        }
    }

    /// Mark this artifact as superseded by an upstream decision change.
    pub fn mark_superseded(&mut self) {
        self.status = WorkflowArtifactStatus::Superseded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_run_lifecycle_transitions() {
        let mut run = WorkflowRun::new(
            "genesis",
            1,
            PathBuf::from("/workspace"),
            WorkflowMode::Standard,
        );
        assert_eq!(run.status, WorkflowRunState::Pending);

        // Pending -> Running
        assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());
        assert_eq!(run.status, WorkflowRunState::Running);

        // Running -> AwaitingApproval
        assert!(
            run.transition_to(WorkflowRunState::AwaitingApproval, None)
                .is_ok()
        );
        assert_eq!(run.status, WorkflowRunState::AwaitingApproval);

        // AwaitingApproval -> Running
        assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());

        // Running -> Completed
        assert!(run.transition_to(WorkflowRunState::Completed, None).is_ok());
        assert!(run.completed_at.is_some());

        // Completed -> Superseded
        assert!(
            run.transition_to(WorkflowRunState::Superseded, None)
                .is_ok()
        );

        // Superseded is terminal: cannot transition to Running
        assert!(run.transition_to(WorkflowRunState::Running, None).is_err());
    }

    #[test]
    fn test_invalid_workflow_run_transitions() {
        let mut run = WorkflowRun::new(
            "genesis",
            1,
            PathBuf::from("/workspace"),
            WorkflowMode::Standard,
        );

        // Cannot jump directly from Pending to Completed
        assert!(
            run.transition_to(WorkflowRunState::Completed, None)
                .is_err()
        );

        // Cannot jump from Pending to Blocked
        assert!(run.transition_to(WorkflowRunState::Blocked, None).is_err());
    }

    #[test]
    fn test_step_run_transitions() {
        let run_id = WorkflowRunId::new();
        let mut step = WorkflowStepRun::new(run_id, "discovery");
        assert_eq!(step.status, WorkflowStepState::Pending);

        assert!(step.transition_to(WorkflowStepState::Running, None).is_ok());
        assert!(
            step.transition_to(WorkflowStepState::AwaitingApproval, None)
                .is_ok()
        );
        assert!(
            step.transition_to(WorkflowStepState::Completed, None)
                .is_ok()
        );
        assert!(step.completed_at.is_some());

        // Completed is terminal
        assert!(
            step.transition_to(WorkflowStepState::Running, None)
                .is_err()
        );
    }

    #[test]
    fn test_step_retry_increments_attempt() {
        let run_id = WorkflowRunId::new();
        let mut step = WorkflowStepRun::new(run_id, "research");
        step.transition_to(WorkflowStepState::Running, None)
            .unwrap();
        step.transition_to(
            WorkflowStepState::Failed,
            Some("Timeout exceeded".to_string()),
        )
        .unwrap();
        assert_eq!(step.attempt_count, 1);

        // Retry transition from Failed -> Running increments attempt
        step.transition_to(WorkflowStepState::Running, None)
            .unwrap();
        assert_eq!(step.attempt_count, 2);
    }
}
