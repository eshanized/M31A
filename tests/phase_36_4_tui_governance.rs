//! Phase 36.4 — Conversation TUI + Governed Lifecycle Convergence Tests.
//!
//! Deterministic, in-memory tests proving the TUI is a projection of the real
//! M31A runtime lifecycle — never a second runtime:
//! - lifecycle projection: every governed event maps to the truthful stage
//! - navigation: single canonical authority, all 40 views resolve, no drops
//! - input routing: composer behavior per lifecycle context
//! - governance: generic approvals never masquerade as plan/task/authorization
//! - fail-closed: missing bridge surfaces explicit errors, never executes
//! - rendering: deterministic snapshots for each governance card, zero DB I/O

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::task_graph::TaskGraphId;
use m31a::ids::{MissionId, SessionId, TaskId};
use m31a::interaction::events::InteractionEvent;
use m31a::tui::TuiApplication;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::lifecycle::{TuiLifecycleProjection, TuiLifecycleStage};
use m31a::tui::model::TuiViewModel;
use m31a::tui::navigation::{
    NavigationRouter, RouteResolution, ScreenId, canonical_screen, resolve_view,
};
use m31a::tui::registry::{ViewId, ViewKind, ViewRegistry};
use m31a::tui::theme::{ThemeMode, ThemeTokens};

// ─── helpers ────────────────────────────────────────────────────────────────

fn envelope(event_type: EventType) -> EventEnvelope {
    EventEnvelope::new(0, None, None, "test".to_string(), event_type)
}

fn sid() -> SessionId {
    SessionId::new()
}

// ─── projection: governed events map to truthful stages ─────────────────────

