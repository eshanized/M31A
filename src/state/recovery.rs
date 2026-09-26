//! Crash recovery classification and state reconstruction engine (MSN-05, EVT-04, D-14, D-15, D-17).

use crate::error::M31AError;
use crate::ids::MissionId;
use crate::persistence::sqlite::repositories::{EventRepository, MissionRepository};
use crate::state::mission::Mission;
use std::sync::Arc;

/// Crash recovery classification determining startup action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashClassification {
    /// State is caught up and consistent with event log (watermark matches latest event sequence).
    SafeToResume,
    /// Aggregate watermark is behind event log; events need to be replayed.
    NeedsRepair { unapplied_events: u64 },
    /// Non-terminal state with interrupted tasks requiring operator attention.
    AmbiguousState { reason: String },
    /// Watermark exceeds event log sequence or state deserialization failed.
    CorruptState { error: String },
}

/// State reconstruction engine that validates watermarks and replays missing events deterministically.
pub struct StateReconstructionEngine<M, E>
where
    M: MissionRepository,
    E: EventRepository,
{
    mission_repo: Arc<M>,
    event_repo: Arc<E>,
}

impl<M, E> StateReconstructionEngine<M, E>
where
    M: MissionRepository,
    E: EventRepository,
{
    /// Create a new state reconstruction engine.
    pub fn new(mission_repo: Arc<M>, event_repo: Arc<E>) -> Self {
        Self {
            mission_repo,
            event_repo,
        }
    }

    /// Classify the recovery state of a mission on startup.
    pub async fn classify(&self, mission_id: MissionId) -> Result<CrashClassification, M31AError> {
        let mission = self
            .mission_repo
            .get(mission_id)
            .await?
            .ok_or_else(|| M31AError::not_found(format!("Mission {}", mission_id)))?;

        let latest_seq = self.event_repo.latest_sequence(Some(mission_id)).await?;

        if mission.last_applied_sequence > latest_seq {
            Ok(CrashClassification::CorruptState {
                error: format!(
                    "Mission watermark {} exceeds latest event sequence {}",
                    mission.last_applied_sequence, latest_seq
                ),
            })
        } else if mission.last_applied_sequence < latest_seq {
            Ok(CrashClassification::NeedsRepair {
                unapplied_events: latest_seq - mission.last_applied_sequence,
            })
        } else {
            Ok(CrashClassification::SafeToResume)
        }
    }

    /// Deterministically reconstruct or catch up a mission by replaying missing events.
    ///
    /// Per D-14 and D-17:
    /// - Normal restart returns immediately if caught up.
    /// - Lagging state fetches events strictly > watermark in ascending sequence order and calls `mission.apply()`.
    /// - Corrupt state (watermark > latest_seq) fails closed.
    pub async fn reconstruct(&self, mission_id: MissionId) -> Result<Mission, M31AError> {
        let mut mission = self
            .mission_repo
            .get(mission_id)
            .await?
            .ok_or_else(|| M31AError::not_found(format!("Mission {}", mission_id)))?;

        let latest_seq = self.event_repo.latest_sequence(Some(mission_id)).await?;

        if mission.last_applied_sequence > latest_seq {
            return Err(M31AError::reconstruction(format!(
                "Corrupt state: watermark {} exceeds latest event sequence {}",
                mission.last_applied_sequence, latest_seq
            )));
        }

        if mission.last_applied_sequence == latest_seq {
            return Ok(mission);
        }

        // Fetch events strictly greater than watermark in ascending sequence order
        let all_events = self.event_repo.list_by_mission(mission_id).await?;
        let missing_events: Vec<_> = all_events
            .into_iter()
            .filter(|e| e.sequence > mission.last_applied_sequence && e.sequence <= latest_seq)
            .collect();

        for event in missing_events {
            mission
                .apply(&event)
                .map_err(|e| M31AError::reconstruction(e.to_string()))?;
        }

        // Persist the caught-up mission state
        self.mission_repo.insert(&mission).await?;

        Ok(mission)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::envelope::EventEnvelope;
    use crate::events::types::EventType;
    use crate::persistence::sqlite::repositories::{
        SqliteEventRepository, SqliteMissionRepository,
    };
    use crate::persistence::sqlite::schema::initialize_database;
    use crate::state_machine::MissionState;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_recovery_safe_to_resume_when_equal() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();

        let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
        let event_repo = Arc::new(SqliteEventRepository::new(pool));
        let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

        let mission_id = MissionId::new();
        let mut mission = Mission::new(mission_id, "Test Objective".to_string());
        mission.last_applied_sequence = 1;
        mission_repo.insert(&mission).await.unwrap();

        let envelope = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Test Objective".to_string(),
            },
        );
        event_repo.append(&envelope).await.unwrap();

        let classification = engine.classify(mission_id).await.unwrap();
        assert_eq!(classification, CrashClassification::SafeToResume);

        let reconstructed = engine.reconstruct(mission_id).await.unwrap();
        assert_eq!(reconstructed.last_applied_sequence, 1);
    }

    #[tokio::test]
    async fn test_recovery_needs_repair_and_replays_events() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();

        let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
        let event_repo = Arc::new(SqliteEventRepository::new(pool));
        let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

        let mission_id = MissionId::new();
        let mut mission = Mission::new(mission_id, "Test Objective".to_string());
        mission.last_applied_sequence = 1;
        mission.status = MissionState::Created;
        mission_repo.insert(&mission).await.unwrap();

        // Event 1 matches watermark
        let env1 = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Test Objective".to_string(),
            },
        );
        // Event 2 is unapplied: MissionPaused
        let env2 = EventEnvelope::new(
            2,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionPaused {
                mission_id,
                reason: "Operator intervention".to_string(),
            },
        );
        event_repo.append(&env1).await.unwrap();
        event_repo.append(&env2).await.unwrap();

        let classification = engine.classify(mission_id).await.unwrap();
        assert_eq!(
            classification,
            CrashClassification::NeedsRepair {
                unapplied_events: 1
            }
        );

        let reconstructed = engine.reconstruct(mission_id).await.unwrap();
        assert_eq!(reconstructed.last_applied_sequence, 2);
        assert_eq!(reconstructed.status, MissionState::Paused);

        // Subsequent classification should now be SafeToResume
        let classification_after = engine.classify(mission_id).await.unwrap();
        assert_eq!(classification_after, CrashClassification::SafeToResume);
    }

    #[tokio::test]
    async fn test_recovery_fails_closed_on_corrupt_watermark() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();

        let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
        let event_repo = Arc::new(SqliteEventRepository::new(pool));
        let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

        let mission_id = MissionId::new();
        let mut mission = Mission::new(mission_id, "Test Objective".to_string());
        // Watermark is ahead of event log (10 > 1)
        mission.last_applied_sequence = 10;
        mission_repo.insert(&mission).await.unwrap();

        let env1 = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Test Objective".to_string(),
            },
        );
        event_repo.append(&env1).await.unwrap();

        let classification = engine.classify(mission_id).await.unwrap();
        assert!(matches!(
            classification,
            CrashClassification::CorruptState { .. }
        ));

        let reconstruct_result = engine.reconstruct(mission_id).await;
        assert!(reconstruct_result.is_err());
        assert!(matches!(
            reconstruct_result.unwrap_err(),
            M31AError::ReconstructionError(_)
        ));
    }
}
