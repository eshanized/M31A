//! Phase 01: Events, IDs, and Kernel Invariants Integration Tests
//!
//! Verifies domain ID generation/serialization/monotonicity, EventEnvelope
//! causal metadata & categorization, cancellation tree propagation, and runtime invariants.

use m31a::events::{EventCategory, EventEnvelope, EventType};
use m31a::ids::{
    AgentId, ArtifactId, CheckpointId, EventId, JobId, MissionId, RequirementId, SessionId, TaskId,
    ToolCallId,
};
use m31a::kernel::{MissionRuntime, RuntimeInvariants};
use m31a::state_machine::{MissionState, TaskState};
use std::panic;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_domain_ids_uuidv7_timestamp_monotonicity() {
    // Arrange: collect sequential MissionIds generated across slight time intervals
    let mut ids = Vec::new();
    for _ in 0..5 {
        ids.push(MissionId::new());
        sleep(Duration::from_millis(2)).await;
    }

    // Act & Assert: UUIDv7 uses timestamp in high-order bits, so byte representations
    // are monotonically non-decreasing over real time.
    for i in 0..ids.len() - 1 {
        let curr = ids[i].as_bytes();
        let next = ids[i + 1].as_bytes();
        assert!(
            curr <= next,
            "UUIDv7 sequence must be monotonically ordered by time: {:?} <= {:?}",
            curr,
            next
        );
    }
}

#[test]
fn test_domain_ids_binary_and_hex_serialization() {
    // Arrange & Act & Assert for all 10 domain ID newtypes:
    // 1. MissionId
    let m = MissionId::new();
    assert_eq!(MissionId::from_bytes(*m.as_bytes()), m);
    let m_json = serde_json::to_string(&m).unwrap();
    assert_eq!(serde_json::from_str::<MissionId>(&m_json).unwrap(), m);
    assert_eq!(MissionId::from_str(&m.to_string()).unwrap(), m);

    // 2. TaskId
    let t = TaskId::new();
    assert_eq!(TaskId::from_bytes(*t.as_bytes()), t);
    let t_json = serde_json::to_string(&t).unwrap();
    assert_eq!(serde_json::from_str::<TaskId>(&t_json).unwrap(), t);
    assert_eq!(TaskId::from_str(&t.to_string()).unwrap(), t);

    // 3. RequirementId
    let r = RequirementId::new();
    assert_eq!(RequirementId::from_bytes(*r.as_bytes()), r);
    let r_json = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<RequirementId>(&r_json).unwrap(), r);
    assert_eq!(RequirementId::from_str(&r.to_string()).unwrap(), r);

    // 4. AgentId
    let a = AgentId::new();
    assert_eq!(AgentId::from_bytes(*a.as_bytes()), a);
    let a_json = serde_json::to_string(&a).unwrap();
    assert_eq!(serde_json::from_str::<AgentId>(&a_json).unwrap(), a);
    assert_eq!(AgentId::from_str(&a.to_string()).unwrap(), a);

    // 5. JobId
    let j = JobId::new();
    assert_eq!(JobId::from_bytes(*j.as_bytes()), j);
    let j_json = serde_json::to_string(&j).unwrap();
    assert_eq!(serde_json::from_str::<JobId>(&j_json).unwrap(), j);
    assert_eq!(JobId::from_str(&j.to_string()).unwrap(), j);

    // 6. SessionId
    let s = SessionId::new();
    assert_eq!(SessionId::from_bytes(*s.as_bytes()), s);
    let s_json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<SessionId>(&s_json).unwrap(), s);
    assert_eq!(SessionId::from_str(&s.to_string()).unwrap(), s);

    // 7. ArtifactId
    let art = ArtifactId::new();
    assert_eq!(ArtifactId::from_bytes(*art.as_bytes()), art);
    let art_json = serde_json::to_string(&art).unwrap();
    assert_eq!(serde_json::from_str::<ArtifactId>(&art_json).unwrap(), art);
    assert_eq!(ArtifactId::from_str(&art.to_string()).unwrap(), art);

    // 8. EventId
    let e = EventId::new();
    assert_eq!(EventId::from_bytes(*e.as_bytes()), e);
    let e_json = serde_json::to_string(&e).unwrap();
    assert_eq!(serde_json::from_str::<EventId>(&e_json).unwrap(), e);
    assert_eq!(EventId::from_str(&e.to_string()).unwrap(), e);

    // 9. ToolCallId
    let tc = ToolCallId::new();
    assert_eq!(ToolCallId::from_bytes(*tc.as_bytes()), tc);
    let tc_json = serde_json::to_string(&tc).unwrap();
    assert_eq!(serde_json::from_str::<ToolCallId>(&tc_json).unwrap(), tc);
    assert_eq!(ToolCallId::from_str(&tc.to_string()).unwrap(), tc);

    // 10. CheckpointId
    let cp = CheckpointId::new();
    assert_eq!(CheckpointId::from_bytes(*cp.as_bytes()), cp);
    let cp_json = serde_json::to_string(&cp).unwrap();
    assert_eq!(serde_json::from_str::<CheckpointId>(&cp_json).unwrap(), cp);
    assert_eq!(CheckpointId::from_str(&cp.to_string()).unwrap(), cp);
}

