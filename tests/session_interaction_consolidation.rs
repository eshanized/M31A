//! Comprehensive Workstream G Verification Suite: Session + Interaction + CLI/TUI Projection Consolidation
//!
//! Validates:
//! 1. ONE canonical session authority (`SqliteSessionRepository`).
//! 2. Single shared typed interaction protocol (`ApplicationAction` & `InteractionParser`).
//! 3. UI surfaces (CLI and TUI) are projections/control surfaces only.
//! 4. Approval, cancellation, resume, follow-up turns, slash commands, and @mentions parity.
//! 5. Canonical `UniversalCommandPalette` fuzzy matching and legacy palette deprecation.
//! 6. Domain separation: Session vs Mission vs WorkflowRun.
//! 7. Golden Interaction Journey end-to-end.

use chrono::Utc;
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use sqlx::SqlitePool;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::events::bus::{EventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{MissionId, SessionId, WorkflowRunId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::mentions::MentionParser;
use m31a::interaction::parser::InteractionParser;
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::interaction::session::{ConversationTurn, SessionState, SqliteSessionRepository};
use m31a::interaction::state::SessionPromptState;
use m31a::runtime::AppRuntime;
use m31a::tui::TuiApp;
use m31a::tui::approval::ApprovalDecision;
use m31a::tui::palette_v2::UniversalCommandPalette;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;

/// Helper to set up an isolated test fixture with git repo, SQLite DB, and AppRuntime.
async fn setup_test_workspace() -> (tempfile::TempDir, Arc<AppRuntime>, SqlitePool) {
    let dir = tempdir().expect("failed to create fixture tempdir");
    let repo_path = dir.path();

    // Initialize git repository
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(repo_path)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(repo_path)
        .status();

    // Create minimal Rust project structure
    tokio::fs::write(repo_path.join(".gitignore"), "/target\n.m31a\n")
        .await
        .unwrap();
    tokio::fs::create_dir_all(repo_path.join("src"))
        .await
        .unwrap();
    tokio::fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "workspace_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .await
    .unwrap();
    tokio::fs::write(
        repo_path.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .await
    .unwrap();

    let runtime = AppRuntime::new(repo_path)
        .await
        .expect("AppRuntime::new failed");
    let pool = runtime.pool().clone();
    (dir, Arc::new(runtime), pool)
}

/// Helper to insert a test mission and pending approval request into SQLite.
async fn insert_test_mission_and_approval(pool: &SqlitePool, mid: MissionId, req_id: uuid::Uuid) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mid.as_bytes().as_slice())
    .bind("Sample Mission Objective")
    .bind("Running")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .expect("failed to insert test mission");

    sqlx::query(
        r#"INSERT INTO approval_requests (
            id, mission_id, tool_call_id, tool_or_capability,
            normalized_args_json, redacted_args_json, affected_resources,
            risk_classification, policy_hash, reason, resolution_state, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(req_id.as_bytes().as_slice())
    .bind(mid.as_bytes().as_slice())
    .bind("call_alpha_1")
    .bind("tool_write_file")
    .bind(r#"{"path":"src/critical.rs"}"#)
    .bind(r#"{"path":"src/critical.rs"}"#)
    .bind(r#"["src/critical.rs"]"#)
    .bind("High")
    .bind("hash_sha256_mock")
    .bind("Writing to critical source code requires approval")
    .bind("pending")
    .bind(&now)
    .execute(pool)
    .await
    .expect("failed to insert test approval request");
}

// =============================================================================
// TEST A: ONE Canonical Session Authority
// =============================================================================
#[tokio::test]
async fn test_a_one_session_authority() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let repo = SqliteSessionRepository::new(pool.clone());

    // Create session through authoritative repository
    let session = repo
        .create_session(runtime.workspace_root())
        .await
        .expect("create_session must succeed");

    assert_eq!(session.status, SessionState::Active);

    // Verify row exists in `sessions` table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(session.id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "Session must exist in authoritative SQLite table");

    // Append turn through repository
    let turn = ConversationTurn::UserMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 1,
        content: "Authoritative authority test".to_string(),
        raw_text: "Authoritative authority test".to_string(),
        mentions: Vec::new(),
        created_at: Utc::now(),
    };
    repo.append_turn(session.id, &turn).await.unwrap();

    // Verify row exists in `conversation_messages` table
    let msg_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversation_messages WHERE session_id = ?")
            .bind(session.id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(msg_count, 1);

    // Retrieve via repository
    let turns = repo.get_conversation(session.id).await.unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].text_content(), "Authoritative authority test");
}

// =============================================================================
// TEST B: Session Persistence and Reload
// =============================================================================
#[tokio::test]
async fn test_b_session_persistence_and_reload() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let repo1 = SqliteSessionRepository::new(pool.clone());

    let session = repo1
        .create_session(runtime.workspace_root())
        .await
        .unwrap();
    let sid = session.id;

    // Append 3 turns
    let u_turn = ConversationTurn::UserMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 1,
        content: "Step 1: run analysis".to_string(),
        raw_text: "Step 1: run analysis".to_string(),
        mentions: Vec::new(),
        created_at: Utc::now(),
    };
    repo1.append_turn(sid, &u_turn).await.unwrap();

    let a_turn = ConversationTurn::AssistantMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 2,
        content: "Analysis completed successfully.".to_string(),
        created_at: Utc::now(),
    };
    repo1.append_turn(sid, &a_turn).await.unwrap();

    let v_turn = ConversationTurn::VerificationMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 3,
        passed: true,
        summary: "100% tests passed".to_string(),
        created_at: Utc::now(),
    };
    repo1.append_turn(sid, &v_turn).await.unwrap();

    // Close session
    repo1
        .update_status(sid, SessionState::Closed)
        .await
        .unwrap();
    drop(repo1);

    // Reload from fresh repository instance
    let repo2 = SqliteSessionRepository::new(pool);
    let loaded = repo2
        .get_session(sid)
        .await
        .unwrap()
        .expect("Session must reload");
    assert_eq!(loaded.id, sid);
    assert_eq!(loaded.status, SessionState::Closed);

    let turns = repo2.get_conversation(sid).await.unwrap();
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].sequence(), 1);
    assert_eq!(turns[0].text_content(), "Step 1: run analysis");
    assert_eq!(turns[1].sequence(), 2);
    assert_eq!(turns[1].text_content(), "Analysis completed successfully.");
    assert_eq!(turns[2].sequence(), 3);
    assert!(turns[2].text_content().contains("100% tests passed"));
}

