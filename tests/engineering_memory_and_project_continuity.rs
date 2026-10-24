//! Phase 25 — Long-Horizon Engineering Memory & Project State Continuity Integration Tests
//!
//! Comprehensive test suite covering the 12 mandated benchmark scenarios:
//! 1. Task interrupted and resumed
//! 2. Model session replaced
//! 3. Previous failed repair remembered
//! 4. Successful architecture decision reused
//! 5. Assumption confirmed
//! 6. Assumption invalidated by file change
//! 7. Prior verification invalidated after source change
//! 8. Review finding carried across turns and addressed
//! 9. Completed task preserved across replan
//! 10. Unrelated memory excluded from context
//! 11. Project-level decision reused by later mission
//! 12. Conflicting memory resolved by current repository/runtime state

use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::context::compiler::ProductionContextCompiler;
use m31a::ids::{CheckId, MissionId, TaskGraphId, TaskId};
use m31a::kernel::memory::{
    AssumptionStatus, DecisionStatus, EngineeringAssumption, EngineeringDecision,
    FailureDiagnosisRecord, FileHashRecord, MemoryScope, RepairStatus, ReviewFindingRecord,
    ReviewFindingSeverity, ReviewFindingStatus, VerificationRecord, VerificationValidity,
};
use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use m31a::memory::continuity::SessionContinuityEngine;
use m31a::memory::reconciler::MemoryReconciler;
use m31a::memory::repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
use m31a::memory::retrieval::{MemoryRetrievalCriteria, TaskAwareMemoryRetriever};
use m31a::persistence::sqlite::repositories::{
    SqliteMissionRepository, SqliteTaskGraphRepository, SqliteTaskRepository,
};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::state::mission::Mission;
use m31a::state::task::Task;
use m31a::state_machine::TaskState;
use m31a::state_machine::agent::AgentRole;

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite connection");
    run_migrations(&pool).await.expect("migrations succeed");
    pool
}

async fn create_test_mission(pool: &sqlx::SqlitePool, mission_id: MissionId) -> Mission {
    let mission = Mission::new(mission_id, "Test Mission".to_string());
    let repo = SqliteMissionRepository::new(pool.clone());
    repo.insert(&mission).await.unwrap();
    mission
}

async fn create_test_task(pool: &sqlx::SqlitePool, mission_id: MissionId, task_id: TaskId) -> Task {
    let task = Task::new(task_id, mission_id, "Test Task".to_string());
    let repo = SqliteTaskRepository::new(pool.clone());
    repo.insert(&task).await.unwrap();
    task
}

async fn create_test_verification_check(
    pool: &sqlx::SqlitePool,
    mission_id: MissionId,
    task_id: TaskId,
    check_id: CheckId,
) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool,
            inputs_normalized, summary, snapshot_hash, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(1i64)
    .bind("passed")
    .bind("cargo test")
    .bind("[]")
    .bind("test check")
    .bind("snap_1")
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

// =========================================================================
// Scenario 1: Task interrupted and resumed
// =========================================================================
#[tokio::test]
async fn test_scenario_1_task_interrupted_and_resumed() {
    let pool = setup_test_db().await;
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Build authentication subsystem".to_string());
    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());

    mission_repo.insert(&mission).await.unwrap();

    let task1_id = TaskId::new();
    let mut task1 = Task::new(task1_id, mission_id, "Setup database schema".to_string());
    task1.status = TaskState::Succeeded;
    task_repo.insert(&task1).await.unwrap();

    let task2_id = TaskId::new();
    let mut task2 = Task::new(
        task2_id,
        mission_id,
        "Implement password hashing".to_string(),
    );
    task2.status = TaskState::Running; // Interrupted mid-execution
    task_repo.insert(&task2).await.unwrap();

    let task3_id = TaskId::new();
    let task3 = Task::new(task3_id, mission_id, "Add JWT token issuance".to_string());
    task_repo.insert(&task3).await.unwrap();

    // Add active decision and active assumption
    let dec = EngineeringDecision::new(
        "dec-hash-01",
        MemoryScope::Mission,
        "Password Hashing Algorithm",
        "Need secure password storage",
        "Selected Argon2id for password hashing",
        "Argon2id provides memory-hard security against GPU cracking",
        "system_architect",
    )
    .with_mission_id(mission_id)
    .with_alternatives(vec!["bcrypt".to_string(), "scrypt".to_string()])
    .with_status(DecisionStatus::Accepted);
    memory_repo.save_decision(&dec).await.unwrap();

    let assump =
        EngineeringAssumption::new(mission_id, "Salt length is 16 bytes", MemoryScope::Mission);
    memory_repo.save_assumption(&assump).await.unwrap();

    // Simulate process crash and recovery via SessionContinuityEngine
    let continuity_engine = SessionContinuityEngine::new(pool.clone());
    let restored = continuity_engine
        .restore_session(mission_id)
        .await
        .expect("session restoration succeeds");

    assert_eq!(restored.mission.id, mission_id);
    assert!(restored.active_task.is_some());
    assert_eq!(restored.active_task.unwrap().id, task2_id);
    assert_eq!(restored.completed_tasks.len(), 1);
    assert_eq!(restored.completed_tasks[0].id, task1_id);
    assert_eq!(restored.pending_tasks.len(), 1);
    assert_eq!(restored.pending_tasks[0].id, task3_id);
    assert_eq!(restored.authoritative_decisions.len(), 1);
    assert_eq!(
        restored.authoritative_decisions[0].title,
        "Password Hashing Algorithm"
    );
    assert_eq!(restored.active_assumptions.len(), 1);
    assert_eq!(
        restored.active_assumptions[0].statement,
        "Salt length is 16 bytes"
    );
}

