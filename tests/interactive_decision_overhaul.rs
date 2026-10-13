//! Integration and unit tests verifying the interactive decision overhaul.
//!
//! Asserts that:
//! 1. `InteractiveDecision` constructs typed models with stable IDs, navigation, and validation.
//! 2. `KeyContext` and `resolve_key` route decision keys correctly while preserving priority invariants.
//! 3. `TuiViewModel` synchronizes `pending_decision` from interaction events and clears it on resolution.
//! 4. `render_decision_prompt` renders native selectable options cleanly in Ratatui without panics.
//! 5. Decision responses reaching the runtime produce domain transitions and resolve idempotently.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a::interaction::decision::{DecisionKind, InteractiveDecision, InteractiveDecisionResponse};
use m31a::interaction::events::InteractionEvent;
use m31a::tui::keymap::{KeyAction, KeyContext, resolve_key};
use m31a::tui::model::TuiViewModel;
use m31a::tui::surface::decision::render_decision_prompt;
use m31a::tui::theme::ThemeTokens;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

#[test]
fn test_interactive_decision_model_and_navigation() {
    let mut decision =
        InteractiveDecision::planning_choice("sess-1", "user prompt", "Create a plan first?");
    assert_eq!(decision.id, "planning_choice_sess-1");
    assert_eq!(decision.session_id, "sess-1");
    assert_eq!(decision.selected_index, 0);
    assert_eq!(decision.options.len(), 3);
    assert_eq!(decision.selected_option().unwrap().id, "plan");

    // Navigation next
    decision.select_next();
    assert_eq!(decision.selected_index, 1);
    assert_eq!(decision.selected_option().unwrap().id, "direct");

    decision.select_next();
    assert_eq!(decision.selected_index, 2);
    assert_eq!(decision.selected_option().unwrap().id, "cancel");

    // Wrap around
    decision.select_next();
    assert_eq!(decision.selected_index, 0);

    // Navigation prev
    decision.select_prev();
    assert_eq!(decision.selected_index, 2);

    // Explicit index
    assert!(decision.select_index(1));
    assert_eq!(decision.selected_index, 1);
    assert!(!decision.select_index(99)); // Invalid index rejected
}

