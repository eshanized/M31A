//! End-to-End Golden Agentic Coding and Autonomous Repair Integration Test.
//!
//! Classification (Phase 29.5): DETERMINISTIC_CONTRACT_TEST — runtime
//! orchestration regression. The model is scripted (`ScriptedAutonomousRepairModel`);
//! this file proves the execution spine (planning → dispatch → tools →
//! verification → git trailers), NOT model intelligence. Authoritative
//! behavioral proof lives in `tests/phase_29_5_real_model.rs` (real NVIDIA model).
//!
//! Verifies the complete minimum agentic execution spine:
//! 1. User prompt enters runtime.
//! 2. Model plans actionable task DAG (`TASK-01` with `Implementer`).
//! 3. Worker supervisor & dispatcher allocate implementer agent under role envelope.
//! 4. Model calls tools (`run_tests`).
//! 5. Tools execute in isolated git worktree, and test failure stdout/stderr feeds back into multi-turn messages.
//! 6. Model observes failure in conversation context and proposes `write_file` fix.
//! 7. Model re-runs `run_tests` and confirms test pass.
//! 8. Model concludes with `ModelProposal::Complete`.
//! 9. Runtime verification completion gate cryptographically verifies evidence and test pass.
//! 10. Git commit with trailers (`M31A-Mission: <id>`) is created and merged to main.

use async_trait::async_trait;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::kernel::seams::context::CompiledContext;
use m31a::model::types::ChatMessage;
use m31a::runtime::AppRuntime;

const FIXED_LIB_RS: &str = r#"//! Calculator implementation.

pub fn evaluate_expression(expr: &str) -> Result<i64, String> {
    let tokens: Vec<&str> = expr.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err("Expected format: <num> <op> <num>".to_string());
    }
    let left: i64 = tokens[0]
        .parse()
        .map_err(|e| format!("Invalid left operand: {e}"))?;
    let op = tokens[1];
    let right: i64 = tokens[2]
        .parse()
        .map_err(|e| format!("Invalid right operand: {e}"))?;

    match op {
        "+" => Ok(left + right),
        "-" => Ok(left - right),
        "*" => Ok(left * right),
        "/" => {
            if right == 0 {
                Err("Division by zero".to_string())
            } else {
                Ok(left / right)
            }
        }
        _ => Err(format!("Unknown operator: {op}")),
    }
}
"#;

/// Scripted autonomous repair model simulating multi-turn reasoning and tool invocation.
#[derive(Default)]
pub struct ScriptedAutonomousRepairModel {
    pub turns_executed: AtomicUsize,
}

impl ScriptedAutonomousRepairModel {
    pub fn new() -> Self {
        Self {
            turns_executed: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ModelCaller for ScriptedAutonomousRepairModel {
    /// Initial planning turn called by `PlanServiceImpl`.
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        assert!(
            context.contains("MISSION OBJECTIVE")
                || context.contains("planning agent")
                || context.contains("Lead Planner (PromptOS V2)")
                || context.contains("Goal to Decompose"),
            "Expected planning prompt in call_model, got: {}",
            context
        );

        let plan_json = serde_json::json!({
            "tasks": [
                {
                    "id": "TASK-01",
                    "title": "Fix calculator parser tests",
                    "description": "Repair evaluate_expression implementation in src/lib.rs",
                    "depends_on": [],
                    "required_capabilities": ["fs.read", "fs.write", "cargo.test"],
                    "role": "Implementer"
                }
            ]
        });

        Ok(ModelProposal::Complete {
            summary: plan_json.to_string(),
            artifacts: vec![],
        })
    }

    /// Multi-turn execution turns called by `WorkerRunner`.
    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let turn = self.turns_executed.fetch_add(1, Ordering::SeqCst);

        match turn {
            0 => {
                // Turn 1: Propose running tests to observe current baseline/failure
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "run_tests",
                        serde_json::json!({
                            "args": ["--test", "parser_test"]
                        }),
                    )],
                })
            }
            1 => {
                // Turn 2: Verify that test failure was captured in the multi-turn context
                let has_test_failure_in_messages = compiled.messages.iter().any(|m| match m {
                    ChatMessage::Tool { content, .. } => {
                        content.contains("FAILED")
                            || content.contains("panicked")
                            || content.contains("not implemented")
                    }
                    _ => false,
                });
                assert!(
                    has_test_failure_in_messages,
                    "Turn 2: Model must observe test failure in multi-turn messages. Messages: {:?}",
                    compiled.messages
                );

                assert!(
                    compiled.system_prompt.contains("run_tests"),
                    "Turn 2: Model must observe run_tests tool execution in working memory prompt"
                );

                // Propose writing the fixed code to src/lib.rs
                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "write_file",
                        serde_json::json!({
                            "path": "src/lib.rs",
                            "content": FIXED_LIB_RS,
                        }),
                    )],
                })
            }
            2 => {
                // Turn 3: Verify that write_file succeeded and propose re-running tests
                let has_write_success = compiled.messages.iter().any(|m| match m {
                    ChatMessage::Tool { content, .. } => {
                        content.contains("bytes_written") || content.contains("src/lib.rs")
                    }
                    _ => false,
                });
                assert!(
                    has_write_success,
                    "Turn 3: Model must observe write_file success in context. Messages: {:?}",
                    compiled.messages
                );

                Ok(ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "run_tests",
                        serde_json::json!({
                            "args": ["--test", "parser_test"]
                        }),
                    )],
                })
            }
            3 => {
                // Turn 4: Verify that test re-run succeeded and propose completion
                let has_test_success = compiled.messages.iter().any(|m| match m {
                    ChatMessage::Tool { content, .. } => {
                        content.contains("test result: ok") || content.contains("passed")
                    }
                    _ => false,
                });
                assert!(
                    has_test_success,
                    "Turn 4: Model must observe passing test evidence in context. Messages: {:?}",
                    compiled.messages
                );

                Ok(ModelProposal::Complete {
                    summary:
                        "Repaired parser implementation in src/lib.rs. Verified all tests pass."
                            .to_string(),
                    artifacts: vec![],
                })
            }
            _ => Ok(ModelProposal::Complete {
                summary: "Work already complete.".to_string(),
                artifacts: vec![],
            }),
        }
    }
}

