//! Phase 10 Integration Tests: 7-Tier Verification Hierarchy, Reviewer Isolation & Completion Gate (VER-01–VER-05).

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

use m31a::agent::envelope::CapabilityEnvelope;
use m31a::ids::{AgentId, ArtifactId, CheckId, MissionId, RequirementId, TaskId};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::verification::EvidenceCompletionGate;
use m31a::verification::hierarchy::VerificationHierarchyEngine;
use m31a::verification::reviewer::{
    IndependentReviewer, ReviewDecision, ReviewFinding, ReviewFindingSeverity, ReviewVerdict,
    ReviewerAgentContext,
};
use m31a::verification::runners::VerificationRunner;
use m31a::verification::types::{CheckStatus, CheckTier, VerificationCheck};

// ============================================================================
// Mock Runner Helper with Call Counter
// ============================================================================

struct CountingRunner {
    pub call_count: Arc<AtomicUsize>,
    pub check_to_return: VerificationCheck,
}

impl CountingRunner {
    pub fn new(call_count: Arc<AtomicUsize>, check_to_return: VerificationCheck) -> Self {
        Self {
            call_count,
            check_to_return,
        }
    }
}

#[async_trait]
impl VerificationRunner for CountingRunner {
    async fn execute(
        &self,
        _mission_id: MissionId,
        _task_id: TaskId,
        _workspace_root: &Path,
        _snapshot_hash: &str,
    ) -> Result<VerificationCheck, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.check_to_return.clone())
    }
}

// ============================================================================
// Database Seeder
// ============================================================================

async fn seed_test_mission_and_task(pool: &sqlx::SqlitePool) -> (MissionId, TaskId) {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Verification hierarchy test mission")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Test task for verification hierarchy")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("worker")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    (mission_id, task_id)
}

// ============================================================================
// Test 1: test_verification_hierarchy_gating (Task 10-01-01)
// ============================================================================

