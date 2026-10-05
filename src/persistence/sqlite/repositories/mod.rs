//! Repository trait definitions and concrete SQLite implementations.
//!
//! Per D-17, persistence behind explicit contracts. Per D-22, repository
//! implementations must ensure exactly one authoritative state per mission.

pub mod agent;
pub mod agent_handoff;
pub mod budget;
pub mod event;
pub mod lifecycle;
pub mod mission;
pub mod report;
pub mod session;
pub mod system_state;
pub mod task;
pub mod task_graph;
pub mod telemetry;

pub use agent::SqliteAgentRepository;
pub use agent_handoff::SqliteAgentHandoffRepository;
pub use budget::SqliteBudgetRepository;
pub use event::SqliteEventRepository;
pub use lifecycle::{
    PersistedLifecycleState, PersistedQuestionRecord, SqliteLifecycleRepository,
    ValidatedTransitionError,
};
pub use mission::SqliteMissionRepository;
pub use report::{CompletionReportMetadata, SqliteReportRepository};
pub use session::SqliteSessionRepository;
pub use system_state::{SqliteSystemStateRepository, SystemStateRepository};
pub use task::SqliteTaskRepository;
pub use task_graph::SqliteTaskGraphRepository;
pub use telemetry::SqliteTelemetryRepository;

use crate::dag::graph::TaskGraph;
use crate::error::M31AError;
use crate::events::EventEnvelope;
use crate::ids::{AgentId, MissionId, SessionId, TaskGraphId, TaskId};
use crate::state::task::{BlockingReason, TaskResult};
use crate::state::{Agent, Mission, Session, Task};
use crate::state_machine::{AgentState, MissionState, TaskState};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

/// Trait for mission persistence operations.
#[async_trait::async_trait]
pub trait MissionRepository: Send + Sync {
    /// Insert a new mission.
    async fn insert(&self, mission: &Mission) -> Result<(), M31AError>;

    /// Get a mission by ID.
    async fn get(&self, id: MissionId) -> Result<Option<Mission>, M31AError>;

    /// Update mission status.
    async fn update_status(&self, id: MissionId, status: MissionState) -> Result<(), M31AError>;

    /// List all missions (for administrative purposes).
    async fn list_all(&self) -> Result<Vec<Mission>, M31AError>;

    /// Update the state reconstruction watermark on a mission.
    async fn update_watermark(&self, id: MissionId, sequence: u64) -> Result<(), M31AError> {
        let _ = (id, sequence);
        Ok(())
    }
}

/// Trait for session persistence operations.
#[async_trait::async_trait]
pub trait SessionRepository: Send + Sync {
    /// Insert a new session.
    async fn insert(&self, session: &Session) -> Result<(), M31AError>;

    /// Get a session by ID.
    async fn get(&self, id: SessionId) -> Result<Option<Session>, M31AError>;

    /// List all sessions for a mission.
    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Session>, M31AError>;

    /// Close a session.
    async fn close(&self, id: SessionId) -> Result<(), M31AError>;
}

/// Trait for task persistence operations.
#[async_trait::async_trait]
pub trait TaskRepository: Send + Sync {
    /// Insert a new task.
    async fn insert(&self, task: &Task) -> Result<(), M31AError>;

    /// Get a task by ID.
    async fn get(&self, id: TaskId) -> Result<Option<Task>, M31AError>;

    /// Update task status.
    async fn update_status(&self, id: TaskId, status: TaskState) -> Result<(), M31AError>;

    /// List all tasks for a mission.
    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Task>, M31AError>;

    /// Count total tasks for a mission.
    async fn count_by_mission(&self, _mission_id: MissionId) -> Result<usize, M31AError> {
        Ok(0)
    }

    /// Count completed/succeeded tasks for a mission.
    async fn count_completed_by_mission(&self, _mission_id: MissionId) -> Result<usize, M31AError> {
        Ok(0)
    }

    /// Retrieve task states map for a mission.
    async fn get_task_states_by_mission(
        &self,
        _mission_id: MissionId,
    ) -> Result<BTreeMap<TaskId, TaskState>, M31AError> {
        Ok(BTreeMap::new())
    }

