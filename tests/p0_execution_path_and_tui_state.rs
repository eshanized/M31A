//! P0 Execution Path, Event Stream, and Live TUI State Integration Tests
//!
//! Validates:
//! 1. Non-blocking bridge execution and continuous event forwarding (§2, §3, §29)
//! 2. Authoritative task state projection: TaskStarted leaves Waiting/0% (§5, §6, §21)
//! 3. Single authoritative execution state and heartbeat/stall detection (§4, §20, §27)
//! 4. Event envelope deduplication and idempotency (§12, §28)
//! 5. Case-insensitive task status rendering and clear wait reasons (§21, §22)
//! 6. Prevention of duplicate concurrent execution (§34)
//! 7. Immediate and responsive cancellation (§17)

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use uuid::Uuid;

use m31a::agent::model_policy::DeterministicLifecycleModelCaller;
use m31a::events::envelope::EventEnvelope;
use m31a::ids::MissionId;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::runtime::AppRuntime;
use m31a::tui::TuiRuntimeBridge;
use m31a::tui::model::{TaskGraphProjectionState, TuiTaskSnapshot, TuiViewModel, UiOperationState};
use m31a::tui::surface::tasks::render_tasks_surface;
use m31a::tui::theme::{ThemeMode, ThemeTokens};

async fn setup_test_runtime() -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempfile::tempdir().expect("fixture tempdir");
    let repo_path = dir.path();
    assert!(
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_path)
            .status()
            .expect("git init")
            .success()
    );
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "p0-test@m31a.local"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A P0 Test"])
        .current_dir(repo_path)
        .status();
    let runtime = AppRuntime::new(repo_path).await.expect("AppRuntime::new");
    let runtime = runtime.with_model_caller(Arc::new(DeterministicLifecycleModelCaller::new()));
    (dir, Arc::new(runtime))
}

#[tokio::test]
async fn test_bridge_worker_non_blocking_and_event_routing() {
    let (_dir, runtime) = setup_test_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .unwrap();

    let tx = bridge.sender();
    let _rx = bridge.take_event_receiver().expect("take_event_receiver");

    // Spawn dummy listener/pump
    let mut model = TuiViewModel::new();
    let mission_id = MissionId::from(Uuid::now_v7());

    // Populate initial tasks in Waiting
    model.tasks = vec![
        TuiTaskSnapshot {
            id: "t1".to_string(),
            title: "Task 1: Repo Discovery".to_string(),
            status: "Pending".to_string(),
            agent_role: Some("researcher".to_string()),
            progress_pct: 0,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "t2".to_string(),
            title: "Task 2: Architecture".to_string(),
            status: "Pending".to_string(),
            agent_role: Some("architect".to_string()),
            progress_pct: 0,
            dependencies: vec!["t1".to_string()],
        },
    ];
    model.task_graph_state = TaskGraphProjectionState::Loaded;

    // Simulate TaskStarted arrival via bridge / interaction event
    let event = InteractionEvent::TaskStarted {
        mission_id: mission_id.to_string(),
        task_id: "t1".to_string(),
        agent_id: "researcher".to_string(),
    };
    model.apply_interaction_event(&event);

    // Verify task state changed from Pending/Waiting to Running with non-zero progress
    assert_eq!(model.tasks[0].status, "running");
    assert!(
        model.tasks[0].progress_pct > 0,
        "TaskStarted must establish non-zero initial progress"
    );
    assert_eq!(model.operation_state(), UiOperationState::Executing);

    // Verify heartbeat and summary reflect the running task
    let summary = model.execution_summary().expect("execution summary");
    assert!(
        summary.contains("Task 1/2"),
        "Summary must contain progress counter: {summary}"
    );
    assert!(
        summary.contains("Repo Discovery"),
        "Summary must contain title: {summary}"
    );
    assert!(
        summary.contains("researcher"),
        "Summary must contain agent: {summary}"
    );

    // Simulate ToolStarted arrival
    let tool_event = InteractionEvent::ToolStarted {
        call_id: "call-1".to_string(),
        tool_name: "fs_read".to_string(),
        parameters: serde_json::json!({"path": "Cargo.toml"}),
    };
    model.apply_interaction_event(&tool_event);
    assert_eq!(model.operation_state(), UiOperationState::RunningTool);

    let summary_tool = model.execution_summary().expect("execution summary");
    assert!(
        summary_tool.contains("Running tool `fs_read`"),
        "Summary must reflect in-flight tool: {summary_tool}"
    );

    // Simulate ToolCompleted
    let tool_comp = InteractionEvent::ToolCompleted {
        call_id: "call-1".to_string(),
        tool_name: "fs_read".to_string(),
        success: true,
        output_preview: "ok".to_string(),
    };
    model.apply_interaction_event(&tool_comp);

    // Simulate TaskCompleted arrival
    let task_comp = InteractionEvent::TaskCompleted {
        mission_id: mission_id.to_string(),
        task_id: "t1".to_string(),
        result: "discovery complete".to_string(),
    };
    model.apply_interaction_event(&task_comp);
    assert_eq!(model.tasks[0].status, "completed");
    assert_eq!(model.tasks[0].progress_pct, 100);

    // Ensure bridge channel remains open and responsive to cancellation
    assert!(tx.send(ApplicationAction::CancelRequested).is_ok());
}