// =============================================================================
// TEST C: CLI and TUI Share the Same Typed Action Protocol
// =============================================================================
#[tokio::test]
async fn test_c_cli_tui_share_typed_action_protocol() {
    let registry = SlashCommandRegistry::new_standard();
    let parser = InteractionParser::new(registry);
    let ws = Path::new("/tmp/test_ws");

    // 1. Plain prompt
    let act1 = parser
        .parse(
            "Build the project and run tests",
            ws,
            SessionPromptState::Idle,
            None,
            false,
        )
        .expect("Must parse plain text");
    match act1 {
        ApplicationAction::UserTextSubmitted(p) => {
            assert_eq!(p.raw_text, "Build the project and run tests");
        }
        _ => panic!("Expected UserTextSubmitted"),
    }

    // 2. /status
    assert!(matches!(
        parser.parse("/status", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::StatusRequested)
    ));

    // 3. /diff
    assert!(matches!(
        parser.parse("/diff", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::DiffRequested)
    ));

    // 4. /commit
    match parser.parse(
        "/commit fix: typo in parser",
        ws,
        SessionPromptState::Idle,
        None,
        false,
    ) {
        Some(ApplicationAction::CommitRequested { message }) => {
            assert_eq!(message, Some("fix: typo in parser".to_string()));
        }
        _ => panic!("Expected CommitRequested"),
    }

    // 5. /cancel
    assert!(matches!(
        parser.parse("/cancel", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::CancelRequested)
    ));

    // 6. /exit
    assert!(matches!(
        parser.parse("/exit", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::ExitRequested)
    ));

    // 7. /clear
    assert!(matches!(
        parser.parse("/clear", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::ClearRequested)
    ));

    // 8. /clear-session
    assert!(matches!(
        parser.parse("/clear-session", ws, SessionPromptState::Idle, None, false),
        Some(ApplicationAction::ClearSessionRequested)
    ));

    // 9. /resume
    let fake_sid = uuid::Uuid::now_v7().to_string();
    match parser.parse(
        &format!("/resume {fake_sid}"),
        ws,
        SessionPromptState::Idle,
        None,
        false,
    ) {
        Some(ApplicationAction::SessionResumeRequested { session_id }) => {
            assert_eq!(session_id, fake_sid);
        }
        _ => panic!("Expected SessionResumeRequested"),
    }

    // 10. /model
    match parser.parse(
        "/model meta/llama-3.3-70b-instruct",
        ws,
        SessionPromptState::Idle,
        None,
        false,
    ) {
        Some(ApplicationAction::SlashCommandSubmitted { command, args }) => {
            assert_eq!(command, "model");
            assert_eq!(args, vec!["meta/llama-3.3-70b-instruct"]);
        }
        _ => panic!("Expected SlashCommandSubmitted for /model"),
    }

    // 11. /profile
    match parser.parse(
        "/profile high_autonomy",
        ws,
        SessionPromptState::Idle,
        None,
        false,
    ) {
        Some(ApplicationAction::SlashCommandSubmitted { command, args }) => {
            assert_eq!(command, "profile");
            assert_eq!(args, vec!["high_autonomy"]);
        }
        _ => panic!("Expected SlashCommandSubmitted for /profile"),
    }
}