// =========================================================================
// Scenario 2: Model session replaced (Fresh Context & XML Trust Envelopes)
// =========================================================================
#[tokio::test]
async fn test_scenario_2_model_session_replaced_fresh_context() {
    let pool = setup_test_db().await;
    let memory_repo = Arc::new(SqliteEngineeringMemoryRepository::new(pool.clone()));
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    create_test_mission(&pool, mission_id).await;
    create_test_task(&pool, mission_id, task_id).await;

    // Persist engineering knowledge
    let dec = EngineeringDecision::new(
        "dec-cookie-01",
        MemoryScope::Mission,
        "Token Storage Format",
        "Prevent credential interception",
        "Store tokens in HTTP-only cookies",
        "Prevents XSS extraction of session tokens",
        "security_architect",
    )
    .with_mission_id(mission_id)
    .with_status(DecisionStatus::Accepted)
    .with_linked_files(vec!["src/auth/cookie.rs".to_string()]);
    memory_repo.save_decision(&dec).await.unwrap();

    let assump = EngineeringAssumption::new(
        mission_id,
        "Cookie domain defaults to host",
        MemoryScope::Mission,
    )
    .with_task_id(task_id)
    .with_target_file("src/auth/cookie.rs", None);
    memory_repo.save_assumption(&assump).await.unwrap();

    // Brand new compiler without previous conversational history
    let compiler = ProductionContextCompiler::new().with_memory_store(memory_repo);

    let req = ContextCompilationRequest::new(mission_id, task_id, 4096)
        .with_role(AgentRole::implementer())
        .with_mission_objective("Implement auth cookie handling in src/auth/cookie.rs")
        .with_task_objective("Implement secure cookie options in src/auth/cookie.rs");

    let compiled = compiler
        .compile_context(req)
        .await
        .expect("compilation succeeds");

    // Verify P1 contains authoritative decisions and assumptions in XML trust envelopes
    assert!(
        compiled
            .system_prompt
            .contains("Authoritative Architectural Decisions:")
    );
    assert!(compiled.system_prompt.contains("Token Storage Format"));
    assert!(compiled.system_prompt.contains("Engineering Assumptions:"));
    assert!(
        compiled
            .system_prompt
            .contains("Cookie domain defaults to host")
    );
    assert!(
        compiled
            .system_prompt
            .contains("<untrusted_evidence source=\"memory://decision/")
    );
    assert!(
        compiled
            .system_prompt
            .contains("<untrusted_evidence source=\"memory://assumption/")
    );
}

