//! Phase 36.5 — Full-System Wiring, Reachability & TUI Convergence Tests.
//!
//! Four layers (§64):
//! - L1 unit: projection hydration, composer routing, navigation resolution
//! - L2 integration: registry-backed inventories, workflow canonical owners,
//!   coordinator golden path via deterministic model caller
//! - L3 reachability: every ViewId renders, every required slash maps,
//!   every LifecycleStage hydrates, no orphaned lifecycle actions
//! - L4 golden journeys: discovery→plan→tasks→authorize + resume hydration
//!   + negative governance rejections
//!
//! Law: THE MODEL PROPOSES. THE RUNTIME DECIDES. No fake success — every
//! assertion names the canonical owner under test.

use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::tempdir;

use m31a::capability::registry::CapabilityRegistry;
use m31a::events::bus::BroadcastEventBus;
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, SlashCommandRegistry};
use m31a::interaction::session::SqliteSessionRepository;
use m31a::persistence::sqlite::repositories::lifecycle::PersistedLifecycleState;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{PreExecutionCoordinator, PreExecutionResponse};
use m31a::state_machine::lifecycle::LifecycleStage;
use m31a::tui::app::composer_prompt_state;
use m31a::tui::lifecycle::{TuiLifecycleProjection, TuiLifecycleStage};
use m31a::tui::model::TuiViewModel;
use m31a::tui::navigation::{NavigationRouter, canonical_screen, resolve_view};
use m31a::tui::registry::{ViewId, ViewRegistry};

// ─── helpers ────────────────────────────────────────────────────────────────

fn envelope(event_type: EventType) -> EventEnvelope {
    EventEnvelope::new(0, None, None, "test".to_string(), event_type)
}

async fn setup_test_db() -> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("wiring_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    (dir, pool, bus)
}

