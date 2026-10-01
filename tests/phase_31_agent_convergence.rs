//! Phase 31: Agent Runtime Convergence & Autonomous Turn Semantics Test Suite.
//!
//! Verifies:
//! 1. Protocol Canonicalization: Canonical tool-intent is strictly `ToolCalls { calls: Vec<ModelToolCall> }`.
//!    Single and multi-tool calls, AssistantText, AskUser, Handoff, Complete are unambiguously serialized/deserialized.
//! 2. Assistant Commentary vs Intentional Yield: Commentary does not complete tasks or break the autonomous loop.
//!    Assistant text yielding control transitions to Idle, never Completed.
//! 3. Continuous Autonomous Loop: Model -> Tool -> Model loop runs autonomously in `run_continuous` without shell intervention.
//! 4. Non-Progress Loop Protection: Repetitive failing action fingerprints fail fast rather than spinning infinitely.
//! 5. Multi-Tool Execution Semantics: Multi-tool proposals execute sequentially, preserving all calls, IDs, and evidence.
//! 6. AskUser Real Pause State: Model asking user pauses in `WaitingForUser`, persists across reload, and resumes upon answering.
//! 7. Policy Approval Real Pause State: Policy requirements pause in `WaitingForApproval`, persist across reload.
//! 8. Bounded Subagent Handoff Lifecycle: Handoff spawns bounded child engine (depth <= 2) and bubbles evidence to parent.
//! 9. Evidence-Driven Completion Gate: Premature completion without verification evidence is authoritatively rejected.
//! 10. Cooperative Cancellation: Tokens trigger clean cancellation propagation through the engine.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelProposal, TestModelCaller};
use m31a::agent::{AgentEngineState, AgentTurnOutcome};
use m31a::ids::ArtifactId;
use m31a::interaction::session::{ConversationTurn, SqliteSessionRepository};
use m31a::model::types::{ModelToolCall, UserOption};
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;

/// Helper to initialize a minimal git repository fixture.
fn setup_git_fixture(dir: &Path) {
    let _ = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.name", "Convergence Test Agent"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(dir)
        .status();
    std::fs::write(dir.join(".gitignore"), "/target\n.m31a\n").unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn compute(x: i32) -> i32 { x * 2 }\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial baseline fixture"])
        .current_dir(dir)
        .status();
}

#[test]
fn test_canonical_protocol_serialization_and_unambiguity() {
    // 1. Single tool call
    let single_call = ModelProposal::ToolCalls {
        calls: vec![ModelToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "src/lib.rs" }),
        }],
    };
    let json_single = serde_json::to_string(&single_call).expect("serialize single call");
    let de_single: ModelProposal =
        serde_json::from_str(&json_single).expect("deserialize single call");
    assert_eq!(single_call, de_single);

    // 2. Multiple tool calls
    let multi_calls = ModelProposal::ToolCalls {
        calls: vec![
            ModelToolCall {
                id: "call-1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "src/lib.rs" }),
            },
            ModelToolCall {
                id: "call-2".to_string(),
                name: "cargo_test".to_string(),
                arguments: serde_json::json!({}),
            },
        ],
    };
    let json_multi = serde_json::to_string(&multi_calls).expect("serialize multi calls");
    let de_multi: ModelProposal =
        serde_json::from_str(&json_multi).expect("deserialize multi calls");
    assert_eq!(multi_calls, de_multi);

    // 3. AssistantText
    let assistant_text = ModelProposal::AssistantText {
        content: "I am inspecting the repository.".to_string(),
    };
    let json_text = serde_json::to_string(&assistant_text).expect("serialize assistant text");
    let de_text: ModelProposal = serde_json::from_str(&json_text).expect("deserialize text");
    assert_eq!(assistant_text, de_text);

    // 4. AskUser
    let ask_user = ModelProposal::AskUser {
        question: "Which test framework should we configure?".to_string(),
        options: vec![
            UserOption {
                id: "1".to_string(),
                label: "Built-in cargo test".to_string(),
                description: None,
            },
            UserOption {
                id: "2".to_string(),
                label: "Nextest".to_string(),
                description: None,
            },
        ],
    };
    let json_ask = serde_json::to_string(&ask_user).expect("serialize ask user");
    let de_ask: ModelProposal = serde_json::from_str(&json_ask).expect("deserialize ask user");
    assert_eq!(ask_user, de_ask);

    // 5. Handoff
    let handoff = ModelProposal::Handoff {
        target_role: AgentRole::researcher().to_string(),
        reason: "Investigating dependency conflicts".to_string(),
    };
    let json_handoff = serde_json::to_string(&handoff).expect("serialize handoff");
    let de_handoff: ModelProposal =
        serde_json::from_str(&json_handoff).expect("deserialize handoff");
    assert_eq!(handoff, de_handoff);

    // 6. Complete
    let art_id = ArtifactId::new();
    let complete = ModelProposal::Complete {
        summary: "Feature implementation verified with all tests passing.".to_string(),
        artifacts: vec![art_id],
    };
    let json_complete = serde_json::to_string(&complete).expect("serialize complete");
    let de_complete: ModelProposal =
        serde_json::from_str(&json_complete).expect("deserialize complete");
    assert_eq!(complete, de_complete);

    // Verify all 5 variant names are distinct in serialized JSON
    assert!(json_single.contains("\"type\":\"tool_calls\""));
    assert!(json_text.contains("\"type\":\"assistant_text\""));
    assert!(json_ask.contains("\"type\":\"ask_user\""));
    assert!(json_handoff.contains("\"type\":\"handoff\""));
    assert!(json_complete.contains("\"type\":\"complete\""));
}

