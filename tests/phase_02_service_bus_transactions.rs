//! Phase 02: EventBus, Transactions, and MissionService Integration Tests
//!
//! Verifies live broadcast pub/sub, multi-criteria event filtering, ACID transaction
//! atomicity & rollbacks, and mission service orchestration (creation, gates, forking).

use futures::StreamExt;
use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use m31a::events::envelope::{EventCategory, EventEnvelope};
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::persistence::sqlite::repositories::{SqliteEventRepository, SqliteMissionRepository};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::persistence::sqlite::transaction::SqliteTransactionManager;
use m31a::state::completion::CompletionContext;
use m31a::state::intake::{AutonomyMode, MissionIntake};
use m31a::state::mission::Mission;
use m31a::state::mission_service::MissionService;
use m31a::state_machine::MissionState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_event_bus_publish_with_zero_subscribers() {
    // Arrange: event bus with no active subscribers
    let bus = BroadcastEventBus::new(32);
    let envelope = EventEnvelope::new(
        1,
        Some(MissionId::new()),
        None,
        "worker".to_string(),
        EventType::MissionCompleted {
            mission_id: MissionId::new(),
        },
    );

    // Act: publish event to empty bus
    let result = bus.publish(envelope).await;

    // Assert: succeeds silently per D-10
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_event_bus_multi_subscriber_broadcast() {
    // Arrange: event bus and 3 independent subscribers
    let bus = BroadcastEventBus::new(32);
    let mut sub1 = bus.subscribe(EventFilter::all()).await;
    let mut sub2 = bus.subscribe(EventFilter::all()).await;
    let mut sub3 = bus.subscribe(EventFilter::all()).await;

    let mission_id = MissionId::new();
    let envelope = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "coordinator".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Multi-subscriber broadcast".to_string(),
        },
    );

    // Act: publish one event
    bus.publish(envelope.clone()).await.unwrap();

    // Assert: all 3 subscribers receive the event
    let msg1 = sub1.next().await.unwrap().unwrap();
    let msg2 = sub2.next().await.unwrap().unwrap();
    let msg3 = sub3.next().await.unwrap().unwrap();

    assert_eq!(msg1.id, envelope.id);
    assert_eq!(msg2.id, envelope.id);
    assert_eq!(msg3.id, envelope.id);
}

#[tokio::test]
async fn test_event_bus_filter_by_mission_id() {
    // Arrange: bus with mission-filtered subscriber
    let bus = BroadcastEventBus::new(32);
    let target_mission = MissionId::new();
    let other_mission = MissionId::new();

    let filter = EventFilter::all().with_mission(target_mission);
    let mut stream = bus.subscribe(filter).await;

    let other_event = EventEnvelope::new(
        1,
        Some(other_mission),
        None,
        "actor".to_string(),
        EventType::MissionStarted {
            mission_id: other_mission,
            objective: "Other".to_string(),
        },
    );
    let target_event = EventEnvelope::new(
        2,
        Some(target_mission),
        None,
        "actor".to_string(),
        EventType::MissionStarted {
            mission_id: target_mission,
            objective: "Target".to_string(),
        },
    );

    // Act: publish both events
    bus.publish(other_event).await.unwrap();
    bus.publish(target_event.clone()).await.unwrap();

    // Assert: stream yields ONLY the target mission event
    let received = stream.next().await.unwrap().unwrap();
    assert_eq!(received.mission_id, Some(target_mission));
    assert_eq!(received.id, target_event.id);
}

#[tokio::test]
async fn test_event_bus_filter_by_category() {
    // Arrange: subscriber filtering exclusively for Diagnostic events
    let bus = BroadcastEventBus::new(32);
    let filter = EventFilter::all().with_category(EventCategory::Diagnostic);
    let mut stream = bus.subscribe(filter).await;

    let durable_event = EventEnvelope::new(
        1,
        None,
        None,
        "actor".to_string(),
        EventType::PlanCreated {
            plan_id: "p1".to_string(),
            mission_id: MissionId::new(),
            summary: "durable".to_string(),
        },
    )
    .with_category(EventCategory::Durable);

    let diagnostic_event = EventEnvelope::new(
        2,
        None,
        None,
        "actor".to_string(),
        EventType::ContextCompiled {
            context_id: "c1".to_string(),
            mission_id: MissionId::new(),
            token_count: 500,
        },
    )
    .with_category(EventCategory::Diagnostic);

    // Act
    bus.publish(durable_event).await.unwrap();
    bus.publish(diagnostic_event.clone()).await.unwrap();

    // Assert: only diagnostic event received
    let received = stream.next().await.unwrap().unwrap();
    assert_eq!(received.category, EventCategory::Diagnostic);
    assert_eq!(received.id, diagnostic_event.id);
}

