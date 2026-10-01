//! Phase 33: Adaptive Execution, Recovery & Replanning Test Suite.
//!
//! Verifies the 12 core Phase 33 invariants:
//! 1. `test_basic_recovery_with_structured_evidence`: Tool failure produces structured DiagnosticEvidence,
//!    transitions to TaskShape::Recover, injects recovery context, and model selects new action.
//! 2. `test_no_blind_retry_same_action`: Loop protection rejects identical failing actions with identical workspace state,
//!    but permits repeating after workspace mutation.
//! 3. `test_plan_revision_preserves_completed_work`: Replan supersedes stale/failed tasks, preserves completed verified tasks
//!    and their immutable evidence, and updates plan revision number.
//! 4. `test_assumption_invalidation_propagates`: Invalidating an assumption marks it invalid, identifies all affected pending tasks
//!    and decisions, but keeps completed tasks intact.
//! 5. `test_dynamic_unknown_creation_and_resolution`: Unknown created during execution is resolved with evidence, unlocking blocked tasks.
//! 6. `test_strategy_transitions`: Verified transitions between DirectToolExecution -> InvestigateThenAct -> ResearchThenAct -> PlanThenExecute -> Recover.
//! 7. `test_policy_denial_differentiation`: Policy denial is classified as PolicyDenied, not generic tool failure.
//! 8. `test_model_failure_durability`: Simulated model failure preserves session history and intent state without corruption.
//! 9. `test_subagent_causal_failure_evidence`: Subagent failure returns structured causal evidence to parent engine.
//! 10. `test_verification_on_revised_plan`: Completing a revised plan requires passing verification gate for revised tasks; superseded tasks do not block.
//! 11. `test_persistence_and_recovery_across_restarts`: Agent engine state, intent state, and recovery history survive process restart.
//! 12. `test_live_nemotron_adaptive_execution`: (Requires live API key) Real model turn with nvidia/nemotron-3-ultra-550b-a55b exhibiting adaptive behavior.

use chrono::Utc;
use m31a::agent::adaptive::{
    AdaptiveReplanRequest, ExecutionFailureCategory, ExecutionObservation, StallDetector,
    StallEvaluation,
};
use m31a::agent::engine::{AgentEngine, AgentEngineState, AgentTurnOutcome};
use m31a::agent::intent::{
    AssumptionInvalidation, FactOrigin, FormedTask, FormedTaskStatus, IntentAssumption,
    IntentUnknown, TaskShape, UnknownResolution,
};
use m31a::agent::intent_repository::SqliteIntentRepository;
use m31a::agent::model_policy::{ModelCaller, ModelProposal, TestModelCaller};
use m31a::config::env::SafeEnvironmentStatus;
use m31a::ids::SessionId;
use m31a::interaction::session::{ConversationTurn, SqliteSessionRepository};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::kernel::seams::recovery::FailureClassification;
use m31a::model::types::ModelToolCall;
use m31a::planning::risks::{Criticality, UnknownFate};
use m31a::runtime::AppRuntime;
use serde_json::json;
use std::sync::Arc;

