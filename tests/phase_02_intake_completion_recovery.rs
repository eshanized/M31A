//! Phase 02: Intake Contract, Completion Gate, and Crash Recovery Integration Tests
//!
//! Verifies external intake normalization & validation, JSON Schema derivation,
//! multi-error completion gate accumulation, side-effect-free event replay,
//! and watermark-based crash recovery.

use m31a::error::M31AError;
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::persistence::sqlite::repositories::{
    EventRepository, SqliteEventRepository, SqliteMissionRepository,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::state::completion::{CompletionContext, CompletionGate, CompletionGateError};
use m31a::state::intake::{
    AutonomyMode, IntakeValidationError, MissionIntake, generate_mission_intake_schema,
};
use m31a::state::mission::Mission;
use m31a::state::recovery::{CrashClassification, StateReconstructionEngine};
use m31a::state_machine::MissionState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_intake_valid_normalization_and_defaults() {
    // Arrange: intake with only an objective specified
    let intake = MissionIntake::new("Implement autonomous test harness");
    let default_workspace = Path::new("/var/workspace/project");

    // Act: normalize with default workspace
    let normalized = intake.normalize(default_workspace).unwrap();

    // Assert: default values applied deterministically
    assert_eq!(normalized.objective, "Implement autonomous test harness");
    assert_eq!(
        normalized.workspace_root,
        PathBuf::from("/var/workspace/project")
    );
    assert_eq!(normalized.mode, AutonomyMode::Safe);
    assert!(normalized.constraints.is_empty());
    assert!(normalized.success_criteria.is_empty());
    assert_eq!(normalized.policy_context.security_level, 1);
    assert_eq!(
        normalized.budget,
        m31a::state::budget::ResourceBudget::default()
    );
}

#[test]
fn test_intake_rejects_empty_and_whitespace_objective() {
    // Arrange: blank objectives
    let empty_intake = MissionIntake::new("");
    let whitespace_intake = MissionIntake::new("   \t \n  ");

    // Act & Assert: both must be rejected
    assert_eq!(
        empty_intake.validate().unwrap_err(),
        IntakeValidationError::EmptyObjective
    );
    assert_eq!(
        whitespace_intake.validate().unwrap_err(),
        IntakeValidationError::EmptyObjective
    );
}

#[test]
fn test_intake_rejects_path_traversal() {
    // Arrange: intakes containing parent directory traversal (..)
    let mut ws_traversal = MissionIntake::new("Valid objective");
    ws_traversal.workspace_root = Some(PathBuf::from("../../etc/shadow"));

    let mut repo_traversal = MissionIntake::new("Valid objective");
    repo_traversal.repository_path = Some(PathBuf::from("subdir/../../root"));

    let mut req_traversal = MissionIntake::new("Valid objective");
    req_traversal.requirements_file = Some(PathBuf::from("../secret.md"));

    // Act & Assert: validate blocks all path traversal attempts
    assert!(matches!(
        ws_traversal.validate(),
        Err(IntakeValidationError::PathTraversal(_))
    ));
    assert!(matches!(
        repo_traversal.validate(),
        Err(IntakeValidationError::PathTraversal(_))
    ));
    assert!(matches!(
        req_traversal.validate(),
        Err(IntakeValidationError::PathTraversal(_))
    ));
}

#[test]
fn test_intake_rejects_unknown_fields() {
    // Arrange: JSON with unmapped unexpected field
    let json = r#"{
        "objective": "Add caching layer",
        "unexpected_backdoor_field": "exploit"
    }"#;

    // Act: deserialize with serde
    let result: Result<MissionIntake, _> = serde_json::from_str(json);

    // Assert: serde(deny_unknown_fields) rejects payload
    assert!(result.is_err(), "Unknown fields must be rejected");
}

#[test]
fn test_intake_json_schema_validity() {
    // Arrange & Act: generate schemars JSON Schema
    let schema = generate_mission_intake_schema();

    // Assert: valid object schema with required objective property
    assert!(schema.is_object());
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(schema_str.contains("\"title\":\"MissionIntake\""));
    assert!(schema_str.contains("\"objective\""));
}