#[tokio::test]
async fn test_verification_hierarchy_gating() {
    let dir = tempdir().unwrap();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let snapshot_hash = "test-snap-01";

    let t3_calls = Arc::new(AtomicUsize::new(0));
    let t6_calls = Arc::new(AtomicUsize::new(0));

    // Case 1: Tier 2 (Compiler) Fails -> Tier 3, Tier 4, and Tier 6 MUST be BLOCKED without executing commands
    let t1_pass = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::Deterministic,
        "det_check",
        "",
        None,
        "T1 deterministic passed",
        snapshot_hash,
    );
    let t2_fail = VerificationCheck::failed(
        mission_id,
        task_id,
        CheckTier::Compiler,
        "cargo check",
        "",
        None,
        "Compilation failed with error[E0308]: mismatched types",
        Some("Compilation".to_string()),
        snapshot_hash,
    );
    let t3_placeholder = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::Tests,
        "cargo test",
        "",
        None,
        "Tests passed",
        snapshot_hash,
    );
    let t4_placeholder = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::StaticAnalysis,
        "clippy",
        "",
        None,
        "Clippy passed",
        snapshot_hash,
    );
    let t5_placeholder = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::DiffInvariants,
        "drift",
        "",
        None,
        "Drift passed",
        snapshot_hash,
    );
    let t6_placeholder = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::IndependentReview,
        "reviewer",
        "",
        None,
        "Review approved",
        snapshot_hash,
    );

    let engine = VerificationHierarchyEngine::new(
        Arc::new(CountingRunner::new(
            Arc::new(AtomicUsize::new(0)),
            t1_pass.clone(),
        )),
        Arc::new(CountingRunner::new(Arc::new(AtomicUsize::new(0)), t2_fail)),
        Arc::new(CountingRunner::new(
            t3_calls.clone(),
            t3_placeholder.clone(),
        )),
        Arc::new(CountingRunner::new(
            Arc::new(AtomicUsize::new(0)),
            t4_placeholder.clone(),
        )),
        Arc::new(CountingRunner::new(
            Arc::new(AtomicUsize::new(0)),
            t5_placeholder.clone(),
        )),
        Some(Arc::new(CountingRunner::new(
            t6_calls.clone(),
            t6_placeholder.clone(),
        ))),
    );

    let checks = engine
        .execute_hierarchy(mission_id, task_id, dir.path(), snapshot_hash, true)
        .await;

    // Verify results
    let t1_result = checks
        .iter()
        .find(|c| c.tier == CheckTier::Deterministic)
        .unwrap();
    assert_eq!(t1_result.status, CheckStatus::Passed);

    let t2_result = checks
        .iter()
        .find(|c| c.tier == CheckTier::Compiler)
        .unwrap();
    assert_eq!(t2_result.status, CheckStatus::Failed);

    let t3_result = checks.iter().find(|c| c.tier == CheckTier::Tests).unwrap();
    assert_eq!(t3_result.status, CheckStatus::Blocked);
    assert!(t3_result.summary.contains("Compiler diagnostics failed"));

    let t4_result = checks
        .iter()
        .find(|c| c.tier == CheckTier::StaticAnalysis)
        .unwrap();
    assert_eq!(t4_result.status, CheckStatus::Blocked);

    let t6_result = checks
        .iter()
        .find(|c| c.tier == CheckTier::IndependentReview)
        .unwrap();
    assert_eq!(t6_result.status, CheckStatus::Blocked);

    // CRITICAL: Verify downstream test commands and review model calls were NEVER made (0 calls)
    assert_eq!(
        t3_calls.load(Ordering::SeqCst),
        0,
        "Tier 3 test runner must not execute on compiler failure"
    );
    assert_eq!(
        t6_calls.load(Ordering::SeqCst),
        0,
        "Tier 6 reviewer must not execute on compiler failure"
    );

    // Case 2: Tier 1 and Tier 2 both Pass -> Tier 3 and Tier 4 execute and report empirical results
    let t2_pass = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::Compiler,
        "cargo check",
        "",
        None,
        "Compilation clean",
        snapshot_hash,
    );

    let engine_passing = VerificationHierarchyEngine::new(
        Arc::new(CountingRunner::new(Arc::new(AtomicUsize::new(0)), t1_pass)),
        Arc::new(CountingRunner::new(Arc::new(AtomicUsize::new(0)), t2_pass)),
        Arc::new(CountingRunner::new(t3_calls.clone(), t3_placeholder)),
        Arc::new(CountingRunner::new(
            Arc::new(AtomicUsize::new(0)),
            t4_placeholder,
        )),
        Arc::new(CountingRunner::new(
            Arc::new(AtomicUsize::new(0)),
            t5_placeholder,
        )),
        Some(Arc::new(CountingRunner::new(
            t6_calls.clone(),
            t6_placeholder,
        ))),
    );

    let passing_checks = engine_passing
        .execute_hierarchy(mission_id, task_id, dir.path(), snapshot_hash, true)
        .await;

    let t3_pass_result = passing_checks
        .iter()
        .find(|c| c.tier == CheckTier::Tests)
        .unwrap();
    assert_eq!(t3_pass_result.status, CheckStatus::Passed);
    assert_eq!(
        t3_calls.load(Ordering::SeqCst),
        1,
        "Tier 3 test runner must execute when compiler succeeds"
    );
    assert_eq!(
        t6_calls.load(Ordering::SeqCst),
        1,
        "Tier 6 reviewer must execute when tests succeed"
    );
}

// ============================================================================
// Test 2: test_reviewer_read_only_isolation (Task 10-01-02)
// ============================================================================

