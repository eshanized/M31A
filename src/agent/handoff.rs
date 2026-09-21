//! Dual-path agent handoff protocol and runtime handoff arbitration (AGT-06, D-13, D-14).
//!
//! Enforces bounded mid-task delegation (MAX_DELEGATION_DEPTH = 2), task-boundary transitions,
//! durable SQLite persistence, and post-commit EventBus emission.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{AgentId, ArtifactId, HandoffId, MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

/// Hard ceiling on recursive mid-task delegation depth (D-13).
pub const MAX_DELEGATION_DEPTH: u32 = 2;

/// Default maximum number of handoffs permitted for a single task instance.
pub const DEFAULT_MAX_HANDOFFS_PER_TASK: u32 = 5;

/// First-class durable domain record representing an AGT-06 agent handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffRecord {
    pub id: HandoffId,
    pub mission_id: MissionId,
    pub source_task_id: TaskId,
    pub source_agent_id: AgentId,
    pub source_role: AgentRole,
    pub target_task_id: Option<TaskId>,
    pub target_role: AgentRole,
    pub target_agent_id: Option<AgentId>,
    pub reason: String,
    pub required_inputs: Vec<String>,
    pub artifacts: Vec<ArtifactId>,
    pub unresolved_questions: Vec<String>,
    pub task_state: String,
    pub created_at: DateTime<Utc>,
}

/// Unverified model proposal for work delegation or role handoff (The model proposes. The runtime decides.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffProposal {
    pub source_task_id: TaskId,
    pub source_agent_id: AgentId,
    pub source_role: AgentRole,
    pub target_role: AgentRole,
    pub reason: String,
    pub required_inputs: Vec<String>,
    pub artifacts: Vec<ArtifactId>,
    pub unresolved_questions: Vec<String>,
    pub current_depth: u32,
}

/// Errors raised during handoff validation or persistence.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum HandoffError {
    #[error("maximum delegation recursion depth exceeded: depth={0}, max=2")]
    MaxDelegationDepthExceeded(u32),
    #[error("maximum handoff count per task exceeded: count={0}, max={1}")]
    MaxHandoffsPerTaskExceeded(u32, u32),
    #[error("database error: {0}")]
    DatabaseError(String),
}

impl From<sqlx::Error> for HandoffError {
    fn from(err: sqlx::Error) -> Self {
        HandoffError::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for HandoffError {
    fn from(err: serde_json::Error) -> Self {
        HandoffError::DatabaseError(err.to_string())
    }
}

/// Abstract repository for durable AGT-06 handoff storage.
#[async_trait]
pub trait AgentHandoffRepository: Send + Sync {
    async fn insert_handoff(&self, record: &HandoffRecord) -> Result<(), HandoffError>;
    async fn find_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<HandoffRecord>, HandoffError>;
    async fn find_by_task(&self, task_id: TaskId) -> Result<Vec<HandoffRecord>, HandoffError>;
}

/// Runtime arbiter governing agent handoffs, recursion limits, and lifecycle event emission (D-13, D-14).
pub struct AgentHandoffArbiter {
    pub max_delegation_depth: u32,
    pub max_handoffs_per_task: u32,
    pub event_bus: Arc<dyn EventBus>,
}

impl AgentHandoffArbiter {
    /// Create a new arbiter with default limits.
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            max_delegation_depth: MAX_DELEGATION_DEPTH,
            max_handoffs_per_task: DEFAULT_MAX_HANDOFFS_PER_TASK,
            event_bus,
        }
    }