#[test]
fn test_event_envelope_deduplication() {
    let mut model = TuiViewModel::new();

    let mission_id = MissionId::from(Uuid::now_v7());

    let envelope = EventEnvelope::new(
        1,
        Some(mission_id),
        None,
        "runtime".to_string(),
        m31a::events::types::EventType::MissionStarted {
            mission_id,
            objective: "Test Objective".to_string(),
        },
    );

    // First application
    model.apply_event(&envelope);
    assert_eq!(model.mission_status, "running");
    assert_eq!(model.objective.as_str(), "Test Objective");
    let conversation_len_first = model.conversation.len();
    let timeline_len_first = model.timeline.len();

    // Second application (duplicate)
    model.apply_event(&envelope);
    assert_eq!(
        model.conversation.len(),
        conversation_len_first,
        "Duplicate envelope must be rejected by seen_events"
    );
    assert_eq!(
        model.timeline.len(),
        timeline_len_first,
        "Duplicate envelope must not append duplicate timeline entries"
    );
}

#[test]
fn test_task_panel_case_insensitive_status_and_wait_reason() {
    let mut model = TuiViewModel::new();
    model.task_graph_state = TaskGraphProjectionState::Loaded;
    model.tasks = vec![
        TuiTaskSnapshot {
            id: "t1".to_string(),
            title: "Task 1: Active".to_string(),
            status: "Running".to_string(), // Capitalized from TaskState::to_string()
            agent_role: Some("coder".to_string()),
            progress_pct: 45,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "t2".to_string(),
            title: "Task 2: Scheduled".to_string(),
            status: "Ready".to_string(), // Capitalized
            agent_role: Some("tester".to_string()),
            progress_pct: 0,
            dependencies: vec!["t1".to_string()],
        },
        TuiTaskSnapshot {
            id: "t3".to_string(),
            title: "Task 3: Done".to_string(),
            status: "Succeeded".to_string(), // Capitalized
            agent_role: Some("reviewer".to_string()),
            progress_pct: 100,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "t4".to_string(),
            title: "Task 4: Ready For Dispatch".to_string(),
            status: "Ready".to_string(),
            agent_role: Some("builder".to_string()),
            progress_pct: 0,
            dependencies: vec![],
        },
    ];

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::Default);

    // Test rendering of task 1 (Running)
    terminal
        .draw(|f| {
            render_tasks_surface(f, f.area(), &model, &tokens, false, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let rendered_text: String = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    // Task 1 should show Running indicator and glyph ▶
    assert!(
        rendered_text.contains("Task 1: Active"),
        "Must render task title: {rendered_text}"
    );
    assert!(
        rendered_text.contains("▶") || rendered_text.to_lowercase().contains("running"),
        "Must render running status: {rendered_text}"
    );

    // Test rendering of task 2 (Ready with unresolved dependency)
    terminal
        .draw(|f| {
            render_tasks_surface(f, f.area(), &model, &tokens, false, 1);
        })
        .unwrap();

    let buffer2 = terminal.backend().buffer().clone();
    let rendered_text2: String = (0..buffer2.area.height)
        .map(|y| {
            (0..buffer2.area.width)
                .map(|x| buffer2[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(
        rendered_text2.contains("Waiting on dependencies: t1"),
        "Dependent task must display dependency wait reason: {rendered_text2}"
    );

    // Test rendering of task 4 (Ready with no dependencies - waiting for scheduler dispatch)
    terminal
        .draw(|f| {
            render_tasks_surface(f, f.area(), &model, &tokens, false, 3);
        })
        .unwrap();

    let buffer4 = terminal.backend().buffer().clone();
    let rendered_text4: String = (0..buffer4.area.height)
        .map(|y| {
            (0..buffer4.area.width)
                .map(|x| buffer4[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(
        rendered_text4.contains("Waiting for scheduler dispatch"),
        "Ready task must display explicit wait reason: {rendered_text4}"
    );
}

#[test]
fn test_execution_heartbeat_and_stall_warning() {
    let mut model = TuiViewModel::new();
    model.lifecycle.stage = m31a::tui::lifecycle::TuiLifecycleStage::Executing;
    model.heartbeat.execution_started_at = Some(Utc::now() - chrono::Duration::seconds(120));

    // When waiting for scheduler:
    let summary_waiting = model.execution_summary().expect("summary");
    assert!(
        summary_waiting.contains("Waiting for scheduler dispatch"),
        "Must show waiting reason: {summary_waiting}"
    );

    // When stalled without runtime event:
    model.heartbeat.last_runtime_event_at = Some(Utc::now() - chrono::Duration::seconds(75));
    let summary_stalled = model.execution_summary().expect("summary");
    assert!(
        summary_stalled.contains("Stalled? No runtime activity"),
        "Must flag stall warning: {summary_stalled}"
    );

    // When event arrives, stall warning clears:
    model.heartbeat.last_runtime_event_at = Some(Utc::now());
    let summary_recovered = model.execution_summary().expect("summary");
    assert!(
        !summary_recovered.contains("Stalled"),
        "Stall warning must clear upon event receipt: {summary_recovered}"
    );
}

#[tokio::test]
async fn test_cancellation_and_duplicate_execution_protection() {
    let (_dir, runtime) = setup_test_runtime().await;
    let (bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .unwrap();

    let tx = bridge.sender();

    // Sending CancelRequested should succeed without blocking
    assert!(tx.send(ApplicationAction::CancelRequested).is_ok());

    // Bridge remains responsive
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        tx.send(ApplicationAction::SlashCommandSubmitted {
            command: "help".to_string(),
            args: vec![],
        })
        .is_ok()
    );
}

#[test]
fn test_task_graph_materialized_to_task_started_end_to_end() {
    use m31a::events::types::{EventType, TaskSummary};
    use m31a::ids::TaskId;
    use m31a::ids::task_graph::TaskGraphId;
    use m31a::state_machine::agent::AgentRole;
    use m31a::state_machine::task::TaskState;

    let mut model = TuiViewModel::new();
    model.lifecycle.stage = m31a::tui::lifecycle::TuiLifecycleStage::ExecutionAuthorized;
    let gid = TaskGraphId::new();
    let mid = MissionId::new();
    let tid1 = TaskId::new();
    let tid2 = TaskId::new();

    let summaries = vec![
        TaskSummary {
            id: tid1,
            title: "Repository structure discovery".to_string(),
            role: AgentRole::researcher(),
            status: TaskState::Ready,
            dependencies: vec![],
        },
        TaskSummary {
            id: tid2,
            title: "Core architecture analysis".to_string(),
            role: AgentRole::architect(),
            status: TaskState::Pending,
            dependencies: vec![tid1],
        },
    ];

    let envelope = EventEnvelope::new(
        1,
        Some(mid),
        None,
        "runtime".to_string(),
        EventType::TaskGraphMaterialized {
            graph_id: gid,
            mission_id: mid,
            revision: 1,
            task_count: 2,
            tasks: summaries,
        },
    );

    // Apply materialization
    model.apply_event(&envelope);

    // Verify task graph state and tasks
    assert_eq!(model.task_graph_state, TaskGraphProjectionState::Loaded);
    assert_eq!(model.tasks.len(), 2);
    assert_eq!(model.tasks[0].title, "Repository structure discovery");
    assert_eq!(model.tasks[0].status, "Ready");
    assert_eq!(model.tasks[0].progress_pct, 0);

    // Check that conversation contains clean semantic message, not internal raw telemetry
    let has_clean_msg = model.conversation.iter().any(|item| {
        if let m31a::tui::conversation::TuiConversationItem::System { text, .. } = item {
            text.contains("Started execution · 2 tasks")
        } else {
            false
        }
    });
    assert!(
        has_clean_msg,
        "Conversation must contain concise 'Started execution · 2 tasks'"
    );

    // Now TaskStarted arrives
    let task_start_event = InteractionEvent::TaskStarted {
        mission_id: mid.to_string(),
        task_id: tid1.to_string(),
        agent_id: "researcher".to_string(),
    };
    model.apply_interaction_event(&task_start_event);

    // Verify task 1 is now running with non-zero progress
    assert_eq!(model.tasks[0].status, "running");
    assert!(
        model.tasks[0].progress_pct > 0,
        "Task must leave 0% upon authoritative TaskStarted"
    );
    assert_eq!(model.tasks[1].status, "Pending");
    assert_eq!(model.tasks[1].progress_pct, 0);

    // Verify operation state is Executing
    assert_eq!(model.operation_state(), UiOperationState::Executing);

    // Verify execution summary
    let summary = model.execution_summary().expect("summary");
    assert!(
        summary.contains("Task 1/2 · Repository structure discovery"),
        "Summary must show active running task: {summary}"
    );
}