    /// Update max_retries for a task by mission_id and candidate_key.
    async fn update_max_retries_by_candidate_key(
        &self,
        _mission_id: MissionId,
        _candidate_key: &str,
        _max_retries: u32,
    ) -> Result<(), M31AError> {
        Ok(())
    }

    /// List candidate key summaries (candidate_key, status, retry_count) for a mission.
    async fn list_candidate_summaries_by_mission(
        &self,
        _mission_id: MissionId,
    ) -> Result<Vec<(String, String, i64)>, M31AError> {
        Ok(Vec::new())
    }

    /// Mark task succeeded with result.
    async fn mark_succeeded(&self, id: TaskId, _result: &TaskResult) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Succeeded).await
    }

    /// Mark task ready.
    async fn mark_ready(&self, id: TaskId) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Ready).await
    }

    /// Mark task for retry with updated count.
    async fn mark_retry(&self, id: TaskId, _retry_count: u32) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Ready).await
    }

    /// Mark task failed with timestamp.
    async fn mark_failed(&self, id: TaskId, _completed_at: DateTime<Utc>) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Failed).await
    }

    /// Mark task blocked with reason.
    async fn mark_blocked(&self, id: TaskId, _reason: &BlockingReason) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Blocked).await
    }

    /// Mark task cancelled with timestamp.
    async fn mark_cancelled(
        &self,
        id: TaskId,
        _completed_at: DateTime<Utc>,
    ) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Cancelled).await
    }

    /// Mark task skipped with reason.
    async fn mark_skipped(&self, id: TaskId, _reason: &str) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Skipped).await
    }

    /// Mark task running with start timestamp.
    async fn mark_running(&self, id: TaskId, _started_at: DateTime<Utc>) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Running).await
    }

    /// Mark task as needs review.
    async fn mark_needs_review(&self, id: TaskId) -> Result<(), M31AError> {
        self.update_status(id, TaskState::NeedsReview).await
    }

    /// Reset any running tasks for a mission to pending (for crash recovery).
    async fn reset_running_tasks_to_pending(
        &self,
        _mission_id: MissionId,
    ) -> Result<usize, M31AError> {
        Ok(0)
    }

    /// Reset a single task to pending status, clearing completion timestamp (for resume/invalidation).
    async fn reset_task_to_pending(&self, id: TaskId) -> Result<(), M31AError> {
        self.update_status(id, TaskState::Pending).await
    }

    /// Get max_retries for a task by ID.
    async fn get_max_retries(&self, _id: TaskId) -> Result<Option<u32>, M31AError> {
        Ok(None)
    }
}

/// Trait for TaskGraph persistence operations (D-13, DAG-01).
#[async_trait::async_trait]
pub trait TaskGraphRepository: Send + Sync {
    /// Retrieve the active task graph for a mission, fully reconstructing in-memory caches.
    async fn get_active_graph(&self, mission_id: MissionId)
    -> Result<Option<TaskGraph>, M31AError>;

    /// Retrieve a task graph by ID, fully reconstructing in-memory caches.
    async fn get_graph_by_id(&self, graph_id: TaskGraphId) -> Result<Option<TaskGraph>, M31AError>;

    /// Update status of a task within the task graph.
    async fn update_task_status(&self, task_id: TaskId, status: TaskState)
    -> Result<(), M31AError>;

    /// Update execution status and result JSON of a task within the task graph.
    async fn update_task_result(
        &self,
        task_id: TaskId,
        status: &str,
        result_json: Option<&str>,
    ) -> Result<(), M31AError>;
}

/// Trait for agent persistence operations.
#[async_trait::async_trait]
pub trait AgentRepository: Send + Sync {
    /// Insert a new agent.
    async fn insert(&self, agent: &Agent) -> Result<(), M31AError>;

    /// Get an agent by ID.
    async fn get(&self, id: AgentId) -> Result<Option<Agent>, M31AError>;

    /// Update agent status.
    async fn update_status(&self, id: AgentId, status: AgentState) -> Result<(), M31AError>;

    /// List all agents for a mission.
    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Agent>, M31AError>;
}