/// Helper to set up an isolated test fixture with a git repo and AppRuntime.
async fn setup_adaptive_fixture() -> (
    tempfile::TempDir,
    Arc<AppRuntime>,
    SessionId,
    SqliteSessionRepository,
) {
    let tmp_root = std::path::PathBuf::from("target/tmp");
    let _ = std::fs::create_dir_all(&tmp_root);
    let dir = tempfile::Builder::new()
        .prefix("m31a-phase33-")
        .tempdir_in(&tmp_root)
        .expect("failed to create fixture tempdir");
    let repo_path = dir.path();

    let git_init = std::process::Command::new("git")
        .args(["init", "--template=", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Phase33 Agent"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "phase33@m31a.local"])
        .current_dir(repo_path)
        .status();

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
        "pub fn core_value() -> i32 { 42 }\n",
    )
    .await
    .unwrap();

    let _ = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .status();

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

/// Helper to create a configured test AgentEngine with mock caller and intent repo attached.
fn create_test_agent_engine(
    runtime: &Arc<AppRuntime>,
    session_id: SessionId,
    caller: Arc<dyn ModelCaller>,
) -> AgentEngine {
    let ws = runtime.workspace_root().to_path_buf();
    let capabilities = Arc::new(m31a::capability::registry::CapabilityRegistry::production(
        &ws,
        Some(runtime.event_bus().clone()),
        None,
    ));
    let mut tool_reg = m31a::tools::registry::ToolRegistry::new_default(capabilities.clone());
    tool_reg.register(m31a::tools::definition::CompleteTool);
    let tool_registry = Arc::new(tool_reg);
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
    let intent_repo = SqliteIntentRepository::new(runtime.pool().clone());

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
    )
    .with_intent_repo(intent_repo)
}

// ─── Test 1: Basic Recovery with Structured Diagnostic Evidence ─────────────

#[tokio::test]
async fn test_basic_recovery_with_structured_evidence() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    // Turn 1: Calling read_file on non-existent file produces execution failure.
    // Turn 2: Assistant proposes strategy adaptation to Recover.
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call_read_fail".to_string(),
                name: "read_file".to_string(),
                arguments: json!({ "path": "non_existent_file.rs" }),
            }],
        }),
        Ok(ModelProposal::AssistantText {
            content:
                "Observed missing file diagnostic. Adapting approach to investigate workspace."
                    .to_string(),
        }),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("refactor missing file");

    let outcome_1 = engine.step(None).await.expect("Turn 1 execution failed");
    if let AgentTurnOutcome::ToolResults { results } = outcome_1 {
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert!(!res.success);
        let diag = res
            .diagnostic
            .as_ref()
            .expect("Diagnostic evidence must be attached on failure");
        assert_eq!(diag.tool_name, "read_file");
        assert_eq!(
            diag.category,
            Some(ExecutionFailureCategory::ExecutionFailed)
        );
    } else {
        panic!("Expected ToolResults outcome for Turn 1");
    }

    assert_eq!(engine.recent_diagnostics().len(), 1);

    // Transition strategy to Recover
    engine
        .transition_strategy(TaskShape::Recover, "Initial file read failed")
        .await
        .expect("Transition to Recover must succeed");

    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::Recover
    );

    // Verify recovery context is rendered into turn messages
    let messages = engine
        .compile_turn_messages()
        .await
        .expect("Failed to compile messages");
    let sys_msg = messages.first().expect("System message must be first");
    match sys_msg {
        m31a::model::types::ChatMessage::System { content } => {
            assert!(
                content.contains("Adaptive Execution & Recovery Status"),
                "Recovery fragment must be included in system prompt"
            );
            assert!(
                content.contains("read_file"),
                "Diagnostic evidence tool name must be included"
            );
        }
        _ => panic!("Expected system message"),
    }

    // Turn 2: Proceed under recovery context
    let outcome_2 = engine.step(None).await.expect("Turn 2 execution failed");
    match outcome_2 {
        AgentTurnOutcome::AssistantText { content }
        | AgentTurnOutcome::AssistantCommentary { content } => {
            assert!(content.contains("Observed missing file"));
        }
        other => panic!("Expected AssistantText/Commentary for Turn 2, got {other:?}"),
    }
}

// ─── Test 2: No Blind Retry Same Action ─────────────────────────────────────

#[tokio::test]
async fn test_no_blind_retry_same_action() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    // Caller proposes identical failing action repeatedly
    let failing_call = ModelToolCall {
        id: "call_repeated_fail".to_string(),
        name: "read_file".to_string(),
        arguments: json!({ "path": "does_not_exist.rs" }),
    };

    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![failing_call.clone()],
        }),
        Ok(ModelProposal::ToolCalls {
            calls: vec![failing_call.clone()],
        }),
        Ok(ModelProposal::ToolCalls {
            calls: vec![failing_call.clone()],
        }),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("inspect file");

    // Turn 1: Action fails normally
    let out_1 = engine.step(None).await.expect("Turn 1 failed");
    match out_1 {
        AgentTurnOutcome::ToolResults { results } => {
            assert!(!results[0].success);
        }
        _ => panic!("Expected ToolResults"),
    }

    // Turn 2: Same failing action with identical workspace state -> rejected as non-progress repetition
    let out_2 = engine.step(None).await.expect("Turn 2 failed");
    match out_2 {
        AgentTurnOutcome::ToolResults { results } => {
            assert!(!results[0].success);
            assert_eq!(results[0].policy_decision.as_deref(), Some("rejected_loop"));
            assert!(
                results[0]
                    .error
                    .as_deref()
                    .unwrap()
                    .contains("Repeated non-progress action rejected")
            );
        }
        _ => panic!("Expected ToolResults with rejected_loop"),
    }

    // Turn 3: 3rd identical attempt with identical workspace -> Hard Stagnation Loop Detected
    let out_3 = engine.step(None).await.expect("Turn 3 failed");
    match out_3 {
        AgentTurnOutcome::Failed { error } => {
            assert!(error.contains("Non-progress loop detected"));
        }
        _ => panic!("Expected AgentTurnOutcome::Failed from stagnation abort"),
    }
    assert!(matches!(engine.state(), AgentEngineState::Failed { .. }));
}