#[tokio::test]
async fn test_event_bus_filter_by_sequence_bounds() {
    // Arrange: subscriber filtering sequence [10, 20]
    let bus = BroadcastEventBus::new(32);
    let filter = EventFilter::all().from_seq(10).to_seq(20);
    let mut stream = bus.subscribe(filter).await;

    let env_low = EventEnvelope::new(
        5,
        None,
        None,
        "actor".to_string(),
        EventType::MissionCompleted {
            mission_id: MissionId::new(),
        },
    );
    let env_in_range = EventEnvelope::new(
        15,
        None,
        None,
        "actor".to_string(),
        EventType::MissionCompleted {
            mission_id: MissionId::new(),
        },
    );
    let env_high = EventEnvelope::new(
        25,
        None,
        None,
        "actor".to_string(),
        EventType::MissionCompleted {
            mission_id: MissionId::new(),
        },
    );

    // Act
    bus.publish(env_low).await.unwrap();
    bus.publish(env_in_range.clone()).await.unwrap();
    bus.publish(env_high).await.unwrap();

    // Assert: only sequence 15 received
    let received = stream.next().await.unwrap().unwrap();
    assert_eq!(received.sequence, 15);
    assert_eq!(received.id, env_in_range.id);
}

#[tokio::test]
async fn test_transaction_commit_mission_with_event_atomic() {
    // Arrange: transaction manager and pool
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("atomic_tx.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let tx_mgr = SqliteTransactionManager::new(pool.clone());

    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Atomic mission".to_string());
    mission.last_applied_sequence = 1;

    let envelope = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Atomic mission".to_string(),
        },
    );

    // Act: commit atomically
    let result = tx_mgr.commit_mission_with_event(&mission, &envelope).await;
    assert!(result.is_ok());

    // Assert: mission exists in DB
    let mission_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mission_count.0, 1);

    // Assert: event exists in DB
    let event_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_log WHERE id = ?")
        .bind(envelope.id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(event_count.0, 1);
}

#[tokio::test]
async fn test_transaction_rollback_on_failure() {
    // Arrange: initial valid mission & event
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("rollback_tx.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let tx_mgr = SqliteTransactionManager::new(pool.clone());

    let mission1_id = MissionId::new();
    let mission1 = Mission::new(mission1_id, "Mission 1".to_string());
    let env1 = EventEnvelope::new(
        1,
        Some(mission1_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id: mission1_id,
            objective: "Mission 1".to_string(),
        },
    );
    tx_mgr
        .commit_mission_with_event(&mission1, &env1)
        .await
        .unwrap();

    // Act: attempt to commit mission2 with duplicate envelope.id (causing PK violation on event_log)
    let mission2_id = MissionId::new();
    let mission2 = Mission::new(mission2_id, "Mission 2 to rollback".to_string());
    let mut dup_envelope = EventEnvelope::new(
        2,
        Some(mission2_id),
        None,
        "system".to_string(),
        EventType::MissionStarted {
            mission_id: mission2_id,
            objective: "Mission 2 to rollback".to_string(),
        },
    );
    dup_envelope.id = env1.id; // Colliding primary key

    let result = tx_mgr
        .commit_mission_with_event(&mission2, &dup_envelope)
        .await;

    // Assert: transaction fails
    assert!(result.is_err());

    // Assert: mission2 was NOT saved to the database
    let mission2_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mission2_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mission2_count.0, 0, "Mission 2 must be rolled back");
}