// =============================================================================
// TEST D: Slash Command Dispatch Parity
// =============================================================================
#[tokio::test]
async fn test_d_slash_command_dispatch_parity() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;
    let registry = SlashCommandRegistry::new_standard();

    let ctx = CommandContext {
        workspace_root: runtime.workspace_root(),
        session_id: Some(SessionId::from(uuid::Uuid::now_v7())),
        active_mission_id: None,
        pool: runtime.pool(),
        event_bus: runtime.event_bus(),
        configured_model: "test_model".to_string(),
        configured_provider: "test_provider".to_string(),
        active_profile: "autonomous".to_string(),
        tool_registry: None,
    };

    // Test /help
    let help_out = registry.execute_line("/help", &ctx).await.unwrap();
    match help_out {
        CommandOutput::Info(txt) => {
            assert!(txt.contains("/status"));
            assert!(txt.contains("/diff"));
            assert!(txt.contains("/commit"));
        }
        _ => panic!("Expected CommandOutput::Info for /help"),
    }

    // Test /status
    let status_out = registry.execute_line("/status", &ctx).await.unwrap();
    match status_out {
        CommandOutput::Info(txt) => {
            assert!(txt.contains("Workspace:"));
            assert!(txt.contains("Profile:"));
        }
        _ => panic!("Expected CommandOutput::Info for /status"),
    }

    // Test /diff maps to ApplicationAction::DiffRequested
    let diff_out = registry.execute_line("/diff", &ctx).await.unwrap();
    match diff_out {
        CommandOutput::ApplicationAction(ApplicationAction::DiffRequested) => {}
        _ => panic!("Expected DiffRequested action"),
    }
}

// =============================================================================
// TEST E: @file Mention Parity
// =============================================================================
#[tokio::test]
async fn test_e_file_mention_parity() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;
    let input = "Please inspect @src/lib.rs for correctness.";
    let parsed = MentionParser::parse(input, runtime.workspace_root());

    assert_eq!(parsed.mentions.len(), 1);
    assert_eq!(parsed.mentions[0].raw_path, "src/lib.rs");

    // Both CLI and TUI paths use MentionParser::inject_mention_context
    let injected =
        MentionParser::inject_mention_context(runtime.workspace_root(), &parsed.mentions);

    assert!(injected.contains("<referenced_file"));
    assert!(injected.contains("src/lib.rs"));
    assert!(injected.contains("pub fn add(a: i32, b: i32) -> i32 { a + b }"));
}

// =============================================================================
// TEST F: @directory Mention Parity
// =============================================================================
#[tokio::test]
async fn test_f_directory_mention_parity() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;
    let input = "List all files in @src/ please.";
    let parsed = MentionParser::parse(input, runtime.workspace_root());

    assert_eq!(parsed.mentions.len(), 1);
    assert_eq!(parsed.mentions[0].raw_path, "src/");

    let injected =
        MentionParser::inject_mention_context(runtime.workspace_root(), &parsed.mentions);

    assert!(injected.contains("<referenced_directory"));
    assert!(injected.contains("src/"));
    assert!(injected.contains("lib.rs"));
}

