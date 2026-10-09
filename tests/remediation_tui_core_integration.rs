//! Comprehensive Integration Test Suite: TUI/Core Integration Remediation (Problems 1–7)
//!
//! Validates:
//! 1. Approval workflow connectivity, rich context preservation, fail-closed resolution.
//! 2. Live propagation of agent, job, checkpoint, and recovery events.
//! 3. Candidate plan and task review artifacts with markdown and dependencies.
//! 4. Task execution details, DAG state preservation, and removal of fabricated progress/checkmarks.
//! 5. Preservation of real verification check IDs, tiers, and evidence (no synthetic IDs).
//! 6. Live budget snapshot updates and recovery telemetry.
//! 7. Bounded action channels and bounded event consumption per tick with backlog wakeup.

use std::sync::Arc;
use std::time::Duration;

use tempfile::tempdir;

use m31a::config::canonical::DEFAULT_TUI_MAX_EVENTS_PER_TICK;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::{EventType, TaskSummary};
use m31a::ids::{ApprovalRequestId, MissionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::kernel::plan::{CandidateTask, CandidateTaskKey};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{RevisionAuthorType, TaskRevision};
use m31a::runtime::AppRuntime;
use m31a::state_machine::TaskState;
use m31a::state_machine::agent::AgentRole;
use m31a::tui::app::TuiApplication;
use m31a::tui::approval::ApprovalDecision;
use m31a::tui::channel::{ActionSendError, TuiActionSender};
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::{TuiBudgetSnapshot, TuiVerificationCheck};
use m31a::tui::runtime_bridge::TuiRuntimeBridge;

async fn setup_runtime() -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("remediation_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let ws = dir.path().to_path_buf();
    let runtime = AppRuntime::from_pool_and_workspace(pool, ws, bus)
        .await
        .expect("runtime setup");
    (dir, Arc::new(runtime))
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 1: Approval Workflow Connectivity, Rich Context & Correct Resolution
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_1_approval_workflow_rich_context_and_fail_closed() {
    let (_dir, runtime) = setup_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut app = TuiApplication::new().with_bridge_tx(bridge.sender());
    let irx = bridge.take_event_receiver().expect("irx");
    app = app.with_interaction_rx(irx);

    // Initial poll
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.poll_updates();

    let request_id = ApprovalRequestId::new();
    let mission_id = MissionId::new();

    // 1. Emit kernel OperatorEscalationRequested with rich context
    let event = EventType::OperatorEscalationRequested {
        request_id: request_id.to_string(),
        mission_id,
        reason: "Execute database migration".to_string(),
        timeout_seconds: Some(120),
        tool_name: Some("db_migrate".to_string()),
        parameters_summary: Some("apply migration 004".to_string()),
        risk_tier: Some("Critical".to_string()),
        agent_role: Some("DatabaseArchitect".to_string()),
    };
    runtime
        .event_bus()
        .publish(EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "test".into(),
            event,
        ))
        .await
        .expect("publish");

    // 2. Poll app updates: verify rich context is preserved and modal automatically opens
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.approval_modal.is_open {
            break;
        }
    }

    assert!(
        app.approval_modal.is_open,
        "Approval modal must open automatically on incoming request"
    );
    let modal_req = app
        .approval_modal
        .current_request
        .as_ref()
        .expect("modal request");
    assert_eq!(modal_req.id, request_id.to_string());
    assert_eq!(modal_req.tool_name, "db_migrate");
    assert_eq!(modal_req.parameters_summary, "apply migration 004");
    assert_eq!(modal_req.risk_tier, "Critical");
    assert_eq!(modal_req.agent_role, "DatabaseArchitect");

    // 3. Fail-closed resolution: submitting an invalid request ID must emit an error and never fake success
    bridge
        .sender()
        .send(ApplicationAction::ApprovalDecision {
            request_id: "non-existent-uuid".to_string(),
            decision: ApprovalDecision::ApproveOnce,
        })
        .expect("send invalid approval");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.model.conversation.iter().any(|c| match c {
            TuiConversationItem::Error { message, .. } => message.contains("invalid request ID"),
            _ => false,
        }) {
            break;
        }
    }

    assert!(
        app.model.conversation.iter().any(|c| match c {
            TuiConversationItem::Error { message, .. } => message.contains("invalid request ID"),
            _ => false,
        }),
        "Submitting malformed approval ID must yield an explicit Error conversation item"
    );
    // Request must still be pending in model since invalid ID could not resolve it
    assert_eq!(app.model.approvals.len(), 1);
    assert!(app.approval_modal.is_open);

    // 4. Real coordinator approval resolution with persisted SQLite request
    let real_req_id = ApprovalRequestId::new();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Approval Test', 'Executing', ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(runtime.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO approval_requests (
            id, mission_id, tool_call_id, tool_or_capability, normalized_args_json,
            redacted_args_json, affected_resources, risk_classification, policy_hash,
            reason, resolution_state, created_at
        ) VALUES (?, ?, 'call_1', 'fs_write', '{}', '{}', '', 'High', 'hash', 'Test reason', 'pending', ?)
        "#,
    )
    .bind(real_req_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(runtime.pool())
    .await
    .unwrap();

    let event_real = EventType::OperatorEscalationRequested {
        request_id: real_req_id.to_string(),
        mission_id,
        reason: "Test real approval".to_string(),
        timeout_seconds: Some(120),
        tool_name: Some("fs_write".to_string()),
        parameters_summary: Some("path=src/test.rs".to_string()),
        risk_tier: Some("High".to_string()),
        agent_role: Some("implementer".to_string()),
    };
    runtime
        .event_bus()
        .publish(EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "test".into(),
            event_real,
        ))
        .await
        .expect("publish real");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app
            .model
            .approvals
            .iter()
            .any(|a| a.id == real_req_id.to_string())
        {
            break;
        }
    }
    assert!(
        app.model
            .approvals
            .iter()
            .any(|a| a.id == real_req_id.to_string()),
        "Real approval request must reach the view model"
    );

    // Now resolve the real request via bridge action
    bridge
        .sender()
        .send(ApplicationAction::ApprovalDecision {
            request_id: real_req_id.to_string(),
            decision: ApprovalDecision::ApproveOnce,
        })
        .expect("send valid decision");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if !app
            .model
            .approvals
            .iter()
            .any(|a| a.id == real_req_id.to_string())
        {
            break;
        }
    }
    assert!(
        !app.model
            .approvals
            .iter()
            .any(|a| a.id == real_req_id.to_string()),
        "Resolved approval must be removed from model approvals"
    );

    // Verify row state in SQLite is updated to 'approved'
    let state = sqlx::query_scalar::<_, String>(
        "SELECT resolution_state FROM approval_requests WHERE id = ?",
    )
    .bind(real_req_id.as_bytes().as_slice())
    .fetch_one(runtime.pool())
    .await
    .unwrap();
    assert_eq!(state, "approved");
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 2: Live Propagation of Agent, Job, Checkpoint, and Recovery Events
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_2_live_propagation_of_agent_job_checkpoint_recovery() {
    let mut app = TuiApplication::new();

    // 1. Agent lifecycle events
    let agent_id = "agent-arch-01".to_string();
    app.model
        .apply_interaction_event(&InteractionEvent::AgentSpawned {
            agent_id: agent_id.clone(),
            mission_id: "m-1".to_string(),
            role: "SystemArchitect".to_string(),
        });
    assert_eq!(app.model.agents.len(), 1);
    assert_eq!(app.model.agents[0].role, "SystemArchitect");
    assert_eq!(app.model.agents[0].state, "spawned");

    app.model
        .apply_interaction_event(&InteractionEvent::AgentStarted {
            agent_id: agent_id.clone(),
            mission_id: "m-1".to_string(),
            task_id: "t-100".to_string(),
        });
    assert_eq!(app.model.agents[0].state, "running");
    assert_eq!(app.model.agents[0].current_task.as_deref(), Some("t-100"));

    app.model
        .apply_interaction_event(&InteractionEvent::AgentStepCompleted {
            agent_id: agent_id.clone(),
            task_id: "t-100".to_string(),
            step_number: 1,
            steps_remaining: 3,
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("completed step 1"))
    );

    app.model
        .apply_interaction_event(&InteractionEvent::AgentCompleted {
            agent_id: agent_id.clone(),
            mission_id: "m-1".to_string(),
            summary: "Architecture design finished".to_string(),
        });
    assert_eq!(app.model.agents[0].state, "completed");
    assert!(app.model.agents[0].current_task.is_none());

    // 2. Job lifecycle events
    let job_id = "job-compile-01".to_string();
    app.model
        .apply_interaction_event(&InteractionEvent::JobStarted {
            job_id: job_id.clone(),
            mission_id: "m-1".to_string(),
            job_type: "cargo-build".to_string(),
        });
    assert_eq!(app.model.jobs.len(), 1);
    assert_eq!(app.model.jobs[0].id, job_id);
    assert_eq!(app.model.jobs[0].status, "running");

    app.model
        .apply_interaction_event(&InteractionEvent::JobCompleted {
            job_id: job_id.clone(),
            mission_id: "m-1".to_string(),
            artifacts: vec![],
        });
    assert_eq!(app.model.jobs[0].status, "completed");

    // 3. Checkpoint events
    app.model
        .apply_interaction_event(&InteractionEvent::CheckpointCreated {
            checkpoint_id: "chk-001".to_string(),
            mission_id: "m-1".to_string(),
            description: "Pre-migration checkpoint".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Checkpoint chk-001 created"))
    );

    app.model
        .apply_interaction_event(&InteractionEvent::CheckpointRestored {
            checkpoint_id: "chk-001".to_string(),
            mission_id: "m-1".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Checkpoint chk-001 restored"))
    );

    // 4. Recovery event
    app.model
        .apply_interaction_event(&InteractionEvent::RecoveryAttempted {
            recovery_id: "rec-01".to_string(),
            mission_id: "m-1".to_string(),
            strategy: "RollbackCheckpoint".to_string(),
            success: true,
        });
    assert_eq!(app.model.recovery_attempts.len(), 1);
    assert_eq!(app.model.recovery_attempts[0].attempt_id, "rec-01");
    assert_eq!(
        app.model.recovery_attempts[0].strategy,
        "RollbackCheckpoint"
    );
    assert_eq!(app.model.recovery_attempts[0].outcome, "succeeded");
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 3: Plan and Task Review Artifacts Expose Markdown and Dependencies
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_3_plan_and_task_review_artifacts_expose_markdown_and_dependencies() {
    let mut app = TuiApplication::new();

    let task_1 = CandidateTask {
        id: CandidateTaskKey::new("t-01"),
        objective: "Setup SQLite database schema".to_string(),
        role: AgentRole::architect(),
        depends_on: vec![],
        ..Default::default()
    };
    let task_2 = CandidateTask {
        id: CandidateTaskKey::new("t-02"),
        objective: "Implement query engine".to_string(),
        role: AgentRole::implementer(),
        depends_on: vec![CandidateTaskKey::new("t-01")],
        ..Default::default()
    };

    let plan_markdown = "# Execution Plan\n\n1. Setup DB\n2. Query engine".to_string();
    let tasks = vec![task_1.clone(), task_2.clone()];

    // Apply PlanForReview with markdown and tasks
    app.model
        .apply_interaction_event(&InteractionEvent::PlanForReview {
            session_id: "sess-01".to_string(),
            revision: 1,
            plan_id: "plan-core-1".to_string(),
            objective: "Database implementation".to_string(),
            task_count: 2,
            content_hash: Some("hash-abc-123".to_string()),
            plan_markdown: Some(plan_markdown.clone()),
            tasks: tasks.clone(),
        });

    let plan_card = app
        .model
        .conversation
        .iter()
        .find_map(|item| match item {
            TuiConversationItem::PlanReview {
                revision,
                plan_id,
                plan_markdown: pm,
                tasks: t,
                ..
            } => Some((*revision, plan_id.clone(), pm.clone(), t.clone())),
            _ => None,
        })
        .expect("PlanReview card must exist in conversation");

    assert_eq!(plan_card.0, 1);
    assert_eq!(plan_card.1, "plan-core-1");
    assert_eq!(plan_card.2.as_deref(), Some(plan_markdown.as_str()));
    assert_eq!(plan_card.3.len(), 2);
    assert_eq!(
        plan_card.3[1].depends_on,
        vec![CandidateTaskKey::new("t-01")]
    );

    // Apply TasksForReview with markdown and tasks
    let task_rev_markdown =
        "# Task Revision 2\n\n- Task 1: Setup DB\n- Task 2: Query Engine".to_string();
    app.model
        .apply_interaction_event(&InteractionEvent::TasksForReview {
            session_id: "sess-01".to_string(),
            plan_revision: 1,
            task_revision: 2,
            task_count: 2,
            content_hash: Some("hash-tasks-456".to_string()),
            task_markdown: Some(task_rev_markdown.clone()),
            tasks: tasks.clone(),
        });

    let task_card = app
        .model
        .conversation
        .iter()
        .find_map(|item| match item {
            TuiConversationItem::TaskReview {
                task_revision,
                task_markdown: tm,
                tasks: t,
                ..
            } => Some((*task_revision, tm.clone(), t.clone())),
            _ => None,
        })
        .expect("TaskReview card must exist in conversation");

    assert_eq!(task_card.0, 2);
    assert_eq!(task_card.1.as_deref(), Some(task_rev_markdown.as_str()));
    assert_eq!(task_card.2.len(), 2);
    assert_eq!(
        task_card.2[1].depends_on,
        vec![CandidateTaskKey::new("t-01")]
    );

    // Test TaskRevision::to_markdown() generates markdown
    let rev = TaskRevision::new(
        "sess-01",
        2,
        1,
        tasks.clone(),
        "planner",
        RevisionAuthorType::Model,
        None,
    );
    let md = rev.to_markdown();
    assert!(md.contains("Task Revision 2"));
    assert!(md.contains("Setup SQLite database schema"));
    assert!(md.contains("Implement query engine"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 4: Task Execution Details & Elimination of Fabricated Progress
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_4_task_execution_details_and_no_fabricated_progress() {
    let mut app = TuiApplication::new();
    let task_id = "task-alpha".to_string();
    let agent_id = "agent-beta".to_string();

    // 1. Task started does not fabricate 10% progress
    app.model
        .apply_interaction_event(&InteractionEvent::TaskStarted {
            mission_id: "m-01".to_string(),
            task_id: task_id.clone(),
            agent_id: agent_id.clone(),
        });

    assert_eq!(app.model.tasks.len(), 1);
    let task = &app.model.tasks[0];
    assert_eq!(task.id, task_id);
    assert_eq!(task.status, "running");
    assert_eq!(task.assigned_agent_id.as_deref(), Some(agent_id.as_str()));
    assert_eq!(
        task.progress_pct, 0,
        "TaskStarted must not fabricate initial progress"
    );

    // 2. Task failure records failure reason and increments retry count
    app.model
        .apply_interaction_event(&InteractionEvent::TaskFailed {
            mission_id: "m-01".to_string(),
            task_id: task_id.clone(),
            error: "Sandbox execution timeout".to_string(),
        });

    let task = &app.model.tasks[0];
    assert_eq!(task.status, "failed");
    assert_eq!(
        task.failure_reason.as_deref(),
        Some("Sandbox execution timeout")
    );
    assert_eq!(task.retry_count, 1);

    // 3. Merging TasksMaterialized does not clobber active state (status, failure reason, retry count)
    let tid = task_id.parse::<TaskId>().unwrap_or_else(|_| TaskId::new());
    let summaries = vec![TaskSummary {
        id: tid,
        title: "Task Alpha Updated Title".to_string(),
        role: AgentRole::implementer(),
        status: TaskState::Ready,
        dependencies: vec![],
    }];

    app.model
        .apply_interaction_event(&InteractionEvent::TasksMaterialized {
            graph_id: "graph-01".to_string(),
            revision: 2,
            tasks: summaries,
        });

    let task = &app.model.tasks[0];
    // Active state must NOT be clobbered back to 'ready'
    assert_eq!(
        task.status, "failed",
        "Materialization must not overwrite existing active/failed state"
    );
    assert_eq!(
        task.failure_reason.as_deref(),
        Some("Sandbox execution timeout")
    );
    assert_eq!(task.retry_count, 1);
    assert_eq!(task.assigned_agent_id.as_deref(), Some(agent_id.as_str()));

    // 4. Task completed records real execution result and 100% progress
    app.model
        .apply_interaction_event(&InteractionEvent::TaskCompleted {
            mission_id: "m-01".to_string(),
            task_id: task_id.clone(),
            result: "Built target/debug/m31a in 4.2s".to_string(),
        });

    let task = &app.model.tasks[0];
    assert_eq!(task.status, "completed");
    assert_eq!(task.progress_pct, 100);
    assert_eq!(
        task.execution_result.as_deref(),
        Some("Built target/debug/m31a in 4.2s")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 5: Synthetic Verification Records Replaced With Real Verification Evidence
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_5_verification_evidence_preserved_no_synthetic_ids() {
    let mut app = TuiApplication::new();
    let real_check_id = "chk_tier2_clippy_gate_99".to_string();

    let check = TuiVerificationCheck {
        check_id: real_check_id.clone(),
        mission_id: "m-01".to_string(),
        task_id: "t-01".to_string(),
        tier_num: 2,
        tier_name: "StaticAnalysis".to_string(),
        status: "passed".to_string(),
        command_or_tool: "cargo clippy -- -D warnings".to_string(),
        inputs_normalized: "src/".to_string(),
        evidence_artifact_id: Some("art-evidence-001".to_string()),
        summary: "Zero warnings detected across codebase".to_string(),
        failure_class: None,
        snapshot_hash: "sha256:fedcba9876543210".to_string(),
        created_at: chrono::Utc::now(),
    };

    app.model
        .apply_interaction_event(&InteractionEvent::VerificationPassed {
            summary: "StaticAnalysis tier passed".to_string(),
            verification_id: Some(real_check_id.clone()),
            check: Some(check.clone()),
        });

    assert_eq!(app.model.verification_checks.len(), 1);
    let recorded = &app.model.verification_checks[0];
    assert_eq!(
        recorded.check_id, real_check_id,
        "Check ID must be the real CheckId, not synthetic check-N"
    );
    assert_eq!(
        recorded.tier_name, "StaticAnalysis",
        "Tier must be StaticAnalysis, not hardcoded deterministic"
    );
    assert_eq!(recorded.tier_num, 2);
    assert_eq!(recorded.status, "passed");
    assert_eq!(
        recorded.evidence_artifact_id.as_deref(),
        Some("art-evidence-001")
    );
    assert_eq!(recorded.snapshot_hash, "sha256:fedcba9876543210");
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 6: Live Budget and Recovery Telemetry
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_6_live_budget_and_recovery_telemetry() {
    let mut app = TuiApplication::new();

    let snapshot = TuiBudgetSnapshot {
        scope: "global".to_string(),
        allocated_cents: 2500,
        consumed_cents: 600,
        remaining_cents: 1900,
        max_tokens: 100_000,
        consumed_tokens: 20_000,
        remaining_tokens: 80_000,
        max_tool_calls: 50,
        consumed_tool_calls: 12,
        is_exhausted: false,
        is_constrained: false,
    };

    app.model
        .apply_interaction_event(&InteractionEvent::BudgetSnapshotUpdated {
            snapshot: Box::new(snapshot),
        });

    let budget = &app.model.budget;
    assert_eq!(budget.allocated_cents, 2500);
    assert_eq!(budget.consumed_cents, 600);
    assert_eq!(budget.remaining_cents, 1900);
    assert_eq!(budget.max_tokens, 100_000);
    assert_eq!(budget.consumed_tokens, 20_000);
    assert_eq!(budget.remaining_tokens, 80_000);
    assert_eq!(budget.max_tool_calls, 50);
    assert_eq!(budget.consumed_tool_calls, 12);
}

// ─────────────────────────────────────────────────────────────────────────────
// Problem 7: Bounded TUI Channels and Bounded Event Consumption Per Tick
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_problem_7_bounded_channels_and_per_tick_backlog_wakeup() {
    // 1. Test Bounded TuiActionSender fails non-blocking when buffer capacity is reached
    let (tx, _rx) = tokio::sync::mpsc::channel::<ApplicationAction>(2);
    let sender = TuiActionSender::Bounded(tx);

    assert!(sender.try_send(ApplicationAction::CancelRequested).is_ok());
    assert!(sender.try_send(ApplicationAction::CancelRequested).is_ok());
    let err = sender.try_send(ApplicationAction::CancelRequested);
    assert_eq!(
        err,
        Err(ActionSendError::Full),
        "Bounded sender must reject on capacity overflow"
    );

    // 2. Test Bounded event consumption per tick and backlog wakeup
    let (etx, erx) = tokio::sync::mpsc::unbounded_channel::<InteractionEvent>();
    let mut app = TuiApplication::new().with_interaction_rx(erx);

    // Enqueue 200 events (more than DEFAULT_TUI_MAX_EVENTS_PER_TICK = 128)
    for i in 0..200 {
        etx.send(InteractionEvent::AssistantOutput {
            text: format!("Message {i}"),
        })
        .expect("send event");
    }

    assert!(!app.has_event_backlog);
    // Poll single tick
    app.poll_updates();

    // Must have processed exactly 128 events and flagged backlog
    assert!(
        app.has_event_backlog,
        "App must record event backlog when queue exceeded per-tick limit"
    );
    assert_eq!(
        app.poll_interval(),
        Duration::ZERO,
        "Immediate 0ms wakeup required when backlog is active"
    );
    assert_eq!(
        app.model.conversation.len(),
        DEFAULT_TUI_MAX_EVENTS_PER_TICK
    );

    // Next tick drains remaining 72 events and clears backlog
    app.poll_updates();
    assert!(
        !app.has_event_backlog,
        "Backlog must clear once queue is fully drained"
    );
    assert_eq!(app.model.conversation.len(), 200);
    assert_eq!(app.poll_interval(), Duration::from_millis(50));
}

// ─────────────────────────────────────────────────────────────────────────────
// Priority 0: Startup Deadline Timeout and Task Abort Hardening
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_startup_deadline_timeout_and_abort() {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let task_handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let abort_handle = task_handle.abort_handle();

    let mut app = TuiApplication::new().with_assembly_timeout(Duration::from_millis(50));
    app.begin_assembly_with_abort_handle(rx, abort_handle);

    assert!(app.model.runtime_status.is_startup());

    // Sleep past the 50ms deadline
    tokio::time::sleep(Duration::from_millis(60)).await;
    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    app.poll_runtime(&mut terminal).await;

    assert!(
        app.model.runtime_status.is_failed(),
        "App must transition to Failed state upon assembly timeout"
    );
    if let m31a::tui::model::RuntimeStartupState::Failed(err) = &app.model.runtime_status {
        assert!(
            err.contains("timed out"),
            "Error message must specify timeout: {err}"
        );
    }
    // Verify background task was aborted
    assert!(
        task_handle.await.unwrap_err().is_cancelled(),
        "Background task must be aborted when assembly timeout expires"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Priority 0: Approval Resolution Correctness & No False Success
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_approval_resolution_nonexistent_uuid_no_false_event() {
    let (_dir, runtime) = setup_runtime().await;
    let bus = runtime.event_bus();
    let mut sub = bus.subscribe(m31a::events::bus::EventFilter::all()).await;

    let nonexistent_id = ApprovalRequestId::new();
    let coordinator = runtime.approval_coordinator();

    let res = coordinator
        .resolve_request(
            nonexistent_id,
            m31a::policy::approval::ApprovalAction::AllowOnce,
            "operator",
        )
        .await;

    assert!(
        res.is_err(),
        "Nonexistent approval request must return error"
    );
    match res {
        Err(m31a::policy::approval::channel::ApprovalError::NotFound(_)) => {}
        other => panic!("Expected NotFound, got: {other:?}"),
    }

    // Ensure no ApprovalResolved event was published
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut got_resolved = false;
    use futures::StreamExt;
    while let Ok(Some(Ok(env))) = tokio::time::timeout(Duration::from_millis(10), sub.next()).await
    {
        if matches!(env.event_type, EventType::ApprovalResolved { .. }) {
            got_resolved = true;
            break;
        }
    }
    assert!(
        !got_resolved,
        "Must not publish ApprovalResolved on NotFound"
    );
}

#[tokio::test]
async fn test_approval_resolution_duplicate_decision_rejected() {
    let (_dir, runtime) = setup_runtime().await;
    let req_id = ApprovalRequestId::new();
    let mission_id = MissionId::new();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Test', 'Executing', ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(runtime.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO approval_requests (
            id, mission_id, tool_call_id, tool_or_capability, normalized_args_json,
            redacted_args_json, affected_resources, risk_classification, policy_hash,
            reason, resolution_state, created_at
        ) VALUES (?, ?, 'call_dup', 'fs_write', '{}', '{}', '', 'High', 'hash', 'Test reason', 'pending', ?)
        "#,
    )
    .bind(req_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(runtime.pool())
    .await
    .unwrap();

    let coordinator = runtime.approval_coordinator();

    // First resolution succeeds
    let res1 = coordinator
        .resolve_request(
            req_id,
            m31a::policy::approval::ApprovalAction::AllowOnce,
            "operator",
        )
        .await;
    assert!(res1.is_ok(), "First resolution must succeed");

    // Duplicate resolution must fail with AlreadyResolved
    let res2 = coordinator
        .resolve_request(
            req_id,
            m31a::policy::approval::ApprovalAction::AllowOnce,
            "operator",
        )
        .await;
    assert!(res2.is_err(), "Duplicate resolution must fail");
    match res2 {
        Err(m31a::policy::approval::channel::ApprovalError::AlreadyResolved(_)) => {}
        other => panic!("Expected AlreadyResolved, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_approval_modal_unsupported_edit_behavior() {
    let mut app = TuiApplication::new();
    let req = m31a::tui::model::TuiApprovalRequest {
        id: "req-123".to_string(),
        tool_name: "fs_write".to_string(),
        agent_role: "developer".to_string(),
        justification: "edit file".to_string(),
        parameters_summary: "path=foo.rs".to_string(),
        risk_tier: "High".to_string(),
        timestamp: chrono::Utc::now(),
    };
    app.approval_modal.open(req);
    assert!(app.approval_modal.is_open);

    // Operator presses 'e'
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let event = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    let action = app.approval_modal.handle_key(event);

    assert!(
        action.is_none(),
        "Edit key must not return an ApplicationAction"
    );
    assert!(
        app.approval_modal.is_open,
        "Approval modal must remain open when Edit is pressed"
    );
    assert!(
        app.approval_modal.notice.is_some(),
        "Notice must be set explaining Edit is unsupported"
    );
    assert!(
        app.approval_modal
            .notice
            .as_ref()
            .unwrap()
            .contains("unsupported"),
        "Notice must state that direct parameter editing is unsupported"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Priority 1: Stale Plan & Task Revision Rejection
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_stale_plan_and_task_revision_rejection() {
    let dir = tempdir().expect("tempdir");
    let pool = initialize_database(&dir.path().join("rev_test.db"))
        .await
        .expect("db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let coordinator = m31a::planning::review::PreExecutionCoordinator::deterministic_test(
        pool.clone(),
        Some(bus.clone()),
    )
    .with_workspace_root(dir.path().to_path_buf());

    let sess_repo = m31a::interaction::session::SqliteSessionRepository::new(pool.clone());
    let session = sess_repo
        .create_session(dir.path())
        .await
        .expect("create session");
    let sid = session.id.to_string();

    let resp = coordinator
        .init_intent(&sid, "Build a microservice API in Rust", "operator")
        .await
        .expect("init intent");

    assert!(matches!(
        resp,
        m31a::planning::review::PreExecutionResponse::PlanForReview { .. }
    ));

    // Attempt accept with stale revision 99 (real revision is 1)
    let stale_act = ApplicationAction::PlanAcceptRequested {
        session_id: Some(sid.to_string()),
        revision: Some(99),
        content_hash: None,
    };
    let res = coordinator.handle_action(stale_act, "operator").await;
    assert!(res.is_err(), "Stale revision must be rejected");
    assert!(res.unwrap_err().contains("Stale plan acceptance"));

    // Accept valid plan
    let valid_plan_act = ApplicationAction::PlanAcceptRequested {
        session_id: Some(sid.to_string()),
        revision: Some(1),
        content_hash: None,
    };
    let task_resp = coordinator
        .handle_action(valid_plan_act, "operator")
        .await
        .expect("accept valid plan");

    assert!(matches!(
        task_resp,
        m31a::planning::review::PreExecutionResponse::TasksForReview { .. }
    ));

    // Attempt accept tasks with stale revision 99
    let stale_task_act = ApplicationAction::TasksAcceptRequested {
        session_id: Some(sid.to_string()),
        revision: Some(99),
        content_hash: None,
    };
    let res_task = coordinator.handle_action(stale_task_act, "operator").await;
    assert!(res_task.is_err(), "Stale task revision must be rejected");
    assert!(res_task.unwrap_err().contains("Stale task acceptance"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Priority 1: Scheduler & Resource Lifecycle Events View Model Projection
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_scheduler_and_resource_lifecycle_events_projection() {
    let mut app = TuiApplication::new();

    // 1. TaskBlocked
    app.model
        .apply_interaction_event(&InteractionEvent::TaskBlocked {
            task_id: "task-101".to_string(),
            mission_id: "m-1".to_string(),
            reason: "Waiting for dependency".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Task task-101 blocked"))
    );

    // 2. TaskUnblocked
    app.model
        .apply_interaction_event(&InteractionEvent::TaskUnblocked {
            task_id: "task-101".to_string(),
            mission_id: "m-1".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Task task-101 unblocked"))
    );

    // 3. TaskRetryScheduled
    app.model
        .apply_interaction_event(&InteractionEvent::TaskRetryScheduled {
            task_id: "task-101".to_string(),
            mission_id: "m-1".to_string(),
            attempt: 2,
            retry_delay_ms: 500,
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("retry #2"))
    );

    // 4. Resource events
    app.model
        .apply_interaction_event(&InteractionEvent::ResourceLeased {
            lease_id: "lease-1".to_string(),
            task_id: "task-101".to_string(),
            resource_key: "fs_workspace".to_string(),
            lock_mode: "Exclusive".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Resource fs_workspace leased"))
    );

    app.model
        .apply_interaction_event(&InteractionEvent::ResourceReleased {
            lease_id: "lease-1".to_string(),
            task_id: "task-101".to_string(),
            resource_key: "fs_workspace".to_string(),
        });
    assert!(
        app.model
            .logs
            .iter()
            .any(|l| l.message.contains("Resource fs_workspace released"))
    );
}