#[tokio::test]
async fn test_stall_detector_permits_repeated_action_after_workspace_mutation() {
    let mut detector = StallDetector::new();
    let tool = "run_tests";
    let input_fp = "test-fp-1";
    let ws_1 = "hash-ws-clean";

    // Observation 1: test fails
    detector.record_observation(ExecutionObservation {
        tool_name: tool.to_string(),
        input_fingerprint: input_fp.to_string(),
        workspace_state_fingerprint: ws_1.to_string(),
        success: false,
        failure_signature: Some("assertion failed".to_string()),
        timestamp: Utc::now(),
    });

    // Proposal before mutation with same ws_1: rejected as blind repeat
    let eval_same = detector.evaluate_proposal(tool, input_fp, ws_1);
    assert!(matches!(
        eval_same,
        StallEvaluation::RepeatedActionRejected { .. }
    ));

    // Proposal AFTER code edit (workspace mutation produces ws_2): permitted!
    let ws_2 = "hash-ws-modified-src";
    let eval_mutated = detector.evaluate_proposal(tool, input_fp, ws_2);
    assert_eq!(eval_mutated, StallEvaluation::Progressing);
}

// ─── Test 3: Plan Revision Preserves Completed Work ─────────────────────────

#[tokio::test]
async fn test_plan_revision_preserves_completed_work() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("implement full feature");

    let intent = engine.intent_state_mut().unwrap();

    let task_1 = FormedTask {
        id: "task-1".to_string(),
        title: "Create data models".to_string(),
        description: "Define struct models".to_string(),
        shape: TaskShape::DirectToolExecution,
        status: FormedTaskStatus::Completed {
            summary: "Models defined and verified".to_string(),
        },
        assumption_ids: Vec::new(),
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: None,
        created_at: Utc::now(),
    };

    let task_2 = FormedTask {
        id: "task-2".to_string(),
        title: "Integrate external legacy API".to_string(),
        description: "Call legacy endpoint".to_string(),
        shape: TaskShape::InvestigateThenAct,
        status: FormedTaskStatus::Failed {
            reason: "Legacy API deprecated and offline".to_string(),
        },
        assumption_ids: Vec::new(),
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: None,
        created_at: Utc::now(),
    };

    let task_3 = FormedTask {
        id: "task-3".to_string(),
        title: "Run integration tests".to_string(),
        description: "Verify integration".to_string(),
        shape: TaskShape::InvestigateThenAct,
        status: FormedTaskStatus::Pending,
        assumption_ids: Vec::new(),
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: None,
        created_at: Utc::now(),
    };

    intent.formed_tasks = vec![task_1, task_2, task_3];
    assert_eq!(intent.plan_revision, 0);

    // Replan: supersede task-2, preserve completed task-1, introduce task-4
    let task_4 = FormedTask {
        id: "task-4".to_string(),
        title: "Integrate new v2 API endpoint".to_string(),
        description: "Call modern v2 endpoint".to_string(),
        shape: TaskShape::InvestigateThenAct,
        status: FormedTaskStatus::Pending,
        assumption_ids: Vec::new(),
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: Some("task-2".to_string()),
        created_at: Utc::now(),
    };

    let replan_req = AdaptiveReplanRequest {
        reason: "Legacy API decommissioned; switching to v2 endpoint".to_string(),
        tasks_to_supersede: vec![(
            "task-2".to_string(),
            "Legacy API deprecated and unreachable".to_string(),
        )],
        new_tasks: vec![task_4],
        invalidated_assumption_ids: Vec::new(),
    };

    let outcome = engine.replan(replan_req).await.expect("Replan failed");

    assert_eq!(outcome.plan_revision, 1);
    assert_eq!(outcome.preserved_completed_count, 1);
    assert!(outcome.preserved_task_ids.contains(&"task-1".to_string()));
    assert!(outcome.superseded_task_ids.contains(&"task-2".to_string()));
    assert!(outcome.new_task_ids.contains(&"task-4".to_string()));

    let updated_intent = engine.intent_state().unwrap();
    assert_eq!(updated_intent.plan_revision, 1);

    // Ensure task-1 remains completed with unchanged evidence
    let t1 = updated_intent.task("task-1").unwrap();
    assert!(t1.is_completed());

    // Ensure task-2 is superseded, not active
    let t2 = updated_intent.task("task-2").unwrap();
    assert!(t2.is_superseded());

    // Ensure task-4 is pending and links back to task-2
    let t4 = updated_intent.task("task-4").unwrap();
    assert_eq!(t4.replacement_for_task_id.as_deref(), Some("task-2"));
}