#[test]
fn test_completion_gate_all_requirements_satisfied() {
    // Arrange: fully satisfied completion context
    let ctx = CompletionContext::all_satisfied();

    // Act & Assert: both evaluate and can_complete succeed
    assert!(CompletionGate::can_complete(&ctx));
    assert!(CompletionGate::evaluate(&ctx).is_ok());
}

#[test]
fn test_completion_gate_accumulates_all_individual_failures() {
    // Arrange: default context where all 6 criteria are false
    let ctx = CompletionContext::default();

    // Act: evaluate completion gate
    let result = CompletionGate::evaluate(&ctx);

    // Assert: does not short-circuit; returns all 6 failure causes
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 6);
    assert!(errors.contains(&CompletionGateError::UnresolvedTasks));
    assert!(errors.contains(&CompletionGateError::UnverifiedRequirements));
    assert!(errors.contains(&CompletionGateError::ReviewNotPassed));
    assert!(errors.contains(&CompletionGateError::FatalPolicyState));
    assert!(errors.contains(&CompletionGateError::IntegrationPolicyFailed));
    assert!(errors.contains(&CompletionGateError::MissingCompletionReport));
}

#[test]
fn test_completion_gate_rejects_empty_evidence() {
    // Arrange: all satisfied except requirements verification
    let mut ctx = CompletionContext::all_satisfied();
    ctx.mandatory_requirements_verified = false;

    // Act & Assert
    assert!(!CompletionGate::can_complete(&ctx));
    let errors = CompletionGate::evaluate(&ctx).unwrap_err();
    assert_eq!(errors, vec![CompletionGateError::UnverifiedRequirements]);
}

#[test]
fn test_completion_gate_rejects_failed_review() {
    // Arrange: review failed
    let mut ctx = CompletionContext::all_satisfied();
    ctx.required_review_passed = false;

    // Act & Assert
    assert!(!CompletionGate::can_complete(&ctx));
    let errors = CompletionGate::evaluate(&ctx).unwrap_err();
    assert_eq!(errors, vec![CompletionGateError::ReviewNotPassed]);
}

#[test]
fn test_mission_apply_event_state_progression() {
    // Arrange: mission aggregate
    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Replay test".to_string());
    assert_eq!(mission.status, MissionState::Created);
    assert_eq!(mission.last_applied_sequence, 0);

    // Act: apply MissionStarted (seq 1)
    let env1 = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Replay test".to_string(),
        },
    );
    mission.apply(&env1).unwrap();
    assert_eq!(mission.status, MissionState::Understanding);
    assert_eq!(mission.last_applied_sequence, 1);
    assert!(mission.started_at.is_some());

    // Act: apply MissionPaused (seq 2)
    let env2 = EventEnvelope::new(
        2,
        Some(mission_id),
        None,
        "operator".to_string(),
        EventType::MissionPaused {
            mission_id,
            reason: "Maintenance".to_string(),
        },
    );
    mission.apply(&env2).unwrap();
    assert_eq!(mission.status, MissionState::Paused);
    assert_eq!(mission.last_applied_sequence, 2);

    // Act: apply MissionResumed (seq 3)
    let env3 = EventEnvelope::new(
        3,
        Some(mission_id),
        None,
        "operator".to_string(),
        EventType::MissionResumed { mission_id },
    );
    mission.apply(&env3).unwrap();
    assert_eq!(mission.status, MissionState::Executing);
    assert_eq!(mission.last_applied_sequence, 3);

    // Act: apply MissionCompleted (seq 4)
    let env4 = EventEnvelope::new(
        4,
        Some(mission_id),
        None,
        "verifier".to_string(),
        EventType::MissionCompleted { mission_id },
    );
    mission.apply(&env4).unwrap();
    assert_eq!(mission.status, MissionState::Completed);
    assert_eq!(mission.last_applied_sequence, 4);
    assert!(mission.completed_at.is_some());
}