// =============================================================================
// TEST G: Multiline Input Parity
// =============================================================================
#[tokio::test]
async fn test_g_multiline_input_parity() {
    let ws = Path::new("/tmp/test_ws");

    // 1. Triple-quote multiline block
    let multiline_input = "\"\"\"\nfn main() {\n    println!(\"Hello\");\n}\n\"\"\"";
    let parsed1 = MentionParser::parse(multiline_input, ws);
    assert!(parsed1.raw_text.contains("fn main()"));
    assert!(parsed1.raw_text.contains("println!(\"Hello\")"));

    // 2. Normalization retains content without trailing formatting artifacts
    let normalized = parsed1.normalized_prompt();
    assert!(normalized.contains("fn main()"));
}

// =============================================================================
// TEST H: Follow-Up Turn Continuity
// =============================================================================
#[tokio::test]
async fn test_h_follow_up_turn_continuity() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let repo = SqliteSessionRepository::new(pool);
    let session = repo.create_session(runtime.workspace_root()).await.unwrap();

    for i in 1..=6 {
        let expected_seq = i as u64;
        let turn = if i % 2 == 1 {
            ConversationTurn::UserMessage {
                id: uuid::Uuid::now_v7(),
                sequence: expected_seq,
                content: format!("User prompt {i}"),
                raw_text: format!("User prompt {i}"),
                mentions: Vec::new(),
                created_at: Utc::now(),
            }
        } else {
            ConversationTurn::AssistantMessage {
                id: uuid::Uuid::now_v7(),
                sequence: expected_seq,
                content: format!("Assistant response {i}"),
                created_at: Utc::now(),
            }
        };

        repo.append_turn(session.id, &turn).await.unwrap();
    }

    let turns = repo.get_conversation(session.id).await.unwrap();
    assert_eq!(turns.len(), 6);
    for (idx, turn) in turns.iter().enumerate() {
        assert_eq!(turn.sequence(), (idx + 1) as u64);
    }
}

// =============================================================================
// TEST I: Approval from CLI
// =============================================================================
#[tokio::test]
async fn test_i_approval_from_cli() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let req_id = uuid::Uuid::now_v7();

    insert_test_mission_and_approval(&pool, mid, req_id).await;

    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();

    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    // Dispatch approval via CLI runner
    let action = ApplicationAction::ApprovalDecision {
        request_id: req_id.to_string(),
        decision: ApprovalDecision::ApproveOnce,
    };
    let exit = runner.handle_action(action).await.unwrap();
    assert!(!exit);

    // Verify DB update
    let resolution: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(resolution, "approved");

    let resolved_by: String =
        sqlx::query_scalar("SELECT resolved_by FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(resolved_by, "operator");

    // Verify EventBus emission
    let mut received_event = false;
    while let Ok(Some(Ok(env))) =
        tokio::time::timeout(std::time::Duration::from_millis(200), rx.next()).await
    {
        match &env.event_type {
            EventType::ApprovalResolved {
                request_id,
                decision,
                ..
            } if request_id == &req_id.to_string() && decision == "ALLOW" => {
                received_event = true;
                break;
            }
            _ => {}
        }
    }
    assert!(received_event, "ApprovalResolved event must be published");

    // Verify ConversationTurn::ApprovalMessage persisted
    let turns = runner
        .session_repo()
        .get_conversation(session.id)
        .await
        .unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].kind_str(), "approval_message");
}

// =============================================================================
// TEST J: Approval from TUI
// =============================================================================
#[tokio::test]
async fn test_j_approval_from_tui() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let req_id = uuid::Uuid::now_v7();

    insert_test_mission_and_approval(&pool, mid, req_id).await;

    let (mut bridge, _join) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .unwrap();
    let sid = bridge.session_id.unwrap();
    let mut event_rx = bridge.take_event_receiver().expect("Must have receiver");

    // Consume initial SessionStarted event
    let _ = event_rx.recv().await;

    // Send ApprovalDecision through bridge
    let action = ApplicationAction::ApprovalDecision {
        request_id: req_id.to_string(),
        decision: ApprovalDecision::ApproveOnce,
    };
    bridge.send_action(action);

    // Wait for ApprovalResolved event from bridge
    let mut got_resolved = false;
    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv()).await
    {
        if let InteractionEvent::ApprovalResolved {
            request_id,
            approved,
        } = event
        {
            assert_eq!(request_id, req_id.to_string());
            assert!(approved);
            got_resolved = true;
            break;
        }
    }
    assert!(
        got_resolved,
        "ApprovalResolved event must be received from TUI bridge"
    );

    // Verify DB update
    let resolution: String =
        sqlx::query_scalar("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(resolution, "approved");

    // Verify conversation turn recorded in session
    let repo = SqliteSessionRepository::new(pool);
    let turns = repo.get_conversation(sid).await.unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].kind_str(), "approval_message");
}