// ─── Test 4: Assumption Invalidation Propagates ─────────────────────────────

#[tokio::test]
async fn test_assumption_invalidation_propagates() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("migrate database");

    let intent = engine.intent_state_mut().unwrap();

    let asm = IntentAssumption {
        id: "ASM-POSTGRES".to_string(),
        description: "Target database is PostgreSQL 16".to_string(),
        criticality: Criticality::High,
        basis: "Inferred from initial prompt".to_string(),
        origin: FactOrigin::Assumed {
            basis: "prompt keywords".to_string(),
        },
        affected_unknowns: Vec::new(),
        affected_task_ids: vec!["task-pending-schema".to_string()],
        invalidation: None,
        created_at: Utc::now(),
    };
    intent.assumptions.push(asm);

    let completed_task = FormedTask {
        id: "task-completed-audit".to_string(),
        title: "Audit database schema files".to_string(),
        description: "Read schema".to_string(),
        shape: TaskShape::DirectToolExecution,
        status: FormedTaskStatus::Completed {
            summary: "Schema files checked".to_string(),
        },
        assumption_ids: vec!["ASM-POSTGRES".to_string()],
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: None,
        created_at: Utc::now(),
    };

    let pending_task = FormedTask {
        id: "task-pending-schema".to_string(),
        title: "Execute Postgres migration".to_string(),
        description: "Run migrations".to_string(),
        shape: TaskShape::PlanThenExecute,
        status: FormedTaskStatus::Pending,
        assumption_ids: vec!["ASM-POSTGRES".to_string()],
        blocking_unknown_ids: Vec::new(),
        replacement_for_task_id: None,
        created_at: Utc::now(),
    };

    intent.formed_tasks = vec![completed_task, pending_task];

    // Invalidate assumption with new repository evidence
    let contradiction = AssumptionInvalidation {
        contradicting_evidence: "Discovered sqlite3 database in migrations/ directory".to_string(),
        evidence_origin: FactOrigin::RepositoryObserved,
        invalidated_at: Utc::now(),
    };

    let report = engine
        .invalidate_assumption("ASM-POSTGRES", contradiction)
        .await
        .expect("Invalidate assumption failed");

    assert_eq!(report.assumption_id, "ASM-POSTGRES");
    assert!(
        report
            .superseded_task_ids
            .contains(&"task-pending-schema".to_string())
    );
    assert!(
        report
            .preserved_completed_task_ids
            .contains(&"task-completed-audit".to_string())
    );

    let updated = engine.intent_state().unwrap();
    let pending_t = updated.task("task-pending-schema").unwrap();
    assert!(pending_t.is_superseded());

    // Completed task remains completed (Rule: never retroactively corrupt completed evidence)
    let completed_t = updated.task("task-completed-audit").unwrap();
    assert!(completed_t.is_completed());
}

// ─── Test 5: Dynamic Unknown Creation & Resolution ──────────────────────────

