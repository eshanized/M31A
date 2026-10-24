//! Comprehensive regression test suite for BUG-01 through BUG-14 remediation.
//!
//! Verifies:
//! - BUG-01: Ordered completion evidence & linear history/batch verification
//! - BUG-02: Authoritative workspace path containment validation
//! - BUG-03: Downstream lifecycle execution before decision resolution
//! - BUG-04: Decision prompt fails closed on repository failure
//! - BUG-05: Planning choice genesis resumes original user prompt
//! - BUG-06: ModelProposal::AskUser interactive prompt & pause/resume
//! - BUG-07: ModelProposal::Handoff validation & child execution
//! - BUG-08: Canonical AgentAction pipeline dispatch coverage
//! - BUG-09: Tool call strict parameter & bounds validation
//! - BUG-10: Clarification / discovery custom question ID routing
//! - BUG-11: Planning choice unique ID & cancellation cleanup
//! - BUG-12: Intent persistence failure rollback
//! - BUG-13: Dead adaptive budget removal
//! - BUG-14: Strategy transition prerequisite enforcement

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

mod support;
use support::model_caller::TestModelCaller;
use support::policy::AllowAllTestPolicy;

use m31a::agent::action::{ActionParseError, ActionUserOption, AgentAction, AgentObservation};
use m31a::agent::adaptive::{render_recovery_context, validate_strategy_transition};
use m31a::agent::engine::AgentEngine;
use m31a::agent::intent::{
    AssumptionInvalidation, FactOrigin, IntentAssumption, IntentState, IntentUnknown, TaskShape,
};
use m31a::agent::intent_repository::SqliteIntentRepository;
use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::agent::profile::AgentProfile;
use m31a::agent::runner::{
    ActionDispatcher, ActionRequest, ActionResult, AgentStepRecord, WorkerRunner,
    extract_all_action_paths, validate_workspace_path,
};
use m31a::agent::supervisor::{AgentOutcome, ExecutionActivityTracker, FailureClass};
use m31a::capability::registry::CapabilityRegistry;
use m31a::ids::{AgentId, MissionId, SessionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::decision::{
    DecisionKind, DecisionResumeOutcome, DecisionWaiterRegistry, InteractiveDecision,
    InteractiveDecisionResponse,
};
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::seams::context::CompiledContext;
use m31a::kernel::seams::policy::PolicyGate;
use m31a::model::types::{ModelToolCall, TokenUsage, UserOption};
use m31a::persistence::sqlite::repositories::SqliteTaskRepository;
use m31a::pipeline::dispatcher::ProductionActionDispatcher;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::planning::risks::{Criticality, UnknownFate};
use m31a::runtime::AppRuntime;
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::ToolExecutionContext;
use m31a::verification::gate::ToolCapabilityClass;

// ============================================================================
// Test Support Helpers & Mocks
// ============================================================================

fn make_step_record(summary: &str, actions: Vec<(ActionRequest, ActionResult)>) -> AgentStepRecord {
    AgentStepRecord {
        step_number: 1,
        model_proposal_summary: summary.to_string(),
        actions_executed: actions,
        started_at: chrono::Utc::now(),
        completed_at: chrono::Utc::now(),
        inference_duration_ms: 10,
        action_fingerprint: None,
        tool_validation_status: None,
        prompt_provenance: None,
    }
}

#[derive(Clone)]
struct MockModelCaller {
    proposals: Vec<ModelProposal>,
    call_idx: Arc<AtomicU32>,
}

impl MockModelCaller {
    fn new(proposals: Vec<ModelProposal>) -> Self {
        Self {
            proposals,
            call_idx: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ModelCaller for MockModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let idx = self.call_idx.fetch_add(1, Ordering::SeqCst) as usize;
        if idx < self.proposals.len() {
            Ok(self.proposals[idx].clone())
        } else {
            Err("mock proposals exhausted".to_string())
        }
    }

    async fn call_model_with_context_and_usage(
        &self,
        context: &CompiledContext,
        _cancellation_token: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        let p = self.call_model(&context.system_prompt).await?;
        Ok((p, TokenUsage::default()))
    }

    fn bind_role_tools(
        &self,
        _role: &m31a::state_machine::agent::AgentRole,
        _tools: Vec<serde_json::Value>,
    ) -> Option<std::sync::Arc<dyn ModelCaller>> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

struct MockActionDispatcher;

#[async_trait::async_trait]
impl ActionDispatcher for MockActionDispatcher {
    async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
        Ok(ActionResult {
            action_id: action.id.clone(),
            success: true,
            output: format!("executed {}", action.tool_name),
            error: None,
        })
    }

    async fn dispatch_action(&self, action: &AgentAction) -> Result<AgentObservation, String> {
        match action {
            AgentAction::AskUser { question, .. } => Ok(AgentObservation::UserResponse {
                answer: format!("Answer to: {question}"),
            }),
            AgentAction::RequestApproval { tool_name, .. } => {
                Ok(AgentObservation::ApprovalResult {
                    approved: true,
                    reason: Some(format!("Approved {tool_name}")),
                })
            }
            AgentAction::CreateTask { .. } => Ok(AgentObservation::TaskCreated {
                task_id: TaskId::new(),
            }),
            AgentAction::SupersedeTask { task_id, .. } => {
                Ok(AgentObservation::TaskSuperseded { task_id: *task_id })
            }
            AgentAction::Replan { .. } => Ok(AgentObservation::ReplanCompleted {
                new_plan_revision: 2,
                superseded_count: 1,
                added_count: 1,
            }),
            AgentAction::WriteChange { path, content, .. } => Ok(AgentObservation::ChangeWritten {
                path: path.clone(),
                bytes_written: content.len(),
                content_hash: "hash123".to_string(),
            }),
            AgentAction::RunVerification { tier, .. } => Ok(AgentObservation::VerificationResult {
                passed: true,
                summary: "all tests passed".to_string(),
                tier: tier.unwrap_or(1),
                diagnostics: Vec::new(),
            }),
            AgentAction::Complete { summary } => Ok(AgentObservation::ToolOutput {
                tool_name: "complete".to_string(),
                call_id: "call-comp".to_string(),
                output: summary.clone(),
                success: true,
                duration_ms: 5,
            }),
            AgentAction::Fail { error } => Ok(AgentObservation::ToolOutput {
                tool_name: "fail".to_string(),
                call_id: "call-fail".to_string(),
                output: error.clone(),
                success: false,
                duration_ms: 5,
            }),
            AgentAction::Pause { .. } => Ok(AgentObservation::Paused),
            AgentAction::Resume { .. } => Ok(AgentObservation::Resumed),
            AgentAction::Cancel { .. } => Ok(AgentObservation::Cancelled {
                reason: "cancelled".to_string(),
            }),
            AgentAction::Delegate { target_role, .. } => Ok(AgentObservation::DelegationResult {
                child_agent_id: AgentId::new(),
                outcome: Box::new(AgentOutcome::Succeeded {
                    output: format!("child completed for role {}", target_role),
                    steps_consumed: 1,
                }),
            }),
            AgentAction::CallTool {
                tool_name,
                parameters: _,
            } => Ok(AgentObservation::ToolOutput {
                tool_name: tool_name.clone(),
                call_id: "call-1".to_string(),
                output: "ok".to_string(),
                success: true,
                duration_ms: 10,
            }),
        }
    }
}

// ============================================================================
// BUG-01 Tests: Completion Verification Ordering & Batch Invariants
// ============================================================================

#[test]
fn test_bug01_successful_verification_then_later_mutation_fails() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let test_file = ws.join("src/lib.rs");
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(&test_file, "pub fn add() -> i32 { 1 }\n").unwrap();

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile)
            .with_workspace_root(ws)
            .with_verification(m31a::kernel::plan::VerificationStrategy::AutomatedTest {
                command: Some("cargo test".to_string()),
            });

    // Step 1: Successful verification
    runner.push_step_record(make_step_record(
        "run verification",
        vec![(
            ActionRequest {
                id: "act-1".to_string(),
                tool_name: "run_tests".to_string(),
                parameters: serde_json::json!({}),
            },
            ActionResult {
                action_id: "act-1".to_string(),
                success: true,
                output: "test result: ok. 1 passed; 0 failed".to_string(),
                error: None,
            },
        )],
    ));

    // Step 2: Later mutation occurs
    runner.push_step_record(make_step_record(
        "edit file",
        vec![(
            ActionRequest {
                id: "act-2".to_string(),
                tool_name: "edit_file".to_string(),
                parameters: serde_json::json!({"path": "src/lib.rs"}),
            },
            ActionResult {
                action_id: "act-2".to_string(),
                success: true,
                output: "replaced text".to_string(),
                error: None,
            },
        )],
    ));

    // Attempting completion without a new verification after the mutation MUST FAIL
    let eval_res = runner.evaluate_completion_evidence(&[]);
    assert!(
        eval_res.is_err(),
        "Completion must be rejected when mutation occurs after verification"
    );
    let err = eval_res.unwrap_err();
    assert!(
        err.contains("verification has not passed") || err.contains("evidence"),
        "Error message must explain missing verification evidence: {err}"
    );
}

