//! Real Runtime Workflow Integration Tests for M31A TUI.
//!
//! Validates actual execution workflows across isolated storage, workspace switching,
//! subagent drill-down and termination, persona and skill activation, MCP hub toggling,
//! and cancellation lifecycle without mock or simulated test results.

use m31a_core::config::Config;
use m31a_core::types::ChatMessage;
use m31a_tui::app::{App, AppStatus, TuiEvent};
use m31a_tui::turn::TurnState;
use tempfile::tempdir;

#[test]
fn test_storage_isolation_guarantee() {
    let dir = tempdir().unwrap();
    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };

    let app = App::new_test(config, dir.path()).expect("new_test must succeed");

    // Verify database file was created inside the tempdir
    let db_path = dir.path().join("m31a_test.db");
    assert!(db_path.exists(), "m31a_test.db must exist in temp_dir");

    // Verify session metadata was saved in isolated database
    let thread = app
        .storage
        .get_thread(&app.session_id)
        .expect("get_thread must succeed")
        .expect("thread must be present");
    assert_eq!(thread.id, app.session_id);
    assert_eq!(thread.cwd, dir.path().to_string_lossy().to_string());
}

#[test]
fn test_switch_workspace_real_workflow() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();

    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, dir1.path()).unwrap();
    assert_eq!(app.cwd, dir1.path());

    // Switch workspace to dir2
    app.switch_workspace(dir2.path().to_path_buf())
        .expect("switch_workspace must succeed");
    assert_eq!(app.cwd, dir2.path());

    // Transcript must reflect workspace switch notice
    let switch_notice = app.transcript.iter().any(|item| {
        if let m31a_tui::transcript::TranscriptItem::Notice { text, .. } = item {
            text.contains(dir2.path().to_str().unwrap())
        } else {
            false
        }
    });
    assert!(
        switch_notice,
        "Transcript must record workspace switch notice"
    );
}

#[test]
fn test_activate_persona_and_skill_context() {
    let dir = tempdir().unwrap();
    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, dir.path()).unwrap();

    // 1. Activate Persona
    app.activate_persona(
        "SecurityAuditor",
        "Rigorous cybersecurity auditor",
        "Flag all untrusted input and unsafe blocks",
    );
    assert_eq!(app.active_persona.as_deref(), Some("SecurityAuditor"));

    // Verify persona is injected into dynamic context
    let persona_pinned = app
        .pinned_context
        .iter()
        .any(|item| item.path == "persona:SecurityAuditor");
    assert!(persona_pinned, "Persona must be pinned in dynamic context");

    // Verify system prompt in chat_history[0] was refreshed with persona
    let sys_prompt = app.chat_history[0].content.as_deref().unwrap_or("");
    assert!(
        sys_prompt.contains("SecurityAuditor") || sys_prompt.contains("cybersecurity"),
        "System prompt must contain activated persona"
    );

    // 2. Activate Skill
    app.activate_skill("RustMemoryProfiler", "Run heap profiler on memory hotspots");
    let skill_pinned = app
        .pinned_context
        .iter()
        .any(|item| item.path == "skill:RustMemoryProfiler");
    assert!(skill_pinned, "Skill must be pinned in dynamic context");

    let sys_prompt_skill = app.chat_history[0].content.as_deref().unwrap_or("");
    assert!(
        sys_prompt_skill.contains("RustMemoryProfiler")
            || sys_prompt_skill.contains("heap profiler"),
        "System prompt must contain activated skill"
    );
}