// =========================================================================
// Scenario 3: Previous failed repair remembered
// =========================================================================
#[tokio::test]
async fn test_scenario_3_previous_failed_repair_remembered() {
    let pool = setup_test_db().await;
    let memory_repo = Arc::new(SqliteEngineeringMemoryRepository::new(pool.clone()));
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    create_test_mission(&pool, mission_id).await;
    create_test_task(&pool, mission_id, task_id).await;

    // Record previous failure diagnosis and failed repair
    let diagnosis = FailureDiagnosisRecord::new(
        mission_id,
        task_id,
        "E0308:mismatched_types",
        "compiler_error",
        "mismatched types: expected Result, found Option",
        "rev_snap_1",
        "Mismatched return type in parse_header",
        "Expected Result<Header, Error>, found Option<Header>",
        "Change return type or map None to Error",
    )
    .with_repair_proposal("{\"mutation\": \"replace_return_with_none\"}");

    // Mark as applied so runtime knows it was tried
    let mut diag = diagnosis;
    diag.repair_status = RepairStatus::Applied;
    diag.recurrence_count = 1;
    memory_repo.save_failure_diagnosis(&diag).await.unwrap();

    // Next turn compiler incorporates diagnosis in P2
    let compiler = ProductionContextCompiler::new().with_memory_store(memory_repo);
    let req = ContextCompilationRequest::new(mission_id, task_id, 4096)
        .with_role(AgentRole::diagnostician())
        .with_mission_objective("Fix compiler error in src/http.rs")
        .with_task_objective("Diagnose and repair header parser in src/http.rs")
        .with_error_context("E0308:mismatched_types in src/http.rs:45");

    let compiled = compiler
        .compile_context(req)
        .await
        .expect("compilation succeeds");

    assert!(
        compiled
            .system_prompt
            .contains("Historical Failure Diagnoses & Repair Attempts:")
    );
    assert!(compiled.system_prompt.contains("E0308:mismatched_types"));
    assert!(
        compiled
            .system_prompt
            .contains("Expected Result<Header, Error>")
    );
    assert!(compiled.system_prompt.contains("replace_return_with_none"));
}

// =========================================================================
// Scenario 4: Successful architecture decision reused & supersession
// =========================================================================
#[tokio::test]
async fn test_scenario_4_successful_architecture_decision_reused_and_superseded() {
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    create_test_mission(&pool, mission_id).await;

    let decision_v1 = EngineeringDecision::new(
        "dec-db-01",
        MemoryScope::Mission,
        "Database Engine Selection",
        "Evaluate persistence storage",
        "Use SQLite embedded for local execution",
        "Eliminates external database daemon dependencies",
        "architect",
    )
    .with_mission_id(mission_id)
    .with_linked_files(vec!["src/storage/db.rs".to_string()])
    .with_status(DecisionStatus::Accepted);

    memory_repo.save_decision(&decision_v1).await.unwrap();

    // Retrieve via TaskAwareMemoryRetriever
    let retriever = TaskAwareMemoryRetriever::new(&memory_repo);
    let target_files = vec!["src/storage/db.rs".to_string()];
    let criteria = MemoryRetrievalCriteria {
        mission_id,
        task_id: None,
        task_objective: "Update storage module in src/storage/db.rs",
        target_files: &target_files,
        target_symbols: &[],
        requirement_keys: &[],
        error_context: None,
    };

    let snapshot = retriever.retrieve_snapshot(&criteria).await.unwrap();
    assert_eq!(snapshot.active_decisions.len(), 1);
    assert_eq!(
        snapshot.active_decisions[0].title,
        "Database Engine Selection"
    );

    // Supersede decision with v2
    let decision_v2 = EngineeringDecision::new(
        "dec-db-02",
        MemoryScope::Mission,
        "Database Engine Selection v2",
        "Concurrent reader requirements",
        "Use SQLite with WAL mode and busy_timeout",
        "Enables concurrent reader access and prevents locked errors",
        "architect",
    )
    .with_mission_id(mission_id)
    .with_linked_files(vec!["src/storage/db.rs".to_string()])
    .with_status(DecisionStatus::Accepted);

    memory_repo
        .supersede_decision(&decision_v1.id, &decision_v2)
        .await
        .unwrap();

    // Old decision is superseded
    let old_dec = memory_repo
        .get_decision(&decision_v1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_dec.status, DecisionStatus::Superseded);
    assert_eq!(old_dec.superseded_by, Some(decision_v2.id));

    // Retriever now yields only the active decision v2
    let snapshot2 = retriever.retrieve_snapshot(&criteria).await.unwrap();
    assert_eq!(snapshot2.active_decisions.len(), 1);
    assert_eq!(
        snapshot2.active_decisions[0].title,
        "Database Engine Selection v2"
    );
}

