//! Comprehensive Phase 30 Verification Suite: Continuous Interactive Coding-Agent Loop.
//!
//! Validates:
//! 1. Model proposes, runtime decides: first-class `ModelProposal` turn types.
//! 2. Multi-tool execution in a single model turn preserving call IDs, ordering, and structured outcomes.
//! 3. AssistantText does NOT complete the task (`AssistantText != Complete`).
//! 4. First-class `AskUser` semantic question pause, session persistence, and resume on user response.
//! 5. Policy approval distinction from AskUser (`PolicyDecision::Ask` pauses in `WaitingForApproval`).
//! 6. Authoritative `EvidenceCompletionGate`: mutating code without passing test evidence rejects completion.
//! 7. Passing test evidence allows completion gate to verify and complete.
//! 8. Bounded subagent delegation (max depth <= 2).
//! 9. Mid-session user steering incorporation with fresh context compilation.
//! 10. `skills_list` and `skills_inspect` dynamic discovery tools integration.
//! 11. InteractiveSessionRunner continuous agent loop end-to-end integration.
//! 12. Real-model NVIDIA NIM execution if configured in environment.

use serde_json::json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use m31a::agent::engine::{AgentEngine, AgentEngineState, AgentTurnOutcome};
use m31a::agent::model_policy::{ModelCaller, ModelProposal, TestModelCaller};
use m31a::config::env::SafeEnvironmentStatus;
use m31a::ids::SessionId;
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::interaction::session::{ConversationTurn, SqliteSessionRepository};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::model::types::{ModelToolCall, UserOption};
use m31a::runtime::AppRuntime;
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::skill::{SkillsInspectInput, SkillsInspectTool, SkillsListInput, SkillsListTool};

/// Set up an isolated test fixture with a initialized git repository and AppRuntime.
async fn setup_agentic_fixture() -> (
    tempfile::TempDir,
    Arc<AppRuntime>,
    SessionId,
    SqliteSessionRepository,
) {
    let tmp_root = std::path::PathBuf::from("target/tmp");
    let _ = std::fs::create_dir_all(&tmp_root);
    let dir = tempfile::Builder::new()
        .prefix("m31a-test-")
        .tempdir_in(&tmp_root)
        .expect("failed to create fixture tempdir");
    let repo_path = dir.path();

    // Initialize git repository without copying hook templates
    let git_init = std::process::Command::new("git")
        .args(["init", "--template=", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
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

    let runtime = Arc::new(
        AppRuntime::new(repo_path)
            .await
            .expect("AppRuntime::new failed"),
    );
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(repo_path)
        .await
        .expect("create_session failed");

    (dir, runtime, session.id, session_repo)
}

/// Helper to create a configured test AgentEngine with a mock caller.
///
/// The engine is bound to the runtime-shared capability and tool registries
/// (Invariant 1/2): fixtures MUST NOT fork shadow registries.
fn create_test_agent_engine(
    runtime: &Arc<AppRuntime>,
    session_id: SessionId,
    caller: Arc<dyn ModelCaller>,
) -> AgentEngine {
    let ws = runtime.workspace_root().to_path_buf();
    let capabilities = runtime.capability_registry().clone();
    let tool_registry = runtime.tool_registry().clone();
    let pipeline_runner = Arc::new(m31a::pipeline::runner::ToolPipelineRunner::new(
        tool_registry.clone(),
    ));
    let policy_gate = Arc::new(m31a::kernel::seams::policy::DefaultPolicyGate);
    let approval_coordinator = runtime.approval_coordinator().clone();
    let completion_gate = Arc::new(m31a::verification::gate::EvidenceCompletionGate::new(
        runtime.pool().clone(),
        runtime.artifact_store().clone(),
        runtime.workspace_root(),
    ));
    let context_compiler = Arc::new(
        m31a::context::compiler::ProductionContextCompiler::new()
            .with_workspace_root(runtime.workspace_root().to_path_buf()),
    );
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());

    AgentEngine::new(
        session_id,
        ws,
        session_repo,
        caller,
        tool_registry,
        pipeline_runner,
        policy_gate,
        approval_coordinator,
        completion_gate,
        context_compiler,
        Some(runtime.event_bus().clone()),
        capabilities,
    )
    .with_prompt_catalog(runtime.prompt_catalog_arc())
    .with_prompt_compiler(runtime.authorities().prompt_compiler().clone())
    .with_scope_repos(
        m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
            runtime.pool().clone(),
        ),
        m31a::persistence::sqlite::repositories::SqliteTaskRepository::new(runtime.pool().clone()),
    )
}

