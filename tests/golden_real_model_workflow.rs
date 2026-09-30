//! Real-Model End-to-End Execution Validation Test.
//!
//! Validates the complete autonomous runtime execution spine using the REAL
//! configured model provider from `.env` (NVIDIA NIM via SSE streaming).
//!
//! Enforces:
//! 1. Real provider credentials loaded from `.env`.
//! 2. Zero mocked model responses (NO `ScriptedAutonomousRepairModel`, NO `MockProvider`).
//! 3. Real user prompt: "Fix the failing parser test in @tests/parser_test.rs and update the implementation in @src/parser.rs."
//! 4. Real model DAG planning.
//! 5. Real model tool calls (`read_file`, `run_tests`, `write_file`).
//! 6. Real tool execution in isolated Git worktree.
//! 7. Real tool result observation fed back into subsequent model requests (captured via RequestTraceHook).
//! 8. Real file repair performed by M31A runtime (never by the test).
//! 9. Real independent runtime verification gate (`EvidenceCompletionGate` running cargo test).
//! 10. Real Git commit with `M31A-Mission:` trailer and merge.

use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::tempdir;

use m31a::config::env::{SafeEnvironmentStatus, load_dotenv};
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::runtime::AppRuntime;

#[tokio::test]
#[ignore = "requires live external LLM API credentials and WAN connection"]
async fn test_golden_real_model_workflow() {
    // -----------------------------------------------------------------------
    // 1. Environment & Credential Loading from .env
    // -----------------------------------------------------------------------
    load_dotenv();
    unsafe {
        std::env::set_var("M31A_REAL_MODEL_TEST", "1");
    }

    let env_status = SafeEnvironmentStatus::probe();
    println!("\n=== M31A REAL MODEL VALIDATION GATE ===");
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
        .args(["config", "user.name", "M31A Real Test Agent"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
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

    // Generate lockfile and commit initial state
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
            // Record sanitized outbound request trace (never print authorization headers or secrets)
            let mut sanitized = payload.clone();
            if let Some(obj) = sanitized.as_object_mut() {
                obj.remove("api_key");
            }
            if let Some(messages) = sanitized.get("messages").and_then(|m| m.as_array()) {
                let count = messages.len();
                if let Some(last_msg) = messages.last() {
                    let role = last_msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                    if role == "tool" {
                        let content =
                            last_msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let preview = if content.len() > 2000 {
                            &content[..2000]
                        } else {
                            content
                        };
                        println!(
                            "[TRACER -> MODEL] Request with {count} msgs, last tool output:\n{preview}"
                        );
                    } else if role == "assistant" {
                        println!("[TRACER -> MODEL] Request with {count} msgs (assistant last)");
                    } else {
                        println!("[TRACER -> MODEL] Request with {count} msgs (initial prompt)");
                    }
                }
            } else {
                println!("[TRACER -> MODEL] Outbound text request to endpoint: {endpoint}");
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
    // 5. Initialize Production Runtime with Real Provider
    // -----------------------------------------------------------------------
    let runtime = AppRuntime::new(repo_path)
        .await
        .expect("failed to instantiate AppRuntime")
        .with_model_provider(Arc::new(real_provider));

    // -----------------------------------------------------------------------
    // 6. Execute Autonomous Repair Mission with Real Model
    // -----------------------------------------------------------------------
    let mission_prompt = "Fix the failing parser test in @tests/parser_test.rs and update the implementation in @src/parser.rs.";
    println!("[MISSION] Submitting real task objective: \"{mission_prompt}\"");

    let summary = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        runtime.run_mission(mission_prompt, Some("autonomous"), false),
    )
    .await
    .expect("Golden real model workflow timed out after 300s")
    .expect("autonomous mission execution failed");

    println!("\n=== MISSION EXECUTION COMPLETED ===");
    println!("Status:          {}", summary.status);
    println!("Tasks Completed: {}", summary.tasks_completed);
    println!("Halt Reason:     {:?}", summary.halt_reason);

    // -----------------------------------------------------------------------
    // 7. Verification: Real Model Outbound Request Trace Inspection
    // -----------------------------------------------------------------------
    let requests = captured_requests.lock().unwrap().clone();
    println!(
        "\n[TRACE AUDIT] Recorded {} outbound model requests:",
        requests.len()
    );
    assert!(
        !requests.is_empty(),
        "Real model must have been invoked at least once"
    );

    let mut found_tool_feedback = false;
    for (idx, (_ep, payload)) in requests.iter().enumerate() {
        if let Some(messages) = payload.get("messages").and_then(|m| m.as_array()) {
            println!("  Request #{idx}: {} messages in context", messages.len());
            for msg in messages {
                if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                    if role == "tool" {
                        found_tool_feedback = true;
                        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                        let preview = if content.len() > 120 {
                            &content[..120]
                        } else {
                            content
                        };
                        println!("    -> Multi-turn tool feedback observed: \"{preview}...\"");
                    } else if role == "assistant"
                        && let Some(calls) = msg.get("tool_calls").and_then(|t| t.as_array())
                    {
                        for call in calls {
                            let name = call
                                .get("name")
                                .or_else(|| call.get("function").and_then(|f| f.get("name")))
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            let args = call
                                .get("arguments")
                                .or_else(|| call.get("function").and_then(|f| f.get("arguments")))
                                .map(|a| a.to_string())
                                .unwrap_or_default();
                            let preview = if args.len() > 120 {
                                &args[..120]
                            } else {
                                &args
                            };
                            println!("    -> Model tool call: {name}({preview}...)");
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 8. Assertions: Workflow Integrity & State Invariants
    // -----------------------------------------------------------------------
    assert_eq!(
        summary.status, "Completed",
        "Mission must reach Completed terminal state"
    );
    assert!(
        summary.tasks_completed >= 1,
        "At least one task must be completed in the plan"
    );
    assert!(
        found_tool_feedback,
        "Real model must have received real tool execution results across multi-turn context"
    );

    // -----------------------------------------------------------------------
    // 9. Verification: File System State in Workspace (Post-Merge)
    // -----------------------------------------------------------------------
    let final_parser = tokio::fs::read_to_string(repo_path.join("src/parser.rs"))
        .await
        .expect("src/parser.rs must exist");
    println!("\n[REPAIR RESULT] Final src/parser.rs contents:\n{final_parser}");
    assert!(
        !final_parser.contains("parser not implemented"),
        "M31A must have replaced the failing placeholder implementation"
    );

    // -----------------------------------------------------------------------
    // 10. Independent Verification: Tests in Workspace Must Pass
    // -----------------------------------------------------------------------
    let post_test = Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .expect("cargo test failed to invoke");
    let post_stdout = String::from_utf8_lossy(&post_test.stdout);
    let post_stderr = String::from_utf8_lossy(&post_test.stderr);
    println!(
        "[VERIFICATION] Post-mission cargo test exit: {}",
        post_test.status
    );
    println!("STDOUT:\n{post_stdout}");
    if !post_stderr.is_empty() {
        println!("STDERR:\n{post_stderr}");
    }
    assert!(
        post_test.status.success(),
        "All tests in workspace must pass after autonomous repair!"
    );

    // -----------------------------------------------------------------------
    // 11. Git History Verification: Trailer Attribution & Merge
    // -----------------------------------------------------------------------
    let git_log = Command::new("git")
        .args(["log", "-n", "5", "--format=%B"])
        .current_dir(repo_path)
        .output()
        .expect("git log failed to invoke");
    let log_content = String::from_utf8_lossy(&git_log.stdout);
    println!("\n[GIT LOG TRAILERS]\n{log_content}");
    assert!(
        log_content.contains("M31A-Mission:"),
        "Git history must contain M31A-Mission trailer attribution!"
    );

    println!("\n=======================================================");
    println!("🎉 REAL MODEL GOLDEN WORKFLOW GATE: PASS");
    println!("=======================================================\n");
}