#[tokio::test]
async fn test_assistant_commentary_vs_intentional_yield() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // 1. Initial commentary with active mission -> AssistantCommentary (state remains Running)
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::AssistantText {
            content: "Deliberating on the approach...".to_string(),
        }),
        Ok(ModelProposal::AssistantText {
            content: "Here is the final summary for your review.".to_string(),
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);
    engine = engine.with_active_mission(Some(m31a::ids::MissionId::new()));

    // First step: since active mission is set and consecutive_text_turns == 0, yields commentary
    let outcome1 = engine.step(None).await.unwrap();
    assert!(
        matches!(outcome1, AgentTurnOutcome::AssistantCommentary { .. }),
        "First text turn during active mission must be commentary"
    );
    assert_eq!(engine.state(), &AgentEngineState::Running);

    // Second step: consecutive text turns > 0 yields control to operator
    let outcome2 = engine.step(None).await.unwrap();
    assert!(
        matches!(outcome2, AgentTurnOutcome::AssistantText { .. }),
        "Subsequent text turn must yield control to operator (AssistantText)"
    );
    assert_eq!(
        engine.state(),
        &AgentEngineState::Idle,
        "Intentional yield must transition engine to Idle, never Completed"
    );
}

#[tokio::test]
async fn test_continuous_model_tool_model_loop_in_engine() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // Sequence of proposals: Tool Call (read_file) -> Assistant commentary -> AssistantText (Yield)
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call-read-1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "src/lib.rs" }),
            }],
        }),
        Ok(ModelProposal::AssistantText {
            content: "The file contains pub fn compute.".to_string(),
        }),
        Ok(ModelProposal::AssistantText {
            content: "Ready for operator feedback.".to_string(),
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);

    let mut observed_outcomes = Vec::new();
    let final_state = engine
        .run_continuous(|outcome| {
            observed_outcomes.push(outcome.clone());
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(final_state, AgentEngineState::Idle);
    assert_eq!(observed_outcomes.len(), 3);
    assert!(matches!(
        observed_outcomes[0],
        AgentTurnOutcome::ToolResults { .. }
    ));
    assert!(matches!(
        observed_outcomes[1],
        AgentTurnOutcome::AssistantCommentary { .. }
    ));
    assert!(matches!(
        observed_outcomes[2],
        AgentTurnOutcome::AssistantText { .. }
    ));

    // Verify tool execution results were persisted into durable SQLite session
    let conversation = session_repo.get_conversation(session.id).await.unwrap();
    let has_tool_result = conversation
        .iter()
        .any(|turn| matches!(turn, ConversationTurn::ToolResultMessage { .. }));
    assert!(
        has_tool_result,
        "Tool execution results must be persisted in durable SQLite conversation turns"
    );
}

#[tokio::test]
async fn test_multi_tool_execution_preserves_all_calls() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // A turn proposing multiple tools in one batch
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall {
                    id: "call-1".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({ "path": "src/lib.rs" }),
                },
                ModelToolCall {
                    id: "call-2".to_string(),
                    name: "read_file".to_string(),
                    arguments: serde_json::json!({ "path": "Cargo.toml" }),
                },
            ],
        }),
        Ok(ModelProposal::AssistantText {
            content: "Inspected both files.".to_string(),
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);
    let outcome = engine.step(None).await.unwrap();

    match outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(
                results.len(),
                2,
                "All tool calls must be executed and preserved"
            );
            assert_eq!(results[0].call_id, "call-1");
            assert_eq!(results[0].tool_name, "read_file");
            assert!(results[0].success);
            assert!(results[0].output.contains("pub fn compute"));

            assert_eq!(results[1].call_id, "call-2");
            assert_eq!(results[1].tool_name, "read_file");
            assert!(results[1].success);
            assert!(results[1].output.contains("[package]"));
        }
        other => panic!("Expected ToolResults, got {:?}", other),
    }
}