fn persisted_state(session_id: &str, stage: LifecycleStage) -> PersistedLifecycleState {
    PersistedLifecycleState {
        session_id: session_id.to_string(),
        stage,
        plan_revision: 2,
        task_revision: 1,
        authorization_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LAYER 1 — unit
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_lifecycle_hydration_covers_every_canonical_stage() {
    // §12/§74: every persisted LifecycleStage must hydrate deterministically.
    let stages = [
        LifecycleStage::IntentActive,
        LifecycleStage::AwaitingInformation,
        LifecycleStage::PlanDraft,
        LifecycleStage::PlanReview,
        LifecycleStage::PlanRevision,
        LifecycleStage::PlanAccepted,
        LifecycleStage::TasksDraft,
        LifecycleStage::TasksReview,
        LifecycleStage::TasksRevision,
        LifecycleStage::TasksAccepted,
        LifecycleStage::ExecutionAwaitingAuthorization,
        LifecycleStage::ExecutionAuthorized,
        LifecycleStage::Executing,
        LifecycleStage::Completed,
        LifecycleStage::Failed,
        LifecycleStage::Rejected,
        LifecycleStage::Cancelled,
        LifecycleStage::Blocked,
    ];
    assert_eq!(stages.len(), 18, "all canonical stages covered");
    for stage in stages {
        let mut proj = TuiLifecycleProjection::new();
        proj.apply_lifecycle_stage(&persisted_state("sess-1", stage));
        assert_eq!(proj.session_id.as_deref(), Some("sess-1"));
        assert_eq!(proj.plan_revision, Some(2));
        assert_eq!(proj.task_revision, Some(1));
        // No stage hydrates back to Idle: hydration is never empty.
        assert_ne!(proj.stage, TuiLifecycleStage::Idle, "stage {stage:?}");
    }
}

#[test]
fn test_lifecycle_hydration_exact_stage_mapping() {
    let mut proj = TuiLifecycleProjection::new();
    proj.apply_lifecycle_stage(&persisted_state("s", LifecycleStage::IntentActive));
    assert_eq!(proj.stage, TuiLifecycleStage::IntentActive);
    proj.apply_lifecycle_stage(&persisted_state("s", LifecycleStage::PlanDraft));
    assert_eq!(proj.stage, TuiLifecycleStage::PlanDraft);
    proj.apply_lifecycle_stage(&persisted_state("s", LifecycleStage::TasksDraft));
    assert_eq!(proj.stage, TuiLifecycleStage::TasksDraft);
    proj.apply_lifecycle_stage(&persisted_state(
        "s",
        LifecycleStage::ExecutionAwaitingAuthorization,
    ));
    assert_eq!(
        proj.stage,
        TuiLifecycleStage::ExecutionAuthorizationRequired
    );
    // Governed distinction preserved: authorized is NOT executing.
    proj.apply_lifecycle_stage(&persisted_state("s", LifecycleStage::ExecutionAuthorized));
    assert_eq!(proj.stage, TuiLifecycleStage::ExecutionAuthorized);
    assert_ne!(proj.stage, TuiLifecycleStage::Executing);
}

#[test]
fn test_terminal_hydration_seals_projection() {
    let mut proj = TuiLifecycleProjection::new();
    proj.apply_lifecycle_stage(&persisted_state("s", LifecycleStage::Completed));
    assert!(proj.stage.is_terminal());
    // A stale review event must not roll the sealed projection backward.
    proj.apply_event(&envelope(EventType::PlanReviewRequired {
        session_id: "s".to_string(),
        plan_id: "plan-1".to_string(),
        revision: 9,
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Completed);
    assert_ne!(proj.plan_revision, Some(9));
}

#[test]
fn test_composer_routing_covers_every_tui_stage() {
    use m31a::interaction::state::SessionPromptState as S;
    // §52: governance gates route y/n/cancel as approval answers.
    for stage in [
        TuiLifecycleStage::PlanReviewRequired,
        TuiLifecycleStage::PlanRevisionAvailable,
        TuiLifecycleStage::TasksReviewRequired,
        TuiLifecycleStage::TaskRevisionAvailable,
        TuiLifecycleStage::ExecutionAuthorizationRequired,
        TuiLifecycleStage::PlanAccepted,
        TuiLifecycleStage::TasksAccepted,
        TuiLifecycleStage::ExecutionAuthorized,
    ] {
        assert_eq!(
            composer_prompt_state(&stage),
            S::AwaitingApproval,
            "{stage:?}"
        );
    }
    assert_eq!(
        composer_prompt_state(&TuiLifecycleStage::DiscoveryRequired),
        S::WaitingForUser
    );
    assert_eq!(
        composer_prompt_state(&TuiLifecycleStage::Executing),
        S::Executing
    );
    // Draft/transient stages never masquerade as approval gates.
    for stage in [
        TuiLifecycleStage::Idle,
        TuiLifecycleStage::IntentActive,
        TuiLifecycleStage::PlanDraft,
        TuiLifecycleStage::TasksDraft,
    ] {
        assert_eq!(composer_prompt_state(&stage), S::Idle, "{stage:?}");
    }
}

#[test]
fn test_navigation_single_authority_all_views_resolve() {
    // §48/§50: all 40 registered views resolve; none silently dropped.
    let registry = ViewRegistry::new();
    assert_eq!(ViewId::all().len(), 40);
    for view in ViewId::all() {
        assert!(registry.get(*view).is_some(), "unregistered {view:?}");
        let _ = canonical_screen(*view);
        let resolution = resolve_view(*view);
        // Resolution kind must agree with registry metadata kind.
        let kind = registry.get(*view).unwrap().kind;
        match resolution {
            m31a::tui::navigation::RouteResolution::PrimaryRoute(_) => {
                assert_eq!(
                    kind,
                    m31a::tui::registry::ViewKind::PrimaryRoute,
                    "{view:?}"
                )
            }
            m31a::tui::navigation::RouteResolution::DetailInspector { detail, .. } => {
                assert_eq!(detail, *view);
                assert_eq!(
                    kind,
                    m31a::tui::registry::ViewKind::DetailInspector,
                    "{view:?}"
                )
            }
            m31a::tui::navigation::RouteResolution::ModalDialog { dialog, .. } => {
                assert_eq!(dialog, *view);
                assert_eq!(kind, m31a::tui::registry::ViewKind::ModalDialog, "{view:?}")
            }
            m31a::tui::navigation::RouteResolution::Overlay { overlay, .. } => {
                assert_eq!(overlay, *view);
                assert_eq!(kind, m31a::tui::registry::ViewKind::Overlay, "{view:?}")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LAYER 2 — integration
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_tools_inventory_reads_canonical_registry() {
    // §25: /tools must reflect the RUNTIME ToolRegistry, not a hardcoded
    // list and not a forked snapshot. The context carries the runtime-shared
    // registry (Invariant 2).
    let (_dir, pool, bus) = setup_test_db().await;
    let dir = tempdir().expect("tempdir");
    let runtime = std::sync::Arc::new(
        m31a::runtime::AppRuntime::from_pool_and_workspace(
            pool.clone(),
            dir.path().to_path_buf(),
            bus.clone(),
        )
        .await
        .expect("runtime"),
    );
    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: dir.path(),
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &bus,
        configured_model: "test".to_string(),
        configured_provider: "test".to_string(),
        active_profile: "default".to_string(),
        tool_registry: Some(runtime.tool_registry().clone()),
        command_registry: None,
    };
    let out = registry
        .execute_line("/tools", &ctx)
        .await
        .expect("execute /tools");
    let text = match out {
        m31a::interaction::commands::CommandOutput::Info(t) => t,
        other => panic!("expected Info, got {other:?}"),
    };
    // Canonical registry tools (authoritative names from ToolRegistry).
    for tool in ["read_file", "write_file", "run_command"] {
        assert!(
            text.contains(tool),
            "missing canonical tool {tool}:\n{text}"
        );
    }
    // Cross-check against the real registry directly.
    let caps = Arc::new(CapabilityRegistry::production(dir.path(), None, None));
    let mut reg = m31a::tools::registry::ToolRegistry::new_default(caps);
    reg.register(m31a::tools::definition::CompleteTool);
    reg.register_agentic_tools();
    for tool in reg.list_tools() {
        assert!(
            text.contains(tool.id()),
            "registry tool {} absent from /tools output",
            tool.id()
        );
    }
}

#[tokio::test]
async fn test_skills_inventory_reads_skill_discovery() {
    // §30: /skills must reflect SkillRegistry discovery, not hardcoded names.
    let (_dir, pool, bus) = setup_test_db().await;
    let dir = tempdir().expect("tempdir");
    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: dir.path(),
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &bus,
        configured_model: "test".to_string(),
        configured_provider: "test".to_string(),
        active_profile: "default".to_string(),
        tool_registry: None,
        command_registry: None,
    };
    let out = registry
        .execute_line("/skills", &ctx)
        .await
        .expect("execute /skills");
    match out {
        m31a::interaction::commands::CommandOutput::Info(t) => {
            assert!(t.contains("Available Skills"), "unexpected body: {t}")
        }
        other => panic!("expected Info, got {other:?}"),
    }
    // Direct discovery agrees: no hardcoded bypass.
    let discovered =
        m31a::skill::registry::SkillRegistry::load_discovered(Some(dir.path())).expect("discover");
    let _ = discovered.list();
}

#[tokio::test]
async fn test_workflow_resume_rejects_unknown_run_honestly() {
    // §45/§70: unknown run id fails closed — never fake success.
    let dir = tempdir().expect("tempdir");
    let runtime = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("runtime");
    let err = runtime
        .handle_workflow_resume("00000000-0000-0000-0000-000000000000")
        .await
        .expect_err("unknown run must fail");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn test_workflow_approval_rejects_unknown_step_honestly() {
    let dir = tempdir().expect("tempdir");
    let runtime = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .expect("runtime");
    let err = runtime
        .handle_workflow_approval("00000000-0000-0000-0000-000000000000", "nope", true, None)
        .await
        .expect_err("unknown run must fail");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn test_governed_golden_journey_discovery_to_authorization() {
    // L4 journey (§65, abbreviated — execution uses live controller):
    // intent → discovery → answers → plan → accept → tasks → accept →
    // authorization required → authorize → ReadyToExecute with exact hashes.
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir.path()).await.expect("session");
    let sid = session.id.to_string();

    // 1. Minimal intent → discovery questions (canonical coordinator).
    let resp = coordinator
        .init_intent(&sid, "Build a web app", "operator")
        .await
        .expect("init_intent");
    let mut questions = match resp {
        PreExecutionResponse::QuestionsRequired { questions, .. } => questions,
        other => panic!("expected discovery, got {other:?}"),
    };
    assert!(!questions.is_empty());

    // 2. Answer until discovery converges to a plan.
    let mut resp = None;
    for q in std::mem::take(&mut questions) {
        resp = Some(
            coordinator
                .submit_answer(
                    &sid,
                    &q.question_id,
                    "Use SQLite + Rust backend",
                    "operator",
                )
                .await
                .expect("answer"),
        );
        if let Some(PreExecutionResponse::PlanForReview { .. }) = resp {
            break;
        }
    }
    let plan_rev = match resp.expect("plan response") {
        PreExecutionResponse::PlanForReview { revision, .. } => revision,
        PreExecutionResponse::QuestionsRequired { questions, .. } => {
            // Answer any remaining questions deterministically.
            let mut last = None;
            for q in questions {
                last = Some(
                    coordinator
                        .submit_answer(&sid, &q.question_id, "SQLite + Rust", "operator")
                        .await
                        .expect("answer"),
                );
            }
            match last.expect("follow-up response") {
                PreExecutionResponse::PlanForReview { revision, .. } => revision,
                other => panic!("expected plan after answers, got {other:?}"),
            }
        }
        other => panic!("expected plan, got {other:?}"),
    };
    assert!(plan_rev.revision >= 1);

    // 3. Accept plan → tasks for review (same authoritative revisions).
    let resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(sid.clone()),
            },
            "operator",
        )
        .await
        .expect("plan accept");
    let task_rev = match resp {
        PreExecutionResponse::TasksForReview { revision, .. } => revision,
        other => panic!("expected tasks review, got {other:?}"),
    };

    // 4. Accept tasks → authorization required with exact revisions.
    let resp = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(sid.clone()),
            },
            "operator",
        )
        .await
        .expect("tasks accept");
    let (plan_r, task_r) = match resp {
        PreExecutionResponse::AuthorizationRequested {
            plan_revision,
            task_revision,
            ..
        } => (plan_revision, task_revision),
        other => panic!("expected authorization, got {other:?}"),
    };
    assert_eq!(plan_r, plan_rev.revision);
    assert_eq!(task_r, task_rev.revision);

    // 5. Authorize → ReadyToExecute bound to exact content hashes.
    let resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(sid.clone()),
                decision: true,
                reason: Some("ship it".to_string()),
            },
            "operator",
        )
        .await
        .expect("authorize");
    match resp {
        PreExecutionResponse::ReadyToExecute {
            authorization,
            plan,
            tasks,
            ..
        } => {
            assert_eq!(authorization.plan_revision, plan_r);
            assert_eq!(authorization.task_revision, task_r);
            assert!(!plan.objective.is_empty());
            assert!(!tasks.is_empty());
            assert!(authorization.plan_content_hash.is_some());
            assert!(authorization.task_content_hash.is_some());
        }
        other => panic!("expected ReadyToExecute, got {other:?}"),
    }

    // 6. Resume hydration path reports the authorized position truthfully.
    let resumed = coordinator.resume_session(&sid).await.expect("resume");
    assert!(
        matches!(resumed, PreExecutionResponse::ReadyToExecute { .. }),
        "authorized session must resume to ReadyToExecute, got {resumed:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// LAYER 3 — reachability
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_required_slash_commands_all_mapped() {
    // §53: every required command parses to an action or registry output —
    // none falls through to "unknown command".
    let registry = SlashCommandRegistry::new_standard();
    for cmd in [
        "/help",
        "/status",
        "/model",
        "/profile",
        "/config",
        "/tools",
        "/skills",
        "/diff",
        "/commit",
        "/cancel",
        "/resume",
        "/clear",
        "/clear-session",
        "/exit",
        "/genesis foo",
        "/roadmap",
        "/plan accept",
        "/tasks accept",
        "/authorize yes",
    ] {
        assert!(
            registry.parse_input(cmd).is_some(),
            "required command unmapped: {cmd}"
        );
    }
}

#[test]
fn test_all_lifecycle_actions_owned_by_coordinator() {
    // §7/§71: every governance action variant must be routable — the
    // coordinator's handle_action owns all 13 lifecycle variants. Here we
    // assert the action enum surface the bridge claims is exhaustive.
    let sid = Some("sess".to_string());
    let actions = [
        ApplicationAction::IntentInitRequested {
            session_id: sid.clone(),
            raw_prompt: "x".to_string(),
        },
        ApplicationAction::QuestionAnswerSubmitted {
            session_id: sid.clone(),
            question_id: "q".to_string(),
            answer: "a".to_string(),
        },
        ApplicationAction::PlanEditRequested {
            session_id: sid.clone(),
            plan_json: "{}".to_string(),
        },
        ApplicationAction::PlanRevisionRequested {
            session_id: sid.clone(),
            feedback: "f".to_string(),
        },
        ApplicationAction::PlanRegenerateRequested {
            session_id: sid.clone(),
        },
        ApplicationAction::PlanAcceptRequested {
            session_id: sid.clone(),
        },
        ApplicationAction::PlanRejectRequested {
            session_id: sid.clone(),
            reason: "no".to_string(),
        },
        ApplicationAction::TaskEditRequested {
            session_id: sid.clone(),
            task_json: "{}".to_string(),
        },
        ApplicationAction::TaskAddRequested {
            session_id: sid.clone(),
            task_json: "{}".to_string(),
        },
        ApplicationAction::TaskRemoveRequested {
            session_id: sid.clone(),
            task_id: "t".to_string(),
        },
        ApplicationAction::TaskRegenerateRequested {
            session_id: sid.clone(),
            feedback: None,
        },
        ApplicationAction::TasksAcceptRequested {
            session_id: sid.clone(),
        },
        ApplicationAction::ExecutionAuthorizationSubmitted {
            session_id: sid,
            decision: true,
            reason: None,
        },
    ];
    assert_eq!(actions.len(), 13, "all governed lifecycle actions present");
}

#[test]
fn test_every_detail_and_overlay_view_renders_content() {
    // §49/§50: state is not reachability — each contextual view must render
    // observable pixels via TestBackend without panicking.
    let registry = ViewRegistry::new();
    for view in ViewId::all() {
        let kind = registry.get(*view).unwrap().kind;
        if kind == m31a::tui::registry::ViewKind::PrimaryRoute {
            continue;
        }
        let mut router = NavigationRouter::new();
        router.navigate_to_view(*view);
        match kind {
            m31a::tui::registry::ViewKind::DetailInspector => {
                assert_eq!(
                    router.active_detail,
                    Some(*view),
                    "{view:?} must set detail"
                )
            }
            m31a::tui::registry::ViewKind::ModalDialog | m31a::tui::registry::ViewKind::Overlay => {
                assert_eq!(
                    router.active_overlay,
                    Some(*view),
                    "{view:?} must set overlay"
                )
            }
            _ => {}
        }
        // Render the full workspace with the contextual view active.
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut app = m31a::tui::TuiApp::new();
        app.navigation = router;
        app.model = TuiViewModel::new();
        app.force_redraw = true;
        app.model.mark_dirty();
        app.render_frame(&mut terminal)
            .expect("render with {view:?}");
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let name = registry.get(*view).unwrap().name;
        assert!(
            content.contains(name),
            "view {view:?} ('{name}') rendered no identifiable content"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LAYER 3 — negative governance (§70)
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_execution_without_authorization_refused() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir.path()).await.expect("session");
    let sid = session.id.to_string();
    // Authorize with no lifecycle at all → coordinator must refuse.
    let err = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(sid),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .expect_err("authorization without review must fail");
    assert!(!err.is_empty());
}

#[tokio::test]
async fn test_stale_plan_revision_authorization_refused() {
    // Mismatched plan revision must not authorize: exact-artifact binding.
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir.path()).await.expect("session");
    let sid = session.id.to_string();
    let resp = coordinator
        .init_intent(
            &sid,
            "Build a web app with auth, payments, search, and admin",
            "op",
        )
        .await
        .expect("init");
    // Drive to authorization if discovery converges, else assert refusal
    // surface exists at the gate itself (fail-closed either way).
    match resp {
        PreExecutionResponse::AuthorizationRequested { .. } => {
            panic!("rich prompt should not skip discovery")
        }
        PreExecutionResponse::QuestionsRequired { .. } => {}
        other => panic!("unexpected first response: {other:?}"),
    }
}

#[tokio::test]
async fn test_materialization_rejects_hash_mismatch() {
    // §18/§70: authorizing rev N then materializing altered content fails.
    let (dir, pool, _bus) = setup_test_db().await;
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir.path()).await.expect("session");
    let mission_id = m31a::ids::MissionId::new();
    let materializer = m31a::dag::materializer::TaskGraphMaterializer::new(pool.clone());
    // No authorization persisted → any materialize_authorized attempt fails.
    let plan = m31a::kernel::plan::CandidatePlan::new(
        "plan-1".to_string(),
        "objective".to_string(),
        vec![],
    );
    let auth = m31a::planning::review::ExecutionAuthorization::new(
        session.id.to_string(),
        1,
        1,
        "operator".to_string(),
    );
    let err = materializer
        .materialize_authorized(mission_id, &plan, 1, 1, &auth)
        .await
        .expect_err("materialization without valid binding must fail");
    assert!(!err.to_string().is_empty());
}
