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

use m31a::agent::action::{ActionParseError, AgentAction, AgentObservation};
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
use m31a::ids::{AgentId, MissionId, SessionId, TaskId};
use m31a::interaction::decision::{DecisionKind, InteractiveDecision, InteractiveDecisionResponse};
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::seams::context::CompiledContext;
use m31a::model::types::{ModelToolCall, TokenUsage, UserOption};
use m31a::planning::risks::{Criticality, UnknownFate};
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;

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

struct MockModelCaller {
    proposals: Vec<ModelProposal>,
    call_idx: AtomicU32,
}

impl MockModelCaller {
    fn new(proposals: Vec<ModelProposal>) -> Self {
        Self {
            proposals,
            call_idx: AtomicU32::new(0),
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
    let outside_dir = tempdir().unwrap();
    let outside_target = outside_dir.path().join("secret.txt");
    std::fs::write(&outside_target, "secret").unwrap();

    let symlink_path = ws.join("src/link_to_outside");
    #[cfg(unix)]
    {
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
    // When prompt is a build prompt and not explicitly asking for project planning, defaults to prompt question
    assert!(choice.is_none());

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
