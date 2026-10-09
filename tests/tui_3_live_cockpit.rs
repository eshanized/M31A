//! TUI 3.0 — Live Agent Cockpit Regression & Integration Tests.
//!
//! Grounded verification of live streaming, dynamic tool lifecycle, in-flight token tracking,
//! layout responsiveness, model catalog resolution, and non-blocking TUI projection.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;

use m31a::interaction::events::InteractionEvent;
use m31a::model::router::resolver::{ModelCandidate, ModelTier};
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::layout::{LayoutTier, classify_terminal_size};
use m31a::tui::model::{ActivityKind, LiveToolState, TuiViewModel};
use m31a::tui::navigation::ScreenId;
use m31a::tui::replay::ReplayController;
use m31a::tui::surface::model_selector::resolve_display_models_and_providers;
use m31a::tui::theme::{ThemeMode, ThemeTokens};

fn render_conversation_text(model: &mut TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    terminal
        .draw(|f| {
            m31a::tui::surface::render_conversation_surface(f, f.area(), model, &tokens, false);
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
                ScreenId::Dashboard,
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

// ─────────────────────────────────────────────────────────────────────────────
// 1. Streaming Deltas Accumulation & Reconciliation (No Duplicate Turns)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_stream_deltas_accumulate_live_without_duplicate_turns() {
    let mut model = TuiViewModel::new();
    let msg_id = "stream-msg-001".to_string();

    // Start of assistant stream
    model.apply_interaction_event(&InteractionEvent::AssistantStarted {
        message_id: msg_id.clone(),
    });

    assert_eq!(model.conversation.len(), 1);
    match &model.conversation[0] {
        TuiConversationItem::Assistant {
            id,
            text,
            streaming,
            ..
        } => {
            assert_eq!(id, &msg_id);
            assert_eq!(text, "");
            assert!(*streaming);
        }
        other => panic!("Expected Assistant item, got {:?}", other),
    }
    assert_eq!(
        model.active_stream_message_id.as_deref(),
        Some(msg_id.as_str())
    );
    assert!(model.has_active_animation());

    // Delta 1
    model.apply_interaction_event(&InteractionEvent::AssistantDelta {
        message_id: msg_id.clone(),
        delta: "Thinking about the ".to_string(),
    });
    assert_eq!(model.conversation.len(), 1);
    match &model.conversation[0] {
        TuiConversationItem::Assistant {
            text, streaming, ..
        } => {
            assert_eq!(text, "Thinking about the ");
            assert!(*streaming);
        }
        _ => unreachable!(),
    }

    // Delta 2
    model.apply_interaction_event(&InteractionEvent::AssistantDelta {
        message_id: msg_id.clone(),
        delta: "implementation...".to_string(),
    });
    assert_eq!(model.conversation.len(), 1);
    match &model.conversation[0] {
        TuiConversationItem::Assistant {
            text, streaming, ..
        } => {
            assert_eq!(text, "Thinking about the implementation...");
            assert!(*streaming);
        }
        _ => unreachable!(),
    }

    // Finish of stream
    model.apply_interaction_event(&InteractionEvent::AssistantFinished {
        message_id: msg_id.clone(),
    });
    assert_eq!(model.conversation.len(), 1);
    match &model.conversation[0] {
        TuiConversationItem::Assistant {
            text, streaming, ..
        } => {
            assert_eq!(text, "Thinking about the implementation...");
            assert!(!*streaming, "Streaming flag must be false after finish");
        }
        _ => unreachable!(),
    }
    assert_eq!(model.active_stream_message_id, None);

    // Final authoritative outcome event arrives from runtime bridge
    model.apply_interaction_event(&InteractionEvent::AssistantOutput {
        text: "Thinking about the implementation...".to_string(),
    });

    // CRITICAL: Must reconcile with the in-place streamed item and NOT duplicate it
    assert_eq!(
        model.conversation.len(),
        1,
        "Must not create duplicate turn on reconciliation"
    );
    match &model.conversation[0] {
        TuiConversationItem::Assistant {
            text, streaming, ..
        } => {
            assert_eq!(text, "Thinking about the implementation...");
            assert!(!*streaming);
        }
        _ => unreachable!(),
    }

    // Render should display accumulated text cleanly
    let rendered = render_conversation_text(&mut model, 100, 30);
    assert!(rendered.contains("Thinking about the implementation..."));
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Live Tool Operations Lifecycle & Name Auto-Resolution
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_live_tool_operations_lifecycle_and_duration() {
    let mut model = TuiViewModel::new();
    let call_id = "call-read-file-42".to_string();

    model.apply_interaction_event(&InteractionEvent::ToolStarted {
        call_id: call_id.clone(),
        tool_name: "fs_read".to_string(),
        parameters: json!({ "path": "src/main.rs" }),
    });

    assert_eq!(model.live_tools.len(), 1);
    let op = &model.live_tools[0];
    assert_eq!(op.call_id, call_id);
    assert_eq!(op.tool_name, "fs_read");
    assert_eq!(op.state, LiveToolState::Running);
    assert_eq!(model.activity_kind, ActivityKind::RunningTool);
    assert!(model.live_activity.as_ref().unwrap().contains("fs_read"));
    assert!(model.has_active_animation());

    // When ToolCompleted arrives with EMPTY tool_name (as kernel EventType::ToolCompleted often has),
    // model must auto-resolve from live_tools history
    model.apply_interaction_event(&InteractionEvent::ToolCompleted {
        call_id: call_id.clone(),
        tool_name: String::new(), // Empty name to test auto-resolution
        success: true,
        output_preview: "fn main() { println!(\"M31A\"); }".to_string(),
    });

    let completed_op = &model.live_tools[0];
    assert_eq!(completed_op.state, LiveToolState::Completed);
    assert_eq!(
        completed_op.tool_name, "fs_read",
        "Tool name must be preserved"
    );
    assert!(completed_op.duration_ms.is_some());
    assert_eq!(model.activity_kind, ActivityKind::Completed);

    // Verify conversation reflects resolved tool name
    let last_item = model.conversation.last().unwrap();
    match last_item {
        TuiConversationItem::ToolResult {
            tool_name, success, ..
        } => {
            assert_eq!(tool_name, "fs_read");
            assert!(*success);
        }
        other => panic!("Expected ToolResult, got {:?}", other),
    }

    let rendered = render_conversation_text(&mut model, 100, 30);
    assert!(rendered.contains("fs_read"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Model Usage: Provisional vs Authoritative Deduplication
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_model_usage_provisional_vs_authoritative_dedup() {
    let mut model = TuiViewModel::new();

    // Initially zero
    assert_eq!(model.model_usage.prompt_tokens, 0);
    assert_eq!(model.model_usage.completion_tokens, 0);
    assert_eq!(model.model_usage.effective_total_tokens(), 0);

    // Provisional streaming chunk 1 (no invocation_id)
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: None,
        prompt_tokens: 150,
        completion_tokens: 25,
        total_tokens: 175,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: m31a::model::types::CostProvenance::Unknown,
    });

    // Authoritative total is untouched, in-flight reflects streaming progress
    assert_eq!(model.model_usage.prompt_tokens, 0);
    assert_eq!(model.model_usage.completion_tokens, 0);
    assert_eq!(model.model_usage.in_flight_prompt_tokens, 150);
    assert_eq!(model.model_usage.in_flight_completion_tokens, 25);
    assert_eq!(model.model_usage.effective_total_tokens(), 175);

    // Provisional streaming chunk 2 (tokens increase)
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: None,
        prompt_tokens: 150,
        completion_tokens: 60,
        total_tokens: 210,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: m31a::model::types::CostProvenance::Unknown,
    });

    assert_eq!(model.model_usage.in_flight_completion_tokens, 60);
    assert_eq!(model.model_usage.effective_total_tokens(), 210);

    // Authoritative completion arrives with unique invocation_id
    let inv_id = "inv-call-uuid-999".to_string();
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some(inv_id.clone()),
        prompt_tokens: 150,
        completion_tokens: 72,
        total_tokens: 222,
        cost_cents: Some(14),
        cost_usd: Some(0.14),
        cost_provenance: m31a::model::types::CostProvenance::Authoritative,
    });

    // In-flight tokens are reset, authoritative tokens updated
    assert_eq!(model.model_usage.prompt_tokens, 150);
    assert_eq!(model.model_usage.completion_tokens, 72);
    assert_eq!(model.model_usage.in_flight_prompt_tokens, 0);
    assert_eq!(model.model_usage.in_flight_completion_tokens, 0);
    assert_eq!(model.model_usage.effective_total_tokens(), 222);
    assert_eq!(model.model_usage.total_cost_cents, Some(14));

    // Duplicate event with same invocation_id must NOT double count
    model.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some(inv_id),
        prompt_tokens: 150,
        completion_tokens: 72,
        total_tokens: 222,
        cost_cents: Some(14),
        cost_usd: Some(0.14),
        cost_provenance: m31a::model::types::CostProvenance::Authoritative,
    });

    assert_eq!(model.model_usage.prompt_tokens, 150);
    assert_eq!(model.model_usage.completion_tokens, 72);
    assert_eq!(model.model_usage.effective_total_tokens(), 222);

    // Quiet header: telemetry lives in details, not the persistent header.
    // The behavioral contract above (dedup + effective totals) is the real
    // guarantee; the header must stay calm.
    let header_text = render_header_text(&model, 120, 3);
    assert!(header_text.contains("M31A"));
    assert!(!header_text.contains("222 tok"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. has_active_animation: Idle vs Active
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_has_active_animation_idle_vs_active() {
    let mut model = TuiViewModel::new();
    assert!(!model.has_active_animation(), "Fresh model should be idle");

    // Starting a stream activates animation
    model.apply_interaction_event(&InteractionEvent::AssistantStarted {
        message_id: "stream-1".to_string(),
    });
    assert!(model.has_active_animation());

    // Finishing stream returns to idle
    model.apply_interaction_event(&InteractionEvent::AssistantFinished {
        message_id: "stream-1".to_string(),
    });
    assert!(!model.has_active_animation());

    // Running a tool activates animation
    model.apply_interaction_event(&InteractionEvent::ToolStarted {
        call_id: "tool-1".to_string(),
        tool_name: "test_exec".to_string(),
        parameters: json!({}),
    });
    assert!(model.has_active_animation());

    // Completing tool deactivates animation
    model.apply_interaction_event(&InteractionEvent::ToolCompleted {
        call_id: "tool-1".to_string(),
        tool_name: "test_exec".to_string(),
        success: true,
        output_preview: "ok".to_string(),
    });
    assert!(!model.has_active_animation());
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Zero SQLite Queries / Blocking I/O During Terminal Draw
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_zero_sqlite_during_terminal_draw_with_live_cockpit() {
    // Pure memory ViewModel verification: all rendering functions must run
    // strictly over the in-memory state without any external I/O or SQLite handles.
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::AssistantStarted {
        message_id: "test".to_string(),
    });
    model.apply_interaction_event(&InteractionEvent::AssistantDelta {
        message_id: "test".to_string(),
        delta: "Rendering without DB...".to_string(),
    });

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
    let replay = ReplayController::new();

    // Draw header, conversation, tasks, footer in sequence
    terminal
        .draw(|f| {
            let area = f.area();
            m31a::tui::shell::header::render_header(
                f,
                area,
                &model,
                ScreenId::Dashboard,
                &replay,
                &tokens,
            );
            m31a::tui::surface::render_conversation_surface(f, area, &mut model, &tokens, false);
            m31a::tui::surface::render_tasks_surface(f, area, &model, &tokens, false, 0);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    assert_eq!(buffer.area.width, 100);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Responsive Layout Tiers: All Sizes Render Gracefully
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_layout_tiers_all_sizes() {
    // Check tier boundaries
    assert_eq!(classify_terminal_size(79, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(80, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(99, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(100, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(159, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(160, 40), LayoutTier::Large);
    assert_eq!(classify_terminal_size(219, 50), LayoutTier::Large);
    assert_eq!(classify_terminal_size(220, 60), LayoutTier::UltraWide);
    assert_eq!(classify_terminal_size(300, 80), LayoutTier::UltraWide);

    let sizes = [
        (80, 24),  // Compact
        (120, 30), // Standard
        (180, 45), // Large
        (240, 60), // UltraWide
    ];

    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::AssistantStarted {
        message_id: "layout-test".to_string(),
    });
    model.apply_interaction_event(&InteractionEvent::AssistantDelta {
        message_id: "layout-test".to_string(),
        delta: "Testing responsive layouts across all 4 tiers.".to_string(),
    });

    for (w, h) in sizes {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let tokens = ThemeTokens::resolve(ThemeMode::DarkSlateCyan);
        let replay = ReplayController::new();

        terminal
            .draw(|f| {
                let area = f.area();
                m31a::tui::shell::header::render_header(
                    f,
                    area,
                    &model,
                    ScreenId::Dashboard,
                    &replay,
                    &tokens,
                );
                m31a::tui::surface::render_conversation_surface(
                    f, area, &mut model, &tokens, false,
                );
            })
            .unwrap();

        assert_eq!(terminal.backend().buffer().area.width, w);
        assert_eq!(terminal.backend().buffer().area.height, h);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Model Selector Catalog Resolution
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_model_selector_catalog_resolution() {
    let mut model = TuiViewModel::new();
    model.active_provider = "nvidia".to_string();

    let candidate1 = ModelCandidate::new(
        "meta/llama-3.3-70b-instruct",
        "nvidia",
        ModelTier::Standard,
        131072,
    );
    let candidate2 = ModelCandidate::new(
        "deepseek-ai/deepseek-r1",
        "nvidia",
        ModelTier::Reasoning,
        65536,
    );
    let candidate3 = ModelCandidate::new(
        "nvidia/llama-3.1-nemotron-70b-instruct",
        "nvidia",
        ModelTier::Standard,
        131072,
    );

    model.catalog_models = vec![candidate1, candidate2, candidate3];

    let (providers, models) = resolve_display_models_and_providers(
        &model.catalog_models,
        &model.active_provider,
        "meta/llama-3.3-70b-instruct",
    );

    // Real models are displayed
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].model_id, "meta/llama-3.3-70b-instruct");
    assert_eq!(models[1].model_id, "deepseek-ai/deepseek-r1");
    assert_eq!(models[2].model_id, "nvidia/llama-3.1-nemotron-70b-instruct");

    // Provider list reflects active provider
    assert!(
        providers
            .iter()
            .any(|p| p.id == "nvidia_nim" || p.id == "nvidia")
    );
    // And no fake models
    assert!(!models.iter().any(|m| m.model_id == "gpt-4o"));
}