// =========================================================================
// Scenario 5: Assumption confirmed through verification
// =========================================================================
#[tokio::test]
async fn test_scenario_5_assumption_confirmed() {
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    create_test_mission(&pool, mission_id).await;

    let assumption = EngineeringAssumption::new(
        mission_id,
        "Config parser supports YAML and JSON",
        MemoryScope::Mission,
    );
    assert_eq!(assumption.status, AssumptionStatus::Active);
    memory_repo.save_assumption(&assumption).await.unwrap();

    // Verify and confirm assumption
    memory_repo
        .update_assumption_status(
            assumption.id,
            AssumptionStatus::Confirmed,
            Some("Verified by running serde tests on YAML/JSON fixtures"),
            None,
        )
        .await
        .unwrap();

    let fetched = memory_repo
        .get_assumption(assumption.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.status, AssumptionStatus::Confirmed);
    assert!(fetched.status.is_valid());
    assert_eq!(
        fetched.evidence_summary.as_deref(),
        Some("Verified by running serde tests on YAML/JSON fixtures")
    );
}

// =========================================================================
// Scenario 6: Assumption invalidated by file change (Reconciliation)
// =========================================================================
#[tokio::test]
async fn test_scenario_6_assumption_invalidated_by_drift() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let src_dir = ws.join("src");
    tokio::fs::create_dir_all(&src_dir).await.unwrap();

    let target_file = src_dir.join("config.rs");
    let original_content = "pub const MAX_RETRIES: u32 = 3;\n";
    tokio::fs::write(&target_file, original_content)
        .await
        .unwrap();

    let mut hasher = Sha256::new();
    hasher.update(original_content.as_bytes());
    let original_hash = format!("{:x}", hasher.finalize());

    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    create_test_mission(&pool, mission_id).await;

    let assumption =
        EngineeringAssumption::new(mission_id, "MAX_RETRIES is set to 3", MemoryScope::Mission)
            .with_target_file("src/config.rs", Some(original_hash));

    memory_repo.save_assumption(&assumption).await.unwrap();

    // Reconcile before mutation: should remain active
    let reconciler = MemoryReconciler::new(&memory_repo, ws);
    let outcome1 = reconciler.reconcile_mission(mission_id).await.unwrap();
    assert_eq!(outcome1.invalidated_assumptions.len(), 0);

    // Modify file on disk to invalidate the assumption
    tokio::fs::write(&target_file, "pub const MAX_RETRIES: u32 = 10;\n")
        .await
        .unwrap();

    let outcome2 = reconciler.reconcile_mission(mission_id).await.unwrap();
    assert_eq!(outcome2.invalidated_assumptions.len(), 1);

    let updated_assumption = memory_repo
        .get_assumption(assumption.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_assumption.status, AssumptionStatus::Invalidated);
    assert!(
        updated_assumption
            .invalidated_by
            .as_deref()
            .unwrap()
            .contains("modified")
    );
}

// =========================================================================
// Scenario 7: Prior verification invalidated after source change
// =========================================================================
#[tokio::test]
async fn test_scenario_7_prior_verification_invalidated_after_source_change() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let src_dir = ws.join("src");
    tokio::fs::create_dir_all(&src_dir).await.unwrap();

    let auth_file = src_dir.join("auth.rs");
    let auth_content = "pub fn check_auth() -> bool { true }\n";
    tokio::fs::write(&auth_file, auth_content).await.unwrap();

    let mut hasher = Sha256::new();
    hasher.update(auth_content.as_bytes());
    let file_hash = format!("{:x}", hasher.finalize());

    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let check_id = CheckId::new();

    create_test_mission(&pool, mission_id).await;
    create_test_task(&pool, mission_id, task_id).await;
    create_test_verification_check(&pool, mission_id, task_id, check_id).await;

    let record = VerificationRecord::new(
        mission_id,
        task_id,
        "REQ-SEC-AUTH",
        check_id,
        "snapshot_abc",
        true,
    )
    .with_files(vec![FileHashRecord {
        path: "src/auth.rs".to_string(),
        hash: file_hash,
    }]);

    memory_repo.save_verification_record(&record).await.unwrap();

    // Verify initially authoritative
    let initial = memory_repo
        .get_verification_record(record.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(initial.validity_status, VerificationValidity::Valid);

    // Mutate src/auth.rs
    tokio::fs::write(&auth_file, "pub fn check_auth() -> bool { false }\n")
        .await
        .unwrap();

    // Reconcile
    let reconciler = MemoryReconciler::new(&memory_repo, ws);
    let outcome = reconciler.reconcile_mission(mission_id).await.unwrap();
    assert_eq!(outcome.stale_verifications.len(), 1);

    let invalidated = memory_repo
        .get_verification_record(record.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        invalidated.validity_status,
        VerificationValidity::StaleDueToDrift
    );
    assert!(invalidated.invalidated_at.is_some());
}

