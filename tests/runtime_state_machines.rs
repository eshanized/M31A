//! Phase 01: State Machine Integration Tests
//!
//! Verifies Mission, Task, and Agent lifecycle state machines,
//! validated transitions, terminal state immutability, and string parsing.

use m31a::state_machine::{
    AgentEvent, AgentState, MissionEvent, MissionState, TaskEvent, TaskState, TransitionError,
    transition_agent, transition_mission, transition_task,
};
use std::str::FromStr;

#[test]
fn test_mission_state_from_str_exhaustive() {
    // Arrange: list all 15 valid MissionState names and an invalid candidate
    let valid_pairs = [
        ("Created", MissionState::Created),
        ("Understanding", MissionState::Understanding),
        ("Researching", MissionState::Researching),
        ("Planning", MissionState::Planning),
        ("Scheduled", MissionState::Scheduled),
        ("Executing", MissionState::Executing),
        ("Verifying", MissionState::Verifying),
        ("Reviewing", MissionState::Reviewing),
        ("Integrating", MissionState::Integrating),
        ("Shipping", MissionState::Shipping),
        ("Completed", MissionState::Completed),
        ("Paused", MissionState::Paused),
        ("Blocked", MissionState::Blocked),
        ("Failed", MissionState::Failed),
        ("Cancelled", MissionState::Cancelled),
    ];

    // Act & Assert: verify every valid state parses and display matches
    for (name, expected_state) in valid_pairs {
        let parsed = MissionState::from_str(name);
        assert_eq!(parsed, Ok(expected_state), "Failed to parse {}", name);
        assert_eq!(format!("{}", expected_state), name);
    }

    // Act & Assert: invalid string must return TransitionError
    let invalid = MissionState::from_str("NonExistentState");
    assert!(invalid.is_err());
}

#[test]
fn test_mission_full_forward_lifecycle() {
    // Arrange: initial state Created
    let mut state = MissionState::Created;
    let sequence = [
        (MissionEvent::Start, MissionState::Understanding),
        (MissionEvent::UnderstandComplete, MissionState::Researching),
        (MissionEvent::ResearchComplete, MissionState::Planning),
        (MissionEvent::PlanComplete, MissionState::Scheduled),
        (MissionEvent::Execute, MissionState::Executing),
        (MissionEvent::Verify, MissionState::Verifying),
        (MissionEvent::Review, MissionState::Reviewing),
        (MissionEvent::Integrate, MissionState::Integrating),
        (MissionEvent::Ship, MissionState::Shipping),
        (MissionEvent::Complete, MissionState::Completed),
    ];

    // Act & Assert: follow full forward progression through all gates
    for (event, expected) in sequence {
        state = transition_mission(state, event)
            .unwrap_or_else(|e| panic!("Failed on event {:?}: {:?}", event, e));
        assert_eq!(state, expected);
    }

    assert!(state.is_terminal());
}

#[test]
fn test_mission_pause_resume_cycles() {
    // Arrange: state is Executing
    let initial = MissionState::Executing;

    // Act: transition to Paused
    let paused = transition_mission(initial, MissionEvent::Pause).expect("Pause should succeed");

    // Assert
    assert_eq!(paused, MissionState::Paused);
    assert!(!paused.is_terminal());

    // Act: resume back to Executing
    let resumed = transition_mission(paused, MissionEvent::Resume).expect("Resume should succeed");

    // Assert
    assert_eq!(resumed, MissionState::Executing);

    // Act: cycle a second time
    let paused_again = transition_mission(resumed, MissionEvent::Pause).unwrap();
    let resumed_again = transition_mission(paused_again, MissionEvent::Resume).unwrap();
    assert_eq!(resumed_again, MissionState::Executing);
}

#[test]
fn test_mission_block_unblock_cycles() {
    // Arrange: non-terminal state Planning
    let planning = MissionState::Planning;

    // Act: Block from Planning
    let blocked_from_planning =
        transition_mission(planning, MissionEvent::Block).expect("Block should succeed");

    // Assert
    assert_eq!(blocked_from_planning, MissionState::Blocked);

    // Act: Unblock returns to Executing
    let unblocked = transition_mission(blocked_from_planning, MissionEvent::Unblock)
        .expect("Unblock should succeed");
    assert_eq!(unblocked, MissionState::Executing);

    // Act: Block from Executing
    let blocked_from_exec =
        transition_mission(unblocked, MissionEvent::Block).expect("Block should succeed");
    assert_eq!(blocked_from_exec, MissionState::Blocked);
}

