//! ExecutionRun aggregate model.
//!
//! Canonical top-level execution entity owning the execution lifecycle,
//! state transitions, and event replay watermarks.

use crate::ids::{ExecutionRunId, SessionId, TaskGraphId};
use crate::state::intake::{AutonomyMode, NormalizedIntake};
use crate::state::policy_context::PolicyContext;
use crate::state::replay::apply_lifecycle_event;
pub use crate::state::replay::ReplayError;
use crate::state_machine::ExecutionRunState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// ExecutionRun aggregate containing authoritative run state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionRun {
    pub id: ExecutionRunId,
    pub objective: String,
    pub constraints: Vec<String>,
    pub requirements: Vec<String>,
    pub success_criteria: Vec<String>,
    pub workspace_root: PathBuf,
    pub mode: AutonomyMode,
    pub policy_context: PolicyContext,
    pub status: ExecutionRunState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub parent_run_id: Option<ExecutionRunId>,
    pub session_id: Option<SessionId>,
    pub task_graph_id: Option<TaskGraphId>,
    pub last_applied_sequence: u64,
}

impl ExecutionRun {
    /// Create a new execution run with default configuration.
    pub fn new(id: ExecutionRunId, objective: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            objective,
            constraints: Vec::new(),
            requirements: Vec::new(),
            success_criteria: Vec::new(),
            workspace_root: PathBuf::new(),
            mode: AutonomyMode::Safe,
            policy_context: PolicyContext::standard(),
            status: ExecutionRunState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_run_id: None,
            session_id: None,
            task_graph_id: None,
            last_applied_sequence: 0,
        }
    }

    /// Construct an execution run from validated, normalized intake.
    pub fn from_normalized(id: ExecutionRunId, normalized: NormalizedIntake) -> Self {
        let now = Utc::now();
        Self {
            id,
            objective: normalized.objective,
            constraints: normalized.constraints,
            requirements: normalized.requirements,
            success_criteria: normalized.success_criteria,
            workspace_root: normalized.workspace_root,
            mode: normalized.mode,
            policy_context: normalized.policy_context,
            status: ExecutionRunState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_run_id: None,
            session_id: None,
            task_graph_id: None,
            last_applied_sequence: 0,
        }
    }

    /// Update status and refresh lifecycle timestamps.
    pub fn update_status(&mut self, new_status: ExecutionRunState) {
        crate::state::lifecycle::update_status(
            &mut self.status,
            &mut self.started_at,
            &mut self.completed_at,
            &mut self.updated_at,
            new_status,
        );
    }

    /// Update the state reconstruction sequence watermark.
    pub fn update_watermark(&mut self, sequence: u64) {
        crate::state::lifecycle::update_watermark(
            &mut self.last_applied_sequence,
            &mut self.updated_at,
            sequence,
        );
    }

    /// Pure, side-effect-free state transition for event replay and recovery.
    pub fn apply(
        &mut self,
        envelope: &crate::events::envelope::EventEnvelope,
    ) -> Result<(), ReplayError> {
        apply_lifecycle_event(
            &mut self.status,
            &mut self.started_at,
            &mut self.completed_at,
            &mut self.last_applied_sequence,
            &mut self.updated_at,
            envelope,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_run_construction() {
        let id = ExecutionRunId::new();
        let run = ExecutionRun::new(id, "Test run".to_string());
        assert_eq!(run.id, id);
        assert_eq!(run.objective, "Test run");
        assert_eq!(run.status, ExecutionRunState::Created);
        assert_eq!(run.last_applied_sequence, 0);
        assert_eq!(run.mode, AutonomyMode::Safe);
        assert!(run.constraints.is_empty());
        assert!(run.task_graph_id.is_none());
        assert!(run.session_id.is_none());
    }

    #[test]
    fn test_execution_run_status_update() {
        let id = ExecutionRunId::new();
        let mut run = ExecutionRun::new(id, "Test".to_string());
        run.update_status(ExecutionRunState::Executing);
        assert_eq!(run.status, ExecutionRunState::Executing);
        assert!(run.started_at.is_some());
    }

    #[test]
    fn test_execution_run_watermark_update() {
        let id = ExecutionRunId::new();
        let mut run = ExecutionRun::new(id, "Test".to_string());
        run.update_watermark(42);
        assert_eq!(run.last_applied_sequence, 42);
    }

    #[test]
    fn test_execution_run_serde_roundtrip() {
        let id = ExecutionRunId::new();
        let mut run = ExecutionRun::new(id, "Test run".to_string());
        run.constraints.push("No GPU".to_string());
        run.success_criteria.push("All tests pass".to_string());
        run.workspace_root = PathBuf::from("/tmp/m31a");
        run.mode = AutonomyMode::Autonomous;
        run.last_applied_sequence = 10;

        let json = serde_json::to_string(&run).unwrap();
        let parsed: ExecutionRun = serde_json::from_str(&json).unwrap();
        assert_eq!(run, parsed);
    }
}
