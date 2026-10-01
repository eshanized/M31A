//! Phase 39 — Evidence Plane, Live Runtime Observability & Product Truth Tests.
//!
//! Verifies:
//! - P0 Real Artifact Explorer: backed by canonical ArtifactService, truthful empty state, no synthetic state.
//! - P0 Real Verification Experience: backed by canonical EvidenceCompletionGate, truthful empty state, non-inferral of success.
//! - P0 Traceability View: end-to-end 7-step chain without fabricated links.
//! - P1 Mission Timeline: canonical event stream ordering by timestamp and sequence.
//! - P1 Failure & Recovery Experience: canonical RecoveryAttemptRecord presentation.
//! - P1 Live Budget Experience: atomic BudgetEnforcer snapshot projection.
//! - P2 Live Profile Matrix: canonical RoleRegistry / AgentProfile projection.
//! - P2 Live Skills Registry: canonical SkillDiscovery projection.
//! - P2 Universal Command Palette Truth: truthful availability and disabled reasons.
//! - Invariant: Zero render-time SQLite database I/O (`sqlite_access_counter == 0`).

use chrono::Utc;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::agent::registry::RoleRegistry;
use m31a::budget::BudgetEnforcer;
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::{ArtifactId, MissionId, SessionId, TaskId};
use m31a::kernel::seams::recovery::FailureClassification;
use m31a::persistence::artifacts::{ArtifactProvenance, ArtifactRecord, ArtifactStatus};
use m31a::recovery::budget::RecoveryAttemptRecord;
use m31a::state::budget::ResourceBudget;
use m31a::state_machine::agent::AgentRole;
use m31a::tui::lifecycle::TuiLifecycleStage;
use m31a::tui::model::{
    TuiArtifactSnapshot, TuiRecoverySnapshot, TuiSkillSnapshot, TuiVerificationCheck, TuiViewModel,
};
use m31a::tui::palette_v2::check_command_availability;
use m31a::tui::theme::{ThemeMode, ThemeTokens};
use m31a::verification::types::{CheckStatus, CheckTier, VerificationCheck};

// ─── Helpers ────────────────────────────────────────────────────────────────

fn create_test_model() -> TuiViewModel {
    let mut model = TuiViewModel::new();
    model.mission_id = Some("mission-001".to_string());
    model.session_id = Some("session-001".to_string());
    model
}

fn envelope(sequence: u64, event_type: EventType) -> EventEnvelope {
    EventEnvelope::new(sequence, None, None, "test-source".to_string(), event_type)
}

// ─── P0: Real Artifact Explorer ─────────────────────────────────────────────

#[test]
fn test_artifact_explorer_truthful_empty_state() {
    let model = create_test_model();
    assert!(model.artifacts.is_empty());

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
            crate_adapters::render_artifacts_for_test(f, f.area(), &model, &tokens);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        content.contains("No artifacts recorded in runtime store"),
        "Empty artifact explorer must render truthful absence message"
    );
    assert_eq!(
        model.sqlite_render_access_count(),
        0,
        "Rendering artifact view must perform zero SQLite queries"
    );
}

#[test]
fn test_artifact_explorer_canonical_records() {
    let mut model = create_test_model();

    let record = ArtifactRecord {
        id: ArtifactId::new(),
        name: "output_contract.json".to_string(),
        logical_path: std::path::PathBuf::from("contracts/output_contract.json"),
        content_hash: "sha256-a1b2c3d4e5f6".to_string(),
        size_bytes: 4096,
        extension: "json".to_string(),
        version: 1,
        status: ArtifactStatus::Valid,
        provenance: ArtifactProvenance {
            mission_id: Some(MissionId::new()),
            task_id: Some(TaskId::new()),
            producer_role: Some("implementer".to_string()),
            ..Default::default()
        },
        created_at: Utc::now(),
    };

    model.load_artifacts(vec![TuiArtifactSnapshot::from(&record)]);
    assert_eq!(model.artifacts.len(), 1);
    assert_eq!(model.artifacts[0].name, "output_contract.json");
    assert_eq!(model.artifacts[0].size_bytes, 4096);
    assert_eq!(model.artifacts[0].version, 1);

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
            crate_adapters::render_artifacts_for_test(f, f.area(), &model, &tokens);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(content.contains("output_contract.json"));
    assert!(content.contains("4.0 KB"));
}

// ─── P0: Real Verification Experience ────────────────────────────────────────