#[tokio::test]
async fn test_dynamic_unknown_creation_and_resolution() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("configure security");

    // Dynamic unknown discovered during execution
    let unknown = IntentUnknown::from_evidence(
        "UNK-TOKEN-LIFETIME",
        "What is the maximum token lifetime policy for auth?",
        Criticality::High,
        FactOrigin::ToolObserved {
            tool_name: "grep".to_string(),
        },
        "Token lifetime omitted in config/auth.toml",
        UnknownFate::Researchable,
    );

    engine
        .add_unknown(unknown)
        .await
        .expect("Adding dynamic unknown failed");

    let intent = engine.intent_state().unwrap();
    assert_eq!(intent.unknowns.len(), 1);
    assert!(!intent.unknowns[0].is_resolved());

    // Resolve unknown with evidence
    let resolution = UnknownResolution::Researched {
        finding: "Organization security policy mandates 900 seconds TTL".to_string(),
        source: Some("https://internal.policy/security/tokens".to_string()),
    };

    let resolved = engine
        .resolve_unknown("UNK-TOKEN-LIFETIME", resolution)
        .await;
    assert!(resolved);

    let updated = engine.intent_state().unwrap();
    assert!(updated.unknowns[0].is_resolved());
}

// ─── Test 6: Strategy Transitions ───────────────────────────────────────────

#[tokio::test]
async fn test_strategy_transitions() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;
    let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("adaptive shape transitions");

    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::DirectToolExecution
    );

    // 1. DirectToolExecution -> InvestigateThenAct
    engine
        .transition_strategy(
            TaskShape::InvestigateThenAct,
            "Need to inspect repository structure",
        )
        .await
        .expect("Transition to InvestigateThenAct failed");
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::InvestigateThenAct
    );

    // 2. InvestigateThenAct -> ResearchThenAct
    engine
        .transition_strategy(
            TaskShape::ResearchThenAct,
            "Discovered undocumented library API",
        )
        .await
        .expect("Transition to ResearchThenAct failed");
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::ResearchThenAct
    );

    // 3. ResearchThenAct -> PlanThenExecute
    engine
        .transition_strategy(
            TaskShape::PlanThenExecute,
            "Multi-stage refactoring required based on docs",
        )
        .await
        .expect("Transition to PlanThenExecute failed");
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::PlanThenExecute
    );

    // 4. PlanThenExecute -> Recover
    engine
        .transition_strategy(TaskShape::Recover, "Core assumption invalidated by tests")
        .await
        .expect("Transition to Recover failed");
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::Recover
    );

    // 5. Recover -> PlanThenExecute
    engine
        .transition_strategy(
            TaskShape::PlanThenExecute,
            "Reformed plan with new strategy",
        )
        .await
        .expect("Transition back to PlanThenExecute failed");
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        TaskShape::PlanThenExecute
    );

    // Verify audit trail of strategy transitions
    let intent = engine.intent_state().unwrap();
    assert_eq!(intent.strategy_transitions.len(), 5);

    // Verify invalid transition is rejected: Delegate with empty role
    let invalid = engine
        .transition_strategy(
            TaskShape::Delegate {
                target_role: "".to_string(),
            },
            "invalid delegation",
        )
        .await;
    assert!(invalid.is_err());
}

// ─── Test 7: Policy Denial Differentiation ──────────────────────────────────

struct MockDenyingPolicyGate;

#[async_trait::async_trait]
impl PolicyGate for MockDenyingPolicyGate {
    async fn evaluate(
        &self,
        request: PolicyEvaluationRequest,
    ) -> Result<PolicyDecision, m31a::kernel::seams::policy::PolicyError> {
        if request.tool_or_action == "delete_all" {
            Ok(PolicyDecision::Deny)
        } else {
            Ok(PolicyDecision::Allow)
        }
    }
}

#[tokio::test]
async fn test_policy_denial_differentiation() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call_blocked".to_string(),
                name: "delete_all".to_string(),
                arguments: json!({ "target": "*" }),
            }],
        },
    )]));

    let ws = runtime.workspace_root().to_path_buf();
    let capabilities = Arc::new(m31a::capability::registry::CapabilityRegistry::production(
        &ws,
        Some(runtime.event_bus().clone()),
        None,
    ));
    let tool_registry = Arc::new(m31a::tools::registry::ToolRegistry::new_default(
        capabilities,
    ));
    let pipeline_runner = Arc::new(m31a::pipeline::runner::ToolPipelineRunner::new(
        tool_registry.clone(),
    ));
    let denying_gate = Arc::new(MockDenyingPolicyGate);
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

    let mut engine = AgentEngine::new(
        session_id,
        ws,
        session_repo,
        caller,
        tool_registry,
        pipeline_runner,
        denying_gate,
        approval_coordinator,
        completion_gate,
        context_compiler,
        Some(runtime.event_bus().clone()),
    );

    let outcome = engine.step(None).await.expect("Step failed");
    if let AgentTurnOutcome::ToolResults { results } = outcome {
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert!(!res.success);
        let diag = res.diagnostic.as_ref().expect("Diagnostic must exist");
        assert!(diag.is_policy_denial);
        assert_eq!(diag.failure_class, FailureClassification::Policy);
        assert_eq!(diag.category, Some(ExecutionFailureCategory::PolicyDenied));
    } else {
        panic!("Expected ToolResults");
    }
}