#[test]
fn test_mission_fail_and_cancel_from_all_active_states() {
    // Arrange: non-terminal states
    let active_states = [
        MissionState::Created,
        MissionState::Understanding,
        MissionState::Researching,
        MissionState::Planning,
        MissionState::Scheduled,
        MissionState::Executing,
        MissionState::Verifying,
        MissionState::Reviewing,
        MissionState::Integrating,
        MissionState::Shipping,
        MissionState::Paused,
        MissionState::Blocked,
    ];

    // Act & Assert: Fail and Cancel must succeed from every non-terminal state
    for state in active_states {
        let failed = transition_mission(state, MissionEvent::Fail);
        assert_eq!(
            failed,
            Ok(MissionState::Failed),
            "Fail should succeed from {:?}",
            state
        );

        let cancelled = transition_mission(state, MissionEvent::Cancel);
        assert_eq!(
            cancelled,
            Ok(MissionState::Cancelled),
            "Cancel should succeed from {:?}",
            state
        );
    }
}

#[test]
fn test_mission_terminal_states_reject_all_events() {
    // Arrange: all terminal states and all possible events
    let terminal_states = [
        MissionState::Completed,
        MissionState::Failed,
        MissionState::Cancelled,
    ];
    let all_events = [
        MissionEvent::Start,
        MissionEvent::UnderstandComplete,
        MissionEvent::ResearchComplete,
        MissionEvent::PlanComplete,
        MissionEvent::Schedule,
        MissionEvent::Execute,
        MissionEvent::Verify,
        MissionEvent::Review,
        MissionEvent::Integrate,
        MissionEvent::Ship,
        MissionEvent::Complete,
        MissionEvent::Pause,
        MissionEvent::Resume,
        MissionEvent::Block,
        MissionEvent::Unblock,
        MissionEvent::Fail,
        MissionEvent::Cancel,
    ];

    // Act & Assert: no event is permitted from any terminal state
    for state in terminal_states {
        assert!(state.is_terminal());
        for event in all_events {
            let result = transition_mission(state, event);
            match result {
                Err(TransitionError::TerminalState { state: s }) => {
                    assert_eq!(s, format!("{:?}", state));
                }
                other => panic!(
                    "Expected TerminalState error from {:?} with event {:?}, got {:?}",
                    state, event, other
                ),
            }
        }
    }
}

#[test]
fn test_mission_invalid_transition_matrix() {
    // Arrange: invalid transitions that skip stages or reverse illegally
    let invalid_pairs = [
        (MissionState::Created, MissionEvent::Execute),
        (MissionState::Created, MissionEvent::Complete),
        (MissionState::Understanding, MissionEvent::Ship),
        (MissionState::Planning, MissionEvent::Complete),
        (MissionState::Executing, MissionEvent::Start),
        (MissionState::Paused, MissionEvent::Start),
        (MissionState::Blocked, MissionEvent::Start),
    ];

    // Act & Assert: each must produce an InvalidTransition error
    for (state, event) in invalid_pairs {
        let result = transition_mission(state, event);
        match result {
            Err(TransitionError::InvalidTransition { .. }) => {
                // Expected
            }
            other => panic!(
                "Expected InvalidTransition from {:?} on {:?}, got {:?}",
                state, event, other
            ),
        }
    }
}

#[test]
fn test_task_state_from_str_exhaustive() {
    // Arrange: 9 task states
    let valid_pairs = [
        ("Pending", TaskState::Pending),
        ("Blocked", TaskState::Blocked),
        ("Ready", TaskState::Ready),
        ("Running", TaskState::Running),
        ("Succeeded", TaskState::Succeeded),
        ("Failed", TaskState::Failed),
        ("Skipped", TaskState::Skipped),
        ("Cancelled", TaskState::Cancelled),
        ("NeedsReview", TaskState::NeedsReview),
    ];

    // Act & Assert: verify FromStr and Display
    for (name, expected) in valid_pairs {
        let parsed = TaskState::from_str(name);
        assert_eq!(parsed, Ok(expected));
        assert_eq!(format!("{}", expected), name);
    }

    let invalid = TaskState::from_str("UnknownTaskState");
    assert!(invalid.is_err());
}

#[test]
fn test_task_full_lifecycle_and_review_loop() {
    // Arrange: task lifecycle through review
    let task = TaskState::Pending;

    // Act & Assert: Pending -> Ready -> Running -> NeedsReview -> Succeeded
    let ready = transition_task(task, TaskEvent::MarkReady).unwrap();
    assert_eq!(ready, TaskState::Ready);

    let running = transition_task(ready, TaskEvent::Start).unwrap();
    assert_eq!(running, TaskState::Running);

    let review = transition_task(running, TaskEvent::RequestReview).unwrap();
    assert_eq!(review, TaskState::NeedsReview);

    let succeeded = transition_task(review, TaskEvent::Complete).unwrap();
    assert_eq!(succeeded, TaskState::Succeeded);
    assert!(succeeded.is_terminal());

    // Act & Assert: NeedsReview -> Failed branch
    let running2 = transition_task(TaskState::Ready, TaskEvent::Start).unwrap();
    let review2 = transition_task(running2, TaskEvent::RequestReview).unwrap();
    let failed = transition_task(review2, TaskEvent::Fail).unwrap();
    assert_eq!(failed, TaskState::Failed);
    assert!(failed.is_terminal());
}