#[test]
fn test_verification_suite_truthful_empty_and_non_inferral() {
    let model = create_test_model();
    assert!(model.verification_checks.is_empty());
    assert_eq!(model.verification_summary.total_checks, 0);

    // Strict invariant: no verification records != passed
    assert_ne!(
        model.verification_summary.overall_status, "passed",
        "Absence of verification records must NEVER infer passed"
    );

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
            crate_adapters::render_verification_for_test(f, f.area(), &model, &tokens);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(
        content.contains("No verification checks recorded")
            || content.contains("not run / pending"),
        "Verification suite must explicitly indicate not run / pending"
    );
}

#[test]
fn test_verification_suite_canonical_checks_and_status_distinctions() {
    let mut model = create_test_model();

    let check1 = VerificationCheck {
        check_id: uuid::Uuid::now_v7().into(),
        mission_id: MissionId::new(),
        task_id: TaskId::new(),
        tier: CheckTier::Tests,
        status: CheckStatus::Passed,
        command_or_tool: "cargo test --lib".to_string(),
        inputs_normalized: "{}".to_string(),
        evidence_artifact_id: Some(ArtifactId::new()),
        summary: "12 unit tests passed".to_string(),
        failure_class: None,
        snapshot_hash: "hash123".to_string(),
        created_at: Utc::now(),
    };

    let check2 = VerificationCheck {
        check_id: uuid::Uuid::now_v7().into(),
        mission_id: MissionId::new(),
        task_id: TaskId::new(),
        tier: CheckTier::StaticAnalysis,
        status: CheckStatus::Failed,
        command_or_tool: "cargo audit".to_string(),
        inputs_normalized: "{}".to_string(),
        evidence_artifact_id: None,
        summary: "1 vulnerability detected in dependency".to_string(),
        failure_class: Some("VulnerabilityFound".to_string()),
        snapshot_hash: "hash456".to_string(),
        created_at: Utc::now(),
    };

    let snaps = vec![
        TuiVerificationCheck::from(&check1),
        TuiVerificationCheck::from(&check2),
    ];
    model.load_verification_checks(snaps);

    assert_eq!(model.verification_checks.len(), 2);
    assert_eq!(model.verification_summary.total_checks, 2);
    assert_eq!(model.verification_summary.passed_count, 1);
    assert_eq!(model.verification_summary.failed_count, 1);
    assert_eq!(model.verification_summary.overall_status, "failed");

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
            crate_adapters::render_verification_for_test(f, f.area(), &model, &tokens);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let content = buffer
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();

    assert!(content.contains("cargo test --lib"));
    assert!(content.contains("cargo audit"));
    assert!(content.contains("passed") || content.contains("PASSED"));
    assert!(content.contains("failed") || content.contains("FAILED"));
}

// ─── P0: Real Traceability View ─────────────────────────────────────────────

#[test]
fn test_traceability_7_step_chain_with_explicit_missing_links() {
    let mut model = create_test_model();
    model.rebuild_traceability();

    // Initial state: with no plan or tasks, traceability chain is empty (no fake links)
    assert!(model.traceability.is_empty());

    // Update with real lifecycle event
    let session = SessionId::new().to_string();
    model.apply_event(&envelope(
        1,
        EventType::PlanAccepted {
            session_id: session.clone(),
            plan_id: "plan-canonical".to_string(),
            revision: 3,
        },
    ));

    assert_eq!(model.traceability.len(), 1);
    let chain = &model.traceability[0];
    assert_eq!(chain.plan_revision, Some(3));
    assert!(chain.user_decision.is_some());
    // Unrecorded links remain None (never fabricated)
    assert!(chain.task_revision.is_none());
    assert!(chain.task_id.is_none());
    assert!(chain.execution_id.is_none());
    assert!(chain.source_change.is_none());
    assert!(chain.verification_id.is_none());
    assert!(chain.evidence_artifact_id.is_none());
}

// ─── P1: Mission Timeline ───────────────────────────────────────────────────

#[test]
fn test_mission_timeline_event_ordering_and_preservation() {
    let mut model = create_test_model();
    let session = SessionId::new().to_string();

    model.apply_event(&envelope(
        1,
        EventType::MissionStarted {
            mission_id: MissionId::new(),
            objective: "Build observability plane".to_string(),
        },
    ));

    model.apply_event(&envelope(
        2,
        EventType::PlanReviewRequired {
            session_id: session.clone(),
            plan_id: "plan-1".to_string(),
            revision: 1,
        },
    ));

    model.apply_event(&envelope(
        3,
        EventType::ArtifactCreated {
            artifact_id: ArtifactId::new(),
            mission_id: MissionId::new(),
            path: "evidence/summary.json".to_string(),
            content_type: "application/json".to_string(),
        },
    ));

    assert_eq!(model.timeline.len(), 3);
    assert_eq!(model.timeline[0].sequence, 1);
    assert_eq!(model.timeline[1].sequence, 2);
    assert_eq!(model.timeline[2].sequence, 3);
    assert_eq!(model.timeline[0].category, "lifecycle");
    assert_eq!(model.timeline[1].category, "plan");
    assert_eq!(model.timeline[2].category, "artifact");
}