#[test]
fn test_mission_apply_ignores_unrelated_events() {
    // Arrange: mission in Executing state
    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Unrelated events".to_string());
    mission.status = MissionState::Executing;

    // Act: apply task and agent events
    let task_event = EventEnvelope::new(
        5,
        Some(mission_id),
        None,
        "agent".to_string(),
        EventType::TaskCompleted {
            task_id: m31a::ids::TaskId::new(),
            mission_id,
            result: "Done".to_string(),
        },
    );
    mission.apply(&task_event).unwrap();

    // Assert: status unchanged, sequence advances
    assert_eq!(mission.status, MissionState::Executing);
    assert_eq!(mission.last_applied_sequence, 5);
}

#[tokio::test]
async fn test_recovery_classification_safe_to_resume() {
    // Arrange: db with matching sequence numbers
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("recovery_safe.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Test Safe".to_string());
    mission.last_applied_sequence = 3;
    mission_repo.insert(&mission).await.unwrap();

    for seq in 1..=3 {
        let env = EventEnvelope::new(
            seq,
            Some(mission_id),
            None,
            "system".to_string(),
            EventType::MissionStateChanged {
                mission_id,
                from: "Created".to_string(),
                to: "Executing".to_string(),
            },
        );
        event_repo.append(&env).await.unwrap();
    }

    // Act: classify
    let classification = engine.classify(mission_id).await.unwrap();

    // Assert: watermark == latest_sequence -> SafeToResume
    assert_eq!(classification, CrashClassification::SafeToResume);
}

#[tokio::test]
async fn test_recovery_classification_needs_repair_and_replays() {
    // Arrange: mission state lagging behind event log (watermark 1 vs log 3)
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("recovery_repair.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Test Repair".to_string());
    mission.last_applied_sequence = 1;
    mission.status = MissionState::Created;
    mission_repo.insert(&mission).await.unwrap();

    let env1 = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Test Repair".to_string(),
        },
    );
    let env2 = EventEnvelope::new(
        2,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionStateChanged {
            mission_id,
            from: "Understanding".to_string(),
            to: "Executing".to_string(),
        },
    );
    let env3 = EventEnvelope::new(
        3,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionPaused {
            mission_id,
            reason: "Power outage".to_string(),
        },
    );

    event_repo.append(&env1).await.unwrap();
    event_repo.append(&env2).await.unwrap();
    event_repo.append(&env3).await.unwrap();

    // Act & Assert: classify shows NeedsRepair with 2 unapplied events
    let classification = engine.classify(mission_id).await.unwrap();
    assert_eq!(
        classification,
        CrashClassification::NeedsRepair {
            unapplied_events: 2
        }
    );

    // Act: reconstruct
    let reconstructed = engine.reconstruct(mission_id).await.unwrap();

    // Assert: state caught up to sequence 3 and Paused status
    assert_eq!(reconstructed.last_applied_sequence, 3);
    assert_eq!(reconstructed.status, MissionState::Paused);

    // Assert: subsequent classification is SafeToResume
    assert_eq!(
        engine.classify(mission_id).await.unwrap(),
        CrashClassification::SafeToResume
    );
}

#[tokio::test]
async fn test_recovery_classification_fails_closed_on_future_watermark() {
    // Arrange: corrupt state where mission watermark > event log sequence
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("recovery_corrupt.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let engine = StateReconstructionEngine::new(mission_repo.clone(), event_repo.clone());

    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Corrupt Mission".to_string());
    mission.last_applied_sequence = 999;
    mission_repo.insert(&mission).await.unwrap();

    let env = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Corrupt Mission".to_string(),
        },
    );
    event_repo.append(&env).await.unwrap();

    // Act: classify
    let classification = engine.classify(mission_id).await.unwrap();
    assert!(matches!(
        classification,
        CrashClassification::CorruptState { .. }
    ));

    // Act: reconstruct fails closed
    let reconstruct_res = engine.reconstruct(mission_id).await;
    assert!(reconstruct_res.is_err());
    assert!(matches!(
        reconstruct_res.unwrap_err(),
        M31AError::ReconstructionError(_)
    ));
}