#[tokio::test]
async fn test_reviewer_read_only_isolation() {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let snapshot_hash = "snap-iso-01";

    // 1. Verify capability envelope is strictly read-only (D-03)
    let envelope: CapabilityEnvelope = IndependentReviewer::capability_envelope();
    assert!(
        !envelope.allow_file_write,
        "Reviewer must not be granted file write permissions"
    );
    assert!(
        !envelope.allow_shell_execution,
        "Reviewer must not be granted shell execution permissions"
    );
    assert!(
        !envelope.allow_network_access,
        "Reviewer must not be granted network access"
    );
    assert!(envelope.allowed_capabilities.contains("repo.read"));
    assert!(envelope.allowed_capabilities.contains("artifacts.read"));
    assert!(envelope.allowed_capabilities.contains("review.submit"));

    // 2. Verify fresh context compilation excludes conversation transcripts
    let reviewer_ctx = ReviewerAgentContext::new(
        "Implement Authentication Gateway",
        "Enforce token verification and rate limiting",
        vec![
            "Reject expired tokens".to_string(),
            "Block excessive requests".to_string(),
        ],
        snapshot_hash,
        "+ pub fn verify_token() -> bool { true }",
        vec![
            "Tier 1 Deterministic: Passed".to_string(),
            "Tier 2 Compiler: Passed".to_string(),
        ],
        Some("cargo test output: 5 passed; 0 failed".to_string()),
    );

    assert!(reviewer_ctx.is_fresh_context());
    let isolated_catalog = m31a::prompt::InMemoryPromptCatalog::with_builtins();
    let isolated_compiler = m31a::prompt::DefaultPromptCompiler::new();
    let prompt = reviewer_ctx.compile_user_prompt(&isolated_catalog, &isolated_compiler);
    assert!(prompt.contains("Implement Authentication Gateway"));
    assert!(prompt.contains("Reject expired tokens"));
    assert!(prompt.contains("+ pub fn verify_token()"));
    assert!(!prompt.contains("User:"));
    assert!(!prompt.contains("Assistant:"));
    assert!(!prompt.contains("conversation_history"));

    // 3. Verify typed ReviewVerdict maps to VerificationCheck
    let verdict = ReviewVerdict {
        review_id: "rev-test-01".to_string(),
        mission_id,
        task_id,
        snapshot_hash: snapshot_hash.to_string(),
        decision: ReviewDecision::Approved,
        findings: vec![ReviewFinding {
            file_path: "src/auth.rs".to_string(),
            line_range: Some((10, 25)),
            severity: ReviewFindingSeverity::Info,
            description: "Token validation conforms to specification".to_string(),
            recommendation: "None required".to_string(),
        }],
        confidence_score: 95,
        rationale: "All acceptance criteria verified with no security regressions".to_string(),
        requirement_coverage: vec!["AUTH-01".to_string(), "AUTH-02".to_string()],
    };

    let check = IndependentReviewer::map_verdict_to_check(&verdict);
    assert_eq!(check.tier, CheckTier::IndependentReview);
    assert_eq!(check.status, CheckStatus::Passed);
    assert!(check.summary.contains("Approved"));
    assert!(check.summary.contains("confidence: 95%"));

    // 4. Verify negative code review decision maps to CheckStatus::Failed
    let mut rejected_verdict = verdict.clone();
    rejected_verdict.decision = ReviewDecision::ChangesRequested;
    let rejected_check = IndependentReviewer::map_verdict_to_check(&rejected_verdict);
    assert_eq!(rejected_check.tier, CheckTier::IndependentReview);
    assert_eq!(rejected_check.status, CheckStatus::Failed);
    assert_eq!(
        rejected_check.failure_class,
        Some("ReviewChangesRequested".to_string())
    );
}

// ============================================================================
// Test 3: test_completion_gate_evidence_enforcement (Task 10-01-03)
// ============================================================================