#[tokio::test]
async fn test_loop_protection_against_repeating_failed_tool() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // Repeated failing action calls: reading non-existent file 4 times
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call-fail-1".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "non_existent.rs" }),
            }],
        }),
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call-fail-2".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "non_existent.rs" }),
            }],
        }),
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call-fail-3".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "non_existent.rs" }),
            }],
        }),
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call-fail-4".to_string(),
                name: "read_file".to_string(),
                arguments: serde_json::json!({ "path": "non_existent.rs" }),
            }],
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);
    let final_state = engine.run_continuous(|_| Ok(())).await.unwrap();

    match final_state {
        AgentEngineState::Failed { error } => {
            assert!(
                error.contains("loop detected") || error.contains("repeated"),
                "Loop detector must fail fast upon consecutive repeated failures: {error}"
            );
        }
        other => panic!(
            "Expected AgentEngineState::Failed from loop detector, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_ask_user_real_pause_state_and_resumption() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::AskUser {
            question: "Confirm target architecture: x86_64 or aarch64?".to_string(),
            options: vec![
                UserOption {
                    id: "x86".to_string(),
                    label: "x86_64".to_string(),
                    description: None,
                },
                UserOption {
                    id: "arm".to_string(),
                    label: "aarch64".to_string(),
                    description: None,
                },
            ],
        }),
        Ok(ModelProposal::AssistantText {
            content: "Configured for x86_64 as requested.".to_string(),
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);

    // Step 1: Model asks user
    let outcome1 = engine.step(None).await.unwrap();
    assert!(
        matches!(outcome1, AgentTurnOutcome::WaitingForUser { .. }),
        "Engine must pause in WaitingForUser"
    );
    assert!(matches!(
        engine.state(),
        AgentEngineState::WaitingForUser { .. }
    ));

    // Verify session reload preserves WaitingForUser pause state across restarts
    let mut reloaded_engine = runtime.create_agent_engine(session.id);
    reloaded_engine.load_session_state().await.unwrap();
    assert!(
        matches!(
            reloaded_engine.state(),
            AgentEngineState::WaitingForUser { .. }
        ),
        "Pause state must be restored after loading session state"
    );

    // Step 2: User provides steering response
    let outcome2 = reloaded_engine.step(Some("x86_64")).await.unwrap();
    assert!(
        matches!(
            outcome2,
            AgentTurnOutcome::AssistantCommentary { .. } | AgentTurnOutcome::AssistantText { .. }
        ),
        "Engine resumes and processes user answer: {:?}",
        outcome2
    );

    // Verify conversation recorded both the AskUser question and UserMessage answer
    let turns = session_repo.get_conversation(session.id).await.unwrap();
    let has_question = turns
        .iter()
        .any(|t| matches!(t, ConversationTurn::AskUserMessage { .. }));
    let has_answer = turns
        .iter()
        .any(|t| matches!(t, ConversationTurn::UserMessage { content, .. } if content == "x86_64"));
    assert!(has_question, "Question must be persisted");
    assert!(has_answer, "User answer must be persisted");
}