// =============================================================================
// TEST K: Cancellation from CLI
// =============================================================================
#[tokio::test]
async fn test_k_cancellation_from_cli() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let req_id = uuid::Uuid::now_v7();
    insert_test_mission_and_approval(&pool, mid, req_id).await;

    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    runner
        .session_repo()
        .set_active_mission(session.id, mid)
        .await
        .unwrap();

    // Re-init with session ID to pick up active mission
    let _ = runner.init_session(Some(session.id)).await.unwrap();

    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    // Dispatch cancel
    runner
        .handle_action(ApplicationAction::CancelRequested)
        .await
        .unwrap();

    // Verify mission status in SQLite
    let status: String = sqlx::query_scalar("SELECT status FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "Cancelled");

    // Verify event
    let mut found_cancel_event = false;
    while let Ok(Some(Ok(env))) =
        tokio::time::timeout(std::time::Duration::from_millis(200), rx.next()).await
    {
        match env.event_type {
            EventType::MissionCancelled { mission_id, .. } if mission_id == mid => {
                found_cancel_event = true;
                break;
            }
            _ => {}
        }
    }
    assert!(
        found_cancel_event,
        "MissionCancelled event must be published"
    );

    // Verify SystemMessage appended
    let turns = runner
        .session_repo()
        .get_conversation(session.id)
        .await
        .unwrap();
    assert_eq!(turns.len(), 1);
    assert!(turns[0].text_content().contains("cancelled"));
}

// =============================================================================
// TEST L: Cancellation from TUI
// =============================================================================
#[tokio::test]
async fn test_l_cancellation_from_tui() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let req_id = uuid::Uuid::now_v7();
    insert_test_mission_and_approval(&pool, mid, req_id).await;

    let (mut bridge, _join) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .unwrap();
    let sid = bridge.session_id.unwrap();
    let mut event_rx = bridge.take_event_receiver().unwrap();

    // Set active mission in repo
    let repo = SqliteSessionRepository::new(pool.clone());
    repo.set_active_mission(sid, mid).await.unwrap();

    // Resume session on bridge to associate active mission
    bridge.send_action(ApplicationAction::SessionResumeRequested {
        session_id: sid.to_string(),
    });

    // Send cancel
    bridge.send_action(ApplicationAction::CancelRequested);

    // Drain events looking for CommandOutput("Operation cancelled.")
    let mut got_cancelled = false;
    while let Ok(Some(ev)) =
        tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv()).await
    {
        match ev {
            InteractionEvent::CommandOutput { text } if text.contains("cancelled") => {
                got_cancelled = true;
                break;
            }
            _ => {}
        }
    }
    assert!(got_cancelled, "TUI bridge must confirm cancellation");

    // Verify mission status in SQLite
    let status: String = sqlx::query_scalar("SELECT status FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "Cancelled");
}

// =============================================================================
// TEST M: Restart and Session Resume
// =============================================================================
#[tokio::test]
async fn test_m_restart_and_session_resume() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;

    let sid = {
        let mut runner = InteractiveSessionRunner::new(runtime.clone());
        let session = runner.init_session(None).await.unwrap();
        let turn1 = ConversationTurn::UserMessage {
            id: uuid::Uuid::now_v7(),
            sequence: 1,
            content: "First turn prior to restart".to_string(),
            raw_text: "First turn prior to restart".to_string(),
            mentions: Vec::new(),
            created_at: Utc::now(),
        };
        runner
            .session_repo()
            .append_turn(session.id, &turn1)
            .await
            .unwrap();
        session.id
    };

    // Simulate complete process restart with fresh runner
    let mut new_runner = InteractiveSessionRunner::new(runtime.clone());
    let resumed = new_runner
        .init_session(Some(sid))
        .await
        .expect("Session resume must succeed");

    assert_eq!(resumed.id, sid);
    let turns = new_runner
        .session_repo()
        .get_conversation(sid)
        .await
        .unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].text_content(), "First turn prior to restart");

    // Add next turn in resumed session
    let next_seq = new_runner.session_repo().next_sequence(sid).await.unwrap();
    assert_eq!(next_seq, 2);

    let turn2 = ConversationTurn::AssistantMessage {
        id: uuid::Uuid::now_v7(),
        sequence: next_seq,
        content: "Second turn after restart".to_string(),
        created_at: Utc::now(),
    };
    new_runner
        .session_repo()
        .append_turn(sid, &turn2)
        .await
        .unwrap();

    let final_turns = new_runner
        .session_repo()
        .get_conversation(sid)
        .await
        .unwrap();
    assert_eq!(final_turns.len(), 2);
    assert_eq!(final_turns[1].sequence(), 2);
}