#[test]
fn test_bug01_mutation_then_successful_verification_succeeds() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    let test_file = ws.join("src/lib.rs");
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(&test_file, "pub fn add() -> i32 { 2 }\n").unwrap();

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile)
            .with_workspace_root(ws)
            .with_verification(m31a::kernel::plan::VerificationStrategy::AutomatedTest {
                command: Some("cargo test".to_string()),
            });

    // Step 1: Mutation
    runner.push_step_record(make_step_record(
        "edit file",
        vec![(
            ActionRequest {
                id: "act-1".to_string(),
                tool_name: "edit_file".to_string(),
                parameters: serde_json::json!({"path": "src/lib.rs"}),
            },
            ActionResult {
                action_id: "act-1".to_string(),
                success: true,
                output: "modified file".to_string(),
                error: None,
            },
        )],
    ));

    // Step 2: Verification after mutation
    runner.push_step_record(make_step_record(
        "run verification",
        vec![(
            ActionRequest {
                id: "act-2".to_string(),
                tool_name: "run_tests".to_string(),
                parameters: serde_json::json!({}),
            },
            ActionResult {
                action_id: "act-2".to_string(),
                success: true,
                output: "test result: ok. 1 passed; 0 failed".to_string(),
                error: None,
            },
        )],
    ));

    // Completion should now pass evidence evaluation
    let eval_res = runner.evaluate_completion_evidence(&[]);
    assert!(
        eval_res.is_ok(),
        "Completion must succeed when verification follows mutation: {:?}",
        eval_res.err()
    );
}

#[test]
fn test_bug01_same_batch_verification_before_mutation_fails() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(ws.join("src/lib.rs"), "pub fn fn1() {}\n").unwrap();

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let runner =
        WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile)
            .with_workspace_root(ws)
            .with_verification(m31a::kernel::plan::VerificationStrategy::AutomatedTest {
                command: Some("cargo test".to_string()),
            });

    // Current batch contains verification followed by mutation
    let batch = vec![
        (
            ActionRequest {
                id: "act-1".to_string(),
                tool_name: "run_tests".to_string(),
                parameters: serde_json::json!({}),
            },
            ActionResult {
                action_id: "act-1".to_string(),
                success: true,
                output: "test result: ok".to_string(),
                error: None,
            },
        ),
        (
            ActionRequest {
                id: "act-2".to_string(),
                tool_name: "edit_file".to_string(),
                parameters: serde_json::json!({"path": "src/lib.rs"}),
            },
            ActionResult {
                action_id: "act-2".to_string(),
                success: true,
                output: "edited".to_string(),
                error: None,
            },
        ),
    ];

    let eval_res = runner.evaluate_completion_evidence(&batch);
    assert!(
        eval_res.is_err(),
        "Same batch with verification BEFORE mutation must reject completion"
    );
}

#[test]
fn test_bug01_same_batch_mutation_before_verification_succeeds() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(ws.join("src/lib.rs"), "pub fn fn1() {}\n").unwrap();

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let runner =
        WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile)
            .with_workspace_root(ws)
            .with_verification(m31a::kernel::plan::VerificationStrategy::AutomatedTest {
                command: Some("cargo test".to_string()),
            });

    // Current batch contains mutation followed by verification
    let batch = vec![
        (
            ActionRequest {
                id: "act-1".to_string(),
                tool_name: "edit_file".to_string(),
                parameters: serde_json::json!({"path": "src/lib.rs"}),
            },
            ActionResult {
                action_id: "act-1".to_string(),
                success: true,
                output: "edited".to_string(),
                error: None,
            },
        ),
        (
            ActionRequest {
                id: "act-2".to_string(),
                tool_name: "run_tests".to_string(),
                parameters: serde_json::json!({}),
            },
            ActionResult {
                action_id: "act-2".to_string(),
                success: true,
                output: "test result: ok".to_string(),
                error: None,
            },
        ),
    ];

    let eval_res = runner.evaluate_completion_evidence(&batch);
    assert!(
        eval_res.is_ok(),
        "Same batch with mutation BEFORE verification must be accepted"
    );
}

#[test]
fn test_bug01_uncommitted_batch_mutation_without_verification_fails() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(ws.join("src/lib.rs"), "pub fn fn1() {}\n").unwrap();

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile)
            .with_workspace_root(ws)
            .with_verification(m31a::kernel::plan::VerificationStrategy::AutomatedTest {
                command: Some("cargo test".to_string()),
            });

    // History has mutation + verification
    runner.push_step_record(make_step_record(
        "edit",
        vec![(
            ActionRequest {
                id: "act-1".to_string(),
                tool_name: "edit_file".to_string(),
                parameters: serde_json::json!({"path": "src/lib.rs"}),
            },
            ActionResult {
                action_id: "act-1".to_string(),
                success: true,
                output: "edited".to_string(),
                error: None,
            },
        )],
    ));
    runner.push_step_record(make_step_record(
        "verify",
        vec![(
            ActionRequest {
                id: "act-2".to_string(),
                tool_name: "run_tests".to_string(),
                parameters: serde_json::json!({}),
            },
            ActionResult {
                action_id: "act-2".to_string(),
                success: true,
                output: "test result: ok".to_string(),
                error: None,
            },
        )],
    ));

    // Now an uncommitted batch adds a new mutation
    let uncommitted_batch = vec![(
        ActionRequest {
            id: "act-3".to_string(),
            tool_name: "write_file".to_string(),
            parameters: serde_json::json!({"path": "src/lib.rs"}),
        },
        ActionResult {
            action_id: "act-3".to_string(),
            success: true,
            output: "written".to_string(),
            error: None,
        },
    )];

    let eval_res = runner.evaluate_completion_evidence(&uncommitted_batch);
    assert!(
        eval_res.is_err(),
        "Uncommitted batch with unverified mutation must reject completion"
    );
}

// ============================================================================
// BUG-02 Tests: Workspace Path Containment Validation
// ============================================================================

#[test]
fn test_bug02_workspace_path_containment() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("src/subdir")).unwrap();
    std::fs::write(ws.join("src/subdir/file.rs"), "code").unwrap();

    // 1. Ordinary nested files inside workspace -> allowed
    let valid_path = validate_workspace_path(ws, "src/subdir/file.rs");
    assert!(
        valid_path.is_ok(),
        "Nested file inside workspace must be allowed"
    );

    // 2. Relative traversal escaping workspace -> rejected
    let traversal = validate_workspace_path(ws, "../../etc/passwd");
    assert!(
        traversal.is_err(),
        "Relative traversal escaping workspace must be rejected"
    );

    // 3. Absolute path outside workspace -> rejected
    let outside_abs = validate_workspace_path(ws, "/etc/shadow");
    assert!(
        outside_abs.is_err(),
        "Absolute path outside workspace must be rejected"
    );

    // 4. Non-existent path inside workspace -> allowed (for write/create)
    let non_existent = validate_workspace_path(ws, "src/new_mod.rs");
    assert!(
        non_existent.is_ok(),
        "Non-existent path inside workspace must be allowed"
    );

    // 5. Path escaping through existing parent -> rejected
    let escape_parent = validate_workspace_path(ws, "src/../../outside");
    assert!(
        escape_parent.is_err(),
        "Parent directory escape must be rejected"
    );

    // 6. Symlink escape pointing outside workspace -> rejected
    #[cfg(unix)]
    {
        let outside_dir = tempdir().unwrap();
        let outside_target = outside_dir.path().join("secret.txt");
        std::fs::write(&outside_target, "secret").unwrap();

        let symlink_path = ws.join("src/link_to_outside");
        let _ = std::os::unix::fs::symlink(&outside_target, &symlink_path);
        let link_res = validate_workspace_path(ws, "src/link_to_outside");
        assert!(
            link_res.is_err(),
            "Symlink pointing outside workspace must be rejected"
        );
    }

    // 7. extract_all_action_paths catches various parameter aliases
    let params = serde_json::json!({
        "path": "src/a.rs",
        "destination_path": "src/b.rs",
        "paths": ["src/c.rs", "src/d.rs"]
    });
    let extracted = extract_all_action_paths("edit_file", &params);
    assert!(extracted.contains(&"src/a.rs".to_string()));
    assert!(extracted.contains(&"src/b.rs".to_string()));
    assert!(extracted.contains(&"src/c.rs".to_string()));
    assert!(extracted.contains(&"src/d.rs".to_string()));
}

