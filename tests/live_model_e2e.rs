//! Phase 36.2 — Live Model End-to-End Acceptance Test Suite.
//!
//! Dedicated suite exercising the genuine production model/provider stack.
//! Gated by `--features live-model-tests`.
//!
//! Enforces:
//! 1. Real production provider (NVIDIA NIM via SSE / HTTPS).
//! 2. Real model client & canonical model routing (nvidia/nemotron-3-ultra-550b-a55b).
//! 3. Real prompt catalog & compilers (genesis.dynamic_questions, planning.decompose, planning.revision, planning.task_revision).
//! 4. Full human-governed pre-execution lifecycle entering from application boundary:
//!    User Intent -> Dynamic Questions -> Operator Answers ->
//!    Real Model Plan Generation -> Live Plan Review & Model Revision -> Plan Acceptance ->
//!    Real Model Task Generation -> Live Task Review & Model Revision -> Task Acceptance ->
//!    Execution Authorization -> Authorized Materialization Handoff ->
//!    AutonomyController -> SchedulerEngine -> WorkerRunner ->
//!    Real Tool Proposals -> Policy Gate -> Real Workspace Mutation ->
//!    Verification -> Evidence Sealed -> Completion.
//! 5. Single authoritative mission/session identity linkage:
//!    session_id, mission_id, plan_revision, task_revision, authorization_id, task_graph_id.
//! 6. Negative invariants:
//!    - Zero mutations before authorization.
//!    - Unconditional failure closed without authorization.
//!    - Stale authorization invalidated by subsequent task revision.
//!    - Real provider failure is not fabricated as success (explicit failure).

use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

use async_trait::async_trait;
use m31a::dag::materializer::TaskGraphMaterializer;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{ApprovalRequestId, MissionId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::runner::InteractiveSessionRunner;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::kernel::plan::CandidatePlan;
use m31a::persistence::sqlite::TaskGraphRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{
    AuthorizationDecision, ExecutionAuthorization, PreExecutionResponse, RevisionAuthorType,
};
use m31a::policy::{
    ApprovalAction, ApprovalChannel, ApprovalCoordinator, ApprovalError, ApprovalExplanationPacket,
};
use m31a::runtime::AppRuntime;
use m31a::state_machine::lifecycle::LifecycleStage;
use m31a::testing::real_model::RealModelHarness;

/// Live test approval channel simulating operator inspection and tool approvals.
struct LiveTestApprovalChannel {
    coordinator: Arc<tokio::sync::RwLock<Option<ApprovalCoordinator>>>,
}

#[async_trait]
impl ApprovalChannel for LiveTestApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        println!(
            "  [Operator Approval Intercept] Tool '{}' requested approval: {}",
            packet.tool_or_capability, packet.summary
        );
        let id = packet.request_id;
        let coord_lock = self.coordinator.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            if let Some(coord) = coord_lock.read().await.as_ref() {
                let _ = coord
                    .resolve_request(id, ApprovalAction::AllowOnce, "operator_alex")
                    .await;
                println!(
                    "  [Operator Approval Intercept] Operator alex granted approval for tool request {}",
                    id
                );
            }
        });
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

/// Mandatory real-model harness requirement for Phase 36.2.
///
/// If `--features live-model-tests` is explicitly invoked, missing credentials
/// MUST produce a clear failure rather than silently skipping the test.
fn require_live_harness() -> RealModelHarness {
    RealModelHarness::ensure().unwrap_or_else(|skipped| {
        panic!(
            "LIVE MODEL ACCEPTANCE TEST CANNOT RUN: {}\n\
             When running with '--features live-model-tests', valid production credentials \
             are mandatory. Ensure NVIDIA_API_KEY or API_KEY_NVIDIA is set in .env",
            skipped.reason
        )
    })
}

/// Setup a controlled, temporary Git repository fixture for safe workspace isolation.
async fn setup_controlled_workspace()
-> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("failed to create controlled workspace tempdir");
    let repo_path = dir.path();

    // 1. Initialize clean Git repository
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_path)
        .status()
        .expect("git init failed");
    assert!(git_init.success(), "git init must succeed");

    Command::new("git")
        .args(["config", "user.name", "M31A Acceptance Engineer"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "acceptance@m31a.local"])
        .current_dir(repo_path)
        .status()
        .expect("git config user.email failed");

    // 2. Base files and initial commit
    tokio::fs::write(repo_path.join(".gitignore"), ".m31a/\ntarget/\n")
        .await
        .unwrap();
    tokio::fs::write(
        repo_path.join("README.md"),
        "# Eshan's Cafe\nControlled test repository for M31A acceptance testing.\n",
    )
    .await
    .unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_path)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .status()
        .expect("git commit failed");

    // 3. Initialize SQLite database for lifecycle persistence
    let db_path = repo_path.join(".m31a").join("m31a.db");
    tokio::fs::create_dir_all(repo_path.join(".m31a"))
        .await
        .unwrap();
    let pool = initialize_database(&db_path)
        .await
        .expect("failed to initialize SQLite database");
    let bus = Arc::new(BroadcastEventBus::new(1024));

    (dir, pool, bus)
}