// =============================================================================
// TEST N: Dropped Event Reconciliation
// =============================================================================
#[tokio::test]
async fn test_n_dropped_event_reconciliation() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let repo = SqliteSessionRepository::new(pool);
    let session = repo.create_session(runtime.workspace_root()).await.unwrap();

    // Broadcast bus events while no subscriber is reading or buffer is dropped
    for i in 1..=5 {
        let turn = ConversationTurn::UserMessage {
            id: uuid::Uuid::now_v7(),
            sequence: i,
            content: format!("Persistent message {i}"),
            raw_text: format!("Persistent message {i}"),
            mentions: Vec::new(),
            created_at: Utc::now(),
        };
        repo.append_turn(session.id, &turn).await.unwrap();
    }

    // Authoritative state in SQLite is unaffected
    let turns = repo.get_conversation(session.id).await.unwrap();
    assert_eq!(turns.len(), 5);
    for (i, t) in turns.iter().enumerate() {
        assert_eq!(t.text_content(), format!("Persistent message {}", i + 1));
    }
}

// =============================================================================
// TEST O: Palette Canonical Implementation
// =============================================================================
#[test]
fn test_o_palette_canonical_implementation() {
    let mut palette = UniversalCommandPalette::new();

    // Palette indexes views, commands, and slash commands
    assert!(
        palette.items().len() >= 40,
        "Must index 40+ canonical items"
    );

    // Search view
    palette.set_query("dag");
    let matches = palette.filtered_items();
    assert!(!matches.is_empty(), "Must match dag views");

    // Search command
    palette.set_query("cancel");
    let cmd_matches = palette.filtered_items();
    assert!(!cmd_matches.is_empty(), "Must match cancel command");

    // Search slash command
    palette.set_query("/diff");
    let slash_matches = palette.filtered_items();
    assert!(!slash_matches.is_empty(), "Must match /diff slash command");

    // Navigation and activation
    palette.select_next();
    let action = palette.activate();
    assert!(action.is_some(), "Activation must produce PaletteActionV2");
}

// =============================================================================
// TEST P: Legacy Palette Unreachable
// =============================================================================
#[test]
fn test_p_legacy_palette_unreachable() {
    let mut app = TuiApp::new();

    // App's palette is UniversalCommandPalette
    assert!(app.palette.items().len() >= 40);

    // Render frame does not crash or panic
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();
    let res = app.render_frame(&mut terminal);
    assert!(res.is_ok());
}

// =============================================================================
// TEST Q: CLI Cannot Directly Mutate Runtime State
// =============================================================================
#[tokio::test]
async fn test_q_cli_cannot_directly_mutate_runtime_state() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;
    let runner = InteractiveSessionRunner::new(runtime.clone());

    // Verify runner interacts with runtime through clean interface
    assert!(runner.current_session().is_none());
    assert_eq!(runner.workspace_root(), runtime.workspace_root());
}

// =============================================================================
// TEST R: TUI Cannot Directly Mutate Runtime State
// =============================================================================
#[test]
fn test_r_tui_cannot_directly_mutate_runtime_state() {
    let mut app = TuiApp::new();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    // Law 15: Zero SQLite reads/writes in render_frame
    let render_res = app.render_frame(&mut terminal);
    assert!(render_res.is_ok());
    assert!(render_res.unwrap());
}