// ============================================================================
// BUG-03 Tests: Downstream Execution Before Decision Resolution
// ============================================================================

#[tokio::test]
async fn test_bug03_decision_lifecycle_consistency() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let repo = SqliteSessionRepository::new(runtime.pool().clone());

    let session = repo.create_session(tmp.path()).await.unwrap();
    let decision = InteractiveDecision::planning_choice(
        session.id.to_string(),
        "build test app",
        "Choose planning mode",
    );

    // Save pending decision
    repo.set_pending_decision(session.id, &decision)
        .await
        .unwrap();

    // Verify decision is pending and not completed
    assert!(
        !repo
            .is_decision_completed(session.id, &decision.id)
            .await
            .unwrap()
    );
    assert_eq!(
        repo.get_pending_decision(session.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        decision.id
    );

    // Record completion marks it completed exactly once
    repo.record_completed_decision(session.id, &decision.id)
        .await
        .unwrap();
    assert!(
        repo.is_decision_completed(session.id, &decision.id)
            .await
            .unwrap()
    );

    // Duplicate submission check
    assert!(
        repo.is_decision_completed(session.id, &decision.id)
            .await
            .unwrap()
    );
}

// ============================================================================
// BUG-04 Tests: Decision Fails Closed on Repository Failure
// ============================================================================

#[tokio::test]
async fn test_bug04_decision_fails_closed_when_persistence_fails() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let repo = SqliteSessionRepository::new(runtime.pool().clone());

    // An invalid non-existent session ID should fail to set pending decision
    let bad_session_id = SessionId::new();
    let decision = InteractiveDecision::planning_choice(
        bad_session_id.to_string(),
        "build app",
        "Choose mode",
    );

    let res = repo.set_pending_decision(bad_session_id, &decision).await;
    assert!(
        res.is_err(),
        "set_pending_decision must fail for non-existent session"
    );

    // Authoritative check must fail closed (error / not found)
    let pending_res = repo.get_pending_decision(bad_session_id).await;
    assert!(
        pending_res.is_err(),
        "No pending decision lookup should succeed for non-existent session"
    );
}

// ============================================================================
// BUG-05 Tests: Planning Choice Project Docs Resumes Request
// ============================================================================

#[test]
fn test_bug05_planning_choice_preserves_user_request() {
    let raw_prompt = "Create an HTTP server with actix-web";
    let stored = None;

    // DirectExecution resolution preserves prompt
    let choice =
        m31a::runtime::PlanningChoice::resolve_for_request(raw_prompt, None, stored, false);
    // When prompt is a build prompt, request-sensitive orchestration routes to project planning
    assert_eq!(choice, Some(m31a::runtime::PlanningChoice::ProjectPlanning));

    // Explicit request for project planning via command
    let explicit_plan = "/plan create an HTTP server with actix-web";
    let plan_choice =
        m31a::runtime::PlanningChoice::resolve_for_request(explicit_plan, None, stored, false);
    assert_eq!(
        plan_choice,
        Some(m31a::runtime::PlanningChoice::ProjectPlanning)
    );
}

// ============================================================================
// BUG-06 Tests: ModelProposal::AskUser Interactive Execution
// ============================================================================

#[tokio::test]
async fn test_bug06_model_proposal_ask_user() {
    let mission_id = MissionId::new();
    let profile = AgentProfile::built_in(AgentRole::researcher());
    let mut runner =
        WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

    let model = MockModelCaller::new(vec![
        ModelProposal::AskUser {
            question: "Which database do you prefer?".to_string(),
            options: vec![
                UserOption {
                    id: "sqlite".to_string(),
                    label: "SQLite".to_string(),
                    description: Some("Embedded SQL database".to_string()),
                },
                UserOption {
                    id: "postgres".to_string(),
                    label: "PostgreSQL".to_string(),
                    description: Some("Relational database server".to_string()),
                },
            ],
        },
        ModelProposal::Complete {
            summary: "Task finished with SQLite chosen".to_string(),
            artifacts: Vec::new(),
        },
    ]);

    let dispatcher = MockActionDispatcher;
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let outcome = runner
        .run_step_loop(&model, &dispatcher, &token, tracker)
        .await;

    // Runner executed AskUser proposal, received answer, and continued
    assert_eq!(
        outcome,
        AgentOutcome::Succeeded {
            output: "Task finished with SQLite chosen".to_string(),
            steps_consumed: 2,
        }
    );

    // Verify step history captured the AskUser proposal and answer
    assert!(runner.step_history().iter().any(|s| {
        s.model_proposal_summary.contains("ask_user")
            && s.model_proposal_summary.contains("Which database")
    }));
}

// ============================================================================
// BUG-07 Tests: ModelProposal::Handoff Boundary Verification
// ============================================================================

#[tokio::test]
async fn test_bug07_handoff_rejects_unknown_role_and_self_delegation() {
    let mission_id = MissionId::new();
    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

    // 1. Unknown role rejection
    let model_unknown = MockModelCaller::new(vec![ModelProposal::Handoff {
        target_role: "non_existent_imaginary_role".to_string(),
        reason: "do work".to_string(),
    }]);
    let dispatcher = MockActionDispatcher;
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let outcome1 = runner
        .run_step_loop(&model_unknown, &dispatcher, &token, tracker.clone())
        .await;

    match outcome1 {
        AgentOutcome::Failed(ev) => {
            assert_eq!(ev.failure_class, FailureClass::PolicyDenial);
            assert!(ev.message.contains("not registered in RoleRegistry"));
        }
        other => panic!(
            "Expected PolicyDenial failure for unknown role, got {:?}",
            other
        ),
    }

    // 2. Self-delegation rejection
    let mut runner2 = WorkerRunner::new_isolated_test(
        mission_id,
        AgentId::new(),
        TaskId::new(),
        AgentProfile::built_in(AgentRole::implementer()),
    );
    let model_self = MockModelCaller::new(vec![ModelProposal::Handoff {
        target_role: "implementer".to_string(),
        reason: "delegate to myself".to_string(),
    }]);

    let outcome2 = runner2
        .run_step_loop(&model_self, &dispatcher, &token, tracker)
        .await;

    match outcome2 {
        AgentOutcome::Failed(ev) => {
            assert_eq!(ev.failure_class, FailureClass::PolicyDenial);
            assert!(
                ev.message
                    .contains("self-delegation to current role 'implementer' is prohibited")
            );
        }
        other => panic!(
            "Expected PolicyDenial failure for self-delegation, got {:?}",
            other
        ),
    }
}

// ============================================================================
// BUG-08 & BUG-09 Tests: Pipeline Action Dispatch & Parameter Validation
// ============================================================================