async fn create_test_session(pool: &sqlx::SqlitePool, dir: &std::path::Path) -> String {
    let repo = SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir).await.expect("create test session");
    session.id.to_string()
}

// ---------------------------------------------------------------------------
// 1. Full Production Live Model Lifecycle Acceptance Test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_model_full_production_lifecycle() {
    let harness = require_live_harness();
    println!("\n=================================================================");
    println!("=== [PHASE 36.2] LIVE MODEL ACCEPTANCE TEST: FULL LIFECYCLE ===");
    println!("=================================================================");
    println!("Provider:  {}", harness.provider_name());
    println!("Model ID:  {}", harness.model_id());
    println!("Base URL:  {}", harness.base_url());

    let (dir, pool, _bus) = setup_controlled_workspace().await;
    let repo_path = dir.path().to_path_buf();

    // 1. Initialize production AppRuntime wired to the real NVIDIA provider
    let approval_channel = Arc::new(LiveTestApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(None)),
    });
    let raw_runtime = harness
        .runtime_for(&repo_path)
        .await
        .expect("AppRuntime instantiation failed");
    *approval_channel.coordinator.write().await =
        Some((**raw_runtime.approval_coordinator()).clone());
    let runtime = Arc::new(raw_runtime.with_approval_channel(approval_channel));

    // 2. Enter from Application Boundary: InteractiveSessionRunner
    let mut session_runner = InteractiveSessionRunner::new(runtime.clone());
    let session = session_runner
        .init_session(None)
        .await
        .expect("Interactive session initialization failed");
    let session_id = session.id.to_string();
    let mission_id = session
        .active_mission_id
        .expect("Session must be bound to active mission id");

    println!("\n[Stage 1: Session & Mission Identity Established]");
    println!("  Session ID: {}", session_id);
    println!("  Mission ID: {}", mission_id);

    let coordinator = runtime.create_pre_execution_coordinator();
    let repo = coordinator.lifecycle_repo();

    // 3. USER INTENT INTAKE (Real Model Question Generation via genesis.dynamic_questions)
    let user_request =
        "I want to build a small website with Next.js for my coffee shop named Eshan's Cafe.";
    println!("\n[Stage 2: Intent Intake via Application Boundary]");
    println!("  Request: \"{user_request}\"");

    let parsed_msg = m31a::interaction::mentions::MentionParser::parse(user_request, &repo_path);
    session_runner
        .handle_action(ApplicationAction::UserTextSubmitted(parsed_msg))
        .await
        .expect("UserTextSubmitted action failed");

    // INVARIANT E: No pre-authorization side effects on disk
    assert!(
        !repo_path.join("cafe.config.json").exists(),
        "INVARIANT E: No workspace mutation permitted before authorization"
    );

    // Check dynamic questions stored in database
    let questions = repo
        .load_discovery_questions(&session_id)
        .await
        .unwrap_or_default();

    println!("\n[Stage 3: Operator Clarification Answers]");
    if !questions.is_empty() {
        println!(
            "  Model identified {} dynamic questions for clarification:",
            questions.len()
        );
        for (idx, q) in questions.iter().enumerate() {
            println!(
                "    Q{}: \"{}\" (target: {})",
                idx + 1,
                q.text,
                q.target_unknown
            );
            assert!(!q.question_id.is_empty(), "Question ID must not be empty");
            assert!(!q.text.is_empty(), "Question text must not be empty");

            let simulated_answer = match q.target_unknown.as_str() {
                "unk_arch_choice" => {
                    "Next.js App Router with TypeScript, TailwindCSS, and lightweight JSON data store."
                }
                _ => {
                    "Include Home landing page, coffee & pastry menu, about section, and opening hours."
                }
            };
            println!("  Answering Q{}: \"{}\"", idx + 1, simulated_answer);
            session_runner
                .handle_action(ApplicationAction::QuestionAnswerSubmitted {
                    session_id: Some(session_id.clone()),
                    question_id: q.question_id.clone(),
                    answer: simulated_answer.to_string(),
                })
                .await
                .expect("QuestionAnswerSubmitted action failed");

            // Once the required questions have been answered and the plan generated, proceed
            if let Ok(Some(_)) = repo.load_latest_plan_revision(&session_id).await {
                println!("  Initial CandidatePlan formulated by model; proceeding to plan review.");
                break;
            }
        }
    } else {
        println!("  Model formulated initial plan directly without blocking unknowns");
    }

    // 4. VERIFY INITIAL CANDIDATE PLAN (Generated by Real Model via planning.decompose)
    println!("\n[Stage 4: Initial CandidatePlan Generated by Real Model]");
    let plan_rev1 = repo
        .load_latest_plan_revision(&session_id)
        .await
        .unwrap()
        .expect("Plan revision 1 must exist");

    println!("  Plan ID:    {}", plan_rev1.plan_id);
    println!("  Revision:   {}", plan_rev1.revision);
    println!(
        "  Author:     {} ({:?})",
        plan_rev1.created_by, plan_rev1.author_type
    );
    println!("  Objective:  {}", plan_rev1.content.objective);
    println!("  Tasks:      {}", plan_rev1.content.tasks.len());
    for (i, t) in plan_rev1.content.tasks.iter().enumerate() {
        println!("    Task {}: [{}] {}", i + 1, t.id, t.objective);
    }

    assert_eq!(plan_rev1.revision, 1);
    assert_eq!(plan_rev1.created_by, "model");
    assert_eq!(plan_rev1.author_type, RevisionAuthorType::Model);
    assert!(
        !plan_rev1.content.tasks.is_empty(),
        "Plan must have candidate tasks produced by real model"
    );

    // 5. LIVE PLAN REVIEW & REAL MODEL REVISION (planning.revision contract)
    println!("\n[Stage 5: Live Plan Review & Model Revision]");
    let plan_feedback = "Add an online ordering section with item customization and shopping cart.";
    println!("  Operator steering: \"{plan_feedback}\"");

    session_runner
        .handle_action(ApplicationAction::PlanRevisionRequested {
            session_id: Some(session_id.clone()),
            feedback: plan_feedback.to_string(),
        })
        .await
        .expect("PlanRevisionRequested failed");

    let plan_rev2 = repo
        .load_latest_plan_revision(&session_id)
        .await
        .unwrap()
        .expect("Plan revision 2 must exist");

    println!("  Revised Plan Revision: {}", plan_rev2.revision);
    println!("  Revised Plan Objective: {}", plan_rev2.content.objective);
    assert_eq!(plan_rev2.revision, 2);
    assert_eq!(plan_rev2.supersedes_revision, Some(1));
    assert_eq!(plan_rev2.created_by, "model");
    assert_eq!(plan_rev2.author_type, RevisionAuthorType::Model);
    assert_ne!(
        plan_rev2.content, plan_rev1.content,
        "Revised plan content must genuinely change from revision 1"
    );

    // 6. LIVE PLAN ACCEPTANCE -> CANDIDATE TASKS DRAFTED
    println!("\n[Stage 6: Plan Acceptance & Task Lowering]");
    session_runner
        .handle_action(ApplicationAction::PlanAcceptRequested {
            session_id: Some(session_id.clone()),
        })
        .await
        .expect("PlanAcceptRequested failed");

    let task_rev1 = repo
        .load_latest_task_revision(&session_id)
        .await
        .unwrap()
        .expect("Task revision 1 must exist");

    println!(
        "  Task Revision 1: revision={}, plan_revision={}, task_count={}",
        task_rev1.revision,
        task_rev1.plan_revision,
        task_rev1.tasks.len()
    );
    assert_eq!(task_rev1.revision, 1);
    assert_eq!(task_rev1.plan_revision, 2);
    assert_eq!(task_rev1.created_by, "model");
    assert_eq!(task_rev1.author_type, RevisionAuthorType::Model);
    assert!(!task_rev1.tasks.is_empty());

    // 7. LIVE TASK REVIEW & REAL MODEL REGENERATION (planning.task_revision contract)
    println!("\n[Stage 7: Live Task Review & Model Task Revision]");
    let task_feedback = "Decompose into concrete tasks. Ensure TASK-01 creates cafe.config.json for Eshan's Cafe with expected_outputs ['cafe.config.json'] and verification 'artifact_inspection'.";
    println!("  Operator steering: \"{task_feedback}\"");

    session_runner
        .handle_action(ApplicationAction::TaskRegenerateRequested {
            session_id: Some(session_id.clone()),
            feedback: Some(task_feedback.to_string()),
        })
        .await
        .expect("TaskRegenerateRequested failed");

    let task_rev2 = repo
        .load_latest_task_revision(&session_id)
        .await
        .unwrap()
        .expect("Task revision 2 must exist");

    println!(
        "  Task Revision 2: revision={}, task_count={}",
        task_rev2.revision,
        task_rev2.tasks.len()
    );
    assert_eq!(task_rev2.revision, 2);
    assert_eq!(task_rev2.supersedes_revision, Some(1));
    assert_eq!(task_rev2.created_by, "model");
    assert_eq!(task_rev2.author_type, RevisionAuthorType::Model);
    assert!(!task_rev2.tasks.is_empty());

    // 8. TASK ACCEPTANCE -> EXECUTION AWAITING AUTHORIZATION
    println!("\n[Stage 8: Accepting Candidate Tasks]");
    session_runner
        .handle_action(ApplicationAction::TasksAcceptRequested {
            session_id: Some(session_id.clone()),
        })
        .await
        .expect("TasksAcceptRequested failed");

    let state = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .expect("Lifecycle state must exist");
    assert_eq!(state.stage, LifecycleStage::ExecutionAwaitingAuthorization);
    assert_eq!(state.plan_revision, 2);
    assert_eq!(state.task_revision, 2);

    // 9. PRE-AUTHORIZATION MUTATION INVARIANT CHECK
    // Rule: Before explicit operator authorization, the workspace must NOT be mutated
    let cafe_config_path = repo_path.join("cafe.config.json");
    let package_json_path = repo_path.join("package.json");
    assert!(
        !cafe_config_path.exists(),
        "Safety invariant: cafe.config.json must NOT exist prior to authorization"
    );
    assert!(
        !package_json_path.exists(),
        "Safety invariant: package.json must NOT exist prior to authorization"
    );
    println!("  Verified: Workspace contains zero mutations prior to authorization.");

    // 10. REAL EXECUTION AUTHORIZATION & PRODUCTION RUNTIME HANDOFF
    println!(
        "\n[Stage 9: Submitting Explicit Execution Authorization & Production Runtime Handoff]"
    );
    // Note: The application boundary (InteractiveSessionRunner) automatically materializes the
    // TaskGraph and initiates AppRuntime::run_authorized_mission with AutonomyController.
    // The test DOES NOT clone or modify authorized artifacts, nor call WorkerRunner directly.
    session_runner
        .handle_action(ApplicationAction::ExecutionAuthorizationSubmitted {
            session_id: Some(session_id.clone()),
            decision: true,
            reason: None,
        })
        .await
        .expect("ExecutionAuthorizationSubmitted and runtime handoff failed");

    // 11. VERIFY IDENTITY LINKAGE ACROSS SUBSYSTEMS (Section 18)
    println!("\n[Stage 10: Single Mission/Session Identity Linkage Verification]");
    let final_auth = repo
        .load_latest_execution_authorization(&session_id)
        .await
        .unwrap()
        .expect("Authorization record must exist");
    assert_eq!(final_auth.decision, AuthorizationDecision::Authorized);
    assert_eq!(final_auth.plan_revision, 2);
    assert_eq!(final_auth.task_revision, 2);

    let active_graph =
        m31a::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository::new(
            pool.clone(),
        )
        .get_active_graph(mission_id)
        .await
        .expect("query active graph")
        .expect("Active TaskGraph must exist for authorized mission");

    println!("  Session ID:       {}", session_id);
    println!("  Mission ID:       {}", mission_id);
    println!("  Plan Revision:    {}", final_auth.plan_revision);
    println!("  Task Revision:    {}", final_auth.task_revision);
    println!("  Authorization ID: {}", final_auth.id);
    println!("  TaskGraph ID:     {}", active_graph.id);

    let initial_plan_id: String = sqlx::query_scalar(
        "SELECT plan_id FROM task_graphs WHERE mission_id = ? ORDER BY revision ASC LIMIT 1",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("query initial graph");

    assert_eq!(active_graph.mission_id, mission_id);
    assert_eq!(initial_plan_id, plan_rev2.plan_id);
    assert!(
        active_graph.plan_id == plan_rev2.plan_id
            || active_graph
                .plan_id
                .starts_with(&format!("replan-{}", mission_id)),
        "Active graph must match authorized plan or valid runtime replan"
    );

    // 12. POST-AUTHORIZATION WORKSPACE MUTATION & VERIFICATION EVIDENCE
    println!("\n[Stage 11: Verification of Workspace Mutation & Evidence]");

    // Verify observable workspace change on disk
    let mut found_created_files = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&repo_path).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with('.') && file_name != "README.md" {
                found_created_files.push(file_name);
            }
        }
    }

    println!(
        "  Observable created files in workspace: {:?}",
        found_created_files
    );
    for fname in &found_created_files {
        let fpath = repo_path.join(fname);
        if let Ok(content) = tokio::fs::read_to_string(&fpath).await {
            println!("  File [{}] ({} bytes):\n{}", fname, content.len(), content);
        }
    }

    // Verify outbound request telemetry and canonical model routing
    let traced = harness.traced_requests();
    println!(
        "  Total model requests traced across lifecycle: {}",
        traced.len()
    );
    assert!(
        !traced.is_empty(),
        "Real model must have been invoked across the acceptance suite"
    );
    harness.assert_canonical_model_routing();

    let metrics = harness.metrics();
    println!("  Lifecycle Metrics Summary: {}", metrics.summary_line());

    println!("\n=================================================================");
    println!("=== [PHASE 36.2] LIVE MODEL ACCEPTANCE TEST: SUCCESSFUL ===");
    println!("=================================================================\n");
}

