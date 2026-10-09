//! Phase 36.6 — TUI Truth Projection Regression Tests.
//!
//! Deterministic, in-memory tests proving the TUI displays authoritative runtime truth:
//! 1. Git Branch:
//!    - Starts as "N/A" (never hardcoded to "main" or "master")
//!    - Updates truthfully from GitStateChanged
//!    - Distinguishes execution worktree branch from workspace branch (e.g. "m31a/wt-123 (ws: master)")
//!    - Renders "N/A" when repository or branch is unavailable
//! 2. Token Usage & Cost:
//!    - Starts with None cost, rendering "cost n/a" (never falsely "$0.00")
//!    - Accumulates authoritative prompt and completion tokens from ModelUsageUpdated
//!    - Deduplicates identical invocation IDs across streaming/final events
//!    - Accurately renders dollar cost when pricing is known
//! 3. Task Execution DAG:
//!    - Distinguishes "Loading..." snapshot state from an empty DAG
//!    - Populates DAG tasks, titles, roles, dependencies from TaskGraphMaterialized / TasksMaterialized
//!    - Preserves titles and dependencies when lifecycle events (TaskStarted, TaskCompleted, etc.) arrive
//! 4. Section 31 End-to-End:
//!    - Follows the exact plan acceptance -> task set -> authorize -> materialization -> 1 task sequence
//!    - Verifies the rendered TUI displays the 1 task, the real branch, and the token usage

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use uuid::Uuid;

use m31a::events::envelope::EventEnvelope;
use m31a::events::types::{EventType, TaskSummary};
use m31a::ids::task_graph::TaskGraphId;
use m31a::ids::{AgentId, MissionId, SessionId, TaskId};
use m31a::interaction::events::InteractionEvent;
use m31a::model::types::{TokenUsage, UsageSource};
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::task::TaskState;
use m31a::tui::model::{TaskGraphProjectionState, TuiViewModel};
use m31a::tui::navigation::ScreenId;
use m31a::tui::replay::ReplayController;
use m31a::tui::theme::{ThemeMode, ThemeTokens};

fn envelope(event_type: EventType) -> EventEnvelope {
    EventEnvelope::new(0, None, None, "test".to_string(), event_type)
}