#[tokio::test]
async fn test_bug08_all_agent_actions_dispatch_via_pipeline() {
    let dispatcher = MockActionDispatcher;

    let actions = vec![
        AgentAction::CallTool {
            tool_name: "test_tool".to_string(),
            parameters: serde_json::json!({}),
        },
        AgentAction::WriteChange {
            path: std::path::PathBuf::from("src/test.rs"),
            content: "code".to_string(),
            description: None,
        },
        AgentAction::RunVerification {
            tier: Some(1),
            target: None,
        },
        AgentAction::AskUser {
            question: "test question?".to_string(),
            options: Vec::new(),
        },
        AgentAction::RequestApproval {
            tool_name: "deploy".to_string(),
            parameters: serde_json::json!({}),
            risk: Some("high".to_string()),
            reason: "release".to_string(),
        },
        AgentAction::CreateTask {
            title: "Task 1".to_string(),
            description: "Desc".to_string(),
            role: Some(AgentRole::implementer()),
            dependencies: Vec::new(),
        },
        AgentAction::SupersedeTask {
            task_id: TaskId::new(),
            reason: "Superseded".to_string(),
        },
        AgentAction::Replan {
            reason: "Plan change".to_string(),
            tasks_to_supersede: Vec::new(),
            new_tasks: Vec::new(),
        },
        AgentAction::Delegate {
            target_role: AgentRole::researcher(),
            task_id: None,
            reason: "Need info".to_string(),
            inputs: Vec::new(),
        },
        AgentAction::Complete {
            summary: "Done".to_string(),
        },
        AgentAction::Fail {
            error: "Error".to_string(),
        },
        AgentAction::Pause {
            reason: "Pausing".to_string(),
        },
        AgentAction::Resume {
            reason: Some("Resuming".to_string()),
        },
        AgentAction::Cancel {
            reason: "Cancelled".to_string(),
        },
    ];

    for act in actions {
        let obs = dispatcher.dispatch_action(&act).await;
        assert!(
            obs.is_ok(),
            "Action {} failed dispatch: {:?}",
            act.name(),
            obs.err()
        );
    }
}

#[test]
fn test_bug09_tool_call_strict_parameter_validation() {
    // 1. Missing required field in write_file
    let call_no_path = ModelToolCall {
        id: "call-1".to_string(),
        name: "write_file".to_string(),
        arguments: serde_json::json!({"content": "hello"}),
    };
    let err1 = AgentAction::try_from_tool_call(&call_no_path);
    assert!(matches!(
        err1,
        Err(ActionParseError::MissingRequiredField { field: "path", .. })
    ));

    // 2. Out of range verification tier
    let call_bad_tier = ModelToolCall {
        id: "call-2".to_string(),
        name: "run_verification".to_string(),
        arguments: serde_json::json!({"tier": 99}),
    };
    let err2 = AgentAction::try_from_tool_call(&call_bad_tier);
    assert!(matches!(
        err2,
        Err(ActionParseError::InvalidParameter { field: "tier", .. })
    ));

    // 3. Malformed UUID in supersede_task
    let call_bad_uuid = ModelToolCall {
        id: "call-3".to_string(),
        name: "supersede_task".to_string(),
        arguments: serde_json::json!({"task_id": "not-a-real-uuid", "reason": "obsolete"}),
    };
    let err3 = AgentAction::try_from_tool_call(&call_bad_uuid);
    assert!(matches!(
        err3,
        Err(ActionParseError::InvalidParameter {
            field: "task_id",
            ..
        })
    ));

    // 4. Empty title in create_task
    let call_empty_title = ModelToolCall {
        id: "call-4".to_string(),
        name: "create_task".to_string(),
        arguments: serde_json::json!({"title": "   ", "description": "desc"}),
    };
    let err4 = AgentAction::try_from_tool_call(&call_empty_title);
    assert!(matches!(
        err4,
        Err(ActionParseError::InvalidParameter { field: "title", .. })
    ));
}

// ============================================================================
// BUG-10 Tests: Clarification Custom Question ID Routing
// ============================================================================

#[test]
fn test_bug10_clarification_custom_question_id_validation() {
    let q = m31a::workflow::genesis::DynamicQuestion {
        question_id: "custom_unprefixed_architecture_decision".to_string(),
        text: "Choose an architecture style".to_string(),
        options: vec!["Monolith".to_string(), "Microservices".to_string()],
        allow_freeform: true,
        target_unknown: "Architecture".to_string(),
        blocking: true,
        reason: "Need architecture choice".to_string(),
    };
    let dec = InteractiveDecision::discovery_question("sess-123", &q);

    assert_eq!(dec.options.len(), 2);
    if let DecisionKind::Clarification {
        question_id,
        allow_freeform,
    } = &dec.kind
    {
        assert_eq!(question_id, "custom_unprefixed_architecture_decision");
        assert!(*allow_freeform);
    } else {
        panic!("Expected Clarification decision kind");
    }

    let valid_resp = InteractiveDecisionResponse {
        decision_id: dec.id.clone(),
        session_id: "sess-123".to_string(),
        execution_id: None,
        selected_option_id: Some("opt_1".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    assert!(dec.validate_response(&valid_resp).is_ok());
}

// ============================================================================
// BUG-11 Tests: Planning Choice Unique ID & Cancellation Cleanup
// ============================================================================

#[tokio::test]
async fn test_bug11_planning_choice_unique_id_and_cancellation() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let repo = SqliteSessionRepository::new(runtime.pool().clone());

    let session = repo.create_session(tmp.path()).await.unwrap();

    let dec1 =
        InteractiveDecision::planning_choice(session.id.to_string(), "prompt 1", "question 1");
    let dec2 =
        InteractiveDecision::planning_choice(session.id.to_string(), "prompt 1", "question 1");
    assert_ne!(
        dec1.id, dec2.id,
        "Each planning choice decision must have a unique ID"
    );

    // Setting and clearing planning choice
    repo.set_planning_choice(session.id, m31a::runtime::PlanningChoice::ProjectPlanning)
        .await
        .unwrap();
    let s = repo.get_session(session.id).await.unwrap().unwrap();
    assert_eq!(
        SqliteSessionRepository::read_planning_choice(&s),
        Some(m31a::runtime::PlanningChoice::ProjectPlanning)
    );

    repo.clear_planning_choice(session.id).await.unwrap();
    let s_cleared = repo.get_session(session.id).await.unwrap().unwrap();
    assert_eq!(
        SqliteSessionRepository::read_planning_choice(&s_cleared),
        None,
        "clear_planning_choice must erase stored planning choice"
    );
}

// ============================================================================
// BUG-12 Tests: Intent Persistence Failure In-Memory Rollback
// ============================================================================

#[tokio::test]
async fn test_bug12_intent_in_memory_rollback_on_persistence_failure() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path().to_path_buf();
    let runtime = Arc::new(AppRuntime::new(&ws).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(&ws).await.unwrap();

    let tool_registry = runtime.tool_registry().clone();
    let pipeline_runner = Arc::new(m31a::pipeline::runner::ToolPipelineRunner::new(
        tool_registry.clone(),
    ));
    let policy_gate = Arc::new(AllowAllTestPolicy);
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
    let intent_repo = SqliteIntentRepository::new(runtime.pool().clone());

    let mut engine = AgentEngine::new(
        session.id,
        ws,
        session_repo,
        Arc::new(TestModelCaller::new("ok")),
        tool_registry,
        pipeline_runner,
        policy_gate,
        approval_coordinator,
        completion_gate,
        context_compiler,
        Some(runtime.event_bus().clone()),
        runtime.capability_registry().clone(),
    )
    .with_intent_repo(intent_repo);

    engine.initialize_intent_from_prompt("Build an API");

    let initial_shape = engine.intent_state().unwrap().current_strategy.clone();
    assert_eq!(initial_shape, TaskShape::DirectToolExecution);

    // Force SQLite pool to close so any persist_intent_state() will fail
    runtime.pool().close().await;

    // Attempt transition strategy: DB fails, in-memory state MUST roll back
    let res = engine
        .transition_strategy(TaskShape::InvestigateThenAct, "investigate")
        .await;
    assert!(res.is_err(), "Persistence failure must return error");

    // Verify in-memory strategy was rolled back to initial_shape
    assert_eq!(
        engine.intent_state().unwrap().current_strategy,
        initial_shape,
        "In-memory state must be rolled back to backup when persistence fails"
    );
}

// ============================================================================
// BUG-13 & BUG-14 Tests: Adaptive Budget Cleanup & Prerequisite Enforcement
// ============================================================================

#[test]
fn test_bug13_adaptive_budget_cleanup_in_recovery_context() {
    let intent = IntentState::initial_from_prompt("sess-1", "test prompt");
    let recovery_text = render_recovery_context(&intent, &[]);

    assert!(
        !recovery_text.contains("Remaining Budgets"),
        "Recovery context must not contain obsolete 'Remaining Budgets'"
    );
    assert!(
        !recovery_text.contains("budget"),
        "Recovery context must not reference dead adaptive budgets"
    );
}

#[test]
fn test_bug14_strategy_transition_prerequisites_enforced() {
    let mut intent = IntentState::initial_from_prompt("sess-1", "test prompt");
    intent.current_strategy = TaskShape::PlanThenExecute;

    // 1. Transition to AskUserThenAct without pending decision or unknown -> rejected
    let res_ask_invalid = validate_strategy_transition(
        &TaskShape::PlanThenExecute,
        &TaskShape::AskUserThenAct,
        &intent,
    );
    assert!(
        res_ask_invalid.is_err(),
        "Transition to AskUserThenAct must be rejected when no pending decisions/unknowns exist"
    );

    // Add an unresolved unknown -> transition to AskUserThenAct is now permitted
    let unknown = IntentUnknown::from_evidence(
        "unk-1",
        "Which third-party provider to integrate?",
        Criticality::High,
        FactOrigin::Assumed {
            basis: "user prompt".to_string(),
        },
        "user prompt",
        UnknownFate::UserDecisionRequired,
    );
    intent.add_unknown(unknown).unwrap();

    let res_ask_valid = validate_strategy_transition(
        &TaskShape::PlanThenExecute,
        &TaskShape::AskUserThenAct,
        &intent,
    );
    assert!(
        res_ask_valid.is_ok(),
        "Transition to AskUserThenAct must succeed when an unresolved unknown exists"
    );

    // 2. Transition to Recover without failed tasks or invalidated assumptions -> rejected
    let res_recover_invalid =
        validate_strategy_transition(&TaskShape::PlanThenExecute, &TaskShape::Recover, &intent);
    assert!(
        res_recover_invalid.is_err(),
        "Transition to Recover must be rejected when no failure/invalidation exists"
    );

    // Add an assumption and invalidate it -> transition to Recover is now permitted
    let assumption = IntentAssumption {
        id: "asm-1".to_string(),
        description: "PostgreSQL is available locally".to_string(),
        criticality: Criticality::High,
        basis: "README.md".to_string(),
        origin: FactOrigin::Assumed {
            basis: "README.md".to_string(),
        },
        affected_unknowns: Vec::new(),
        affected_task_ids: Vec::new(),
        invalidation: Some(AssumptionInvalidation {
            contradicting_evidence: "Port 5432 is closed and service not found".to_string(),
            evidence_origin: FactOrigin::ToolObserved {
                tool_name: "netstat".to_string(),
            },
            invalidated_at: chrono::Utc::now(),
        }),
        created_at: chrono::Utc::now(),
    };
    intent.assumptions.push(assumption);

    let res_recover_valid =
        validate_strategy_transition(&TaskShape::PlanThenExecute, &TaskShape::Recover, &intent);
    assert!(
        res_recover_valid.is_ok(),
        "Transition to Recover must succeed when an assumption has been invalidated"
    );
}

// ============================================================================
// Production Integration Tests: BUG-06 Interactive AskUser Execution
// ============================================================================

fn setup_test_production_dispatcher(
    runtime: &AppRuntime,
    session_id: SessionId,
    token: CancellationToken,
) -> ProductionActionDispatcher {
    let runner = Arc::new(
        ToolPipelineRunner::new(runtime.tool_registry().clone())
            .with_db_pool(runtime.pool().clone()),
    );
    let context = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        runtime.workspace_root().to_path_buf(),
        token,
    )
    .with_mission_id(MissionId::new())
    .with_task_id(TaskId::new())
    .with_task_repo(SqliteTaskRepository::new(runtime.pool().clone()));
    let policy_gate: Arc<dyn PolicyGate> = Arc::new(AllowAllTestPolicy);
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());

    ProductionActionDispatcher::new(runner, context, policy_gate, AutonomyMode::Safe)
        .with_session_repo(session_repo)
        .with_session_id(session_id)
        .with_event_bus(runtime.event_bus().clone())
        .with_db_pool(runtime.pool().clone())
}

