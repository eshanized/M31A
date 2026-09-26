//! Mission orchestration service handling intake, transitions, and immutable forks (MSN-02, MSN-03, MSN-06, D-07).

use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::MissionId;
use crate::persistence::sqlite::repositories::{SqliteEventRepository, SqliteMissionRepository};
use crate::persistence::sqlite::transaction::SqliteTransactionManager;
use crate::state::completion::{CompletionContext, CompletionGate};
use crate::state::intake::MissionIntake;
use crate::state::mission::Mission;
use crate::state_machine::MissionState;
use crate::state_machine::mission::{MissionEvent, transition_mission};
use std::path::Path;
use std::sync::Arc;

/// Mission orchestration service coordinating domain workflows and persistence boundaries.
pub struct MissionService {
    transaction_manager: Arc<SqliteTransactionManager>,
    mission_repo: Arc<SqliteMissionRepository>,
    event_repo: Arc<SqliteEventRepository>,
    event_bus: Arc<dyn EventBus>,
}

impl MissionService {
    /// Create a new MissionService instance.
    pub fn new(
        transaction_manager: Arc<SqliteTransactionManager>,
        mission_repo: Arc<SqliteMissionRepository>,
        event_repo: Arc<SqliteEventRepository>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            transaction_manager,
            mission_repo,
            event_repo,
            event_bus,
        }
    }

    /// Create a new mission from intake specifications (D-07).
    ///
    /// Validates intake, normalizes paths, acquires a sequence number, and atomically persists
    /// the mission aggregate and initial lifecycle event before notifying the event bus.
    pub async fn create_mission(
        &self,
        intake: MissionIntake,
        default_workspace: &Path,
    ) -> Result<Mission, M31AError> {
        let normalized = intake
            .normalize(default_workspace)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let id = MissionId::new();
        let next_seq = self.event_repo.next_sequence(None).await?;

        let mut mission = Mission::from_normalized(id, normalized);
        mission.last_applied_sequence = next_seq;

        let envelope = EventEnvelope::new(
            next_seq,
            Some(id),
            None,
            "system".to_string(),
            EventType::MissionStarted {
                mission_id: id,
                objective: mission.objective.clone(),
            },
        );

        self.transaction_manager
            .commit_mission_with_event(&mission, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        Ok(mission)
    }

    /// Transition a mission to a new state through the state machine (MSN-02).
    ///
    /// Validates the transition, enforces completion criteria if completing (MSN-03),
    /// persists updated state and lifecycle event atomically, and broadcasts to event bus.
    pub async fn transition_mission(
        &self,
        id: MissionId,
        target_state: MissionState,
        actor: &str,
        completion_context: Option<&CompletionContext>,
    ) -> Result<Mission, M31AError> {
        let mut mission = self
            .mission_repo
            .get(id)
            .await?
            .ok_or_else(|| M31AError::not_found(format!("Mission {}", id)))?;

        let event = target_state_to_event(mission.status, target_state)?;
        let validated_next_state = transition_mission(mission.status, event)
            .map_err(|e| M31AError::transition(e.to_string()))?;

        if validated_next_state == MissionState::Completed {
            let ctx = completion_context.ok_or_else(|| {
                M31AError::validation("Mission completion requires a verified CompletionContext")
            })?;
            CompletionGate::evaluate(ctx).map_err(|errs| {
                M31AError::validation(format!("Completion gate verification failed: {:?}", errs))
            })?;
        }

        let next_seq = self.event_repo.next_sequence(Some(id)).await?;
        let previous_state = mission.status;
        mission.update_status(validated_next_state);
        mission.update_watermark(next_seq);

        let event_type = match validated_next_state {
            MissionState::Completed => EventType::MissionCompleted { mission_id: id },
            MissionState::Failed => EventType::MissionFailed {
                mission_id: id,
                reason: format!("Mission marked failed by actor {}", actor),
            },
            MissionState::Cancelled => EventType::MissionCancelled {
                mission_id: id,
                reason: format!("Mission cancelled by actor {}", actor),
            },
            MissionState::Paused => EventType::MissionPaused {
                mission_id: id,
                reason: format!("Mission paused by actor {}", actor),
            },
            MissionState::Executing if previous_state == MissionState::Paused => {
                EventType::MissionResumed { mission_id: id }
            }
            _ => EventType::MissionStateChanged {
                mission_id: id,
                from: previous_state.to_string(),
                to: validated_next_state.to_string(),
            },
        };

        let envelope = EventEnvelope::new(next_seq, Some(id), None, actor.to_string(), event_type);

        self.transaction_manager
            .commit_mission_with_event(&mission, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        Ok(mission)
    }

    /// Fork an existing mission into a new independent mission (MSN-06).
    ///
    /// Clones specification and context while assigning a new MissionId and referencing
    /// parent_mission_id. The parent mission remains completely immutable.
    pub async fn fork_mission(
        &self,
        parent_id: MissionId,
        overrides: Option<MissionIntake>,
        default_workspace: &Path,
    ) -> Result<Mission, M31AError> {
        let parent = self
            .mission_repo
            .get(parent_id)
            .await?
            .ok_or_else(|| M31AError::not_found(format!("Parent mission {}", parent_id)))?;

        let child_id = MissionId::new();
        let next_seq = self.event_repo.next_sequence(None).await?;

        let (
            objective,
            constraints,
            requirements,
            success_criteria,
            workspace_root,
            mode,
            policy_context,
            budget,
        ) = if let Some(intake) = overrides {
            let norm = intake
                .normalize(default_workspace)
                .map_err(|e| M31AError::validation(e.to_string()))?;
            (
                norm.objective,
                if !norm.constraints.is_empty() {
                    norm.constraints
                } else {
                    parent.constraints.clone()
                },
                if !norm.requirements.is_empty() {
                    norm.requirements
                } else {
                    parent.requirements.clone()
                },
                if !norm.success_criteria.is_empty() {
                    norm.success_criteria
                } else {
                    parent.success_criteria.clone()
                },
                norm.workspace_root,
                norm.mode,
                norm.policy_context,
                norm.budget,
            )
        } else {
            (
                parent.objective.clone(),
                parent.constraints.clone(),
                parent.requirements.clone(),
                parent.success_criteria.clone(),
                parent.workspace_root.clone(),
                parent.mode,
                parent.policy_context.clone(),
                parent.budget.clone(),
            )
        };

        let now = chrono::Utc::now();
        let child = Mission {
            id: child_id,
            objective,
            constraints,
            requirements,
            success_criteria,
            workspace_root,
            mode,
            policy_context,
            budget,
            status: MissionState::Created,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_mission: Some(parent_id),
            task_graph_id: None,
            last_applied_sequence: next_seq,
        };

        let envelope = EventEnvelope::new(
            next_seq,
            Some(child_id),
            None,
            "system".to_string(),
            EventType::MissionStarted {
                mission_id: child_id,
                objective: child.objective.clone(),
            },
        );

        self.transaction_manager
            .commit_mission_with_event(&child, &envelope)
            .await?;

        let _ = self.event_bus.publish(envelope).await;

        // Verify parent mission remains unmodified (D-06, MSN-06)
        let parent_after = self
            .mission_repo
            .get(parent_id)
            .await?
            .ok_or_else(|| M31AError::not_found("Parent vanished"))?;
        if parent != parent_after {
            return Err(M31AError::internal(
                "Parent mission was mutated during fork",
            ));
        }

        Ok(child)
    }
}

fn target_state_to_event(
    current: MissionState,
    target: MissionState,
) -> Result<MissionEvent, M31AError> {
    match target {
        MissionState::Understanding => Ok(MissionEvent::Start),
        MissionState::Researching => Ok(MissionEvent::UnderstandComplete),
        MissionState::Planning => Ok(MissionEvent::ResearchComplete),
        MissionState::Scheduled => Ok(MissionEvent::PlanComplete),
        MissionState::Executing => {
            if current == MissionState::Paused {
                Ok(MissionEvent::Resume)
            } else {
                Ok(MissionEvent::Execute)
            }
        }
        MissionState::Verifying => Ok(MissionEvent::Verify),
        MissionState::Reviewing => Ok(MissionEvent::Review),
        MissionState::Integrating => Ok(MissionEvent::Integrate),
        MissionState::Shipping => Ok(MissionEvent::Ship),
        MissionState::Completed => Ok(MissionEvent::Complete),
        MissionState::Paused => Ok(MissionEvent::Pause),
        MissionState::Blocked => Ok(MissionEvent::Block),
        MissionState::Failed => Ok(MissionEvent::Fail),
        MissionState::Cancelled => Ok(MissionEvent::Cancel),
        _ => Err(M31AError::transition(format!(
            "Invalid target state transition from {:?} to {:?}",
            current, target
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::BroadcastEventBus;
    use crate::persistence::sqlite::schema::initialize_database;
    use std::path::PathBuf;
    use tempfile::tempdir;

    async fn setup_test_service() -> (
        MissionService,
        Arc<SqliteMissionRepository>,
        tempfile::TempDir,
    ) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();

        let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
        let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
        let event_repo = Arc::new(SqliteEventRepository::new(pool));
        let event_bus = Arc::new(BroadcastEventBus::default());

        let service = MissionService::new(tx_mgr, mission_repo.clone(), event_repo, event_bus);
        (service, mission_repo, dir)
    }

    #[tokio::test]
    async fn test_create_mission_happy_path() {
        let (service, repo, _dir) = setup_test_service().await;
        let mut intake = MissionIntake::new("Build something great");
        intake.constraints = Some(vec!["no rustc warnings".to_string()]);

        let mission = service
            .create_mission(intake, &PathBuf::from("/workspace"))
            .await
            .unwrap();

        assert_eq!(mission.objective, "Build something great");
        assert_eq!(mission.status, MissionState::Created);
        assert_eq!(mission.constraints, vec!["no rustc warnings"]);

        let stored = repo.get(mission.id).await.unwrap().expect("Exists");
        assert_eq!(stored.id, mission.id);
        assert_eq!(stored.last_applied_sequence, 1);
    }

    #[tokio::test]
    async fn test_transition_mission_with_completion_gate() {
        let (service, _repo, _dir) = setup_test_service().await;
        let intake = MissionIntake::new("Mission to complete");
        let mission = service
            .create_mission(intake, &PathBuf::from("/workspace"))
            .await
            .unwrap();

        // Forward progression: Created -> Understanding -> Researching -> Planning -> Scheduled -> Executing -> Verifying -> Reviewing -> Integrating -> Shipping -> Completed
        let m = service
            .transition_mission(mission.id, MissionState::Understanding, "user", None)
            .await
            .unwrap();
        assert_eq!(m.status, MissionState::Understanding);

        let _m = service
            .transition_mission(mission.id, MissionState::Researching, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Planning, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Scheduled, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Executing, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Verifying, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Reviewing, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Integrating, "user", None)
            .await
            .unwrap();
        let _m = service
            .transition_mission(mission.id, MissionState::Shipping, "user", None)
            .await
            .unwrap();

        // Attempting to complete without context must fail
        let fail_no_ctx = service
            .transition_mission(mission.id, MissionState::Completed, "user", None)
            .await;
        assert!(fail_no_ctx.is_err());

        // Attempting to complete with unsatisfied context must fail
        let unsatisfied = CompletionContext::default();
        let fail_unsatisfied = service
            .transition_mission(
                mission.id,
                MissionState::Completed,
                "user",
                Some(&unsatisfied),
            )
            .await;
        assert!(fail_unsatisfied.is_err());

        // Complete with all criteria satisfied succeeds
        let satisfied = CompletionContext::all_satisfied();
        let completed = service
            .transition_mission(
                mission.id,
                MissionState::Completed,
                "user",
                Some(&satisfied),
            )
            .await
            .unwrap();
        assert_eq!(completed.status, MissionState::Completed);
    }

    #[tokio::test]
    async fn test_fork_mission_immutability() {
        let (service, repo, _dir) = setup_test_service().await;
        let intake = MissionIntake::new("Original Mission");
        let parent = service
            .create_mission(intake, &PathBuf::from("/workspace"))
            .await
            .unwrap();

        let child = service
            .fork_mission(parent.id, None, &PathBuf::from("/workspace"))
            .await
            .unwrap();

        assert_ne!(child.id, parent.id);
        assert_eq!(child.parent_mission, Some(parent.id));
        assert_eq!(child.objective, parent.objective);
        assert_eq!(child.status, MissionState::Created);

        // Verify parent in database was not mutated
        let parent_in_db = repo.get(parent.id).await.unwrap().unwrap();
        assert_eq!(parent_in_db, parent);
    }
}