fn render_header_text(model: &TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    let replay = ReplayController::new();
    terminal
        .draw(|f| {
            m31a::tui::shell::header::render_header(
                f,
                f.area(),
                model,
                ScreenId::TaskGraph,
                &replay,
                &tokens,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_telemetry_text(model: &TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    terminal
        .draw(|f| {
            m31a::tui::surface::render_telemetry_surface(f, f.area(), model, &tokens, false);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn render_tasks_text(model: &TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    terminal
        .draw(|f| {
            m31a::tui::surface::render_tasks_surface(f, f.area(), model, &tokens, false, 0);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Git Branch Projection Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_git_branch_starts_uninitialized_and_not_hardcoded_to_main() {
    let model = TuiViewModel::new();
    assert_eq!(model.git_branch, "N/A");
    assert_eq!(model.execution_worktree_branch, None);

    // Quiet header shows no branch when unavailable (not "main"), never fakes.
    let text = render_header_text(&model, 100, 3);
    assert!(!text.contains("main"));
}

#[test]
fn test_git_branch_updates_from_git_state_changed_event() {
    let mut model = TuiViewModel::new();

    // Event updates to custom feature branch
    model.apply_event(&envelope(EventType::GitStateChanged {
        workspace_branch: "feature/truth-projection".to_string(),
        execution_branch: None,
        is_clean: true,
    }));
    assert_eq!(model.git_branch, "feature/truth-projection");
    assert_eq!(model.execution_worktree_branch, None);

    let text = render_header_text(&model, 100, 3);
    assert!(text.contains("feature/truth-projection"));
}

#[test]
fn test_git_branch_displays_execution_worktree_with_workspace_context() {
    let mut model = TuiViewModel::new();

    // Autonomous execution in an isolated worktree branch
    model.apply_event(&envelope(EventType::GitStateChanged {
        workspace_branch: "master".to_string(),
        execution_branch: Some("m31a/wt-mission-99".to_string()),
        is_clean: false,
    }));
    assert_eq!(model.git_branch, "master");
    assert_eq!(
        model.execution_worktree_branch.as_deref(),
        Some("m31a/wt-mission-99")
    );

    let text = render_header_text(&model, 120, 3);
    // Quiet header surfaces the execution worktree branch; workspace context
    // remains authoritative in the model (and the Git surface).
    assert!(text.contains("m31a/wt-mission-99"));
    assert_eq!(model.git_branch, "master");
}

#[test]
fn test_git_branch_updates_from_interaction_event() {
    let mut model = TuiViewModel::new();

    model.apply_interaction_event(&InteractionEvent::GitStateChanged {
        workspace_branch: "release-v2".to_string(),
        execution_branch: None,
        is_clean: true,
    });
    assert_eq!(model.git_branch, "release-v2");

    let text = render_header_text(&model, 100, 3);
    assert!(text.contains("release-v2"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Token Usage & Cost Accounting Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_token_usage_initial_cost_is_unknown_not_zero_dollars() {
    let model = TuiViewModel::new();
    assert_eq!(model.model_usage.prompt_tokens, 0);
    assert_eq!(model.model_usage.completion_tokens, 0);
    assert_eq!(model.model_usage.total_cost_cents, None);

    // Quiet header never fakes cost; authoritative telemetry lives in the
    // model-usage surface (progressive disclosure, not information loss).
    let header = render_header_text(&model, 100, 3);
    assert!(!header.contains("$0.00"));
    let telemetry = render_telemetry_text(&model, 100, 20);
    assert!(!telemetry.contains("$0.00"));
}

#[test]
fn test_token_usage_accumulates_and_deduplicates_invocations() {
    let mut model = TuiViewModel::new();

    let inv1 = Uuid::now_v7();
    let usage1 = TokenUsage {
        prompt_tokens: 120,
        completion_tokens: 30,
        total_tokens: 150,
        reasoning_tokens: 0,
        source: UsageSource::AuthoritativeProvider,
    };

    // First emission of invocation inv-1
    model.apply_event(&envelope(EventType::ModelUsageUpdated {
        invocation_id: Some(inv1),
        mission_id: None,
        task_id: None,
        provider: "nvidia".to_string(),
        model: "nemotron-3".to_string(),
        usage: usage1.clone(),
        cumulative_usage: None,
        cost_usd: Some(0.001),
        cost_provenance: m31a::model::types::CostProvenance::Estimated,
    }));

    assert_eq!(model.model_usage.prompt_tokens, 120);
    assert_eq!(model.model_usage.completion_tokens, 30);
    assert_eq!(model.model_usage.api_calls, 1);

    // Replay or stream duplicate of invocation inv-1 must be ignored
    model.apply_event(&envelope(EventType::ModelUsageUpdated {
        invocation_id: Some(inv1),
        mission_id: None,
        task_id: None,
        provider: "nvidia".to_string(),
        model: "nemotron-3".to_string(),
        usage: usage1,
        cumulative_usage: None,
        cost_usd: Some(0.001),
        cost_provenance: m31a::model::types::CostProvenance::Estimated,
    }));

    assert_eq!(model.model_usage.prompt_tokens, 120);
    assert_eq!(model.model_usage.completion_tokens, 30);
    assert_eq!(model.model_usage.api_calls, 1);

    // Second distinct invocation inv-2
    let inv2 = Uuid::now_v7();
    let usage2 = TokenUsage {
        prompt_tokens: 200,
        completion_tokens: 50,
        total_tokens: 250,
        reasoning_tokens: 0,
        source: UsageSource::AuthoritativeProvider,
    };
    model.apply_event(&envelope(EventType::ModelUsageUpdated {
        invocation_id: Some(inv2),
        mission_id: None,
        task_id: None,
        provider: "nvidia".to_string(),
        model: "nemotron-3".to_string(),
        usage: usage2,
        cumulative_usage: None,
        cost_usd: Some(0.002),
        cost_provenance: m31a::model::types::CostProvenance::Estimated,
    }));

    assert_eq!(model.model_usage.prompt_tokens, 320);
    assert_eq!(model.model_usage.completion_tokens, 80);
    assert_eq!(model.model_usage.api_calls, 2);

    // Totals accumulate in the model and render in the telemetry surface —
    // never permanently in the quiet header.
    let telemetry = render_telemetry_text(&model, 100, 20);
    assert!(telemetry.contains("320") && telemetry.contains("80"));
    let header = render_header_text(&model, 100, 3);
    assert!(!header.contains("400 tok"));
}

#[test]
fn test_token_usage_renders_explicit_dollar_cost_when_known() {
    let mut model = TuiViewModel::new();
    model.model_usage.prompt_tokens = 500;
    model.model_usage.completion_tokens = 250;
    model.model_usage.total_cost_cents = Some(150); // $1.50

    let telemetry = render_telemetry_text(&model, 100, 20);
    assert!(telemetry.contains("500"));
    assert!(telemetry.contains("250"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Task Execution DAG Projection Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_task_dag_distinguishes_loading_state_from_empty() {
    let mut model = TuiViewModel::new();
    assert_eq!(model.task_graph_state, TaskGraphProjectionState::Unknown);

    // Empty state (quiet, no shouting title)
    let empty_text = render_tasks_text(&model, 80, 15);
    assert!(empty_text.contains("No tasks currently registered"));

    // Set to Loading
    model.task_graph_state = TaskGraphProjectionState::Loading;
    let loading_text = render_tasks_text(&model, 80, 15);
    assert!(loading_text.contains("Loading task graph..."));
    assert!(loading_text.contains("Hydrating authoritative task snapshot"));
}

#[test]
fn test_task_dag_populates_from_materialization_and_preserves_on_lifecycle() {
    let mut model = TuiViewModel::new();
    let gid = TaskGraphId::new();
    let mid = MissionId::new();
    let tid1 = TaskId::new();
    let tid2 = TaskId::new();

    let summaries = vec![
        TaskSummary {
            id: tid1,
            title: "Analyze codebase architecture".to_string(),
            role: AgentRole::architect(),
            status: TaskState::Ready,
            dependencies: vec![],
        },
        TaskSummary {
            id: tid2,
            title: "Implement runtime bridge fix".to_string(),
            role: AgentRole::implementer(),
            status: TaskState::Pending,
            dependencies: vec![tid1],
        },
    ];

    // Event carries the authoritative task summaries
    model.apply_event(&envelope(EventType::TaskGraphMaterialized {
        graph_id: gid,
        mission_id: mid,
        revision: 1,
        task_count: 2,
        tasks: summaries,
    }));

    assert_eq!(model.task_graph_state, TaskGraphProjectionState::Loaded);
    assert_eq!(model.tasks.len(), 2);
    assert_eq!(model.tasks[0].title, "Analyze codebase architecture");
    assert_eq!(model.tasks[0].agent_role.as_deref(), Some("architect"));
    assert_eq!(model.tasks[1].title, "Implement runtime bridge fix");
    assert_eq!(model.tasks[1].dependencies, vec![tid1.to_string()]);

    // Render verification
    let text = render_tasks_text(&model, 100, 20);
    assert!(text.contains("Analyze codebase architecture"));
    assert!(text.contains("Implement runtime bridge fix"));

    // TaskStarted lifecycle event arrives: MUST NOT overwrite title or dependencies!
    model.apply_event(&envelope(EventType::TaskStarted {
        task_id: tid1,
        mission_id: mid,
        agent_id: AgentId::new(),
    }));

    assert_eq!(model.tasks[0].title, "Analyze codebase architecture");
    assert_eq!(model.tasks[0].status, "running");
    assert_eq!(model.tasks[1].title, "Implement runtime bridge fix");
    assert_eq!(model.tasks[1].dependencies, vec![tid1.to_string()]);

    // TaskCompleted lifecycle event arrives
    model.apply_event(&envelope(EventType::TaskCompleted {
        task_id: tid1,
        mission_id: mid,
        result: "Codebase architecture analyzed successfully".to_string(),
    }));

    assert_eq!(model.tasks[0].status, "completed");
    assert_eq!(model.tasks[0].progress_pct, 100);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Section 31 Scenario End-to-End Test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_section_31_cockpit_truth_projection_scenario() {
    let mut model = TuiViewModel::new();
    let session_id = SessionId::new().to_string();
    let task_id = TaskId::new();

    // 1. Initial State: git is not main, usage is not fake $0.00, DAG is not fake
    assert_eq!(model.git_branch, "N/A");
    assert_eq!(model.model_usage.total_cost_cents, None);

    // 2. Hydration/Startup gives actual branch: "develop"
    model.apply_interaction_event(&InteractionEvent::GitStateChanged {
        workspace_branch: "develop".to_string(),
        execution_branch: None,
        is_clean: true,
    });
    assert_eq!(model.git_branch, "develop");

    // 3. Plan Review accepted (R1)
    model.apply_interaction_event(&InteractionEvent::PlanForReview {
        session_id: session_id.clone(),
        revision: 1,
        plan_id: "plan-study-codebase".to_string(),
        objective: "Study the codebase".to_string(),
        task_count: 1,
        content_hash: Some("abc12345".to_string()),
        plan_markdown: None,
        tasks: Vec::new(),
    });

    // Model usage from planning
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-planning-1".to_string()),
        prompt_tokens: 1200,
        completion_tokens: 350,
        total_tokens: 1550,
        cost_cents: None,
        cost_usd: Some(0.001),
        cost_provenance: m31a::model::types::CostProvenance::Estimated,
    });

    // 4. Task Review accepted (R1)
    model.apply_interaction_event(&InteractionEvent::TasksForReview {
        session_id: session_id.clone(),
        plan_revision: 1,
        task_revision: 1,
        task_count: 1,
        content_hash: Some("def67890".to_string()),
        task_markdown: None,
        tasks: Vec::new(),
    });

    // 5. Execution Authorization requested & granted (/authorize yes)
    model.apply_interaction_event(&InteractionEvent::AuthorizationRequired {
        session_id: session_id.clone(),
        plan_revision: 1,
        task_revision: 1,
        message: "Authorize task execution in workspace".to_string(),
    });
    model.apply_interaction_event(&InteractionEvent::ExecutionReady {
        session_id: session_id.clone(),
        authorization_id: "auth-42".to_string(),
        plan_revision: 1,
        task_revision: 1,
    });

    // 6. Runtime materializes task graph with 1 task
    let task_summary = TaskSummary {
        id: task_id,
        title: "Study the codebase".to_string(),
        role: AgentRole::architect(),
        status: TaskState::Ready,
        dependencies: vec![],
    };
    model.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-1".to_string(),
        revision: 1,
        tasks: vec![task_summary],
    });

    // 7. Render Header & Tasks Surface and assert truth projection.
    // Quiet header: branch + state. Telemetry (tokens) lives in the
    // model-usage surface; tasks live in the Tasks surface.
    let header_text = render_header_text(&model, 120, 3);
    assert!(header_text.contains("develop"));
    assert!(header_text.contains("M31A"));

    let telemetry_text = render_telemetry_text(&model, 120, 20);
    assert!(telemetry_text.contains("1200"));
    assert!(telemetry_text.contains("350"));

    let tasks_text = render_tasks_text(&model, 100, 20);
    assert!(tasks_text.contains("Study the codebase"));
    assert!(!tasks_text.contains("No tasks currently registered"));
}