// ============================================================================
// 1. Multi-tool execution in a single model turn
// ============================================================================

#[tokio::test]
async fn test_multi_tool_execution_preserves_all_calls_and_results() {
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let calls = vec![
        ModelToolCall {
            id: "call_glob_1".to_string(),
            name: "glob".to_string(),
            arguments: json!({ "pattern": "src/*.rs" }),
        },
        ModelToolCall {
            id: "call_read_2".to_string(),
            name: "read_file".to_string(),
            arguments: json!({ "path": "src/lib.rs" }),
        },
    ];

    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::ToolCalls { calls },
    )]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);

    let outcome = engine
        .step(Some("Inspect the workspace src directory"))
        .await
        .expect("step must succeed");

    match outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 2, "both tool calls must execute");
            assert_eq!(results[0].tool_name, "glob");
            assert_eq!(results[0].call_id, "call_glob_1");
            assert!(results[0].success, "glob should succeed");

            assert_eq!(results[1].tool_name, "read_file");
            assert_eq!(results[1].call_id, "call_read_2");
            assert!(results[1].success, "read_file should succeed");
            assert!(results[1].output.contains("pub fn add"));
        }
        other => panic!("expected ToolResults outcome, got {other:?}"),
    }

    // Verify durable session recorded every step and result with sequence numbers
    let turns = session_repo
        .get_conversation(session_id)
        .await
        .expect("conversation turns");
    assert!(
        turns.len() >= 5,
        "must record user msg, tool call 1, result 1, tool call 2, result 2"
    );

    let has_call_1 = turns.iter().any(|t| match t {
        ConversationTurn::ToolCallMessage { call_id, .. } => call_id == "call_glob_1",
        _ => false,
    });
    let has_res_1 = turns.iter().any(|t| match t {
        ConversationTurn::ToolResultMessage { call_id, .. } => call_id == "call_glob_1",
        _ => false,
    });
    let has_call_2 = turns.iter().any(|t| match t {
        ConversationTurn::ToolCallMessage { call_id, .. } => call_id == "call_read_2",
        _ => false,
    });
    let has_res_2 = turns.iter().any(|t| match t {
        ConversationTurn::ToolResultMessage { call_id, .. } => call_id == "call_read_2",
        _ => false,
    });

    assert!(has_call_1 && has_res_1 && has_call_2 && has_res_2);
}

// ============================================================================
// 2. AssistantText does NOT complete the task
// ============================================================================

#[tokio::test]
async fn test_assistant_text_does_not_complete_task() {
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let narrative = "I have reviewed the codebase structure and will begin work shortly.";
    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::AssistantText {
            content: narrative.to_string(),
        },
    )]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);

    let outcome = engine
        .step(Some("Plan the upcoming changes"))
        .await
        .expect("step must succeed");

    match outcome {
        AgentTurnOutcome::AssistantText { content } => {
            assert_eq!(content, narrative);
        }
        other => panic!("expected AssistantText, got {other:?}"),
    }

    // Engine must NOT be in completed state!
    assert_ne!(
        *engine.state(),
        AgentEngineState::Completed {
            final_summary: String::new()
        }
    );

    // Session records AssistantMessage
    let turns = session_repo.get_conversation(session_id).await.unwrap();
    let has_assistant_msg = turns.iter().any(|t| match t {
        ConversationTurn::AssistantMessage { content, .. } => content == narrative,
        _ => false,
    });
    assert!(has_assistant_msg);
}