// ─── P1: Failure & Recovery Experience ──────────────────────────────────────

#[test]
fn test_failure_recovery_canonical_attempts() {
    let mut model = create_test_model();

    let attempt = RecoveryAttemptRecord {
        mission_id: MissionId::new(),
        task_id: TaskId::new(),
        strategy: "RevertAndRetry".to_string(),
        attempt_number: 1,
        failure_class: FailureClassification::Compilation,
        result: "Success".to_string(),
        backoff_delay_ms: 500,
        budget_consumed: 1,
        remaining_class_budget: 2,
        remaining_overall_budget: 5,
        action_taken: "Reverted task git worktree and retried with refined prompt".to_string(),
        mutation_fingerprint: None,
        semantic_signature: None,
    };

    model.load_recovery_attempts(vec![TuiRecoverySnapshot::from(&attempt)]);
    assert_eq!(model.recovery_attempts.len(), 1);
    assert_eq!(model.recovery_attempts[0].strategy, "RevertAndRetry");
    assert_eq!(model.recovery_attempts[0].attempt_number, 1);
    assert_eq!(model.recovery_attempts[0].outcome, "Success");
    assert_eq!(model.recovery_attempts[0].backoff_delay_ms, 500);
}

// ─── P1: Live Budget Experience ─────────────────────────────────────────────

#[test]
fn test_live_budget_atomic_snapshot_projection() {
    let mut model = create_test_model();

    let limits = ResourceBudget {
        max_agent_steps: Some(50),
        max_tokens: Some(100_000),
        max_cost_usd: Some(10.0),
        ..Default::default()
    };
    let enforcer = BudgetEnforcer::new(limits);
    let mut snap = enforcer.snapshot();
    snap.tokens_consumed = 25_000;
    snap.cost_consumed_usd = 2.50;
    snap.agent_steps_consumed = 12;

    model.update_budget((&snap).into());

    assert_eq!(model.budget.max_tokens, 100_000);
    assert_eq!(model.budget.consumed_tokens, 25_000);
    assert_eq!(model.budget.remaining_tokens, 75_000);
    assert_eq!(model.budget.allocated_cents, 1000);
    assert_eq!(model.budget.consumed_cents, 250);
    assert_eq!(model.budget.remaining_cents, 750);
    assert_eq!(model.budget.max_tool_calls, 50);
    assert_eq!(model.budget.consumed_tool_calls, 12);
    assert!(!model.budget.is_exhausted);
    assert!(!model.budget.is_constrained);
}

// ─── P2: Live Profile Matrix ────────────────────────────────────────────────

#[test]
fn test_profile_matrix_canonical_builtins() {
    let registry = RoleRegistry::with_builtins();
    let all_ids = registry.all_ids();

    assert!(all_ids.len() >= 8, "Must contain all core agent roles");
    assert!(all_ids.contains(&"implementer".to_string()));
    assert!(all_ids.contains(&"planner".to_string()));
    assert!(all_ids.contains(&"verifier".to_string()));

    let implementer_def = registry
        .resolve(&AgentRole::new("implementer"))
        .expect("implementer must exist");
    assert!(implementer_def.write_tools_permitted);
    assert!(implementer_def.profile.max_steps > 0);
}

// ─── P2: Live Skills Registry ───────────────────────────────────────────────

#[test]
fn test_skills_registry_truthful_empty_and_records() {
    let mut model = create_test_model();
    assert!(model.skills.is_empty());

    let skill = TuiSkillSnapshot {
        id: "git-ops".to_string(),
        name: "Git Operations".to_string(),
        description: "Autonomous branching and committing".to_string(),
        origin_tier: "builtin".to_string(),
        capabilities: vec!["git:read".to_string(), "git:write".to_string()],
        verification_tier: "Tier1".to_string(),
        risk_tier: "Medium".to_string(),
        status: "available".to_string(),
    };

    model.load_skills(vec![skill]);
    assert_eq!(model.skills.len(), 1);
    assert_eq!(model.skills[0].id, "git-ops");
    assert_eq!(model.skills[0].status, "available");
}

