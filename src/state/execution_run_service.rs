//! ExecutionRun orchestration service coordinating lifecycle, transitions, and persistence boundaries.

use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::ExecutionRunId;
use crate::persistence::sqlite::repositories::{
    SqliteEventRepository, SqliteExecutionRunRepository,
};
use crate::persistence::sqlite::transaction::SqliteTransactionManager;
use crate::state::completion::{CompletionContext, CompletionGate};
use crate::state::fork::{ForkInputs, ForkParent, resolve_fork_inputs};
use crate::state::execution_run::ExecutionRun;
use crate::state::intake::{AutonomyMode, MissionIntake};
use crate::state_machine::ExecutionRunState;
use crate::state_machine::execution_run::transition_execution_run;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Orchestration service coordinating ExecutionRun lifecycle and persistence boundaries.
pub struct ExecutionRunService {
    transaction_manager: Arc<SqliteTransactionManager>,
    run_repo: Arc<SqliteExecutionRunRepository>,
    event_repo: Arc<SqliteEventRepository>,
    event_bus: Arc<dyn EventBus>,
}

impl ExecutionRunService {
    /// Create a new ExecutionRunService instance.
    pub fn new(
        transaction_manager: Arc<SqliteTransactionManager>,
        run_repo: Arc<SqliteExecutionRunRepository>,
        event_repo: Arc<SqliteEventRepository>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            transaction_manager,
            run_repo,
            event_repo,
            event_bus,
        }
    }

    /// Create a new execution run from intake specifications.
    pub async fn create_run(
        &self,
        intake: MissionIntake,
        default_workspace: &Path,
    ) -> Result<ExecutionRun, M31AError> {
        let normalized = intake
            .normalize(default_workspace)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let id = ExecutionRunId::new();
        let next_seq = self.event_repo.next_sequence(None).await?;

        let mut run = ExecutionRun::from_normalized(id, normalized);
        run.last_applied_sequence = next_seq;

        let envelope = EventEnvelope::new_for_run(
            next_seq,
            Some(id),
            None,
            "system".to_string(),
            EventType::MissionStarted {
                mission_id: crate::ids::MissionId::from(*id.as_uuid()),
                objective: run.objective.clone(),
            },
        );

        self.transaction_manager
            .commit_execution_run_with_event(&run, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        Ok(run)
    }

    /// Create an execution run directly (for direct execution workflows bypassing Genesis).
    pub async fn create_direct_run(
        &self,
        objective: String,
        workspace_root: PathBuf,
        mode: AutonomyMode,
    ) -> Result<ExecutionRun, M31AError> {
        let id = ExecutionRunId::new();
        let next_seq = self.event_repo.next_sequence(None).await?;

        let mut run = ExecutionRun::new(id, objective);
        run.workspace_root = workspace_root;
        run.mode = mode;
        run.last_applied_sequence = next_seq;

        let envelope = EventEnvelope::new_for_run(
            next_seq,
            Some(id),
            None,
            "system".to_string(),
            EventType::MissionStarted {
                mission_id: crate::ids::MissionId::from(*id.as_uuid()),
                objective: run.objective.clone(),
            },
        );

        self.transaction_manager
            .commit_execution_run_with_event(&run, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        Ok(run)
    }

    /// Transition an execution run to a new state through the state machine.
    pub async fn transition_run(
        &self,
        id: ExecutionRunId,
        target_state: ExecutionRunState,
        actor: &str,
        completion_context: Option<&CompletionContext>,
    ) -> Result<ExecutionRun, M31AError> {
        let mut run = self
            .run_repo
            .get(id)
            .await?
            .ok_or_else(|| M31AError::not_found(format!("ExecutionRun {}", id)))?;

        let event = ExecutionRunState::target_state_to_event(run.status, target_state)
            .map_err(|e| M31AError::transition(e.to_string()))?;
        let validated_next_state = transition_execution_run(run.status, event)
            .map_err(|e| M31AError::transition(e.to_string()))?;

        if validated_next_state == ExecutionRunState::Completed {
            let ctx = completion_context.ok_or_else(|| {
                M31AError::validation(
                    "ExecutionRun completion requires a verified CompletionContext",
                )
            })?;
            CompletionGate::evaluate(ctx).map_err(|errs| {
                M31AError::validation(format!("Completion gate verification failed: {:?}", errs))
            })?;
        }

        let next_seq = self.event_repo.next_sequence(None).await?;
        let previous_state = run.status;
        run.update_status(validated_next_state);
        run.update_watermark(next_seq);

        let mission_id = crate::ids::MissionId::from(*id.as_uuid());
        let event_type = match validated_next_state {
            ExecutionRunState::Completed => EventType::MissionCompleted { mission_id },
            ExecutionRunState::Failed => EventType::MissionFailed {
                mission_id,
                reason: format!("Execution run marked failed by actor {}", actor),
            },
            ExecutionRunState::Cancelled => EventType::MissionCancelled {
                mission_id,
                reason: format!("Execution run cancelled by actor {}", actor),
            },
            ExecutionRunState::Paused => EventType::MissionPaused {
                mission_id,
                reason: format!("Execution run paused by actor {}", actor),
            },
            ExecutionRunState::Executing if previous_state == ExecutionRunState::Paused => {
                EventType::MissionResumed { mission_id }
            }
            _ => EventType::MissionStateChanged {
                mission_id,
                from: previous_state.to_string(),
                to: validated_next_state.to_string(),
            },
        };

        let envelope =
            EventEnvelope::new_for_run(next_seq, Some(id), None, actor.to_string(), event_type);

        self.transaction_manager
            .commit_execution_run_with_event(&run, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        Ok(run)
    }

    /// Fork an existing execution run into a new independent run.
    pub async fn fork_run(
        &self,
        parent_id: ExecutionRunId,
        overrides: Option<MissionIntake>,
        default_workspace: &Path,
    ) -> Result<ExecutionRun, M31AError> {
        let parent =
            self.run_repo.get(parent_id).await?.ok_or_else(|| {
                M31AError::not_found(format!("Parent execution run {}", parent_id))
            })?;

        let child_id = ExecutionRunId::new();
        let next_seq = self.event_repo.next_sequence(None).await?;

        let fork_inputs = resolve_fork_inputs(
            ForkParent {
                objective: &parent.objective,
                constraints: &parent.constraints,
                requirements: &parent.requirements,
                success_criteria: &parent.success_criteria,
                workspace_root: &parent.workspace_root,
                mode: parent.mode,
                policy_context: &parent.policy_context,
            },
            overrides,
            default_workspace,
        )?;

        let ForkInputs {
            objective,
            constraints,
            requirements,
            success_criteria,
            workspace_root,
            mode,
            policy_context,
        } = fork_inputs;

        let now = chrono::Utc::now();
        let child = ExecutionRun {
            id: child_id,
            objective,
            constraints,
            requirements,
            success_criteria,
            workspace_root,
            mode,
            policy_context,
            status: ExecutionRunState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_run_id: Some(parent_id),
            session_id: parent.session_id,
            task_graph_id: None,
            last_applied_sequence: next_seq,
        };

        let envelope = EventEnvelope::new_for_run(
            next_seq,
            Some(child_id),
            None,
            "system".to_string(),
            EventType::MissionStarted {
                mission_id: crate::ids::MissionId::from(*child_id.as_uuid()),
                objective: child.objective.clone(),
            },
        );

        self.transaction_manager
            .commit_execution_run_with_event(&child, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        // Verify parent run remains unmodified
        let parent_after = self
            .run_repo
            .get(parent_id)
            .await?
            .ok_or_else(|| M31AError::not_found("Parent vanished"))?;
        if parent != parent_after {
            return Err(M31AError::internal(
                "Parent execution run was mutated during fork",
            ));
        }

        Ok(child)
    }

    /// Retrieve an execution run by its ID.
    pub async fn get(&self, id: ExecutionRunId) -> Result<Option<ExecutionRun>, M31AError> {
        self.run_repo.get(id).await
    }

    /// List all execution runs.
    pub async fn list(&self) -> Result<Vec<ExecutionRun>, M31AError> {
        self.run_repo.list_all().await
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::BroadcastEventBus;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_test_service() -> (ExecutionRunService, Arc<SqliteExecutionRunRepository>) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        crate::persistence::sqlite::schema::run_migrations(&pool)
            .await
            .unwrap();

        let tx_manager = Arc::new(SqliteTransactionManager::new(pool.clone()));
        let run_repo = Arc::new(SqliteExecutionRunRepository::new(pool.clone()));
        let event_repo = Arc::new(SqliteEventRepository::new(pool));
        let event_bus = Arc::new(BroadcastEventBus::new(100));

        let service = ExecutionRunService::new(tx_manager, run_repo.clone(), event_repo, event_bus);

        (service, run_repo)
    }

    #[tokio::test]
    async fn test_create_and_transition_run() {
        let (service, _repo) = setup_test_service().await;
        let run = service
            .create_direct_run(
                "Build feature".to_string(),
                PathBuf::from("/test"),
                AutonomyMode::Safe,
            )
            .await
            .unwrap();

        assert_eq!(run.status, ExecutionRunState::Created);

        let executing = service
            .transition_run(run.id, ExecutionRunState::Executing, "test_actor", None)
            .await
            .unwrap();

        assert_eq!(executing.status, ExecutionRunState::Executing);
    }
}