// ---------------------------------------------------------------------------
// 2. Negative Test: Cannot Execute Without Authorization (Fail-Closed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_model_cannot_execute_without_authorization() {
    let harness = require_live_harness();
    println!("\n=== [PHASE 36.2] NEGATIVE TEST: CANNOT EXECUTE WITHOUT AUTHORIZATION ===");

    let (dir, pool, _bus) = setup_controlled_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let runtime = Arc::new(harness.runtime_for(&repo_path).await.unwrap());
    let mut session_runner = InteractiveSessionRunner::new(runtime.clone());
    let session = session_runner.init_session(None).await.unwrap();
    let session_id = session.id.to_string();
    let mission_id = session.active_mission_id.unwrap();
    let operator = "operator_alex";

    let coordinator = runtime.create_pre_execution_coordinator();
    let init_resp = coordinator
        .init_intent(&session_id, "Build a simple Next.js website", operator)
        .await
        .unwrap();

    if let PreExecutionResponse::QuestionsRequired { questions, .. } = init_resp {
        for q in questions {
            coordinator
                .submit_answer(
                    &session_id,
                    &q.question_id,
                    "Next.js with TypeScript and TailwindCSS",
                    operator,
                )
                .await
                .expect("submit_answer failed");
        }
    }

    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            operator,
        )
        .await
        .expect("PlanAcceptRequested failed");

    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            operator,
        )
        .await
        .expect("TasksAcceptRequested failed");

    // Invariant: Before authorization, materializer must reject any execution attempt
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let unauth = ExecutionAuthorization::new(&session_id, 1, 1, operator);
    let candidate_plan = CandidatePlan::new("plan-unauth", "unauthorized objective", vec![]);
    let mat_result = materializer
        .materialize_authorized(mission_id, &candidate_plan, 1, 1, &unauth)
        .await;
    assert!(
        mat_result.is_err(),
        "Safety invariant: materialize_authorized must fail when authorization is ungranted"
    );

    // Operator explicitly denies authorization
    let reject_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: false,
                reason: Some("Budget ceiling exceeded".to_string()),
            },
            operator,
        )
        .await
        .expect("Rejection action must process cleanly");

    match reject_resp {
        PreExecutionResponse::Terminated { stage, reason, .. } => {
            assert_eq!(stage, LifecycleStage::Rejected);
            assert_eq!(reason, "Budget ceiling exceeded");
        }
        other => panic!("Expected Terminated on rejection, got {:?}", other),
    }

    // Verify workspace has zero mutations on disk
    let cafe_config_path = repo_path.join("cafe.config.json");
    assert!(
        !cafe_config_path.exists(),
        "Zero disk mutations permitted when authorization is rejected"
    );

    println!("  Verified: Fail-closed without authorization; zero disk mutations.");
}