#[tokio::test]
async fn test_completion_gate_evidence_enforcement() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_gate.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let store_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&store_dir));

    let (mission_id, task_id) = seed_test_mission_and_task(&pool).await;
    let gate = EvidenceCompletionGate::new(pool.clone(), artifact_store.clone(), dir.path());

    let current_snapshot = "snap-clean-01";

    // Scenario 1: Submits completion proposal with NO verification records (Law 6 Violation)
    let decision1 = gate
        .evaluate_task_completion(mission_id, task_id, current_snapshot)
        .await
        .unwrap();

    assert!(
        !decision1.is_satisfied,
        "Gate must reject proposal without empirical verification records"
    );
    assert!(
        decision1
            .violations
            .iter()
            .any(|v| v.contains("Law 6 violation")),
        "Expected Law 6 violation on missing verification records"
    );

    // Scenario 2: Check has a stale / mismatched snapshot_hash
    let stale_check_id = CheckId::new();
    let now = chrono::Utc::now().to_rfc3339();
    let null_blob: Option<&[u8]> = None;
    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool,
            inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(stale_check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(2) // Compiler tier
    .bind("passed")
    .bind("cargo check")
    .bind("")
    .bind(null_blob)
    .bind("Compilation succeeded")
    .bind("stale-hash-old") // Stale snapshot!
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let decision2 = gate
        .evaluate_task_completion(mission_id, task_id, current_snapshot)
        .await
        .unwrap();

    // Phase 29: stale rows (superseded attempts) are ignored for pass/fail,
    // never re-litigated — otherwise recovery could never succeed through
    // re-verification. A stale passed check alone does NOT satisfy the gate
    // (current evidence is still required), but it must not produce
    // mismatch violations either.
    assert!(
        !decision2.is_satisfied,
        "Gate must not satisfy on stale evidence alone"
    );
    assert!(
        decision2
            .violations
            .iter()
            .any(|v| v.contains("No current verification checks recorded")),
        "Gate must demand current evidence, got: {:?}",
        decision2.violations
    );
    assert!(
        !decision2
            .violations
            .iter()
            .any(|v| v.contains("Snapshot hash mismatch")),
        "Stale rows must be ignored, not re-litigated, got: {:?}",
        decision2.violations
    );

    // Scenario 3: Check references a non-existent artifact
    let missing_artifact_id = ArtifactId::new();
    let missing_art_check_id = CheckId::new();
    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool,
            inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(missing_art_check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(3) // Tests tier
    .bind("passed")
    .bind("cargo test")
    .bind("")
    .bind(missing_artifact_id.as_bytes().as_slice()) // Non-existent artifact!
    .bind("Tests passed")
    .bind(current_snapshot)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let decision3 = gate
        .evaluate_task_completion(mission_id, task_id, current_snapshot)
        .await
        .unwrap();

    assert!(
        !decision3.is_satisfied,
        "Gate must reject checks referencing missing artifacts"
    );
    assert!(
        decision3
            .violations
            .iter()
            .any(|v| v.contains("does not exist in artifact store")),
        "Expected missing artifact violation"
    );

    // Scenario 4: Clean state with valid artifact, valid snapshot, and mandatory requirement coverage
    // Store valid artifact in FsArtifactStore
    let valid_artifact_id = ArtifactId::new();
    let sample_log =
        b"running 3 tests\ntest a ... ok\ntest b ... ok\ntest c ... ok\ntest result: ok. 3 passed";
    artifact_store
        .store(valid_artifact_id, sample_log, "log")
        .await
        .unwrap();

    // Clean up previous test checks for task
    sqlx::query("DELETE FROM verification_checks WHERE task_id = ?")
        .bind(task_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let valid_check_id = CheckId::new();
    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool,
            inputs_normalized, evidence_artifact_id, summary, failure_class, snapshot_hash, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
        "#,
    )
    .bind(valid_check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(3) // Tests tier
    .bind("passed")
    .bind("cargo test")
    .bind("")
    .bind(valid_artifact_id.as_bytes().as_slice())
    .bind("All 3 unit tests passed")
    .bind(current_snapshot)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Bind mandatory requirement coverage
    let req_id = RequirementId::new();
    let cov_id = uuid::Uuid::now_v7();
    sqlx::query(
        r#"
        INSERT INTO requirement_check_coverage (
            id, requirement_id, check_id, coverage_role, is_mandatory, created_at
        )
        VALUES (?, ?, ?, ?, 1, ?)
        "#,
    )
    .bind(cov_id.as_bytes().as_slice())
    .bind(req_id.as_bytes().as_slice())
    .bind(valid_check_id.as_bytes().as_slice())
    .bind("primary")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let decision4 = gate
        .evaluate_task_completion(mission_id, task_id, current_snapshot)
        .await
        .unwrap();

    assert!(
        decision4.is_satisfied,
        "Gate must approve when all requirements, artifacts, and snapshots match"
    );
    assert!(decision4.violations.is_empty());
    assert!(decision4.evidence_summary.contains("Verified 1 check(s)"));

    // Verify all decisions are recorded durably in completion_gate_decisions table
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM completion_gate_decisions WHERE task_id = ?")
            .bind(task_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        count.0, 4,
        "All 4 completion gate evaluations must be persisted in completion_gate_decisions"
    );
}