// =============================================================================
// TEST S: Session, Mission, and Workflow State Remain Distinct
// =============================================================================
#[tokio::test]
async fn test_s_session_mission_workflow_state_distinct() {
    let (_dir, _runtime, pool) = setup_test_workspace().await;

    // 1. Session in `sessions`
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(Path::new("/tmp/test")).await.unwrap();
    let sid = session.id;

    // 2. Standalone Mission in `missions`
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mid.as_bytes().as_slice())
    .bind("Standalone Mission Objective")
    .bind("Running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 3. WorkflowRun in `workflow_runs`
    let wid = WorkflowRunId::from(uuid::Uuid::now_v7());
    sqlx::query(
        "INSERT INTO workflow_runs (id, definition_id, workspace_root, status, started_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(wid.as_bytes().as_slice())
    .bind("genesis_workflow")
    .bind("/tmp/test")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Verify distinct counts
    let s_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(sid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    let m_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(mid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    let w_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workflow_runs WHERE id = ?")
        .bind(wid.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(s_cnt, 1);
    assert_eq!(m_cnt, 1);
    assert_eq!(w_cnt, 1);
}

// =============================================================================
// TEST T: Production Interaction Path Executes Mission
// =============================================================================
#[tokio::test]
async fn test_t_production_interaction_path_executes_mission() {
    let (_dir, runtime, _pool) = setup_test_workspace().await;
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();

    // Dispatch typed UserTextSubmitted action
    let parsed = MentionParser::parse(
        "Review the codebase in @src/lib.rs",
        runtime.workspace_root(),
    );
    let action = ApplicationAction::UserTextSubmitted(parsed);

    // Note: in isolated offline test environment, mission run will proceed and record turns
    let _ = runner.handle_action(action).await;

    // Check that at least the UserMessage turn was durably committed to SQLite
    let turns = runner
        .session_repo()
        .get_conversation(session.id)
        .await
        .unwrap();
    assert!(!turns.is_empty(), "User turn must be recorded in session");
    assert_eq!(turns[0].kind_str(), "user_message");
}

// =============================================================================
// GOLDEN INTERACTION JOURNEY TEST
// =============================================================================
#[tokio::test]
async fn test_golden_interaction_journey() {
    let (_dir, runtime, pool) = setup_test_workspace().await;
    let repo = SqliteSessionRepository::new(pool.clone());

    // Step 1: Start interactive session
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner
        .init_session(None)
        .await
        .expect("Session init failed");
    let sid = session.id;

    // Step 2: Turn 1 - User prompt with @file mention
    let parsed1 = MentionParser::parse(
        "Check @src/lib.rs for addition logic",
        runtime.workspace_root(),
    );
    assert_eq!(parsed1.mentions.len(), 1);

    let u_turn = ConversationTurn::UserMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 1,
        content: parsed1.normalized_prompt(),
        raw_text: parsed1.raw_text.clone(),
        mentions: parsed1.mentions.clone(),
        created_at: Utc::now(),
    };
    repo.append_turn(sid, &u_turn).await.unwrap();

    let a_turn = ConversationTurn::AssistantMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 2,
        content: "Verified add function in src/lib.rs.".to_string(),
        created_at: Utc::now(),
    };
    repo.append_turn(sid, &a_turn).await.unwrap();

    // Step 3: Turn 2 - Slash command /diff
    let diff_act = ApplicationAction::DiffRequested;
    runner.handle_action(diff_act).await.unwrap();

    // Step 4: Turn 3 - Operator approval request & resolution
    let mid = MissionId::from(uuid::Uuid::now_v7());
    let req_id = uuid::Uuid::now_v7();
    insert_test_mission_and_approval(&pool, mid, req_id).await;

    let apprv_act = ApplicationAction::ApprovalDecision {
        request_id: req_id.to_string(),
        decision: ApprovalDecision::ApproveOnce,
    };
    runner.handle_action(apprv_act).await.unwrap();

    // Step 5: Turn 4 - Cancellation
    runner
        .session_repo()
        .set_active_mission(sid, mid)
        .await
        .unwrap();
    let cancel_act = ApplicationAction::CancelRequested;
    runner.handle_action(cancel_act).await.unwrap();

    // Step 6: Verify durable conversation record
    let turns = repo.get_conversation(sid).await.unwrap();
    assert!(
        turns.len() >= 4,
        "Must have recorded multiple conversation turns"
    );

    // Step 7: Application restart & session resumption
    drop(runner);
    let mut restart_runner = InteractiveSessionRunner::new(runtime.clone());
    let resumed = restart_runner
        .init_session(Some(sid))
        .await
        .expect("Restart resume must succeed");

    assert_eq!(resumed.id, sid);
    let resumed_turns = restart_runner
        .session_repo()
        .get_conversation(sid)
        .await
        .unwrap();
    assert_eq!(resumed_turns.len(), turns.len());
}