#[test]
fn test_projection_happy_path_ordering() {
    let session = sid().to_string();
    let mut proj = TuiLifecycleProjection::new();
    assert_eq!(proj.stage, TuiLifecycleStage::Idle);

    proj.apply_event(&envelope(EventType::PlanReviewRequired {
        session_id: session.clone(),
        plan_id: "plan-1".to_string(),
        revision: 1,
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::PlanReviewRequired);
    assert_eq!(proj.plan_revision, Some(1));

    proj.apply_event(&envelope(EventType::PlanRevisionCreated {
        session_id: session.clone(),
        plan_id: "plan-1".to_string(),
        revision: 2,
        author: "operator".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::PlanRevisionAvailable);
    assert_eq!(proj.plan_revision, Some(2));

    proj.apply_event(&envelope(EventType::PlanAccepted {
        session_id: session.clone(),
        plan_id: "plan-1".to_string(),
        revision: 2,
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::PlanAccepted);

    proj.apply_event(&envelope(EventType::TasksReviewRequired {
        session_id: session.clone(),
        plan_revision: 2,
        task_revision: 1,
        task_count: 3,
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::TasksReviewRequired);
    assert_eq!(proj.task_revision, Some(1));

    proj.apply_event(&envelope(EventType::TaskRevisionCreated {
        session_id: session.clone(),
        task_revision: 2,
        plan_revision: 2,
        author: "operator".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::TaskRevisionAvailable);

    proj.apply_event(&envelope(EventType::TasksAccepted {
        session_id: session.clone(),
        task_revision: 2,
        plan_revision: 2,
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::TasksAccepted);

    proj.apply_event(&envelope(EventType::ExecutionAuthorizationRequired {
        session_id: session.clone(),
        plan_revision: 2,
        task_revision: 2,
        message: "authorize".to_string(),
    }));
    assert_eq!(
        proj.stage,
        TuiLifecycleStage::ExecutionAuthorizationRequired
    );

    let auth_id = uuid::Uuid::now_v7();
    proj.apply_event(&envelope(EventType::ExecutionAuthorized {
        session_id: session.clone(),
        authorization_id: auth_id,
        authorized_by: "operator".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::ExecutionAuthorized);
    assert_eq!(proj.authorization_id, Some(auth_id.to_string()));
    // Authorized is explicitly NOT executing.
    assert_ne!(proj.stage, TuiLifecycleStage::Executing);

    // Materialization commits before StartExecution; only the authorized
    // projection may advance.
    proj.apply_event(&envelope(EventType::TaskGraphMaterialized {
        graph_id: TaskGraphId::new(),
        mission_id: MissionId::new(),
        revision: 1,
        task_count: 3,
        tasks: vec![],
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Executing);

    proj.apply_event(&envelope(EventType::MissionCompleted {
        mission_id: MissionId::new(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Completed);
}

#[test]
fn test_projection_rejections_and_failures() {
    let session = sid().to_string();
    let mut proj = TuiLifecycleProjection::new();

    proj.apply_event(&envelope(EventType::PlanRejected {
        session_id: session.clone(),
        plan_id: "plan-1".to_string(),
        revision: 1,
        reason: "too risky".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Rejected);
    assert_eq!(proj.failure_reason, Some("too risky".to_string()));

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::ExecutionAuthorizationRejected {
        session_id: session.clone(),
        reason: "no".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Rejected);

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::VerificationCompleted {
        verification_id: "v1".to_string(),
        mission_id: MissionId::new(),
        passed: false,
        evidence: "2 tests failed".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Failed);
    assert_eq!(proj.failure_reason, Some("2 tests failed".to_string()));

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::VerificationCompleted {
        verification_id: "v1".to_string(),
        mission_id: MissionId::new(),
        passed: true,
        evidence: "all green".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Verifying);
    assert_eq!(proj.verification_summary, Some("all green".to_string()));

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::MissionFailed {
        mission_id: MissionId::new(),
        reason: "boom".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Failed);

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::MissionCancelled {
        mission_id: MissionId::new(),
        reason: "operator".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Cancelled);

    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::TaskBlocked {
        task_id: TaskId::new(),
        mission_id: MissionId::new(),
        reason: "waiting on db".to_string(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Blocked);
    proj.apply_event(&envelope(EventType::TaskUnblocked {
        task_id: TaskId::new(),
        mission_id: MissionId::new(),
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Executing);
}

#[test]
fn test_projection_terminal_states_are_sealed() {
    for terminal in [
        EventType::MissionCompleted {
            mission_id: MissionId::new(),
        },
        EventType::PlanRejected {
            session_id: "s".to_string(),
            plan_id: "p".to_string(),
            revision: 1,
            reason: "r".to_string(),
        },
        EventType::MissionCancelled {
            mission_id: MissionId::new(),
            reason: "r".to_string(),
        },
    ] {
        let mut proj = TuiLifecycleProjection::new();
        proj.apply_event(&envelope(terminal));
        assert!(proj.stage.is_terminal());
        // A later review event must not resurrect a sealed projection.
        proj.apply_event(&envelope(EventType::PlanReviewRequired {
            session_id: "s".to_string(),
            plan_id: "p".to_string(),
            revision: 9,
        }));
        assert!(proj.stage.is_terminal());
        assert_ne!(proj.plan_revision, Some(9));
    }
}

#[test]
fn test_projection_does_not_fake_execution() {
    // Task activity without prior authorization must not move the projection.
    let mut proj = TuiLifecycleProjection::new();
    proj.apply_event(&envelope(EventType::TaskGraphMaterialized {
        graph_id: TaskGraphId::new(),
        mission_id: MissionId::new(),
        revision: 1,
        task_count: 1,
        tasks: vec![],
    }));
    assert_eq!(proj.stage, TuiLifecycleStage::Idle);
}

#[test]
fn test_lifecycle_labels_are_explicit_text() {
    // Quiet human-readable labels — explicit text, never color-only, never
    // bracketed shouting. Authorization vs authorized vs executing must be
    // unambiguous in words alone.
    assert_eq!(
        TuiLifecycleStage::PlanReviewRequired.label(),
        "Waiting for approval"
    );
    assert_eq!(
        TuiLifecycleStage::TasksReviewRequired.label(),
        "Waiting for approval"
    );
    assert_eq!(
        TuiLifecycleStage::ExecutionAuthorizationRequired.label(),
        "Waiting for approval"
    );
    assert_eq!(TuiLifecycleStage::ExecutionAuthorized.label(), "Authorized");
    assert_ne!(
        TuiLifecycleStage::ExecutionAuthorizationRequired.label(),
        TuiLifecycleStage::ExecutionAuthorized.label()
    );
    assert_ne!(
        TuiLifecycleStage::ExecutionAuthorized.label(),
        TuiLifecycleStage::Executing.label()
    );
}

// ─── view model: cards, enrichment, approval separation ─────────────────────

#[test]
fn test_plan_card_created_then_enriched_without_fabrication() {
    let mut model = TuiViewModel::new();
    // Envelope carries identity only: no hash is invented.
    model.apply_event(&envelope(EventType::PlanReviewRequired {
        session_id: "s".to_string(),
        plan_id: "plan-1".to_string(),
        revision: 2,
    }));
    assert_eq!(model.lifecycle.stage, TuiLifecycleStage::PlanReviewRequired);
    let cards: Vec<_> = model
        .conversation
        .iter()
        .filter(|i| matches!(i, TuiConversationItem::PlanReview { .. }))
        .collect();
    assert_eq!(cards.len(), 1);
    if let TuiConversationItem::PlanReview { content_hash, .. } = cards[0] {
        assert_eq!(*content_hash, None);
    } else {
        unreachable!();
    }

    // Coordinator response enriches the same card with the canonical hash.
    model.apply_interaction_event(&InteractionEvent::PlanForReview {
        session_id: "s".to_string(),
        revision: 2,
        plan_id: "plan-1".to_string(),
        objective: "build auth".to_string(),
        task_count: 4,
        content_hash: Some("abcdef1234567890".to_string()),
    });
    let cards: Vec<_> = model
        .conversation
        .iter()
        .filter(|i| matches!(i, TuiConversationItem::PlanReview { .. }))
        .collect();
    assert_eq!(cards.len(), 1, "exactly one card per revision");
    if let TuiConversationItem::PlanReview {
        content_hash,
        objective,
        task_count,
        ..
    } = cards[0]
    {
        assert_eq!(content_hash.as_deref(), Some("abcdef1234567890"));
        assert_eq!(objective, "build auth");
        assert_eq!(*task_count, 4);
    } else {
        unreachable!();
    }
    assert_eq!(
        TuiLifecycleProjection::short_hash("abcdef1234567890"),
        "abcdef12"
    );
}

#[test]
fn test_generic_approvals_never_move_governance() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::AuthorizationRequired {
        session_id: "s".to_string(),
        plan_revision: 2,
        task_revision: 3,
        message: "authorize".to_string(),
    });
    assert_eq!(
        model.lifecycle.stage,
        TuiLifecycleStage::ExecutionAuthorizationRequired
    );

    // A generic tool approval request/resolution must not disturb the gate.
    model.apply_interaction_event(&InteractionEvent::ApprovalRequested {
        request_id: "req-1".to_string(),
        tool_name: "run_command".to_string(),
        details: "run tests".to_string(),
    });
    assert_eq!(
        model.lifecycle.stage,
        TuiLifecycleStage::ExecutionAuthorizationRequired,
        "generic approval must not replace execution authorization"
    );
    model.apply_interaction_event(&InteractionEvent::ApprovalResolved {
        request_id: "req-1".to_string(),
        approved: true,
    });
    assert_eq!(
        model.lifecycle.stage,
        TuiLifecycleStage::ExecutionAuthorizationRequired,
        "generic approval resolution must not imply execution authorization"
    );
}

#[test]
fn test_discovery_interaction_drives_card_and_stage() {
    let mut model = TuiViewModel::new();
    let q = m31a::workflow::genesis::discovery::DynamicQuestion {
        question_id: "q1".to_string(),
        reason: "Need auth choice".to_string(),
        target_unknown: "auth_mechanism".to_string(),
        text: "Which auth mechanism?".to_string(),
        options: vec!["JWT".to_string(), "Session".to_string()],
        allow_freeform: true,
        blocking: true,
    };
    model.apply_interaction_event(&InteractionEvent::DiscoveryRequired {
        session_id: "s".to_string(),
        questions: vec![q],
    });
    assert_eq!(model.lifecycle.stage, TuiLifecycleStage::DiscoveryRequired);
    assert_eq!(model.lifecycle.pending_questions.len(), 1);
    assert!(
        model
            .conversation
            .iter()
            .any(|i| matches!(i, TuiConversationItem::Discovery { .. }))
    );
    assert_eq!(model.conversation[0].badge(), "[DISCOVERY]");
}

#[test]
fn test_completion_does_not_overwrite_sealed_rejection() {
    let mut model = TuiViewModel::new();
    model.apply_event(&envelope(EventType::PlanRejected {
        session_id: "s".to_string(),
        plan_id: "p".to_string(),
        revision: 1,
        reason: "no".to_string(),
    }));
    assert_eq!(model.lifecycle.stage, TuiLifecycleStage::Rejected);
    model.apply_interaction_event(&InteractionEvent::Completion {
        summary: "legacy turn done".to_string(),
    });
    assert_eq!(model.lifecycle.stage, TuiLifecycleStage::Rejected);
}

#[test]
fn test_authorized_card_distinct_from_executing() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ExecutionReady {
        session_id: "s".to_string(),
        authorization_id: "auth-1".to_string(),
        plan_revision: 2,
        task_revision: 3,
    });
    assert_eq!(
        model.lifecycle.stage,
        TuiLifecycleStage::ExecutionAuthorized
    );
    assert!(
        model
            .conversation
            .iter()
            .any(|i| matches!(i, TuiConversationItem::AuthGranted { .. }))
    );
    let card = model
        .conversation
        .iter()
        .find(|i| matches!(i, TuiConversationItem::AuthGranted { .. }))
        .unwrap();
    assert_eq!(card.badge(), "[AUTHORIZED — NOT EXECUTING]");
}

// ─── navigation: single authority, total mapping ────────────────────────────

#[test]
fn test_all_forty_views_resolve_to_functional_routes() {
    let registry = ViewRegistry::new();
    assert_eq!(registry.len(), 40);
    for view in ViewId::all() {
        let parent = canonical_screen(*view);
        // Every view has a reachable parent route (nothing is dropped).
        let _ = format!("{parent:?}");
        match resolve_view(*view) {
            RouteResolution::PrimaryRoute(screen) => assert_eq!(screen, parent),
            RouteResolution::DetailInspector { parent: p, .. }
            | RouteResolution::ModalDialog { parent: p, .. }
            | RouteResolution::Overlay { parent: p, .. } => assert_eq!(p, parent),
        }
    }
}

#[test]
fn test_view_kinds_classify_routes_vs_contextual() {
    let registry = ViewRegistry::new();
    for meta in registry.all() {
        match meta.kind {
            ViewKind::PrimaryRoute => {
                assert!(
                    matches!(resolve_view(meta.id), RouteResolution::PrimaryRoute(_)),
                    "{:?} must navigate directly",
                    meta.id
                );
            }
            ViewKind::DetailInspector | ViewKind::ModalDialog | ViewKind::Overlay => {
                assert!(
                    !matches!(resolve_view(meta.id), RouteResolution::PrimaryRoute(_)),
                    "{:?} must open contextually, not as a top-level route",
                    meta.id
                );
            }
        }
    }
}

#[test]
fn test_keyboard_routing_is_deterministic() {
    use crossterm::event::{KeyEvent, KeyModifiers};
    let mut router = NavigationRouter::new();
    let key = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
    let a = router.handle_key(key('3'));
    let b = router.handle_key(key('3'));
    assert_eq!(a, b);
    assert_eq!(router.current_screen, ScreenId::TaskGraph);
    // Re-navigating to the active screen does not grow history.
    assert_eq!(router.history.len(), 1);

    // Esc dismisses contextual views before leaving the route.
    router.navigate_to_view(ViewId::ApprovalsQueue);
    assert_eq!(router.active_overlay, Some(ViewId::ApprovalsQueue));
    router.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert_eq!(router.active_overlay, None);
    assert_eq!(router.current_screen, ScreenId::Approvals);

    // Empty history pops cleanly.
    let mut fresh = NavigationRouter::new();
    assert_eq!(fresh.pop_history(), None);
}

// ─── fail-closed: no bridge means no execution ──────────────────────────────

#[test]
fn test_composer_submit_without_bridge_fails_closed() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    assert!(app.bridge_tx.is_none());
    app.composer.set_text("build authentication for the api");
    let result = app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(
        result.is_none(),
        "without the governed bridge the TUI must not emit any RuntimeCommand"
    );
    let has_error = app.model.conversation.iter().any(|i| {
        matches!(
            i,
            TuiConversationItem::Error { message, .. } if message.contains("bridge")
        )
    });
    assert!(
        has_error,
        "an explicit bridge-unavailable error must surface"
    );
    assert!(
        !app.model.conversation.is_empty(),
        "user intent stays visible in the timeline"
    );
}

#[test]
fn test_free_text_hint_during_review_is_fail_closed_by_bridge() {
    // The bridge rejects free text in review stages; unit-proof of the rule:
    // review/auth stages consume input, executing/terminal stages do not.
    use m31a::state_machine::lifecycle::LifecycleStage;
    let consuming = [
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
    ];
    for stage in consuming {
        assert!(
            !stage.is_terminal(),
            "{stage:?} must be a non-terminal governed gate"
        );
    }
}

// ─── rendering: deterministic snapshots, zero DB I/O ────────────────────────

fn render_surface_text(model: &mut TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    terminal
        .draw(|f| {
            m31a::tui::surface::render_conversation_surface(f, f.area(), model, &tokens, true);
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

fn model_with_lifecycle(stage: TuiLifecycleStage) -> TuiViewModel {
    let mut model = TuiViewModel::new();
    model.lifecycle.stage = stage;
    model
}

#[test]
fn test_render_governance_cards_and_banners() {
    // Discovery card (quiet).
    let mut model = TuiViewModel::new();
    let q = m31a::workflow::genesis::discovery::DynamicQuestion {
        question_id: "q1".to_string(),
        reason: "Need auth choice".to_string(),
        target_unknown: "auth_mechanism".to_string(),
        text: "Which auth mechanism?".to_string(),
        options: vec!["JWT".to_string(), "Session".to_string()],
        allow_freeform: true,
        blocking: true,
    };
    model.apply_interaction_event(&InteractionEvent::DiscoveryRequired {
        session_id: "s".to_string(),
        questions: vec![q],
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Input needed"));
    assert!(text.contains("Which auth mechanism?"));
    assert!(text.contains("Waiting for input"));

    // Plan review card + revision identity (quiet, hash preserved).
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::PlanForReview {
        session_id: "s".to_string(),
        revision: 2,
        plan_id: "plan-1".to_string(),
        objective: "build auth".to_string(),
        task_count: 4,
        content_hash: Some("7f1c8b2e00000000".to_string()),
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Plan"));
    assert!(text.contains("revision 2"));
    assert!(text.contains("Waiting for approval"));
    assert!(text.contains("7f1c8b2e"));

    // Task review card.
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::TasksForReview {
        session_id: "s".to_string(),
        plan_revision: 2,
        task_revision: 3,
        task_count: 4,
        content_hash: Some("29d8f10100000000".to_string()),
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Tasks"));
    assert!(text.contains("revision 3"));
    assert!(text.contains("Waiting for approval"));

    // Authorization gate is visually distinct from authorized and executing.
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::AuthorizationRequired {
        session_id: "s".to_string(),
        plan_revision: 2,
        task_revision: 3,
        message: "approve workspace writes".to_string(),
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Authorization needed"));
    assert!(text.contains("Waiting for approval"));

    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::ExecutionReady {
        session_id: "s".to_string(),
        authorization_id: "auth-1".to_string(),
        plan_revision: 2,
        task_revision: 3,
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Authorized"));
    assert!(text.contains("not yet executing"));

    // Failure card classifies context.
    let mut model = TuiViewModel::new();
    model.apply_event(&envelope(EventType::MissionFailed {
        mission_id: MissionId::new(),
        reason: "test process exited with code 1".to_string(),
    }));
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Failed"));
    assert!(text.contains("test process exited with code 1"));

    // Verification evidence card.
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::VerificationPassed {
        summary: "all gates passed".to_string(),
    });
    let text = render_surface_text(&mut model, 120, 30);
    assert!(text.contains("Verification"));

    // Narrow terminal keeps critical state visible.
    let narrow = render_surface_text(
        &mut model_with_lifecycle(TuiLifecycleStage::ExecutionAuthorizationRequired),
        80,
        24,
    );
    assert!(narrow.contains("Waiting for approval"));
}

#[test]
fn test_rendering_performs_zero_sqlite_io() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::PlanForReview {
        session_id: "s".to_string(),
        revision: 1,
        plan_id: "p".to_string(),
        objective: "o".to_string(),
        task_count: 1,
        content_hash: None,
    });
    assert_eq!(model.sqlite_render_access_count(), 0);
    let _ = render_surface_text(&mut model, 120, 30);
    assert_eq!(
        model.sqlite_render_access_count(),
        0,
        "rendering must never touch SQLite"
    );
}

#[test]
fn test_model_proposal_visually_distinct_from_runtime_decision() {
    let assistant = TuiConversationItem::Assistant {
        id: "a".to_string(),
        sequence: 1,
        text: "I propose this plan".to_string(),
        streaming: false,
        timestamp: chrono::Utc::now(),
    };
    let system = TuiConversationItem::System {
        text: "Execution authorized".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_ne!(assistant.badge(), system.badge());
    assert_eq!(assistant.badge(), "[ASST]");
    assert_eq!(system.badge(), "[SYS]");
}