    /// Evaluate a handoff proposal, enforce recursion and count bounds, persist record, and emit domain event (D-13, D-14, D-15).
    pub async fn evaluate_and_record_handoff<R: AgentHandoffRepository>(
        &self,
        repo: &R,
        mission_id: MissionId,
        proposal: HandoffProposal,
    ) -> Result<HandoffRecord, HandoffError> {
        // 1. Enforce maximum delegation recursion depth (T-06-09)
        if proposal.current_depth >= self.max_delegation_depth {
            return Err(HandoffError::MaxDelegationDepthExceeded(
                proposal.current_depth,
            ));
        }

        // 2. Enforce per-task fanout cap
        let existing = repo.find_by_task(proposal.source_task_id).await?;
        if existing.len() as u32 >= self.max_handoffs_per_task {
            return Err(HandoffError::MaxHandoffsPerTaskExceeded(
                existing.len() as u32,
                self.max_handoffs_per_task,
            ));
        }

        // 3. Construct durable HandoffRecord
        let record = HandoffRecord {
            id: HandoffId::new(),
            mission_id,
            source_task_id: proposal.source_task_id,
            source_agent_id: proposal.source_agent_id,
            source_role: proposal.source_role,
            target_task_id: None,
            target_role: proposal.target_role,
            target_agent_id: None,
            reason: proposal.reason.clone(),
            required_inputs: proposal.required_inputs,
            artifacts: proposal.artifacts,
            unresolved_questions: proposal.unresolved_questions,
            task_state: "delegated".to_string(),
            created_at: Utc::now(),
        };

        // 4. Durably persist in database
        repo.insert_handoff(&record).await?;

        // 5. Emit domain event on EventBus post-commit (D-14, D-15)
        let event = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            format!("agent:{}", record.source_agent_id),
            EventType::AgentHandoffRecorded {
                handoff_id: record.id,
                mission_id,
                source_agent_id: record.source_agent_id,
                target_role: record.target_role.to_string(),
                reason: record.reason.clone(),
            },
        );
        let _ = self.event_bus.publish(event).await;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::{BroadcastEventBus, EventFilter};
    use futures::StreamExt;
    use std::sync::Mutex;

    struct InMemoryHandoffRepo {
        records: Mutex<Vec<HandoffRecord>>,
    }

    #[async_trait]
    impl AgentHandoffRepository for InMemoryHandoffRepo {
        async fn insert_handoff(&self, record: &HandoffRecord) -> Result<(), HandoffError> {
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }

        async fn find_by_mission(
            &self,
            mission_id: MissionId,
        ) -> Result<Vec<HandoffRecord>, HandoffError> {
            let list = self
                .records
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.mission_id == mission_id)
                .cloned()
                .collect();
            Ok(list)
        }

        async fn find_by_task(&self, task_id: TaskId) -> Result<Vec<HandoffRecord>, HandoffError> {
            let list = self
                .records
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.source_task_id == task_id)
                .cloned()
                .collect();
            Ok(list)
        }
    }

    #[tokio::test]
    async fn test_arbiter_records_handoff_and_emits_event() {
        let bus = Arc::new(BroadcastEventBus::new(100));
        let mut rx = bus.subscribe(EventFilter::all()).await;
        let arbiter = AgentHandoffArbiter::new(bus);
        let repo = InMemoryHandoffRepo {
            records: Mutex::new(Vec::new()),
        };

        let mission_id = MissionId::new();
        let proposal = HandoffProposal {
            source_task_id: TaskId::new(),
            source_agent_id: AgentId::new(),
            source_role: AgentRole::implementer(),
            target_role: AgentRole::reviewer(),
            reason: "implementation complete, ready for code review".to_string(),
            required_inputs: vec!["git diff".to_string()],
            artifacts: vec![],
            unresolved_questions: vec![],
            current_depth: 0,
        };

        let record = arbiter
            .evaluate_and_record_handoff(&repo, mission_id, proposal)
            .await
            .unwrap();

        assert_eq!(record.source_role, AgentRole::implementer());
        assert_eq!(record.target_role, AgentRole::reviewer());

        // Check EventBus emission
        let event = rx.next().await.unwrap().unwrap();
        match event.event_type {
            EventType::AgentHandoffRecorded {
                handoff_id,
                target_role,
                reason,
                ..
            } => {
                assert_eq!(handoff_id, record.id);
                assert_eq!(target_role, "reviewer");
                assert_eq!(reason, "implementation complete, ready for code review");
            }
            other => panic!("expected AgentHandoffRecorded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_arbiter_rejects_excessive_delegation_depth() {
        let bus = Arc::new(BroadcastEventBus::new(100));
        let arbiter = AgentHandoffArbiter::new(bus);
        let repo = InMemoryHandoffRepo {
            records: Mutex::new(Vec::new()),
        };

        let proposal = HandoffProposal {
            source_task_id: TaskId::new(),
            source_agent_id: AgentId::new(),
            source_role: AgentRole::implementer(),
            target_role: AgentRole::reviewer(),
            reason: "deep recursion attempt".to_string(),
            required_inputs: vec![],
            artifacts: vec![],
            unresolved_questions: vec![],
            current_depth: 2, // At or exceeding MAX_DELEGATION_DEPTH (2)
        };

        let err = arbiter
            .evaluate_and_record_handoff(&repo, MissionId::new(), proposal)
            .await
            .unwrap_err();

        assert_eq!(err, HandoffError::MaxDelegationDepthExceeded(2));
    }
}
