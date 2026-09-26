//! Mission aggregate model (D-01, D-02, D-03, D-04, D-15).

use crate::ids::{MissionId, TaskGraphId};
use crate::state::budget::ResourceBudget;
use crate::state::intake::{AutonomyMode, NormalizedIntake};
use crate::state::policy_context::PolicyContext;
use crate::state_machine::MissionState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Replay error during pure aggregate event application.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplayError {
    #[error("Event sequence {event_seq} is out of order for aggregate watermark {current_seq}")]
    OutOfOrderSequence { current_seq: u64, event_seq: u64 },
    #[error("Invalid state transition during replay from {from} via {event}")]
    InvalidReplayTransition { from: String, event: String },
}

/// Mission aggregate containing authoritative mission state.
///
/// Per D-01: Flat aggregate with 15+ scalar fields and bounded collections.
/// Per D-02: TaskGraph is separate; Mission stores `Option<TaskGraphId>`.
/// Per D-03 & D-04: Agents, artifacts, checkpoints, verification records are
/// separate persisted entities with typed MissionId foreign keys, not unbounded Vecs.
/// Per D-15: `last_applied_sequence` is the authoritative watermark for state reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mission {
    pub id: MissionId,
    pub objective: String,
    pub constraints: Vec<String>,
    pub requirements: Vec<String>,
    pub success_criteria: Vec<String>,
    pub workspace_root: PathBuf,
    pub mode: AutonomyMode,
    pub policy_context: PolicyContext,
    pub budget: ResourceBudget,
    pub status: MissionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub parent_mission: Option<MissionId>,
    pub task_graph_id: Option<TaskGraphId>,
    pub last_applied_sequence: u64,
}

impl Mission {
    /// Create a new mission with default configuration.
    pub fn new(id: MissionId, objective: String) -> Self {
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
            budget: ResourceBudget::default(),
            status: MissionState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_mission: None,
            task_graph_id: None,
            last_applied_sequence: 0,
        }
    }

    /// Construct a mission from validated, normalized intake.
    pub fn from_normalized(id: MissionId, normalized: NormalizedIntake) -> Self {
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
            budget: normalized.budget,
            status: MissionState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_mission: None,
            task_graph_id: None,
            last_applied_sequence: 0,
        }
    }

    /// Update status and refresh updated_at timestamp.
    pub fn update_status(&mut self, new_status: MissionState) {
        if self.started_at.is_none() && new_status != MissionState::Created {
            self.started_at = Some(Utc::now());
        }
        if new_status.is_terminal() && self.completed_at.is_none() {
            self.completed_at = Some(Utc::now());
        }
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    /// Update the state reconstruction sequence watermark.
    pub fn update_watermark(&mut self, sequence: u64) {
        self.last_applied_sequence = sequence;
        self.updated_at = Utc::now();
    }

    /// Pure, side-effect-free state transition for event replay and recovery (D-17).
    pub fn apply(
        &mut self,
        envelope: &crate::events::envelope::EventEnvelope,
    ) -> Result<(), ReplayError> {
        use crate::events::types::EventType;

        match &envelope.event_type {
            EventType::MissionStarted { .. } => {
                self.status = MissionState::Understanding;
                if self.started_at.is_none() {
                    self.started_at = Some(envelope.timestamp);
                }
            }
            EventType::MissionCompleted { .. } => {
                self.status = MissionState::Completed;
                self.completed_at = Some(envelope.timestamp);
            }
            EventType::MissionFailed { .. } => {
                self.status = MissionState::Failed;
                self.completed_at = Some(envelope.timestamp);
            }
            EventType::MissionCancelled { .. } => {
                self.status = MissionState::Cancelled;
                self.completed_at = Some(envelope.timestamp);
            }
            EventType::MissionPaused { .. } => {
                self.status = MissionState::Paused;
            }
            EventType::MissionResumed { .. } => {
                self.status = MissionState::Executing;
            }
            EventType::MissionStateChanged { to, .. } => {
                if let Ok(state) = to.parse::<MissionState>() {
                    self.status = state;
                    if state.is_terminal() {
                        self.completed_at = Some(envelope.timestamp);
                    }
                }
            }
            _ => {
                // Other events do not directly transition mission status
            }
        }

        self.last_applied_sequence = envelope.sequence;
        self.updated_at = envelope.timestamp;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;

    #[test]
    fn test_mission_construction() {
        let id = MissionId::new();
        let mission = Mission::new(id, "Test mission".to_string());
        assert_eq!(mission.id, id);
        assert_eq!(mission.objective, "Test mission");
        assert_eq!(mission.status, MissionState::Created);
        assert_eq!(mission.last_applied_sequence, 0);
        assert_eq!(mission.mode, AutonomyMode::Safe);
        assert!(mission.constraints.is_empty());
        assert!(mission.task_graph_id.is_none());
    }

    #[test]
    fn test_mission_status_update() {
        let id = MissionId::new();
        let mut mission = Mission::new(id, "Test".to_string());
        mission.update_status(MissionState::Understanding);
        assert_eq!(mission.status, MissionState::Understanding);
        assert!(mission.started_at.is_some());
    }

    #[test]
    fn test_mission_watermark_update() {
        let id = MissionId::new();
        let mut mission = Mission::new(id, "Test".to_string());
        mission.update_watermark(42);
        assert_eq!(mission.last_applied_sequence, 42);
    }

    #[test]
    fn test_mission_serde_roundtrip() {
        let id = MissionId::new();
        let mut mission = Mission::new(id, "Test mission".to_string());
        mission.constraints.push("No GPU".to_string());
        mission.success_criteria.push("All tests pass".to_string());
        mission.workspace_root = PathBuf::from("/tmp/m31a");
        mission.mode = AutonomyMode::Autonomous;
        mission.last_applied_sequence = 10;

        let json = serde_json::to_string(&mission).unwrap();
        let parsed: Mission = serde_json::from_str(&json).unwrap();
        assert_eq!(mission, parsed);
    }
}