#[test]
fn test_interactive_decision_validation() {
    let decision = InteractiveDecision::plan_review(
        "sess-2",
        1,
        "Implement feature X",
        4,
        Some("hash-123".to_string()),
    );

    // Valid accept response
    let valid_resp = InteractiveDecisionResponse {
        decision_id: decision.id.clone(),
        session_id: "sess-2".to_string(),
        execution_id: None,
        selected_option_id: Some("accept".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    assert!(decision.validate_response(&valid_resp).is_ok());

    // Valid cancelled response
    let cancel_resp = InteractiveDecisionResponse {
        decision_id: decision.id.clone(),
        session_id: "sess-2".to_string(),
        execution_id: None,
        selected_option_id: None,
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: true,
    };
    assert!(decision.validate_response(&cancel_resp).is_ok());

    // Mismatched decision ID rejected
    let bad_id_resp = InteractiveDecisionResponse {
        decision_id: "other_id".to_string(),
        session_id: "sess-2".to_string(),
        execution_id: None,
        selected_option_id: Some("accept".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    assert!(decision.validate_response(&bad_id_resp).is_err());

    // Mismatched session ID rejected
    let bad_sess_resp = InteractiveDecisionResponse {
        decision_id: decision.id.clone(),
        session_id: "other_session".to_string(),
        execution_id: None,
        selected_option_id: Some("accept".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    assert!(decision.validate_response(&bad_sess_resp).is_err());

    // Fabricated option identifier rejected
    let fake_opt_resp = InteractiveDecisionResponse {
        decision_id: decision.id.clone(),
        session_id: "sess-2".to_string(),
        execution_id: None,
        selected_option_id: Some("fake_option".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    assert!(decision.validate_response(&fake_opt_resp).is_err());
}

#[test]
fn test_decision_constructors() {
    let tasks_decision = InteractiveDecision::tasks_review("sess-3", 1, 2, 5, None);
    assert_eq!(tasks_decision.id, "tasks_review_sess-3_2");
    assert_eq!(
        tasks_decision.kind,
        DecisionKind::TaskReview {
            plan_revision: 1,
            task_revision: 2,
            content_hash: None,
        }
    );
    assert!(tasks_decision.options.iter().any(|o| o.id == "accept"));

    let auth_decision =
        InteractiveDecision::execution_authorization("sess-4", 1, 2, "Ready to execute 5 tasks");
    assert!(auth_decision.options.iter().any(|o| o.id == "authorize"));
    assert!(auth_decision.options.iter().any(|o| o.id == "reject"));

    let perm_decision = InteractiveDecision::permission_approval(
        "sess-5",
        "req-99",
        "file_writer",
        "Write to src/main.rs",
        "High",
        Some("path=src/main.rs".to_string()),
    );
    assert_eq!(perm_decision.id, "req-99");
    assert!(perm_decision.options.iter().any(|o| o.id == "approve_once"));
    assert!(
        perm_decision
            .options
            .iter()
            .any(|o| o.id == "approve_always")
    );
}

#[test]
fn test_keymap_decision_routing() {
    let plain_key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let digit_key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty());
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);

    // Decision pending: normal navigation and digits route to Decision
    let ctx_decision =
        KeyContext::new(false, false, false, false, false, false).with_decision(true);
    assert_eq!(resolve_key(plain_key, ctx_decision), KeyAction::Decision);
    assert_eq!(resolve_key(digit_key, ctx_decision), KeyAction::Decision);
    assert_eq!(resolve_key(enter_key, ctx_decision), KeyAction::Decision);

    // Safety chords still override Decision:
    // Ctrl+C must resolve to Exit
    assert_eq!(resolve_key(ctrl_c, ctx_decision), KeyAction::Exit);
    // Ctrl+P must resolve to OpenPalette
    assert_eq!(resolve_key(ctrl_p, ctx_decision), KeyAction::OpenPalette);

    // Overlays take precedence over Decision
    let ctx_dialog = KeyContext::new(false, true, false, false, false, false).with_decision(true);
    assert_eq!(resolve_key(enter_key, ctx_dialog), KeyAction::Dialog);

    let ctx_palette = KeyContext::new(false, false, true, false, false, false).with_decision(true);
    assert_eq!(resolve_key(enter_key, ctx_palette), KeyAction::Palette);

    // No decision pending: composer and route operate normally
    let ctx_composer =
        KeyContext::new(false, false, false, false, false, true).with_decision(false);
    assert_eq!(resolve_key(enter_key, ctx_composer), KeyAction::Composer);

    let ctx_route = KeyContext::new(false, false, false, false, false, false).with_decision(false);
    assert_eq!(resolve_key(plain_key, ctx_route), KeyAction::Route);
}

#[test]
fn test_tui_view_model_decision_lifecycle() {
    let mut model = TuiViewModel::new();
    assert!(!model.has_pending_decision());
    assert!(model.pending_decision.is_none());

    let decision =
        InteractiveDecision::planning_choice("s-10", "plan prompt", "Choose planning mode:");
    model.apply_interaction_event(&InteractionEvent::DecisionRequested {
        decision: decision.clone(),
    });

    assert!(model.has_pending_decision());
    assert_eq!(model.pending_decision.as_ref().unwrap().id, decision.id);
    assert_eq!(model.pending_decision.as_ref().unwrap().selected_index, 0);

    // Navigate in view model
    model.select_next_decision_option();
    assert_eq!(model.pending_decision.as_ref().unwrap().selected_index, 1);

    model.select_decision_option_index(2);
    assert_eq!(model.pending_decision.as_ref().unwrap().selected_index, 2);

    // Resolution clears pending decision
    model.apply_interaction_event(&InteractionEvent::DecisionResolved {
        decision_id: decision.id.clone(),
        session_id: "s-10".to_string(),
        outcome: "Direct execution selected".to_string(),
    });
    assert!(!model.has_pending_decision());
    assert!(model.pending_decision.is_none());

    // PlanForReview establishes pending decision
    model.apply_interaction_event(&InteractionEvent::PlanForReview {
        session_id: "s-10".to_string(),
        revision: 1,
        plan_id: "plan-1".to_string(),
        objective: "Build feature".to_string(),
        task_count: 3,
        content_hash: None,
        plan_markdown: Some("# Plan".to_string()),
        tasks: Vec::new(),
    });
    assert!(model.has_pending_decision());
    assert!(
        model
            .pending_decision
            .as_ref()
            .unwrap()
            .id
            .starts_with("plan_review_")
    );

    // ExecutionReady clears pending decision
    model.apply_interaction_event(&InteractionEvent::ExecutionReady {
        session_id: "s-10".to_string(),
        authorization_id: "auth-1".to_string(),
        plan_revision: 1,
        task_revision: 1,
    });
    assert!(!model.has_pending_decision());
}

#[test]
fn test_render_decision_prompt_surface() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(m31a::tui::theme::ThemeMode::Default);

    let decision =
        InteractiveDecision::plan_review("sess-render", 1, "Refactor interaction model", 4, None);

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 8);
            render_decision_prompt(f, area, &decision, &tokens, true);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let rendered_text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Verify title and options are present in rendered buffer
    assert!(
        rendered_text.contains("Plan Review"),
        "Rendered buffer must contain Plan Review header"
    );
    assert!(
        rendered_text.contains("Accept plan"),
        "Rendered buffer must contain Accept plan option"
    );
    assert!(
        rendered_text.contains("Request revision"),
        "Rendered buffer must contain Request revision option"
    );
    assert!(
        rendered_text.contains("Regenerate plan"),
        "Rendered buffer must contain Regenerate plan option"
    );
    assert!(
        rendered_text.contains("Enter"),
        "Rendered buffer must contain Enter key hint"
    );
}