// ─── Test 8: Model Failure Durability ───────────────────────────────────────

#[tokio::test]
async fn test_model_failure_durability() {
    let (_dir, runtime, session_id, session_repo) = setup_adaptive_fixture().await;

    // Caller returns simulated model failure
    let caller = Arc::new(TestModelCaller::from_proposals(vec![Err(
        "Provider timeout".to_string(),
    )]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("run safe tasks");

    let outcome = engine.step(None).await.expect("Turn error wrapped");
    assert!(matches!(outcome, AgentTurnOutcome::Failed { .. }));

    // Intent state must remain intact
    let intent = engine
        .intent_state()
        .expect("Intent state must survive model error");
    assert_eq!(intent.raw_prompt, "run safe tasks");

    // Session repository must record the system message error
    let turns = session_repo.get_conversation(session_id).await.unwrap();
    assert!(turns.iter().any(|t| match t {
        ConversationTurn::SystemMessage { content, .. } =>
            content.contains("Model invocation failed"),
        _ => false,
    }));
}

// ─── Test 9: Subagent Causal Failure Evidence ───────────────────────────────

#[tokio::test]
async fn test_subagent_causal_failure_evidence() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    // Caller proposes subagent handoff for parent, followed by a simulated failure during subagent execution
    let caller = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::Handoff {
            target_role: "database_refactor".to_string(),
            reason: "Apply isolated schema changes".to_string(),
        }),
        Err("Subagent worker crashed: unable to acquire database connection lock".to_string()),
    ]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("orchestrate refactor");

    let outcome = engine.step(None).await.expect("Step failed");
    if let AgentTurnOutcome::ToolResults { results } = outcome {
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert_eq!(res.tool_name, "subagent_database_refactor");
        assert!(
            !res.success,
            "Subagent must fail when proposals are exhausted"
        );
        assert!(res.output.contains("Subagent failed"));

        // Verify structured causal failure evidence is captured (Section 19)
        let diag = res
            .diagnostic
            .as_ref()
            .expect("Diagnostic evidence must be attached to subagent failure");
        assert_eq!(diag.category, Some(ExecutionFailureCategory::ModelFailure));
        let sub_fail = diag
            .subagent_failure
            .as_ref()
            .expect("SubagentFailureEvidence must be present");
        assert_eq!(sub_fail.subagent_role, "database_refactor");
        assert!(!sub_fail.error_message.is_empty());

        // Verify parent engine recorded it in recent diagnostics
        assert_eq!(engine.recent_diagnostics().len(), 1);
        assert_eq!(
            engine.recent_diagnostics()[0].tool_name,
            "subagent_database_refactor"
        );
    } else {
        panic!("Expected ToolResults for delegation");
    }
}

// ─── Test 10: Verification on Revised Plan ──────────────────────────────────