#[tokio::test]
async fn test_mission_service_create_mission_happy_path() {
    // Arrange: test service setup
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("svc_create.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let event_bus = Arc::new(BroadcastEventBus::default());

    let service = MissionService::new(tx_mgr, mission_repo.clone(), event_repo, event_bus);

    let mut intake = MissionIntake::new("Refactor authentication module");
    intake.mode = Some(AutonomyMode::Autonomous);

    // Act: create mission
    let mission = service
        .create_mission(intake, Path::new("/workspace"))
        .await
        .unwrap();

    // Assert: attributes set
    assert_eq!(mission.objective, "Refactor authentication module");
    assert_eq!(mission.status, MissionState::Created);
    assert_eq!(mission.mode, AutonomyMode::Autonomous);
    assert_eq!(mission.last_applied_sequence, 1);

    // Assert: persisted in database
    let stored = mission_repo.get(mission.id).await.unwrap().unwrap();
    assert_eq!(stored.id, mission.id);
    assert_eq!(stored.mode, AutonomyMode::Autonomous);
}

#[tokio::test]
async fn test_mission_service_transition_state_machine_validation() {
    // Arrange: mission created in Created state
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("svc_trans.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let event_bus = Arc::new(BroadcastEventBus::default());

    let service = MissionService::new(tx_mgr, mission_repo.clone(), event_repo, event_bus);

    let intake = MissionIntake::new("Validation test");
    let mission = service
        .create_mission(intake, Path::new("/workspace"))
        .await
        .unwrap();

    // Act: attempt illegal state jump (Created -> Executing directly without planning)
    let result = service
        .transition_mission(mission.id, MissionState::Executing, "user", None)
        .await;

    // Assert: state machine rejects transition
    assert!(result.is_err());

    // Assert: mission remains Created in DB
    let stored = mission_repo.get(mission.id).await.unwrap().unwrap();
    assert_eq!(stored.status, MissionState::Created);
}

#[tokio::test]
async fn test_mission_service_transition_completion_gate_enforcement() {
    // Arrange: advance mission to Shipping state
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("svc_gate.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let event_bus = Arc::new(BroadcastEventBus::default());

    let service = MissionService::new(tx_mgr, mission_repo.clone(), event_repo, event_bus);

    let intake = MissionIntake::new("Gate test");
    let mission = service
        .create_mission(intake, Path::new("/workspace"))
        .await
        .unwrap();

    let states = [
        MissionState::Understanding,
        MissionState::Researching,
        MissionState::Planning,
        MissionState::Scheduled,
        MissionState::Executing,
        MissionState::Verifying,
        MissionState::Reviewing,
        MissionState::Integrating,
        MissionState::Shipping,
    ];
    for st in states {
        service
            .transition_mission(mission.id, st, "operator", None)
            .await
            .unwrap();
    }

    // Act 1: attempt completion without CompletionContext
    let err_no_ctx = service
        .transition_mission(mission.id, MissionState::Completed, "operator", None)
        .await;
    assert!(err_no_ctx.is_err());

    // Act 2: attempt completion with failing CompletionContext
    let mut failing_ctx = CompletionContext::all_satisfied();
    failing_ctx.mandatory_requirements_verified = false;
    let err_failing = service
        .transition_mission(
            mission.id,
            MissionState::Completed,
            "operator",
            Some(&failing_ctx),
        )
        .await;
    assert!(err_failing.is_err());

    // Act 3: complete with all_satisfied
    let satisfied = CompletionContext::all_satisfied();
    let completed = service
        .transition_mission(
            mission.id,
            MissionState::Completed,
            "operator",
            Some(&satisfied),
        )
        .await
        .unwrap();

    // Assert: completed status achieved
    assert_eq!(completed.status, MissionState::Completed);
    let stored = mission_repo.get(mission.id).await.unwrap().unwrap();
    assert_eq!(stored.status, MissionState::Completed);
}

#[tokio::test]
async fn test_mission_service_fork_mission_immutability() {
    // Arrange: parent mission
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("svc_fork.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let event_bus = Arc::new(BroadcastEventBus::default());

    let service = MissionService::new(tx_mgr, mission_repo.clone(), event_repo, event_bus);

    let intake = MissionIntake::new("Parent Mission to fork");
    let parent = service
        .create_mission(intake, Path::new("/workspace"))
        .await
        .unwrap();

    // Act: fork child mission
    let child = service
        .fork_mission(parent.id, None, Path::new("/workspace"))
        .await
        .unwrap();

    // Assert: child has distinct identity and references parent
    assert_ne!(child.id, parent.id);
    assert_eq!(child.parent_mission, Some(parent.id));
    assert_eq!(child.objective, parent.objective);
    assert_eq!(child.status, MissionState::Created);

    // Assert: parent record in SQLite remains unchanged (MSN-06)
    let parent_in_db = mission_repo.get(parent.id).await.unwrap().unwrap();
    assert_eq!(parent_in_db, parent);
}

#[tokio::test]
async fn test_mission_service_fork_with_overrides() {
    // Arrange: parent mission
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("svc_fork_overrides.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let tx_mgr = Arc::new(SqliteTransactionManager::new(pool.clone()));
    let mission_repo = Arc::new(SqliteMissionRepository::new(pool.clone()));
    let event_repo = Arc::new(SqliteEventRepository::new(pool));
    let event_bus = Arc::new(BroadcastEventBus::default());

    let service = MissionService::new(tx_mgr, mission_repo, event_repo, event_bus);

    let intake = MissionIntake::new("Base mission");
    let parent = service
        .create_mission(intake, Path::new("/workspace"))
        .await
        .unwrap();

    // Act: fork with overrides (different objective and Unattended mode)
    let mut override_intake = MissionIntake::new("Specialized child mission");
    override_intake.mode = Some(AutonomyMode::Unattended);

    let child = service
        .fork_mission(
            parent.id,
            Some(override_intake),
            Path::new("/workspace/custom"),
        )
        .await
        .unwrap();

    // Assert: overrides applied
    assert_eq!(child.objective, "Specialized child mission");
    assert_eq!(child.mode, AutonomyMode::Unattended);
    assert_eq!(child.workspace_root, PathBuf::from("/workspace/custom"));
    assert_eq!(child.parent_mission, Some(parent.id));
}