// =========================================================================
// Scenario 8: Review finding carried across turns and addressed
// =========================================================================
#[tokio::test]
async fn test_scenario_8_review_finding_lifecycle() {
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    create_test_mission(&pool, mission_id).await;
    create_test_task(&pool, mission_id, task_id).await;

    // Reviewer records finding
    let finding = ReviewFindingRecord::new(
        mission_id,
        task_id,
        "src/api/auth.rs",
        ReviewFindingSeverity::Error,
        "Missing CSRF token validation",
        "Inject CSRF protection middleware",
    )
    .with_lines(15, 30);

    memory_repo.save_review_finding(&finding).await.unwrap();

    // Finding is Open
    let open_findings = memory_repo
        .list_review_findings(mission_id, Some(task_id), Some(ReviewFindingStatus::Open))
        .await
        .unwrap();
    assert_eq!(open_findings.len(), 1);
    assert_eq!(
        open_findings[0].description,
        "Missing CSRF token validation"
    );

    // Agent addresses the finding
    memory_repo
        .update_review_finding_status(
            finding.id,
            ReviewFindingStatus::Addressed,
            Some("Added CsrfProtectionLayer middleware to router"),
            Some("agent_implementer"),
        )
        .await
        .unwrap();

    let resolved = memory_repo
        .get_review_finding(finding.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.status, ReviewFindingStatus::Addressed);
    assert!(resolved.status.is_resolved());
    assert_eq!(resolved.resolved_by.as_deref(), Some("agent_implementer"));
}

// =========================================================================
// Scenario 9: Completed task preserved across replan
// =========================================================================
#[tokio::test]
async fn test_scenario_9_completed_task_preserved_across_replan() {
    let pool = setup_test_db().await;
    let mission_id = MissionId::new();
    let mut mission = Mission::new(mission_id, "Replan demonstration".to_string());
    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());
    let _graph_repo = SqliteTaskGraphRepository::new(pool.clone());

    // Initial plan revision 1 with Task A (Succeeded) and Task B (Failed)
    let graph_id_v1 = TaskGraphId::new();
    mission.task_graph_id = Some(graph_id_v1);
    mission_repo.insert(&mission).await.unwrap();

    let task_a_id = TaskId::new();
    let mut task_a = Task::new(task_a_id, mission_id, "Task A: Setup repo".to_string());
    task_a.status = TaskState::Succeeded;
    task_repo.insert(&task_a).await.unwrap();

    let task_b_id = TaskId::new();
    let mut task_b = Task::new(task_b_id, mission_id, "Task B: Flawed approach".to_string());
    task_b.status = TaskState::Failed;
    task_repo.insert(&task_b).await.unwrap();

    // Replan: Plan Revision 2 introduced, Task C added as Pending
    let task_c_id = TaskId::new();
    let task_c = Task::new(
        task_c_id,
        mission_id,
        "Task C: Alternative approach".to_string(),
    );
    task_repo.insert(&task_c).await.unwrap();

    // Check continuity restoration
    let continuity = SessionContinuityEngine::new(pool.clone());
    let state = continuity.restore_session(mission_id).await.unwrap();

    // Task A remains succeeded and preserved
    assert_eq!(state.completed_tasks.len(), 1);
    assert_eq!(state.completed_tasks[0].id, task_a_id);
    // Task B is recorded as failed
    assert_eq!(state.failed_tasks.len(), 1);
    assert_eq!(state.failed_tasks[0].id, task_b_id);
    // Task C is ready as pending
    assert_eq!(state.pending_tasks.len(), 1);
    assert_eq!(state.pending_tasks[0].id, task_c_id);
}