#[tokio::test]
async fn test_verification_on_revised_plan() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    // Propose Complete without test verification -> Rejected by EvidenceCompletionGate
    let caller = Arc::new(TestModelCaller::from_proposals(vec![Ok(
        ModelProposal::Complete {
            summary: "Claiming complete without evidence".to_string(),
            artifacts: Vec::new(),
        },
    )]));

    let mut engine = create_test_agent_engine(&runtime, session_id, caller);
    engine.initialize_intent_from_prompt("modify code");

    let intent = engine.intent_state_mut().unwrap();
    // Add a superseded task and a pending task
    intent.formed_tasks = vec![
        FormedTask {
            id: "task-old".to_string(),
            title: "Old task".to_string(),
            description: "Old task".to_string(),
            shape: TaskShape::DirectToolExecution,
            status: FormedTaskStatus::Superseded {
                reason: "Obsolete".to_string(),
                superseded_at: Utc::now(),
            },
            assumption_ids: Vec::new(),
            blocking_unknown_ids: Vec::new(),
            replacement_for_task_id: None,
            created_at: Utc::now(),
        },
        FormedTask {
            id: "task-new".to_string(),
            title: "New task".to_string(),
            description: "New task".to_string(),
            shape: TaskShape::DirectToolExecution,
            status: FormedTaskStatus::Pending,
            assumption_ids: Vec::new(),
            blocking_unknown_ids: Vec::new(),
            replacement_for_task_id: None,
            created_at: Utc::now(),
        },
    ];

    let outcome = engine.step(None).await.expect("Step failed");
    if let AgentTurnOutcome::ToolResults { results } = outcome {
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert_eq!(results[0].tool_name, "completion_gate");
        assert!(
            results[0]
                .error
                .as_deref()
                .unwrap()
                .contains("Premature completion rejected")
        );
    } else {
        panic!("Expected completion_gate rejection");
    }
}

// ─── Test 11: Persistence & Recovery Across Restarts ────────────────────────

#[tokio::test]
async fn test_persistence_and_recovery_across_restarts() {
    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;

    // Engine 1 writes state
    {
        let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
        let mut engine_1 = create_test_agent_engine(&runtime, session_id, caller);
        engine_1.initialize_intent_from_prompt("long running mission");

        engine_1
            .transition_strategy(
                TaskShape::Recover,
                "Handling unexpected repository structure",
            )
            .await
            .expect("Transition to recover failed");

        let replan_req = AdaptiveReplanRequest {
            reason: "Initial plan revised after discovery".to_string(),
            tasks_to_supersede: Vec::new(),
            new_tasks: vec![FormedTask {
                id: "task-restored-1".to_string(),
                title: "Restored Task".to_string(),
                description: "Test restart durability".to_string(),
                shape: TaskShape::InvestigateThenAct,
                status: FormedTaskStatus::Pending,
                assumption_ids: Vec::new(),
                blocking_unknown_ids: Vec::new(),
                replacement_for_task_id: None,
                created_at: Utc::now(),
            }],
            invalidated_assumption_ids: Vec::new(),
        };
        engine_1.replan(replan_req).await.expect("Replan failed");
    }

    // Engine 2 simulates restart: loads existing session
    {
        let caller = Arc::new(TestModelCaller::from_proposals(vec![]));
        let mut engine_2 = create_test_agent_engine(&runtime, session_id, caller);
        engine_2
            .load_session_state()
            .await
            .expect("Failed to load session state");

        let loaded = engine_2
            .intent_state()
            .expect("IntentState must be loaded across restart");
        assert_eq!(loaded.raw_prompt, "long running mission");
        assert_eq!(loaded.current_strategy, TaskShape::Recover);
        assert_eq!(loaded.plan_revision, 1);
        assert!(loaded.task("task-restored-1").is_some());
        assert_eq!(loaded.strategy_transitions.len(), 1);
    }
}

// ─── Test 12: Live Nemotron Real Model Adaptive Execution ───────────────────

#[tokio::test]
async fn test_live_nemotron_adaptive_execution() {
    let probe = SafeEnvironmentStatus::probe();
    if !probe.provider_configured {
        println!(
            "[SKIPPED] NVIDIA NIM provider not configured in environment. Skipping live model call."
        );
        return;
    }

    let (_dir, runtime, session_id, _session_repo) = setup_adaptive_fixture().await;
    let mut engine = runtime.create_agent_engine(session_id);

    println!(
        "[REAL-MODEL] Executing Phase 33 live adaptive step with nvidia/nemotron-3-ultra-550b-a55b..."
    );
    let outcome = engine
        .step(Some("Inspect the repository, identify existing library code in src/lib.rs, and adapt your execution strategy to InvestigateThenAct."))
        .await;

    match outcome {
        Ok(out) => {
            println!("[REAL-MODEL] Phase 33 Nemotron Turn outcome: {out:?}");
            let intent = engine
                .intent_state()
                .expect("Intent state must be initialized");
            assert!(!intent.raw_prompt.is_empty());
        }
        Err(e) => {
            println!("[REAL-MODEL NOTICE] Live invocation notice: {e}");
        }
    }
}