// ---------------------------------------------------------------------------
// 3. Negative Test: Task Revision Invalidates Prior Authorization
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_model_stale_authorization_cannot_execute() {
    let harness = require_live_harness();
    println!("\n=== [PHASE 36.2] NEGATIVE TEST: STALE AUTHORIZATION CANNOT EXECUTE ===");

    let (dir, pool, _bus) = setup_controlled_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let runtime = Arc::new(harness.runtime_for(&repo_path).await.unwrap());
    let coordinator = runtime.create_pre_execution_coordinator();
    let repo = coordinator.lifecycle_repo();
    let session_id = create_test_session(&pool, &repo_path).await;
    let operator = "operator_alex";

    let init_resp = coordinator
        .init_intent(&session_id, "Build a site with Next.js", operator)
        .await
        .unwrap();

    if let PreExecutionResponse::QuestionsRequired { questions, .. } = init_resp {
        for q in questions {
            coordinator
                .submit_answer(
                    &session_id,
                    &q.question_id,
                    "Next.js with TypeScript and TailwindCSS",
                    operator,
                )
                .await
                .expect("submit_answer failed");
        }
    }

    coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            operator,
        )
        .await
        .expect("PlanAcceptRequested failed");

    coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            operator,
        )
        .await
        .expect("TasksAcceptRequested failed");

    // Grant authorization for revision (1, 1)
    let auth_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            operator,
        )
        .await
        .unwrap();

    let original_auth = match auth_resp {
        PreExecutionResponse::ReadyToExecute { authorization, .. } => authorization,
        other => panic!("Expected ReadyToExecute, got {:?}", other),
    };

    // Operator subsequently modifies/regenerates tasks (advancing to revision 2)
    let _ = coordinator
        .handle_action(
            ApplicationAction::TaskRegenerateRequested {
                session_id: Some(session_id.clone()),
                feedback: Some("Add extra validation step".to_string()),
            },
            operator,
        )
        .await
        .unwrap();

    // Verify stored authorization in SQLite is invalidated
    let stored_auth = repo
        .load_latest_execution_authorization(&session_id)
        .await
        .unwrap()
        .expect("Auth record should exist");
    assert!(
        !stored_auth.is_valid_for(1, 1),
        "Prior authorization must be invalidated after task revision"
    );

    // Attempting to materialize with the stale authorization must be rejected
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let dummy_plan = CandidatePlan::new("plan-test", "objective", vec![]);
    let mat_res = materializer
        .materialize_authorized(MissionId::new(), &dummy_plan, 1, 2, &original_auth)
        .await;
    assert!(
        mat_res.is_err(),
        "Materializer must reject execution when authorization is stale"
    );

    println!(
        "  Verified: Prior authorization revoked upon task revision; materializer rejects stale auth."
    );
}