// ============================================================================
// 3. AskUser first-class pause, persistence, and resume
// ============================================================================

#[tokio::test]
async fn test_ask_user_first_class_pause_and_resume() {
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let question = "Which database backend should we configure for storage?";
    let options = vec![
        UserOption {
            id: "postgres".to_string(),
            label: "PostgreSQL".to_string(),
            description: None,
        },
        UserOption {
            id: "sqlite".to_string(),
            label: "SQLite".to_string(),
            description: None,
        },
    ];

    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::AskUser {
            question: question.to_string(),
            options: options.clone(),
        }),
        Ok(ModelProposal::AssistantText {
            content: "Configuring PostgreSQL database connection.".to_string(),
        }),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);

    // Step 1: Model asks user
    let outcome1 = engine
        .step(Some("Initialize persistent database storage"))
        .await
        .expect("step 1 must succeed");

    match outcome1 {
        AgentTurnOutcome::WaitingForUser {
            question: q,
            options: opts,
        } => {
            assert_eq!(q, question);
            assert_eq!(opts.len(), 2);
        }
        other => panic!("expected WaitingForUser, got {other:?}"),
    }

    assert!(matches!(
        engine.state(),
        AgentEngineState::WaitingForUser { .. }
    ));

    // Durable turn contains AskUserMessage
    let turns = session_repo.get_conversation(session_id).await.unwrap();
    assert!(turns.iter().any(|t| match t {
        ConversationTurn::AskUserMessage { question: q, .. } => q == question,
        _ => false,
    }));

    // Step 2: Operator provides answer
    engine
        .provide_user_response("postgres")
        .await
        .expect("provide_user_response must succeed");

    assert_eq!(*engine.state(), AgentEngineState::Running);

    // Step 3: Resume continuous loop with fresh compiled context
    let outcome2 = engine.step(None).await.expect("step 2 must succeed");
    match outcome2 {
        AgentTurnOutcome::AssistantText { content } => {
            assert!(content.contains("PostgreSQL"));
        }
        other => panic!("expected AssistantText after answer, got {other:?}"),
    }
}

// ============================================================================
// 4. Policy approval pause and resolution
// ============================================================================

struct AlwaysAskPolicyGate;

#[async_trait::async_trait]
impl PolicyGate for AlwaysAskPolicyGate {
    async fn evaluate(
        &self,
        _request: PolicyEvaluationRequest,
    ) -> Result<PolicyDecision, m31a::kernel::seams::policy::PolicyError> {
        Ok(PolicyDecision::Ask)
    }
}