/// Trait for event persistence operations.
#[async_trait::async_trait]
pub trait EventRepository: Send + Sync {
    /// Append an event envelope to the event log.
    async fn append(&self, envelope: &EventEnvelope) -> Result<(), M31AError>;

    /// List events by mission ID, ordered by sequence.
    async fn list_by_mission(&self, mission_id: MissionId)
    -> Result<Vec<EventEnvelope>, M31AError>;

    /// Get the latest sequence number for a mission (or globally if None).
    async fn latest_sequence(&self, mission_id: Option<MissionId>) -> Result<u64, M31AError>;
}

// Compile-time test: verify traits can be mocked
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::M31AError;
    use crate::events::EventEnvelope;
    use crate::ids::{AgentId, MissionId, SessionId, TaskId};
    use crate::state::{Agent, Mission, Session, Task};
    use crate::state_machine::{AgentState, MissionState, TaskState};
    use async_trait::async_trait;
    use std::sync::Arc;

    // Mock implementations for compile-time verification
    struct MockMissionRepository;
    struct MockSessionRepository;
    struct MockTaskRepository;
    struct MockAgentRepository;
    struct MockEventRepository;

    #[async_trait]
    impl MissionRepository for MockMissionRepository {
        async fn insert(&self, _: &Mission) -> Result<(), M31AError> {
            Ok(())
        }
        async fn get(&self, _: MissionId) -> Result<Option<Mission>, M31AError> {
            Ok(None)
        }
        async fn update_status(&self, _: MissionId, _: MissionState) -> Result<(), M31AError> {
            Ok(())
        }
        async fn list_all(&self) -> Result<Vec<Mission>, M31AError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl SessionRepository for MockSessionRepository {
        async fn insert(&self, _: &Session) -> Result<(), M31AError> {
            Ok(())
        }
        async fn get(&self, _: SessionId) -> Result<Option<Session>, M31AError> {
            Ok(None)
        }
        async fn list_by_mission(&self, _: MissionId) -> Result<Vec<Session>, M31AError> {
            Ok(vec![])
        }
        async fn close(&self, _: SessionId) -> Result<(), M31AError> {
            Ok(())
        }
    }

    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn insert(&self, _: &Task) -> Result<(), M31AError> {
            Ok(())
        }
        async fn get(&self, _: TaskId) -> Result<Option<Task>, M31AError> {
            Ok(None)
        }
        async fn update_status(&self, _: TaskId, _: TaskState) -> Result<(), M31AError> {
            Ok(())
        }
        async fn list_by_mission(&self, _: MissionId) -> Result<Vec<Task>, M31AError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl AgentRepository for MockAgentRepository {
        async fn insert(&self, _: &Agent) -> Result<(), M31AError> {
            Ok(())
        }
        async fn get(&self, _: AgentId) -> Result<Option<Agent>, M31AError> {
            Ok(None)
        }
        async fn update_status(&self, _: AgentId, _: AgentState) -> Result<(), M31AError> {
            Ok(())
        }
        async fn list_by_mission(&self, _: MissionId) -> Result<Vec<Agent>, M31AError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl EventRepository for MockEventRepository {
        async fn append(&self, _: &EventEnvelope) -> Result<(), M31AError> {
            Ok(())
        }
        async fn list_by_mission(&self, _: MissionId) -> Result<Vec<EventEnvelope>, M31AError> {
            Ok(vec![])
        }
        async fn latest_sequence(&self, _: Option<MissionId>) -> Result<u64, M31AError> {
            Ok(0)
        }
    }

    #[test]
    fn test_traits_compile_and_can_be_mocked() {
        // If this compiles, the traits are correctly defined and can be implemented
        let _mission_repo: Arc<dyn MissionRepository> = Arc::new(MockMissionRepository);
        let _session_repo: Arc<dyn SessionRepository> = Arc::new(MockSessionRepository);
        let _task_repo: Arc<dyn TaskRepository> = Arc::new(MockTaskRepository);
        let _agent_repo: Arc<dyn AgentRepository> = Arc::new(MockAgentRepository);
        let _event_repo: Arc<dyn EventRepository> = Arc::new(MockEventRepository);
    }
}