// =========================================================================
// Scenario 10: Unrelated memory excluded from context
// =========================================================================
#[tokio::test]
async fn test_scenario_10_unrelated_memory_excluded_from_context() {
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    create_test_mission(&pool, mission_id).await;

    // Decision A for kernel
    let dec_a = EngineeringDecision::new(
        "dec-kernel-01",
        MemoryScope::Mission,
        "Kernel Architecture",
        "Layer design review",
        "Use pure domain models",
        "Enforces L0 invariants",
        "architect",
    )
    .with_mission_id(mission_id)
    .with_linked_files(vec!["src/kernel/mod.rs".to_string()])
    .with_status(DecisionStatus::Accepted);
    memory_repo.save_decision(&dec_a).await.unwrap();

    // Decision B for TUI
    let dec_b = EngineeringDecision::new(
        "dec-tui-01",
        MemoryScope::Mission,
        "TUI Color Palette",
        "Terminal display options",
        "Use ANSI 16 colors for maximum terminal compatibility",
        "Avoids rendering glitches on basic terminals",
        "ui_designer",
    )
    .with_mission_id(mission_id)
    .with_linked_files(vec!["src/tui/render.rs".to_string()])
    .with_status(DecisionStatus::Accepted);
    memory_repo.save_decision(&dec_b).await.unwrap();

    // Query for kernel work
    let retriever = TaskAwareMemoryRetriever::new(&memory_repo);
    let kernel_files = vec!["src/kernel/mod.rs".to_string()];
    let criteria = MemoryRetrievalCriteria {
        mission_id,
        task_id: None,
        task_objective: "Refactor kernel types in src/kernel/mod.rs",
        target_files: &kernel_files,
        target_symbols: &[],
        requirement_keys: &[],
        error_context: None,
    };

    let snapshot = retriever.retrieve_snapshot(&criteria).await.unwrap();
    assert_eq!(snapshot.active_decisions.len(), 1);
    assert_eq!(snapshot.active_decisions[0].title, "Kernel Architecture");
    // Ensure TUI decision was excluded
    assert!(
        !snapshot
            .active_decisions
            .iter()
            .any(|d| d.title.contains("TUI"))
    );
}

// =========================================================================
// Scenario 11: Project-level decision reused by later mission
// =========================================================================
#[tokio::test]
async fn test_scenario_11_project_level_decision_reused_by_later_mission() {
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_1_id = MissionId::new();
    let mission_2_id = MissionId::new();

    create_test_mission(&pool, mission_1_id).await;
    create_test_mission(&pool, mission_2_id).await;

    // Project-scoped decision recorded in Mission 1
    let project_dec = EngineeringDecision::new(
        "dec-proj-01",
        MemoryScope::Project,
        "Single Crate Architecture Invariant",
        "Project-wide crate boundary constraint",
        "All subsystems must reside in a single crate using Rust module hierarchy",
        "PRD Section 75 non-negotiable rule",
        "core_team",
    )
    .with_mission_id(mission_1_id)
    .with_status(DecisionStatus::Accepted);
    memory_repo.save_decision(&project_dec).await.unwrap();

    // Mission 2 starts and retrieves decisions
    let retriever = TaskAwareMemoryRetriever::new(&memory_repo);
    let criteria = MemoryRetrievalCriteria {
        mission_id: mission_2_id,
        task_id: None,
        task_objective: "Add new feature in mission 2",
        target_files: &[],
        target_symbols: &[],
        requirement_keys: &[],
        error_context: None,
    };

    let snapshot = retriever.retrieve_snapshot(&criteria).await.unwrap();
    assert_eq!(snapshot.active_decisions.len(), 1);
    assert_eq!(
        snapshot.active_decisions[0].title,
        "Single Crate Architecture Invariant"
    );
}

// =========================================================================
// Scenario 12: Conflicting memory resolved by current repository/runtime state
// =========================================================================
#[tokio::test]
async fn test_scenario_12_conflicting_memory_resolved_by_repo_reality() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    // Memory contains assumption about a file that does not exist on disk
    let pool = setup_test_db().await;
    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    create_test_mission(&pool, mission_id).await;

    let assumption = EngineeringAssumption::new(
        mission_id,
        "Legacy file exists in codebase",
        MemoryScope::Mission,
    )
    .with_target_file("src/legacy_file.rs", Some("deadbeef1234".to_string()));

    memory_repo.save_assumption(&assumption).await.unwrap();

    // Run reconciliation against real workspace where file does NOT exist
    let reconciler = MemoryReconciler::new(&memory_repo, ws);
    let outcome = reconciler.reconcile_mission(mission_id).await.unwrap();
    assert_eq!(outcome.invalidated_assumptions.len(), 1);

    let fetched = memory_repo
        .get_assumption(assumption.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.status, AssumptionStatus::Invalidated);
    assert!(
        fetched
            .invalidated_by
            .as_deref()
            .unwrap()
            .contains("no longer exists")
    );
}