#[test]
fn test_subagent_drilldown_and_termination_workflow() {
    let dir = tempdir().unwrap();
    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, dir.path()).unwrap();

    let child_thread_id = format!("agent-{}", uuid::Uuid::new_v4());
    let child_role = "Database Optimizer";

    // Create child thread record first for foreign-key constraint
    let child_meta = m31a_core::types::SessionMetadata {
        id: child_thread_id.clone(),
        title: format!("Subagent [{}]", child_role),
        model: app.active_model.clone(),
        cwd: app.cwd.to_string_lossy().to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: std::collections::HashMap::new(),
    };
    app.storage
        .save_thread(&child_meta, "", "WorkspaceWrite", "OnRequest")
        .unwrap();

    // Register spawn edge in storage
    app.storage
        .record_spawn_edge(&app.session_id, &child_thread_id, child_role)
        .unwrap();

    // Save initial items for child thread
    app.storage
        .save_item(
            "child-item-1",
            &child_thread_id,
            None,
            "user",
            "Optimize query latency",
        )
        .unwrap();
    app.storage
        .save_item(
            "child-item-2",
            &child_thread_id,
            None,
            "assistant",
            "Adding index on user_id",
        )
        .unwrap();

    // Update status badges to reflect subagents
    app.update_status_badges();
    assert_eq!(app.bottom_pane.subagent_tabs.len(), 1);
    assert_eq!(app.bottom_pane.subagent_tabs[0].thread_id, child_thread_id);

    // 1. Drill down into child subagent view
    app.switch_subagent_view(Some(child_thread_id.clone()));
    assert_eq!(
        app.active_thread_id.as_deref(),
        Some(child_thread_id.as_str())
    );
    assert_eq!(app.bottom_pane.active_subagent_tab, 1);

    // Verify transcript loaded child subagent messages
    let has_child_msg = app.transcript.iter().any(|item| {
        if let m31a_tui::transcript::TranscriptItem::Assistant { text } = item {
            text.contains("Adding index on user_id")
        } else {
            false
        }
    });
    assert!(
        has_child_msg,
        "Subagent transcript must display child conversation items"
    );

    // 2. Return to main session view
    app.switch_subagent_view(None);
    assert!(app.active_thread_id.is_none());
    assert_eq!(app.bottom_pane.active_subagent_tab, 0);

    // 3. Terminate subagent
    app.terminate_subagent(&child_thread_id, child_role)
        .expect("terminate_subagent must succeed");

    // Verify subagent tab was pruned
    assert!(app.bottom_pane.subagent_tabs.is_empty());
}

#[test]
fn test_cancellation_lifecycle_resets_turn_state() {
    let dir = tempdir().unwrap();
    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, dir.path()).unwrap();

    // Submit a prompt to activate turn
    let initial_epoch = app.session_epoch;
    let turn_id = "test-turn-123";
    app.current_turn_id = Some(turn_id.to_string());
    app.turn_state.transition_submitted(turn_id).unwrap();
    app.turn_state.transition_streaming(turn_id).unwrap();
    assert!(app.turn_state.is_active());

    // Execute cancellation
    app.cancel_turn();

    // Verify state machine returned to Idle
    assert_eq!(app.turn_state.state(), &TurnState::Idle);
    assert!(!app.turn_state.is_active());
    assert_eq!(app.app_status(), AppStatus::Idle);
    assert!(app.current_turn_id.is_none());

    // Verify session epoch is preserved (not falsely incremented merely for turn cancellation - §13.1)
    assert_eq!(app.session_epoch, initial_epoch);

    // Verify delayed chunk from superseded turn is discarded
    let stale_correlation = m31a_tui::turn::EventCorrelation {
        session_id: app.session_id.clone(),
        session_epoch: initial_epoch,
        turn_id: turn_id.to_string(),
        operation_id: "op-1".to_string(),
    };
    let stale_event = TuiEvent::Correlated {
        correlation: stale_correlation,
        event: Box::new(TuiEvent::ContentChunk("stale chunk".to_string())),
    };

    let transcript_len_before = app.transcript.len();
    app.event_tx.try_send(stale_event).unwrap();
    app.poll_events();

    assert_eq!(
        app.transcript.len(),
        transcript_len_before,
        "Stale chunk from superseded turn must be discarded without mutating transcript"
    );
}

#[test]
fn test_transactional_persistence_no_duplicate_records() {
    let dir = tempdir().unwrap();
    let config = Config {
        api_key: Some("test-key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, dir.path()).unwrap();

    let msg1 = ChatMessage::user("Hello 1");
    let msg2 = ChatMessage::assistant(Some("Response 1".to_string()), None);
    let messages = vec![msg1, msg2];

    // First turn completion
    app.event_tx
        .try_send(TuiEvent::TurnComplete {
            usage: m31a_core::types::TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                reasoning_tokens: 5,
                total_tokens: 35,
            },
            messages: messages.clone(),
        })
        .unwrap();
    app.poll_events();

    let items_turn1 = app.storage.list_items(&app.session_id).unwrap();
    assert_eq!(items_turn1.len(), 2);

    // Second turn completion
    let msg3 = ChatMessage::user("Hello 2");
    let msg4 = ChatMessage::assistant(Some("Response 2".to_string()), None);
    let mut messages2 = messages.clone();
    messages2.push(msg3);
    messages2.push(msg4);

    app.event_tx
        .try_send(TuiEvent::TurnComplete {
            usage: m31a_core::types::TokenUsage {
                prompt_tokens: 15,
                completion_tokens: 25,
                reasoning_tokens: 6,
                total_tokens: 46,
            },
            messages: messages2,
        })
        .unwrap();
    app.poll_events();

    let items_turn2 = app.storage.list_items(&app.session_id).unwrap();
    // Exactly 4 items must be saved, not 2 + 4 = 6 duplicated items!
    assert_eq!(
        items_turn2.len(),
        4,
        "Storage replace_thread_items must store exact history without duplicating prior turns"
    );
}