#[tokio::test]
async fn test_policy_approval_pause_and_resolution() {
    // Single-authority approval flow: `PolicyDecision::Ask` resolves through
    // the REAL `ApprovalCoordinator` inside the pipeline. The user-visible
    // approval ID is a registered coordinator request (Invariant 4); operator
    // resolution unblocks execution; the tool then executes for real.
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "edit_file",
                json!({
                    "path": "src/lib.rs",
                    "old_content": "a + b",
                    "new_content": "a.saturating_add(b)"
                }),
            )],
        },
    )]));

    let ws = runtime.workspace_root().to_path_buf();
    let capabilities = runtime.capability_registry().clone();
    let tool_registry = runtime.tool_registry().clone();
    // Pipeline wired to the authoritative coordinator + pool: approval requests
    // persist and block with timeout (no fabricated IDs, no silent drops).
    let pipeline_runner = Arc::new(
        m31a::pipeline::runner::ToolPipelineRunner::new(tool_registry.clone())
            .with_db_pool(runtime.pool().clone())
            .with_approval_coordinator(runtime.approval_coordinator().clone()),
    );
    let policy_gate: Arc<dyn PolicyGate> = Arc::new(AlwaysAskPolicyGate);
    let approval_coordinator = runtime.approval_coordinator().clone();
    let completion_gate = Arc::new(m31a::verification::gate::EvidenceCompletionGate::new(
        runtime.pool().clone(),
        runtime.artifact_store().clone(),
        runtime.workspace_root(),
    ));
    let context_compiler = Arc::new(
        m31a::context::compiler::ProductionContextCompiler::new()
            .with_workspace_root(runtime.workspace_root().to_path_buf()),
    );

    let mut engine = AgentEngine::new(
        session_id,
        ws,
        session_repo.clone(),
        caller,
        tool_registry,
        pipeline_runner,
        policy_gate,
        approval_coordinator.clone(),
        completion_gate,
        context_compiler,
        Some(runtime.event_bus().clone()),
        capabilities,
    )
    .with_prompt_catalog(runtime.prompt_catalog_arc())
    .with_prompt_compiler(runtime.authorities().prompt_compiler().clone())
    .with_autonomy_mode(m31a::state::intake::AutonomyMode::Autonomous)
    .with_scope_repos(
        m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
            runtime.pool().clone(),
        ),
        m31a::persistence::sqlite::repositories::SqliteTaskRepository::new(runtime.pool().clone()),
    );

    // Drive the turn in the background: it blocks inside the coordinator
    // awaiting the operator decision.
    let mut engine_handle = tokio::spawn(async move {
        engine
            .step(Some("Modify add function with saturating add"))
            .await
    });

    // Wait for the REAL approval request to be registered with the
    // coordinator (never a fabricated UUID).
    let pending = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let ids = approval_coordinator.pending_request_ids().await;
            if let Some(id) = ids.first().copied() {
                return id;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("a real coordinator approval request must be registered");
    assert!(
        !pending.to_string().is_empty(),
        "coordinator request id must be non-empty"
    );

    // Operator approves through the authoritative coordinator.
    approval_coordinator
        .resolve_request(
            pending,
            m31a::policy::approval::ApprovalAction::AllowOnce,
            "test-operator",
        )
        .await
        .expect("coordinator resolution must succeed");

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), &mut engine_handle)
        .await
        .expect("approved turn must complete")
        .expect("spawn join")
        .expect("step must succeed");

    match outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].tool_name, "edit_file");
            assert!(
                results[0].success,
                "approved edit must execute: {:?}",
                results[0].error
            );
        }
        other => panic!("expected ToolResults after approval, got {other:?}"),
    }

    // The approval-gated tool call and its result are durably recorded.
    let turns = session_repo.get_conversation(session_id).await.unwrap();
    assert!(turns.iter().any(|t| matches!(
        t,
        ConversationTurn::ToolCallMessage { tool_name, .. } if tool_name == "edit_file"
    )));
    assert!(
        turns
            .iter()
            .any(|t| matches!(t, ConversationTurn::ToolResultMessage { .. }))
    );
}

// ============================================================================
// 5. Anti-premature completion gate requires verified evidence
// ============================================================================

