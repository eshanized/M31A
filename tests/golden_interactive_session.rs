//! Golden End-to-End Interactive Session Workflow Validation Test.
//!
//! Validates the complete Developer Interaction Layer & Conversational CLI
//! using the REAL configured model provider from `.env` (NVIDIA NIM via SSE streaming).
//!
//! Enforces:
//! 1. Real provider credentials loaded from `.env`.
//! 2. Zero mocked model responses (NO ScriptedAutonomousRepairModel, NO MockProvider).
//! 3. Real interactive session creation and SQLite persistence.
//! 4. Natural language task input with `@file` mentions resolved against the workspace.
//! 5. Autonomous mission execution and independent verification gate.
//! 6. Conversational follow-up turn: "Show me the diff."
//! 7. Conversational follow-up turn: "Commit it." with `M31A-Mission:` trailers.
//! 8. Fresh runtime recreation simulating application restart.
//! 9. Session resumption reconstructing monotonic, multi-turn conversation history.

use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::tempdir;

use m31a::config::env::{SafeEnvironmentStatus, load_dotenv};
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::runtime::AppRuntime;

#[tokio::test]
#[ignore = "requires live external LLM API credentials and WAN connection"]
async fn test_golden_interactive_session() {
    // -----------------------------------------------------------------------
    // 1. Environment & Credential Loading from .env
    // -----------------------------------------------------------------------
    load_dotenv();
    unsafe {
        std::env::set_var("M31A_REAL_MODEL_TEST", "1");
    }

    let env_status = SafeEnvironmentStatus::probe();
    println!("\n=== M31A REAL MODEL INTERACTIVE SESSION GATE ===");
    println!("Provider configured: {}", env_status.provider_configured);
    println!("API key configured:  {}", env_status.api_key_configured);
    println!("Model configured:    {}", env_status.model_configured);

    assert!(
        env_status.api_key_configured,
        "REAL MODEL VALIDATION REQUIREMENT: API key must be present in .env (NVIDIA_API_KEY or API_KEY_NVIDIA)"
    );

    // -----------------------------------------------------------------------
    // 2. Setup Real Git Repository Fixture
    // -----------------------------------------------------------------------
    let dir = tempdir().expect("failed to create fixture tempdir");
    let repo_path = dir.path();

    // Initialize Git repository on main branch
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success(), "git init must succeed");

    Command::new("git")
        .args(["config", "user.name", "M31A Interactive Test Agent"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "interactive-agent@m31a.local"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.email failed");

    // .gitignore
    tokio::fs::write(repo_path.join(".gitignore"), "/target\n.m31a\n")
        .await
        .unwrap();

    // Cargo.toml
    let cargo_toml = r#"[package]
name = "calculator"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    tokio::fs::write(repo_path.join("Cargo.toml"), cargo_toml)
        .await
        .unwrap();

    // src/lib.rs
    tokio::fs::create_dir_all(repo_path.join("src"))
        .await
        .unwrap();
    let initial_lib_rs = r#"//! Calculator library entrypoint.

pub mod parser;
"#;
    tokio::fs::write(repo_path.join("src/lib.rs"), initial_lib_rs)
        .await
        .unwrap();

    // src/parser.rs (Intentionally failing placeholder with domain specification)
    let initial_parser_rs = r#"//! Expression parser implementation.
//!
//! Evaluates whitespace-delimited binary arithmetic expressions: `<num> <op> <num>`.
//! Supports operations: `+`, `-`, `*`.
//!
//! Returns `Ok(result)` on success, or `Err(msg)` on invalid format or unknown operator.

pub fn parse_expression(expr: &str) -> Result<i64, String> {
    // Intentionally failing placeholder to verify autonomous repair.
    // Split `expr` by whitespace into [left, op, right], parse left/right with str::parse::<i64>(), and apply op (+, -, *).
    Err("parser not implemented".to_string())
}
"#;
    tokio::fs::write(repo_path.join("src/parser.rs"), initial_parser_rs)
        .await
        .unwrap();

    // tests/parser_test.rs
    tokio::fs::create_dir_all(repo_path.join("tests"))
        .await
        .unwrap();
    let parser_test_rs = r#"use calculator::parser::parse_expression;

#[test]
fn test_parse_simple_addition() {
    assert_eq!(parse_expression("2 + 3"), Ok(5));
}

#[test]
fn test_parse_multiplication() {
    assert_eq!(parse_expression("4 * 5"), Ok(20));
}

#[test]
fn test_parse_subtraction() {
    assert_eq!(parse_expression("10 - 3"), Ok(7));
}
"#;
    tokio::fs::write(repo_path.join("tests/parser_test.rs"), parser_test_rs)
        .await
        .unwrap();

    // Generate lockfile and commit initial baseline state
    let _ = Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(repo_path)
        .status();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_path)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial commit with failing parser tests"])
        .current_dir(repo_path)
        .status()
        .expect("git commit failed");

    // -----------------------------------------------------------------------
    // 3. Confirm Initial Tests Deterministically Fail
    // -----------------------------------------------------------------------
    let pre_check = Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .expect("cargo test failed to invoke");
    assert!(
        !pre_check.status.success(),
        "Pre-condition: Initial fixture tests must fail before autonomous repair"
    );
    println!("[FIXTURE] Pre-check test failure verified as expected.");

    // -----------------------------------------------------------------------
    // 4. Instantiate Real Model Provider with Request Tracer
    // -----------------------------------------------------------------------
    let captured_requests: Arc<Mutex<Vec<(String, serde_json::Value)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let tracer_sink = captured_requests.clone();

    let real_provider = NvidiaProvider::new(None, None)
        .expect("failed to initialize NvidiaProvider with .env credentials")
        .with_request_tracer(Arc::new(move |endpoint, payload| {
            let mut sanitized = payload.clone();
            if let Some(obj) = sanitized.as_object_mut() {
                obj.remove("api_key");
            }
            if let Some(messages) = sanitized.get("messages").and_then(|m| m.as_array()) {
                let count = messages.len();
                if let Some(last_msg) = messages.last() {
                    let role = last_msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                    if role == "tool" {
                        let content = last_msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        println!(
                            "[stage] tracer-tool-request msgs={count} output_bytes={}",
                            content.len()
                        );
                    } else if role == "assistant" {
                        let tool_calls_count = last_msg
                            .get("tool_calls")
                            .and_then(|tc| tc.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        println!(
                            "[stage] tracer-assistant-request msgs={count} tool_calls_count={tool_calls_count}"
                        );
                    } else {
                        println!("[stage] tracer-user-request msgs={count}");
                    }
                }
            }
            tracer_sink
                .lock()
                .unwrap()
                .push((endpoint.to_string(), sanitized));
        }));

    println!(
        "[PROVIDER] Connected to real provider: base_url = {}",
        real_provider.base_url()
    );

    // -----------------------------------------------------------------------
    // 5. Initialize Production Runtime & Interactive Session Runner
    // -----------------------------------------------------------------------
    let runtime = AppRuntime::new(repo_path)
        .await
        .expect("failed to instantiate AppRuntime")
        .with_model_provider(Arc::new(real_provider));
    let runtime_arc = Arc::new(runtime);

    let mut runner = InteractiveSessionRunner::new(runtime_arc.clone());
    let session = runner
        .init_session(None)
        .await
        .expect("failed to initialize interactive session");
    let session_id = session.id;
    println!("[SESSION] Initialized session: {}", session_id);

    // -----------------------------------------------------------------------
    // 6. Turn 1: Natural Language Task with @file Mentions
    // -----------------------------------------------------------------------
    let turn1_input = "Fix the failing parser test in @tests/parser_test.rs and update the implementation in @src/parser.rs.";
    println!("\n[TURN 1] Operator input: \"{turn1_input}\"");

    let turn1_res = tokio::time::timeout(
        std::time::Duration::from_secs(360),
        runner.handle_input(turn1_input),
    )
    .await
    .expect("Interactive turn 1 timed out after 360s")
    .expect("Turn 1 execution failed");

    assert!(!turn1_res, "Turn 1 should not request session exit");

    // Verify workspace tests now pass
    let post_test = Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .expect("cargo test failed to invoke");
    assert!(
        post_test.status.success(),
        "Tests in workspace must pass after autonomous repair in Turn 1"
    );
    println!("[TURN 1] Workspace cargo test succeeded!");

    // Verify repaired file contents
    let repaired_parser = tokio::fs::read_to_string(repo_path.join("src/parser.rs"))
        .await
        .expect("src/parser.rs must exist");
    assert!(
        !repaired_parser.contains("parser not implemented"),
        "Failing placeholder must be replaced with working code"
    );

    // -----------------------------------------------------------------------
    // 7. Turn 2: Follow-up conversational inspection: "Show me the diff."
    // -----------------------------------------------------------------------
    let turn2_input = "Show me the diff.";
    println!("\n[TURN 2] Operator follow-up: \"{turn2_input}\"");

    let turn2_res = runner
        .handle_input(turn2_input)
        .await
        .expect("Turn 2 execution failed");
    assert!(!turn2_res, "Turn 2 should not request session exit");

    // Verify diff content via runtime
    let diff = runtime_arc
        .get_git_diff()
        .await
        .expect("failed to get git diff");
    println!("[stage] git-diff-preview bytes={}", diff.len());
    assert!(
        diff.contains("parse_expression") || diff.contains("src/parser.rs"),
        "Diff must show changes made to src/parser.rs"
    );

    // -----------------------------------------------------------------------
    // 8. Turn 3: Follow-up conversational action: "Commit it."
    // -----------------------------------------------------------------------
    let turn3_input = "Commit it.";
    println!("\n[TURN 3] Operator follow-up: \"{turn3_input}\"");

    let turn3_res = runner
        .handle_input(turn3_input)
        .await
        .expect("Turn 3 execution failed");
    assert!(!turn3_res, "Turn 3 should not request session exit");

    // Verify commit in git log with M31A trailer
    let git_log = Command::new("git")
        .args(["log", "-n", "3", "--format=%B"])
        .current_dir(repo_path)
        .output()
        .expect("git log failed to invoke");
    let log_content = String::from_utf8_lossy(&git_log.stdout);
    println!(
        "[stage] git-log-trailers lines={}",
        log_content.lines().count()
    );
    assert!(
        log_content.contains("M31A-Mission:"),
        "Git commit must contain M31A-Mission trailer attribution"
    );

    // -----------------------------------------------------------------------
    // 9. Session Resumption Across Runtime Re-initialization
    // -----------------------------------------------------------------------
    println!("\n[RESTART] Simulating CLI termination and session resumption...");
    drop(runner);
    drop(runtime_arc);

    // Fresh runtime on same workspace and database
    let fresh_runtime = AppRuntime::new(repo_path)
        .await
        .expect("failed to instantiate fresh AppRuntime");

    let resumed_session = fresh_runtime
        .resume_session(session_id)
        .await
        .expect("failed to resume session from database");

    let history = fresh_runtime
        .session_repo()
        .get_conversation(session_id)
        .await
        .expect("failed to get conversation history from database");

    println!(
        "[RESUMED] Successfully resumed session: {}",
        resumed_session.id
    );
    assert_eq!(
        resumed_session.id, session_id,
        "Resumed session ID must match original"
    );
    assert!(
        resumed_session.active_mission_id.is_some(),
        "Active mission ID must be retained across restarts"
    );

    println!(
        "[HISTORY] Reconstructed {} conversation turns:",
        history.len()
    );
    for turn in &history {
        println!(
            "  Sequence #{}: [{}] {}",
            turn.sequence(),
            turn.kind_str(),
            turn.text_content()
        );
    }

    // Verify turns are ordered monotonically starting at 1
    assert!(
        history.len() >= 4,
        "Must have recorded multiple conversation turns across all 3 actions"
    );
    for (i, turn) in history.iter().enumerate() {
        assert_eq!(
            turn.sequence(),
            (i + 1) as u64,
            "Turn sequences must be strictly monotonic starting at 1"
        );
    }

    // Verify first turn recorded the @file mentions
    if let m31a::interaction::session::ConversationTurn::UserMessage {
        mentions, raw_text, ..
    } = &history[0]
    {
        assert!(raw_text.contains("@tests/parser_test.rs"));
        assert_eq!(
            mentions.len(),
            2,
            "Both @file mentions must be recorded on the user turn"
        );
        assert_eq!(
            mentions[0].status,
            m31a::interaction::mentions::ResolutionStatus::Resolved,
            "tests/parser_test.rs must be resolved"
        );
        assert_eq!(
            mentions[1].status,
            m31a::interaction::mentions::ResolutionStatus::Resolved,
            "src/parser.rs must be resolved"
        );
    } else {
        panic!("First turn must be a UserMessage with parsed mentions");
    }

    println!("\n=======================================================");
    println!("🎉 GOLDEN INTERACTIVE WORKFLOW GATE: PASS");
    println!("=======================================================\n");
}