#[test]
fn test_domain_ids_invalid_deserialization_fails() {
    // Arrange: malformed UUID strings
    let malformed_strings = [
        "not-a-uuid",
        "12345",
        "",
        "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz",
    ];

    // Act & Assert: FromStr and JSON deserialization reject invalid strings
    for bad in malformed_strings {
        assert!(MissionId::from_str(bad).is_err());
        assert!(TaskId::from_str(bad).is_err());
        let json = format!("\"{}\"", bad);
        assert!(serde_json::from_str::<MissionId>(&json).is_err());
        assert!(serde_json::from_str::<TaskId>(&json).is_err());
    }
}

#[test]
fn test_event_envelope_causal_tracking() {
    // Arrange: causal IDs
    let correlation_id = "corr_abc_987".to_string();
    let causation_id = EventId::new();
    let mission_id = MissionId::new();

    // Act: build envelope with causal provenance
    let envelope = EventEnvelope::new(
        100,
        Some(mission_id),
        None,
        "planner".to_string(),
        EventType::PlanCreated {
            plan_id: "plan-1".to_string(),
            mission_id,
            summary: "Initial DAG".to_string(),
        },
    )
    .with_causation(Some(correlation_id.clone()), Some(causation_id))
    .with_category(EventCategory::Diagnostic);

    // Assert: fields preserved
    assert_eq!(envelope.correlation_id, Some(correlation_id));
    assert_eq!(envelope.causation_id, Some(causation_id));
    assert_eq!(envelope.category, EventCategory::Diagnostic);
    assert_eq!(envelope.sequence, 100);
}

#[test]
fn test_event_envelope_category_parsing() {
    // Arrange & Act & Assert: valid variants with case variations
    assert_eq!(
        EventCategory::from_str("durable").unwrap(),
        EventCategory::Durable
    );
    assert_eq!(
        EventCategory::from_str("DURABLE").unwrap(),
        EventCategory::Durable
    );
    assert_eq!(
        EventCategory::from_str("ephemeral").unwrap(),
        EventCategory::Ephemeral
    );
    assert_eq!(
        EventCategory::from_str("Ephemeral").unwrap(),
        EventCategory::Ephemeral
    );
    assert_eq!(
        EventCategory::from_str("diagnostic").unwrap(),
        EventCategory::Diagnostic
    );
    assert_eq!(
        EventCategory::from_str("DIAGNOSTIC").unwrap(),
        EventCategory::Diagnostic
    );

    // Invalid category
    assert!(EventCategory::from_str("unrecognized").is_err());
}

#[test]
fn test_event_envelope_serde_fidelity() {
    // Arrange: create a complete envelope
    let mission_id = MissionId::new();
    let session_id = SessionId::new();
    let causation_id = EventId::new();
    let envelope = EventEnvelope::new(
        42,
        Some(mission_id),
        Some(session_id),
        "scheduler".to_string(),
        EventType::TaskStarted {
            task_id: TaskId::new(),
            mission_id,
            agent_id: AgentId::new(),
        },
    )
    .with_causation(Some("trace-42".to_string()), Some(causation_id))
    .with_category(EventCategory::Durable);

    // Act: serialize and deserialize JSON
    let json = serde_json::to_string(&envelope).unwrap();
    let deserialized: EventEnvelope = serde_json::from_str(&json).unwrap();

    // Assert: full fidelity preserved
    assert_eq!(envelope.id, deserialized.id);
    assert_eq!(envelope.sequence, deserialized.sequence);
    assert_eq!(envelope.mission_id, deserialized.mission_id);
    assert_eq!(envelope.session_id, deserialized.session_id);
    assert_eq!(envelope.actor, deserialized.actor);
    assert_eq!(envelope.category, deserialized.category);
    assert_eq!(envelope.correlation_id, deserialized.correlation_id);
    assert_eq!(envelope.causation_id, deserialized.causation_id);
    assert_eq!(envelope.schema_version, deserialized.schema_version);
    assert_eq!(envelope.event_type.name(), deserialized.event_type.name());
}