#[tokio::test]
async fn test_anti_premature_completion_gate_rejection_and_success() {
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    // First model tries to write a file, then prematurely proposes Complete without running tests.
    // The completion gate MUST reject this completion proposal.
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "write_file",
                json!({
                    "path": "src/new_feature.rs",
                    "content": "pub fn multiply(a: i32, b: i32) -> i32 { a * b }\n"
                }),
            )],
        }),
        Ok(ModelProposal::Complete {
            summary: "Implemented new_feature.rs successfully!".to_string(),
            artifacts: Vec::new(),
        }),
        // Now after running tests:
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new("run_tests", json!({}))],
        }),
        Ok(ModelProposal::Complete {
            summary: "Verified with tests and completed!".to_string(),
            artifacts: Vec::new(),
        }),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);

    // 1. Mutate workspace
    let o1 = engine
        .step(Some("Add multiply function"))
        .await
        .expect("write_file step");
    assert!(matches!(o1, AgentTurnOutcome::ToolResults { .. }));

    // 2. Propose completion prematurely
    let o2 = engine.step(None).await.expect("premature complete step");
    match o2 {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].tool_name, "completion_gate");
            assert!(!results[0].success);
            assert!(
                results[0]
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Premature completion rejected")
            );
        }
        other => panic!("expected completion gate rejection ToolResults, got {other:?}"),
    }

    // 3. Execute tests
    let o3 = engine.step(None).await.expect("run_tests step");
    assert!(matches!(o3, AgentTurnOutcome::ToolResults { .. }));

    // 4. Propose completion again -> now it must pass!
    let o4 = engine.step(None).await.expect("final complete step");
    match o4 {
        AgentTurnOutcome::Completed { summary } => {
            assert!(summary.contains("Verified with tests and completed!"));
            assert!(matches!(engine.state(), AgentEngineState::Completed { .. }));
        }
        other => panic!("expected verified Completed outcome, got {other:?}"),
    }

    let turns = session_repo.get_conversation(session_id).await.unwrap();
    assert!(turns.iter().any(|t| match t {
        ConversationTurn::VerificationMessage { passed, .. } => *passed,
        _ => false,
    }));
}

// ============================================================================
// 6. Subagent delegation bounded to max depth
// ============================================================================

#[tokio::test]
async fn test_subagent_delegation_bounded_depth() {
    let (_dir, runtime, session_id, _session_repo) = setup_agentic_fixture().await;

    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::Handoff {
            target_role: "researcher".to_string(),
            reason: "Investigate performance optimization techniques".to_string(),
        },
    )]));

    // When engine is initialized with depth = 2, subagent handoff MUST be rejected
    let mut engine =
        create_test_agent_engine(&runtime, session_id, caller).with_delegation_depth(2);

    let outcome = engine
        .step(Some("Delegate research at depth ceiling"))
        .await
        .unwrap();

    match outcome {
        AgentTurnOutcome::ToolResults { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].tool_name, "handoff");
            assert!(!results[0].success);
            assert!(
                results[0]
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("maximum subagent depth of 2 exceeded")
            );
        }
        other => panic!("expected handoff depth rejection, got {other:?}"),
    }
}

// ============================================================================
// 7. Mid-session user steering incorporation
// ============================================================================

#[tokio::test]
async fn test_user_steering_incorporation_mid_session() {
    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::AssistantText {
            content: "Planning to use PostgreSQL.".to_string(),
        }),
        Ok(ModelProposal::AssistantText {
            content: "Adjusting architecture to use SQLite per user correction.".to_string(),
        }),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);

    // Initial prompt
    let _ = engine.step(Some("Configure database")).await.unwrap();

    // User steering correction
    let outcome = engine
        .step(Some("Actually use SQLite instead of PostgreSQL"))
        .await
        .unwrap();

    match outcome {
        AgentTurnOutcome::AssistantText { content } => {
            assert!(content.contains("Adjusting architecture to use SQLite"));
        }
        other => panic!("expected steered AssistantText, got {other:?}"),
    }

    let turns = session_repo.get_conversation(session_id).await.unwrap();
    let has_steering_turn = turns.iter().any(|t| match t {
        ConversationTurn::UserMessage { raw_text, .. } => {
            raw_text.contains("Actually use SQLite instead of PostgreSQL")
        }
        _ => false,
    });
    assert!(
        has_steering_turn,
        "durable session must record steering turn"
    );
}

// ============================================================================
// 8. Skills discovery tools: skills_list and skills_inspect
// ============================================================================