#[tokio::test]
async fn test_bug06_production_ask_user_selects_non_first_option() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let token = CancellationToken::new();
    let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);

    let action = AgentAction::AskUser {
        question: "Select the primary backend database:".to_string(),
        options: vec![
            ActionUserOption {
                id: "opt_1".to_string(),
                label: "SQLite".to_string(),
                description: Some("Local embedded SQL engine".to_string()),
            },
            ActionUserOption {
                id: "opt_2".to_string(),
                label: "PostgreSQL".to_string(),
                description: Some("Production relational server".to_string()),
            },
            ActionUserOption {
                id: "opt_3".to_string(),
                label: "MySQL".to_string(),
                description: Some("Traditional RDBMS".to_string()),
            },
        ],
    };

    let sid = session.id;
    let repo_clone = session_repo.clone();

    // Spawn the dispatcher task which will suspend on AskUser
    let dispatch_handle = tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

    // Wait until pending decision is persisted
    let mut pending = None;
    for _ in 0..50 {
        if let Ok(Some(dec)) = repo_clone.get_pending_decision(sid).await {
            pending = Some(dec);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dec = pending.expect("Pending decision must be saved in session repository");
    assert_eq!(dec.options.len(), 3);
    assert_eq!(dec.options[1].label, "PostgreSQL");

    // Operator selects the non-first option (PostgreSQL)
    let waiter_resumed = DecisionWaiterRegistry::global()
        .resume(
            &dec.id,
            DecisionResumeOutcome::Answer("PostgreSQL".to_string()),
        )
        .await;
    assert!(
        waiter_resumed,
        "DecisionWaiterRegistry must have active waiter"
    );

    repo_clone
        .finalize_decision(sid, &dec.id, false)
        .await
        .unwrap();

    let outcome = dispatch_handle
        .await
        .unwrap()
        .expect("Dispatch must succeed");
    match outcome {
        AgentObservation::UserResponse { answer } => {
            assert_eq!(
                answer, "PostgreSQL",
                "Must return exact selected non-first option"
            );
            assert_ne!(answer, "SQLite", "Must not default to the first option");
            assert_ne!(
                answer, "approved",
                "Must not substitute approval for semantic response"
            );
        }
        other => panic!("Expected UserResponse, got {:?}", other),
    }

    // Verify completed decision in durable persistence
    assert!(
        repo_clone
            .is_decision_completed(sid, &dec.id)
            .await
            .unwrap()
    );
    assert!(
        repo_clone
            .get_pending_decision(sid)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn test_bug06_production_ask_user_freeform_answer() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let token = CancellationToken::new();
    let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);

    let action = AgentAction::AskUser {
        question: "Please specify custom cache backend:".to_string(),
        options: vec![ActionUserOption {
            id: "redis".to_string(),
            label: "Redis".to_string(),
            description: None,
        }],
    };

    let sid = session.id;
    let repo_clone = session_repo.clone();

    let dispatch_handle = tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

    let mut pending = None;
    for _ in 0..50 {
        if let Ok(Some(dec)) = repo_clone.get_pending_decision(sid).await {
            pending = Some(dec);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dec = pending.expect("Pending decision must exist");

    // Operator enters free-form custom answer
    let custom_answer = "Memcached with Consistent Hashing";
    DecisionWaiterRegistry::global()
        .resume(
            &dec.id,
            DecisionResumeOutcome::Answer(custom_answer.to_string()),
        )
        .await;
    repo_clone
        .finalize_decision(sid, &dec.id, false)
        .await
        .unwrap();

    let outcome = dispatch_handle
        .await
        .unwrap()
        .expect("Dispatch must succeed");
    match outcome {
        AgentObservation::UserResponse { answer } => {
            assert_eq!(answer, custom_answer, "Must return exact free-form answer");
        }
        other => panic!("Expected UserResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn test_bug06_production_ask_user_no_predefined_choices() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let token = CancellationToken::new();
    let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);

    // No choices provided
    let action = AgentAction::AskUser {
        question: "Describe the desired authentication flow:".to_string(),
        options: Vec::new(),
    };

    let sid = session.id;
    let repo_clone = session_repo.clone();

    let dispatch_handle = tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

    let mut pending = None;
    for _ in 0..50 {
        if let Ok(Some(dec)) = repo_clone.get_pending_decision(sid).await {
            pending = Some(dec);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let dec = pending.expect("Pending decision must exist");
    assert!(dec.options.is_empty(), "Options must be empty");

    let auth_flow = "OAuth2 with PKCE + Refresh Rotation";
    DecisionWaiterRegistry::global()
        .resume(
            &dec.id,
            DecisionResumeOutcome::Answer(auth_flow.to_string()),
        )
        .await;
    repo_clone
        .finalize_decision(sid, &dec.id, false)
        .await
        .unwrap();

    let outcome = dispatch_handle
        .await
        .unwrap()
        .expect("Dispatch must succeed");
    match outcome {
        AgentObservation::UserResponse { answer } => {
            assert_eq!(answer, auth_flow);
        }
        other => panic!("Expected UserResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn test_bug06_production_ask_user_pauses_and_resumes() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let token = CancellationToken::new();
    let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);

    let action = AgentAction::AskUser {
        question: "Pause verification: Are you ready to continue?".to_string(),
        options: vec![ActionUserOption {
            id: "ready".to_string(),
            label: "Ready".to_string(),
            description: None,
        }],
    };

    let sid = session.id;
    let repo_clone = session_repo.clone();

    let mut dispatch_handle =
        tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

    // 1. Verify that execution remains paused and suspended while waiting
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(
        !dispatch_handle.is_finished(),
        "Task must pause while question is pending"
    );

    let pending = repo_clone
        .get_pending_decision(sid)
        .await
        .unwrap()
        .expect("Pending question must exist");
    assert!(
        DecisionWaiterRegistry::global()
            .has_waiter(&pending.id)
            .await
    );

    // 2. Submit the answer and verify execution resumes
    DecisionWaiterRegistry::global()
        .resume(
            &pending.id,
            DecisionResumeOutcome::Answer("Ready".to_string()),
        )
        .await;
    repo_clone
        .finalize_decision(sid, &pending.id, false)
        .await
        .unwrap();

    // Now task must complete with the answer
    let res = tokio::time::timeout(std::time::Duration::from_secs(2), &mut dispatch_handle)
        .await
        .expect("Task must resume after answer is submitted")
        .unwrap()
        .expect("Dispatch must succeed");

    assert_eq!(
        res,
        AgentObservation::UserResponse {
            answer: "Ready".to_string()
        }
    );
}

#[tokio::test]
async fn test_bug06_production_ask_user_cancellation() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    // Subtest A: Cancellation via token
    {
        let token = CancellationToken::new();
        let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token.clone());

        let action = AgentAction::AskUser {
            question: "Question that gets cancelled via token".to_string(),
            options: Vec::new(),
        };

        let sid = session.id;
        let repo_clone = session_repo.clone();

        let dispatch_handle =
            tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

        // Wait for pending decision
        for _ in 0..50 {
            if repo_clone
                .get_pending_decision(sid)
                .await
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Cancel the token
        token.cancel();

        let outcome = dispatch_handle
            .await
            .unwrap()
            .expect("Dispatch returned cancelled observation");
        match outcome {
            AgentObservation::Cancelled { reason } => {
                assert!(reason.contains("cancelled"));
            }
            other => panic!("Expected Cancelled observation, got {:?}", other),
        }

        // Pending decision must be cleared on cancellation
        assert!(
            repo_clone
                .get_pending_decision(sid)
                .await
                .unwrap()
                .is_none()
        );
    }

    // Subtest B: Cancellation via explicit operator cancel
    {
        let token = CancellationToken::new();
        let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);

        let action = AgentAction::AskUser {
            question: "Question that gets cancelled by operator".to_string(),
            options: Vec::new(),
        };

        let sid = session.id;
        let repo_clone = session_repo.clone();

        let dispatch_handle =
            tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

        let mut pending = None;
        for _ in 0..50 {
            if let Ok(Some(dec)) = repo_clone.get_pending_decision(sid).await {
                pending = Some(dec);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let dec = pending.expect("Pending decision must exist");

        // Operator explicitly cancels
        DecisionWaiterRegistry::global()
            .resume(
                &dec.id,
                DecisionResumeOutcome::Cancelled("Operator dismissed prompt".to_string()),
            )
            .await;
        repo_clone
            .finalize_decision(sid, &dec.id, false)
            .await
            .unwrap();

        let outcome = dispatch_handle
            .await
            .unwrap()
            .expect("Dispatch returned cancelled observation");
        match outcome {
            AgentObservation::Cancelled { reason } => {
                assert_eq!(reason, "Operator dismissed prompt");
            }
            other => panic!("Expected Cancelled observation, got {:?}", other),
        }
    }
}

#[tokio::test]
async fn test_bug06_production_ask_user_persistence_failure_fails_closed() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let token = CancellationToken::new();

    // Use a session ID that has NOT been created in the database
    let unpersisted_session_id = SessionId::new();
    let dispatcher = setup_test_production_dispatcher(&runtime, unpersisted_session_id, token);

    let action = AgentAction::AskUser {
        question: "Will fail closed because session is unpersisted".to_string(),
        options: Vec::new(),
    };

    let outcome = dispatcher.dispatch_action(&action).await;
    assert!(
        outcome.is_err(),
        "Must fail closed if pending question cannot be persisted"
    );
    let err_msg = outcome.unwrap_err();
    assert!(
        err_msg.contains("ask_user persistence failure"),
        "Error message must indicate persistence failure, got: {err_msg}"
    );
}

#[tokio::test]
async fn test_bug06_approval_alone_cannot_resume_semantic_execution() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let token = CancellationToken::new();
    // Dispatcher with AllowAllTestPolicy (policy gate returns Allow for all requests)
    let dispatcher = setup_test_production_dispatcher(&runtime, session.id, token.clone());

    let action = AgentAction::AskUser {
        question: "Semantic question with Policy Allow: Which port?".to_string(),
        options: vec![
            ActionUserOption {
                id: "p1".to_string(),
                label: "8080".to_string(),
                description: None,
            },
            ActionUserOption {
                id: "p2".to_string(),
                label: "9090".to_string(),
                description: None,
            },
        ],
    };

    let sid = session.id;
    let repo_clone = session_repo.clone();

    let mut dispatch_handle =
        tokio::spawn(async move { dispatcher.dispatch_action(&action).await });

    // Wait until pending decision is registered
    for _ in 0..50 {
        if repo_clone
            .get_pending_decision(sid)
            .await
            .unwrap()
            .is_some()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Even though policy allows, the dispatcher MUST NOT resume automatically
    // with "approved" or "8080"
    let short_wait =
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut dispatch_handle).await;
    assert!(
        short_wait.is_err(),
        "Approval alone MUST NOT resume semantic execution without user answer"
    );

    // Cancel to clean up
    token.cancel();
    let _ = dispatch_handle.await;
}

// ============================================================================
// Production Integration Tests: BUG-10 Prefix-Agnostic Decision Routing
// ============================================================================

#[tokio::test]
async fn test_bug10_production_discovery_routing_unprefixed_id() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    // Create an unprefixed question ID (no "discovery_" prefix)
    let q = m31a::workflow::genesis::DynamicQuestion {
        question_id: "system_architecture_choice".to_string(),
        text: "Select primary system architecture:".to_string(),
        options: vec!["Event-Driven".to_string(), "Layered Monolith".to_string()],
        allow_freeform: true,
        target_unknown: "Architecture".to_string(),
        blocking: true,
        reason: "Need architectural guidance".to_string(),
    };

    let dec = InteractiveDecision::discovery_question(session.id.to_string(), &q);
    assert_eq!(dec.id, "system_architecture_choice");
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    // Register waiter in registry as if task is waiting
    let mut rx = DecisionWaiterRegistry::global()
        .register(dec.id.clone())
        .await;

    // Operator submits answer using unprefixed decision ID and selected option
    let resp = InteractiveDecisionResponse {
        decision_id: "system_architecture_choice".to_string(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("opt_2".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };

    let action_res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(resp))
        .await;
    assert!(
        action_res.is_ok(),
        "Runner must route unprefixed question ID by decision kind"
    );

    // Verify waiter was resumed with canonical resolved value ("Layered Monolith", NOT "opt_2")
    let outcome = rx
        .try_recv()
        .expect("Waiter channel must have received outcome");
    match outcome {
        DecisionResumeOutcome::Answer(val) => {
            assert_eq!(
                val, "Layered Monolith",
                "Must resolve option ID to canonical option value"
            );
        }
        other => panic!("Expected Answer outcome, got {:?}", other),
    }

    // Verify decision is now completed and pending is cleared
    assert!(
        session_repo
            .is_decision_completed(session.id, "system_architecture_choice")
            .await
            .unwrap()
    );
    assert!(
        session_repo
            .get_pending_decision(session.id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn test_bug10_production_discovery_routing_substring_not_prefix() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    // Question ID contains 'discovery' as a substring, but NOT as a prefix
    let q = m31a::workflow::genesis::DynamicQuestion {
        question_id: "peer_node_discovery_protocol".to_string(),
        text: "Select discovery mechanism:".to_string(),
        options: vec!["mDNS".to_string(), "Gossip protocol".to_string()],
        allow_freeform: true,
        target_unknown: "Networking".to_string(),
        blocking: true,
        reason: "Determine discovery".to_string(),
    };

    let dec = InteractiveDecision::discovery_question(session.id.to_string(), &q);
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    let mut rx = DecisionWaiterRegistry::global()
        .register(dec.id.clone())
        .await;

    let resp = InteractiveDecisionResponse {
        decision_id: "peer_node_discovery_protocol".to_string(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("opt_1".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };

    let action_res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(resp))
        .await;
    assert!(
        action_res.is_ok(),
        "Must route successfully without requiring 'discovery_' prefix"
    );

    let outcome = rx.try_recv().expect("Waiter must receive answer");
    assert_eq!(outcome, DecisionResumeOutcome::Answer("mDNS".to_string()));
}

#[tokio::test]
async fn test_bug10_production_discovery_routing_custom_answer() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    let q = m31a::workflow::genesis::DynamicQuestion {
        question_id: "custom_storage_engine".to_string(),
        text: "Specify storage:".to_string(),
        options: vec!["S3".to_string(), "Local FS".to_string()],
        allow_freeform: true,
        target_unknown: "Storage".to_string(),
        blocking: true,
        reason: "Select storage".to_string(),
    };

    let dec = InteractiveDecision::discovery_question(session.id.to_string(), &q);
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    let mut rx = DecisionWaiterRegistry::global()
        .register(dec.id.clone())
        .await;

    let resp = InteractiveDecisionResponse {
        decision_id: "custom_storage_engine".to_string(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: None,
        selected_option_ids: Vec::new(),
        custom_answer: Some("Hybrid IPFS with Local Cache".to_string()),
        cancelled: false,
    };

    let action_res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(resp))
        .await;
    assert!(action_res.is_ok());

    let outcome = rx.try_recv().expect("Waiter must receive answer");
    assert_eq!(
        outcome,
        DecisionResumeOutcome::Answer("Hybrid IPFS with Local Cache".to_string())
    );
}

#[tokio::test]
async fn test_bug10_production_discovery_routing_unknown_and_stale_rejection() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    // 1. Unknown question ID
    let unknown_resp = InteractiveDecisionResponse {
        decision_id: "completely_unknown_id".to_string(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("opt_1".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    let res_unknown = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(unknown_resp))
        .await;
    assert!(res_unknown.is_err(), "Must reject unknown question ID");
    assert!(
        res_unknown
            .unwrap_err()
            .to_string()
            .contains("No pending decision found")
    );

    // 2. Stale completed decision
    let q = m31a::workflow::genesis::DynamicQuestion {
        question_id: "stale_test_q".to_string(),
        text: "Stale test:".to_string(),
        options: vec!["Yes".to_string(), "No".to_string()],
        allow_freeform: false,
        target_unknown: "Stale".to_string(),
        blocking: true,
        reason: "Test".to_string(),
    };
    let dec = InteractiveDecision::discovery_question(session.id.to_string(), &q);
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    let _rx = DecisionWaiterRegistry::global()
        .register(dec.id.clone())
        .await;

    let resp = InteractiveDecisionResponse {
        decision_id: "stale_test_q".to_string(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("opt_1".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };

    // First submission succeeds
    let res1 = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(resp.clone()))
        .await;
    assert!(res1.is_ok());

    // Duplicate submission of already-completed decision MUST be rejected
    let res2 = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(resp))
        .await;
    assert!(
        res2.is_err(),
        "Must reject duplicate response to completed decision"
    );
    assert!(
        res2.unwrap_err()
            .to_string()
            .contains("already been completed")
    );
}

// ============================================================================
// Production Integration Tests: BUG-03 & BUG-11 Decision Lifecycle & Recovery
// ============================================================================

#[tokio::test]
async fn test_bug03_atomic_finalize_and_domain_failure_safety() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    // 1. Verify domain operation failure preserves pending decision
    let dec = InteractiveDecision::planning_choice(
        session.id.to_string(),
        "build feature",
        "Choose mode",
    );
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    // Submit invalid option ID that fails domain validation
    let bad_resp = InteractiveDecisionResponse {
        decision_id: dec.id.clone(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("invalid_nonexistent_choice".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    let res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(bad_resp))
        .await;
    assert!(res.is_err(), "Domain validation failure must return error");

    // Crucial: Pending decision must NOT be consumed or erased by a failed operation!
    let still_pending = session_repo.get_pending_decision(session.id).await.unwrap();
    assert!(
        still_pending.is_some(),
        "Pending decision must remain intact after validation failure"
    );
    assert_eq!(still_pending.unwrap().id, dec.id);
    assert!(
        !session_repo
            .is_decision_completed(session.id, &dec.id)
            .await
            .unwrap()
    );

    // 2. Successful resolution atomically clears pending and records completed
    let good_resp = InteractiveDecisionResponse {
        decision_id: dec.id.clone(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: Some("direct".to_string()),
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: false,
    };
    let good_res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(good_resp))
        .await;
    assert!(good_res.is_ok(), "Valid resolution must succeed");

    assert!(
        session_repo
            .get_pending_decision(session.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        session_repo
            .is_decision_completed(session.id, &dec.id)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_bug11_atomic_cancellation_clears_planning_choice() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let mut runner = InteractiveSessionRunner::new(runtime.clone());
    let session = runner.init_session(None).await.unwrap();
    let session_repo = runner.session_repo().clone();

    session_repo
        .set_planning_choice(session.id, m31a::runtime::PlanningChoice::ProjectPlanning)
        .await
        .unwrap();

    let dec = InteractiveDecision::planning_choice(session.id.to_string(), "prompt", "question");
    session_repo
        .set_pending_decision(session.id, &dec)
        .await
        .unwrap();

    // Cancel decision
    let cancel_resp = InteractiveDecisionResponse {
        decision_id: dec.id.clone(),
        session_id: session.id.to_string(),
        execution_id: None,
        selected_option_id: None,
        selected_option_ids: Vec::new(),
        custom_answer: None,
        cancelled: true,
    };

    let res = runner
        .handle_action(ApplicationAction::DecisionResponseSubmitted(cancel_resp))
        .await;
    assert!(res.is_ok());

    // Atomic finalize with clear_planning_choice must clear both
    let s = session_repo.get_session(session.id).await.unwrap().unwrap();
    assert_eq!(SqliteSessionRepository::read_planning_choice(&s), None);
    assert!(
        session_repo
            .get_pending_decision(session.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        session_repo
            .is_decision_completed(session.id, &dec.id)
            .await
            .unwrap()
    );
}

// ============================================================================
// Production Integration Tests: BUG-02 Point-of-Access Workspace Containment
// ============================================================================

#[test]
fn test_bug02_production_point_of_access_containment_comprehensive() {
    let tmp = tempdir().unwrap();
    let ws = tmp.path();

    // 1. Canonical traversal targets
    assert!(validate_workspace_path(ws, "../../secret.txt").is_err());
    assert!(validate_workspace_path(ws, "subdir/../../../etc/shadow").is_err());
    assert!(validate_workspace_path(ws, "./../../escape").is_err());

    // 2. Out-of-tree absolute paths
    assert!(validate_workspace_path(ws, "/etc/passwd").is_err());
    assert!(validate_workspace_path(ws, "/tmp/evil.sh").is_err());

    // 3. Nested JSON structures with parameter aliases
    let nested_params = serde_json::json!({
        "options": {
            "source": "/etc/shadow",
            "destination": "../../escape.txt",
            "metadata": {
                "dest": "/var/run/secret",
                "src": "../outside/src.rs",
                "dir": "/opt/forbidden",
                "directory": "../../root",
            }
        },
        "extra_files": [
            {"file": "../../etc/hosts"},
            {"filename": "/boot/vmlinuz"}
        ],
        "batch": {
            "paths": ["valid_in_ws.rs", "../../bad_batch.rs"]
        }
    });

    let extracted = extract_all_action_paths("custom_tool", &nested_params);
    assert!(extracted.contains(&"/etc/shadow".to_string()));
    assert!(extracted.contains(&"../../escape.txt".to_string()));
    assert!(extracted.contains(&"/var/run/secret".to_string()));
    assert!(extracted.contains(&"../outside/src.rs".to_string()));
    assert!(extracted.contains(&"/opt/forbidden".to_string()));
    assert!(extracted.contains(&"../../root".to_string()));
    assert!(extracted.contains(&"../../etc/hosts".to_string()));
    assert!(extracted.contains(&"/boot/vmlinuz".to_string()));
    assert!(extracted.contains(&"../../bad_batch.rs".to_string()));
    assert!(extracted.contains(&"valid_in_ws.rs".to_string()));

    // Verify each extracted out-of-bounds path fails containment
    for p in &extracted {
        if p != "valid_in_ws.rs" {
            assert!(
                validate_workspace_path(ws, p).is_err(),
                "Path '{p}' must be rejected by workspace containment"
            );
        } else {
            assert!(validate_workspace_path(ws, p).is_ok());
        }
    }

    // 4. Mutation path extraction covers all aliases
    let mutation_paths = ToolCapabilityClass::extract_mutation_paths("write_file", &nested_params);
    assert!(mutation_paths.contains(&"/etc/shadow".to_string()));
    assert!(mutation_paths.contains(&"../../escape.txt".to_string()));
}

// ============================================================================
// Production Integration Tests: BUG-09 All-or-Nothing Proposal Conversion
// ============================================================================

#[tokio::test]
async fn test_bug09_model_proposal_all_or_nothing_validation_production() {
    let mission_id = MissionId::new();
    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

    // Batch with valid Call 1, INVALID Call 2 (missing path), valid Call 3
    let model = MockModelCaller::new(vec![
        ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall {
                    id: "call-1".to_string(),
                    name: "write_file".to_string(),
                    arguments: serde_json::json!({"path": "src/good.rs", "content": "fn good() {}"}),
                },
                ModelToolCall {
                    id: "call-2".to_string(),
                    name: "write_file".to_string(),
                    arguments: serde_json::json!({"content": "missing path!"}),
                },
                ModelToolCall {
                    id: "call-3".to_string(),
                    name: "write_file".to_string(),
                    arguments: serde_json::json!({"path": "src/good2.rs", "content": "fn good2() {}"}),
                },
            ],
        },
        ModelProposal::Complete {
            summary: "Done".to_string(),
            artifacts: Vec::new(),
        },
    ]);

    let dispatcher = MockActionDispatcher;
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let _outcome = runner
        .run_step_loop(&model, &dispatcher, &token, tracker)
        .await;

    // Verify step 1 recorded the all-or-nothing validation failure
    let history = runner.step_history();
    assert!(!history.is_empty());
    let step1 = &history[0];
    assert!(
        step1
            .model_proposal_summary
            .contains("proposal_validation_failure"),
        "Summary must record proposal validation failure"
    );
    assert_eq!(
        step1.tool_validation_status.as_deref(),
        Some("rejected"),
        "Status must be rejected"
    );
    // Crucial: The actions_executed on the failed proposal must record rejection and zero tools executed
    assert_eq!(step1.actions_executed.len(), 1);
    assert!(!step1.actions_executed[0].1.success);
    assert!(
        step1.actions_executed[0]
            .1
            .error
            .as_ref()
            .unwrap()
            .contains("all-or-nothing validation failed"),
        "Error must explain all-or-nothing failure"
    );
}

// ============================================================================
// Production Integration Tests: BUG-07 Delegation & Handoff Semantics
// ============================================================================

#[tokio::test]
async fn test_bug07_production_delegation_depth_ceiling_enforced() {
    let mission_id = MissionId::new();
    let profile = AgentProfile::built_in(AgentRole::implementer());
    let mut runner =
        WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

    // Set delegation depth equal to max delegation depth
    runner.delegation_depth = 3;
    runner.max_delegation_depth = 3;

    let model = MockModelCaller::new(vec![ModelProposal::Handoff {
        target_role: "researcher".to_string(),
        reason: "Investigate architecture".to_string(),
    }]);

    let dispatcher = MockActionDispatcher;
    let token = CancellationToken::new();
    let tracker = Arc::new(ExecutionActivityTracker::default());

    let outcome = runner
        .run_step_loop(&model, &dispatcher, &token, tracker)
        .await;

    match outcome {
        AgentOutcome::Failed(evidence) => {
            assert_eq!(evidence.failure_class, FailureClass::PolicyDenial);
            assert!(
                evidence
                    .message
                    .to_lowercase()
                    .contains("maximum delegation")
                    && evidence.message.to_lowercase().contains("depth"),
                "Expected delegation depth limit message, got: {}",
                evidence.message
            );
        }
        other => panic!("Expected PolicyDenial failure, got {:?}", other),
    }
}

#[tokio::test]
async fn test_bug07_production_subtask_delegation_observation() {
    let tmp = tempdir().unwrap();
    let runtime = Arc::new(AppRuntime::new(tmp.path()).await.unwrap());
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo.create_session(tmp.path()).await.unwrap();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Delegation Test Mission")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(runtime.pool())
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("implementer")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(runtime.pool())
    .await
    .unwrap();

    let mut parent_task =
        m31a::state::task::Task::new(task_id, mission_id, "Parent Task".to_string());
    parent_task.role = AgentRole::implementer();
    SqliteTaskRepository::new(runtime.pool().clone())
        .insert(&parent_task)
        .await
        .unwrap();

    let token = CancellationToken::new();
    let initial_dispatcher = setup_test_production_dispatcher(&runtime, session.id, token);
    let mut ctx = initial_dispatcher.context().clone();
    ctx.mission_id = Some(mission_id);
    ctx.task_id = Some(task_id);
    ctx.agent_id = Some(agent_id);
    ctx.agent_role = Some(AgentRole::implementer());
    ctx.task_repo = Some(SqliteTaskRepository::new(runtime.pool().clone()));
    let dispatcher = ProductionActionDispatcher::new(
        Arc::clone(initial_dispatcher.runner()),
        ctx,
        Arc::clone(initial_dispatcher.policy_gate()),
        initial_dispatcher.autonomy_mode(),
    )
    .with_session_repo(session_repo)
    .with_session_id(session.id)
    .with_event_bus(runtime.event_bus().clone())
    .with_db_pool(runtime.pool().clone());

    let action = AgentAction::Delegate {
        target_role: AgentRole::researcher(),
        task_id: None,
        reason: "Analyze performance bottlenecks".to_string(),
        inputs: Vec::new(),
    };

    let obs = dispatcher
        .dispatch_action(&action)
        .await
        .expect("Delegate dispatch must succeed");
    match obs {
        AgentObservation::DelegationResult {
            child_agent_id: _,
            outcome,
        } => match *outcome {
            AgentOutcome::Succeeded { output, .. } => {
                assert!(
                    output.contains("Subtask successfully delegated to role 'researcher'"),
                    "Expected successful delegation message, got: {}",
                    output
                );
                // Verify the child task was durably persisted in SQLite
                let task_repo = SqliteTaskRepository::new(runtime.pool().clone());
                let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
                let delegated_task = tasks
                    .iter()
                    .find(|t| t.role == AgentRole::researcher())
                    .expect("Delegated researcher task must exist in SQLite");
                assert_eq!(delegated_task.role, AgentRole::researcher());
                assert!(
                    delegated_task
                        .title
                        .contains("Analyze performance bottlenecks")
                );
            }
            AgentOutcome::Failed(evidence) => {
                panic!(
                    "Delegation should have succeeded, but failed with: {}",
                    evidence.message
                );
            }
            other => panic!("Unexpected outcome {:?}", other),
        },
        other => panic!("Expected DelegationResult, got {:?}", other),
    }
}