#[test]
fn test_cancellation_hierarchy_propagation() {
    // Arrange: MissionRuntime root and nested children
    let runtime = MissionRuntime::new();
    let child1 = runtime.spawn_child();
    let child2 = runtime.spawn_child();
    let grandchild = child1.child_token();

    assert!(!runtime.is_cancelled());
    assert!(!child1.is_cancelled());
    assert!(!child2.is_cancelled());
    assert!(!grandchild.is_cancelled());

    // Act: cancel root
    runtime.shutdown();

    // Assert: all descendants cancelled
    assert!(runtime.is_cancelled());
    assert!(child1.is_cancelled());
    assert!(child2.is_cancelled());
    assert!(grandchild.is_cancelled());
}

#[test]
fn test_cancellation_child_isolation() {
    // Arrange: root and two children
    let runtime = MissionRuntime::new();
    let child1 = runtime.spawn_child();
    let child2 = runtime.spawn_child();

    // Act: cancel child1 only
    child1.cancel();

    // Assert: child1 cancelled, but root and child2 remain active
    assert!(child1.is_cancelled());
    assert!(!runtime.is_cancelled());
    assert!(!child2.is_cancelled());
}

#[tokio::test]
async fn test_cancellation_with_tokio_select() {
    // Arrange: child token and worker task
    let runtime = MissionRuntime::new();
    let child = runtime.spawn_child();

    let child_for_task = child.clone();
    let task_handle = tokio::spawn(async move {
        tokio::select! {
            _ = sleep(Duration::from_secs(60)) => "completed_sleep",
            _ = child_for_task.cancelled() => "aborted_by_token",
        }
    });

    // Act: cancel child token after short delay
    sleep(Duration::from_millis(10)).await;
    child.cancel();

    // Assert: select exited via cancellation branch
    let result = task_handle.await.unwrap();
    assert_eq!(result, "aborted_by_token");
}

#[test]
fn test_runtime_invariants_assert_task_states() {
    let invariants = RuntimeInvariants;

    // Arrange & Act & Assert: valid states do not panic
    invariants.assert_task_not_blocked(&TaskState::Pending);
    invariants.assert_task_not_blocked(&TaskState::Ready);
    invariants.assert_task_not_succeeded(&TaskState::Running);
    invariants.assert_not_terminal(&TaskState::Ready);

    // Panics on Blocked
    let blocked_panic = panic::catch_unwind(|| {
        invariants.assert_task_not_blocked(&TaskState::Blocked);
    });
    assert!(blocked_panic.is_err());

    // Panics on Succeeded
    let succeeded_panic = panic::catch_unwind(|| {
        invariants.assert_task_not_succeeded(&TaskState::Succeeded);
    });
    assert!(succeeded_panic.is_err());

    // Panics on terminal states
    for term in [
        TaskState::Succeeded,
        TaskState::Failed,
        TaskState::Skipped,
        TaskState::Cancelled,
    ] {
        let term_panic = panic::catch_unwind(|| {
            invariants.assert_not_terminal(&term);
        });
        assert!(term_panic.is_err());
    }
}

#[test]
fn test_runtime_invariants_assert_mission_states() {
    let invariants = RuntimeInvariants;

    // Non-completed states do not panic
    invariants.assert_mission_not_completed(&MissionState::Created);
    invariants.assert_mission_not_completed(&MissionState::Executing);
    invariants.assert_mission_not_completed(&MissionState::Verifying);

    // Panics on Completed
    let completed_panic = panic::catch_unwind(|| {
        invariants.assert_mission_not_completed(&MissionState::Completed);
    });
    assert!(completed_panic.is_err());
}

#[test]
fn test_runtime_invariants_side_effect_policy() {
    let invariants = RuntimeInvariants;

    // Passing when policy decided
    invariants.assert_side_effect_has_policy("write_file", true);

    // Panics when policy NOT decided
    let no_policy_panic = panic::catch_unwind(|| {
        invariants.assert_side_effect_has_policy("write_file", false);
    });
    assert!(no_policy_panic.is_err());
}