#[tokio::test]
async fn test_skills_discovery_tools_registered_and_executable() {
    let (_dir, runtime, _session_id, _session_repo) = setup_agentic_fixture().await;

    let ws = runtime.workspace_root().to_path_buf();
    let capabilities = Arc::new(m31a::capability::registry::CapabilityRegistry::production(
        &ws,
        Some(runtime.event_bus().clone()),
        None,
    ));
    let ctx = ToolExecutionContext::new(capabilities, ws.clone(), CancellationToken::new());

    let list_tool = SkillsListTool;
    assert_eq!(list_tool.id(), "skills_list");
    let list_output = list_tool
        .execute(&ctx, SkillsListInput {})
        .await
        .expect("skills_list must succeed");

    println!("Discovered {} engineering skills", list_output.skills.len());
    for s in &list_output.skills {
        assert!(!s.id.is_empty());
        assert!(!s.name.is_empty());
    }

    let inspect_tool = SkillsInspectTool;
    assert_eq!(inspect_tool.id(), "skills_inspect");

    // Inspecting unknown skill produces resource not found
    let inspect_err = inspect_tool
        .execute(
            &ctx,
            SkillsInspectInput {
                skill_id: "nonexistent_skill_xyz".to_string(),
            },
        )
        .await;
    assert!(
        inspect_err.is_err(),
        "inspecting nonexistent skill must fail"
    );
}

// ============================================================================
// 9. InteractiveSessionRunner continuous agent loop integration
// ============================================================================

#[tokio::test]
async fn test_interactive_session_runner_agentic_loop() {
    let (_dir, runtime, _session_id, _session_repo) = setup_agentic_fixture().await;

    let mut runner = InteractiveSessionRunner::new(runtime);

    // Initialize session
    let sess = runner.init_session(None).await.expect("init_session");
    assert_eq!(runner.current_session().unwrap().id, sess.id);

    // Test slash command dispatch
    let should_exit = runner
        .handle_input("/status")
        .await
        .expect("status command");
    assert!(!should_exit);

    // Test diff command
    let should_exit = runner.handle_input("/diff").await.expect("diff command");
    assert!(!should_exit);

    // Test exit command
    let should_exit = runner.handle_input("/exit").await.expect("exit command");
    assert!(should_exit);
}

// ============================================================================
// 10. Live Real-Model Nemotron Test (if API key configured)
// ============================================================================

#[tokio::test]
async fn test_real_model_nemotron_agentic_loop_if_configured() {
    let probe = SafeEnvironmentStatus::probe();
    if !probe.provider_configured {
        println!(
            "[SKIPPED] NVIDIA NIM provider not configured in environment. Skipping live model call."
        );
        return;
    }

    let (_dir, runtime, session_id, session_repo) = setup_agentic_fixture().await;

    let mut engine = runtime.create_agent_engine(session_id);

    println!("[REAL-MODEL] Invoking continuous agent loop with real provider...");
    let outcome = engine
        .step(Some(
            "Inspect the repository using available tools and summarize what you find.",
        ))
        .await;

    match outcome {
        Ok(out) => {
            println!("[REAL-MODEL] Turn 1 outcome: {out:?}");
            match out {
                AgentTurnOutcome::AssistantText { content } => {
                    assert!(!content.trim().is_empty());
                }
                AgentTurnOutcome::ToolResults { results } => {
                    assert!(!results.is_empty());
                }
                AgentTurnOutcome::WaitingForUser { question, .. } => {
                    assert!(!question.is_empty());
                }
                AgentTurnOutcome::WaitingForApproval { request_id, .. } => {
                    assert!(!request_id.is_empty());
                }
                AgentTurnOutcome::Completed { summary } => {
                    assert!(!summary.is_empty());
                }
                _ => {}
            }
        }
        Err(e) => {
            println!("[REAL-MODEL NOTICE] Live invocation error (e.g. rate limit/network): {e}");
        }
    }

    let turns = session_repo.get_conversation(session_id).await.unwrap();
    assert!(
        !turns.is_empty(),
        "real model invocation must record durable turns"
    );
}