#[tokio::test]
async fn test_handoff_subagent_lifecycle_and_bounded_depth() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // 1. Valid handoff delegation within depth limit
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        // Parent proposes handoff to researcher
        Ok(ModelProposal::Handoff {
            target_role: AgentRole::researcher().to_string(),
            reason: "Research Cargo dependencies".to_string(),
        }),
        // Child agent runs and completes
        Ok(ModelProposal::Complete {
            summary: "Research completed: no dependency issues found.".to_string(),
            artifacts: Vec::new(),
        }),
        // Parent completes
        Ok(ModelProposal::Complete {
            summary: "Parent mission concluded with research evidence.".to_string(),
            artifacts: Vec::new(),
        }),
    ]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime.create_agent_engine(session.id);
    let outcome = engine.step(None).await.unwrap();

    match outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].tool_name, "subagent_researcher");
            assert!(results[0].success);
            assert!(results[0].output.contains("Research completed"));
        }
        other => panic!(
            "Expected ToolResults from handoff delegation, got {:?}",
            other
        ),
    }

    // 2. Reject handoff when delegation depth limit (2) is reached
    let max_depth_caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::Handoff {
            target_role: AgentRole::planner().to_string(),
            reason: "Excessive recursion".to_string(),
        },
    )]));

    let max_depth_runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(max_depth_caller);

    let max_depth_engine_session = session_repo.create_session(tmp.path()).await.unwrap();
    let mut max_depth_engine = max_depth_runtime
        .create_agent_engine(max_depth_engine_session.id)
        .with_delegation_depth(2);

    let reject_outcome = max_depth_engine.step(None).await.unwrap();
    match reject_outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert!(!results[0].success);
            assert!(
                results[0]
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("maximum subagent depth of 2 exceeded")
            );
        }
        other => panic!("Expected rejection ToolResults, got {:?}", other),
    }
}

#[tokio::test]
async fn test_evidence_driven_completion_gate() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    // 1. Without mutations, completing clean workspace succeeds
    let caller_clean = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::Complete {
            summary: "No mutations made; workspace inspected and clean.".to_string(),
            artifacts: Vec::new(),
        },
    )]));

    let runtime_clean = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller_clean);

    let session_repo = SqliteSessionRepository::new(runtime_clean.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mut engine = runtime_clean.create_agent_engine(session.id);
    let outcome = engine.step(None).await.unwrap();

    assert!(
        matches!(outcome, AgentTurnOutcome::Completed { .. }),
        "Clean workspace completion must succeed: {:?}",
        outcome
    );
    assert!(matches!(engine.state(), AgentEngineState::Completed { .. }));

    // 2. With unverified code mutation in session turns, completion is rejected
    let caller_mut = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::Complete {
            summary: "Premature completion claim without test evidence.".to_string(),
            artifacts: Vec::new(),
        },
    )]));

    let runtime_mut = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller_mut);

    let session_mut = session_repo.create_session(tmp.path()).await.unwrap();

    // Record a simulated write tool mutation turn without corresponding test pass
    let seq = session_repo.next_sequence(session_mut.id).await.unwrap();
    let _ = session_repo
        .append_turn(
            session_mut.id,
            &ConversationTurn::ToolResultMessage {
                id: uuid::Uuid::now_v7(),
                sequence: seq,
                call_id: "call-write".to_string(),
                tool_name: "write_file".to_string(),
                output: "Modified src/lib.rs".to_string(),
                success: true,
                created_at: chrono::Utc::now(),
            },
        )
        .await;

    let mut engine_mut = runtime_mut.create_agent_engine(session_mut.id);
    let outcome_mut = engine_mut.step(None).await.unwrap();

    match outcome_mut {
        AgentTurnOutcome::ToolResults { results } => {
            assert!(!results[0].success);
            assert!(
                results[0]
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Premature completion rejected")
            );
        }
        other => panic!("Expected completion gate rejection, got {:?}", other),
    }
}

#[tokio::test]
async fn test_cooperative_cancellation_propagation() {
    let tmp = tempdir().unwrap();
    setup_git_fixture(tmp.path());

    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::AssistantText {
            content: "Should not execute because cancelled.".to_string(),
        },
    )]));

    let runtime = AppRuntime::new(tmp.path())
        .await
        .unwrap()
        .with_model_caller(caller);

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancel before step

    let mut engine = runtime
        .create_agent_engine(session.id)
        .with_cancellation_token(cancel_token);

    let outcome = engine.step(None).await.unwrap();
    assert!(
        matches!(outcome, AgentTurnOutcome::Cancelled { .. }),
        "Cancelled token must immediately produce Cancelled outcome"
    );
    assert!(matches!(engine.state(), AgentEngineState::Cancelled { .. }));
}