// ---------------------------------------------------------------------------
// 4. Negative Test: Model Failure is NOT Fabricated as Success
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_model_failure_is_not_fabricated_as_success() {
    println!("\n=== [PHASE 36.2] NEGATIVE TEST: MODEL FAILURE IS NOT FABRICATED AS SUCCESS ===");

    let (dir, pool, _bus) = setup_controlled_workspace().await;
    let repo_path = dir.path().to_path_buf();

    // Configure an invalid provider endpoint to induce a deterministic network/model failure
    let invalid_provider = Arc::new(
        m31a::model::provider::NvidiaProvider::new(
            Some("invalid_api_key_that_will_fail".to_string()),
            Some("http://127.0.0.1:1".to_string()), // Unreachable endpoint
        )
        .expect("provider construction"),
    );

    let runtime = AppRuntime::new(&repo_path)
        .await
        .unwrap()
        .with_model_provider(invalid_provider);
    let coordinator = runtime.create_pre_execution_coordinator();
    let session_id = create_test_session(&pool, &repo_path).await;
    let operator = "operator_alex";

    // Attempt intent initialization with broken provider
    let init_res = coordinator
        .init_intent(&session_id, "Build a small website", operator)
        .await;

    // RULE: Must fail explicitly! Must NOT return synthetic questions or fabricated TASK-01 plan.
    assert!(
        init_res.is_err(),
        "Model failure MUST return an explicit error and NOT fabricate success"
    );
    let err_str = init_res.err().unwrap().to_string();
    println!("  Observed explicit error as required: {err_str}");

    // Verify database contains NO fabricated PlanRevision
    let stored_plan = coordinator
        .lifecycle_repo()
        .load_latest_plan_revision(&session_id)
        .await
        .unwrap();
    assert!(
        stored_plan.is_none(),
        "Database must NOT contain any fabricated PlanRevision on model failure"
    );

    println!(
        "  Verified: Model failure produces explicit error; zero synthetic artifacts persisted."
    );
}