#[test]
fn test_task_skip_and_cancel() {
    // Arrange: Ready and Pending tasks
    let ready = TaskState::Ready;
    let pending = TaskState::Pending;
    let running = TaskState::Running;

    // Act & Assert: skip from Ready and Pending
    let skipped1 = transition_task(ready, TaskEvent::Skip).unwrap();
    assert_eq!(skipped1, TaskState::Skipped);
    assert!(skipped1.is_terminal());

    let skipped2 = transition_task(pending, TaskEvent::Skip).unwrap();
    assert_eq!(skipped2, TaskState::Skipped);

    // Act & Assert: cancel from Running
    let cancelled = transition_task(running, TaskEvent::Cancel).unwrap();
    assert_eq!(cancelled, TaskState::Cancelled);
    assert!(cancelled.is_terminal());
}

#[test]
fn test_task_terminal_states_reject_all_events() {
    // Arrange: all 4 terminal task states
    let terminals = [
        TaskState::Succeeded,
        TaskState::Failed,
        TaskState::Skipped,
        TaskState::Cancelled,
    ];
    let all_events = [
        TaskEvent::Unblock,
        TaskEvent::MarkReady,
        TaskEvent::Start,
        TaskEvent::Complete,
        TaskEvent::Fail,
        TaskEvent::Skip,
        TaskEvent::Cancel,
        TaskEvent::RequestReview,
    ];

    // Act & Assert: terminal rejection
    for state in terminals {
        assert!(state.is_terminal());
        for event in all_events {
            let result = transition_task(state, event);
            assert!(
                matches!(result, Err(TransitionError::TerminalState { .. })),
                "Expected TerminalState error from {:?} on {:?}",
                state,
                event
            );
        }
    }
}

#[test]
fn test_task_invalid_transitions() {
    // Arrange: invalid transition combinations
    let invalid_pairs = [
        (TaskState::Pending, TaskEvent::Start),
        (TaskState::Pending, TaskEvent::Complete),
        (TaskState::Blocked, TaskEvent::Start),
        (TaskState::Ready, TaskEvent::Unblock),
        (TaskState::Ready, TaskEvent::Complete),
        (TaskState::Running, TaskEvent::Unblock),
        (TaskState::Running, TaskEvent::MarkReady),
    ];

    // Act & Assert
    for (state, event) in invalid_pairs {
        let result = transition_task(state, event);
        assert!(
            matches!(result, Err(TransitionError::InvalidTransition { .. })),
            "Expected InvalidTransition for {:?} on {:?}",
            state,
            event
        );
    }
}

#[test]
fn test_agent_lifecycle_transitions() {
    // Arrange: starting agent
    let agent = AgentState::Starting;

    // Act & Assert: Starting -> Initializing -> Running -> Completing -> Completed
    let init = transition_agent(agent, AgentEvent::Initialize).unwrap();
    assert_eq!(init, AgentState::Initializing);

    let running = transition_agent(init, AgentEvent::Start).unwrap();
    assert_eq!(running, AgentState::Running);

    let paused = transition_agent(running, AgentEvent::Pause).unwrap();
    assert_eq!(paused, AgentState::Paused);

    let resumed = transition_agent(paused, AgentEvent::Resume).unwrap();
    assert_eq!(resumed, AgentState::Running);

    let completing = transition_agent(resumed, AgentEvent::Complete).unwrap();
    assert_eq!(completing, AgentState::Completing);

    let completed = transition_agent(completing, AgentEvent::Complete).unwrap();
    assert_eq!(completed, AgentState::Completed);
    assert!(completed.is_terminal());
}

#[test]
fn test_agent_terminal_rejection_and_errors() {
    // Arrange: terminal agent states and active states
    let terminals = [
        AgentState::Completed,
        AgentState::Failed,
        AgentState::Cancelled,
    ];
    let active = [
        AgentState::Starting,
        AgentState::Initializing,
        AgentState::Running,
        AgentState::Paused,
        AgentState::Completing,
    ];

    // Act & Assert: terminal rejection
    for state in terminals {
        assert!(state.is_terminal());
        let result = transition_agent(state, AgentEvent::Start);
        assert!(matches!(result, Err(TransitionError::TerminalState { .. })));
    }

    // Act & Assert: active states can fail and cancel
    for state in active {
        assert!(transition_agent(state, AgentEvent::Fail).is_ok());
        assert!(transition_agent(state, AgentEvent::Cancel).is_ok());
    }
}