// ─── P2: Command Palette Truth ──────────────────────────────────────────────

#[test]
fn test_command_palette_truthful_availability() {
    let mut model = create_test_model();

    // 1. In Idle state:
    model.lifecycle.stage = TuiLifecycleStage::Idle;
    let (avail, reason) = check_command_availability("gov_plan_accept", Some(&model));
    assert!(!avail);
    assert_eq!(
        reason,
        Some("Requires PlanReviewRequired lifecycle stage"),
        "Must provide truthful disabled reason"
    );

    let (avail, reason) = check_command_availability("gov_tasks_accept", Some(&model));
    assert!(!avail);
    assert_eq!(reason, Some("Requires TasksReviewRequired lifecycle stage"));

    let (avail, reason) = check_command_availability("gov_authorize_yes", Some(&model));
    assert!(!avail);
    assert_eq!(
        reason,
        Some("Requires ExecutionAuthorizationRequired lifecycle stage")
    );

    // 2. In PlanReviewRequired state:
    model.lifecycle.stage = TuiLifecycleStage::PlanReviewRequired;
    let (avail, reason) = check_command_availability("gov_plan_accept", Some(&model));
    assert!(avail);
    assert!(reason.is_none());

    // 3. Pause command truth:
    model.mission_status = "idle".to_string();
    let (avail, reason) = check_command_availability("cmd_pause_mission", Some(&model));
    assert!(!avail);
    assert_eq!(reason, Some("No active running mission to pause"));

    model.mission_status = "running".to_string();
    let (avail, reason) = check_command_availability("cmd_pause_mission", Some(&model));
    assert!(avail);
    assert!(reason.is_none());

    // 4. Resume command truth:
    let (avail, reason) = check_command_availability("cmd_resume_mission", Some(&model));
    assert!(!avail);
    assert_eq!(reason, Some("Mission is not currently paused"));

    model.mission_status = "paused".to_string();
    let (avail, reason) = check_command_availability("cmd_resume_mission", Some(&model));
    assert!(avail);
    assert!(reason.is_none());
}

// ─── Invariant: Zero Render-Time Database I/O ───────────────────────────────

#[test]
fn test_zero_render_time_database_queries() {
    let mut model = create_test_model();

    // Populate with representative data
    model.load_artifacts(vec![TuiArtifactSnapshot {
        id: "art-1".to_string(),
        name: "test.rs".to_string(),
        logical_path: "src/test.rs".to_string(),
        content_hash: "hash".to_string(),
        size_bytes: 100,
        extension: "rs".to_string(),
        version: 1,
        status: "active".to_string(),
        mission_id: None,
        task_id: None,
        producer_role: None,
        parent_artifact_ids: vec![],
        verification_check_ids: vec![],
        created_at: Utc::now(),
        storage_state: "stored".to_string(),
    }]);

    model.load_verification_checks(vec![TuiVerificationCheck {
        check_id: "check-1".to_string(),
        mission_id: "m-1".to_string(),
        task_id: "t-1".to_string(),
        tier_num: 1,
        tier_name: "Unit".to_string(),
        status: "passed".to_string(),
        command_or_tool: "cargo test".to_string(),
        inputs_normalized: "{}".to_string(),
        evidence_artifact_id: None,
        summary: "passed".to_string(),
        failure_class: None,
        snapshot_hash: "hash".to_string(),
        created_at: Utc::now(),
    }]);

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
            crate_adapters::render_artifacts_for_test(f, f.area(), &model, &tokens);
            crate_adapters::render_verification_for_test(f, f.area(), &model, &tokens);
        })
        .unwrap();

    assert_eq!(
        model.sqlite_render_access_count(),
        0,
        "Law 15 / TUI-03 violation: SQLite query executed during render"
    );
}

// ─── Test Rendering Adapters ────────────────────────────────────────────────

mod crate_adapters {
    use super::*;
    use ratatui::Frame;
    use ratatui::layout::Rect;

    pub fn render_artifacts_for_test(
        f: &mut Frame,
        area: Rect,
        model: &TuiViewModel,
        tokens: &ThemeTokens,
    ) {
        m31a::tui::surface::render_artifacts_surface(f, area, model, tokens, true, 0);
    }

    pub fn render_verification_for_test(
        f: &mut Frame,
        area: Rect,
        model: &TuiViewModel,
        tokens: &ThemeTokens,
    ) {
        m31a::tui::surface::render_verification_surface(f, area, model, tokens, true, 0);
    }
}