#[tokio::test]
async fn test_golden_autonomous_repair_workflow() {
    let dir = tempdir().expect("failed to create fixture tempdir");
    let repo_path = dir.path();

    // 1. Initialize Git repository
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.email failed");

    // 2. Setup Cargo package with failing test
    tokio::fs::write(repo_path.join(".gitignore"), "/target\n.m31a\n")
        .await
        .unwrap();

    let cargo_toml = r#"[package]
name = "calculator"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    tokio::fs::write(repo_path.join("Cargo.toml"), cargo_toml)
        .await
        .unwrap();

    tokio::fs::create_dir_all(repo_path.join("src"))
        .await
        .unwrap();
    let initial_lib_rs = r#"//! Calculator implementation.

pub fn evaluate_expression(_expr: &str) -> Result<i64, String> {
    // Intentionally failing placeholder to verify autonomous repair
    Err("not implemented".to_string())
}
"#;
    tokio::fs::write(repo_path.join("src/lib.rs"), initial_lib_rs)
        .await
        .unwrap();

    tokio::fs::create_dir_all(repo_path.join("tests"))
        .await
        .unwrap();
    let parser_test_rs = r#"use calculator::evaluate_expression;

#[test]
fn test_evaluate_addition() {
    assert_eq!(evaluate_expression("2 + 3"), Ok(5));
}

#[test]
fn test_evaluate_multiplication() {
    assert_eq!(evaluate_expression("4 * 5"), Ok(20));
}
"#;
    tokio::fs::write(repo_path.join("tests/parser_test.rs"), parser_test_rs)
        .await
        .unwrap();

    // 3. Initial commit (generate lockfile first so it is tracked from inception)
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

    // 4. Confirm initial tests actually fail
    let pre_check = Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .expect("cargo test failed to invoke");
    assert!(
        !pre_check.status.success(),
        "Initial fixture tests must fail before agentic repair"
    );

    // 5. Instantiate AppRuntime with scripted model caller
    let model = Arc::new(ScriptedAutonomousRepairModel::new());
    let runtime = AppRuntime::new(repo_path)
        .await
        .expect("failed to instantiate AppRuntime")
        .with_model_caller(model.clone());

    // 6. Execute autonomous repair mission
    let mission_prompt = "Fix the calculator evaluate_expression function to pass parser tests";
    let summary = runtime
        .run_mission(mission_prompt, Some("autonomous"), false)
        .await
        .expect("autonomous mission failed to execute");

    // 7. Verify mission status and execution statistics
    assert_eq!(
        summary.status, "Completed",
        "Mission status must be Completed"
    );
    assert!(
        summary.tasks_completed >= 1,
        "Expected at least 1 completed task, got {}",
        summary.tasks_completed
    );
    assert_eq!(
        model.turns_executed.load(Ordering::SeqCst),
        4,
        "Expected exactly 4 model turns (test -> write -> test -> complete)"
    );

    // 8. Verify tests pass in the workspace after worktree merge
    let post_test = tokio::process::Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("post-mission cargo test failed to invoke");
    assert!(
        post_test.status.success(),
        "Tests in workspace must pass after autonomous repair! Output:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&post_test.stdout),
        String::from_utf8_lossy(&post_test.stderr)
    );

    // 9. Verify workspace src/lib.rs was updated
    let final_lib = tokio::fs::read_to_string(repo_path.join("src/lib.rs"))
        .await
        .unwrap();
    assert!(
        final_lib.contains("tokens.len() != 3"),
        "Workspace src/lib.rs must contain the repaired implementation"
    );

    // 10. Verify git commit trailer was created and merged
    let git_log = tokio::process::Command::new("git")
        .args(["log", "-n", "5", "--format=%B"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("git log failed to invoke");
    let log_content = String::from_utf8_lossy(&git_log.stdout);
    assert!(
        log_content.contains("M31A-Mission:"),
        "Git history must contain M31A-Mission trailer! Git log:\n{}",
        log_content
    );
}
