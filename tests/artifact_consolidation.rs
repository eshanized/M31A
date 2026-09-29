//! Comprehensive Workstream F Verification Suite: Artifact Lifecycle & Provenance Consolidation
//!
//! Enforces:
//! 1. ONE canonical artifact lifecycle authority (`ArtifactService`).
//! 2. Separation of Concerns:
//!    - Domain: Logical identity + metadata + provenance + lifecycle
//!    - Storage: FsArtifactStore (content-addressed / UUID blob payloads)
//!    - Ledger: SQLite (`artifacts`, `workflow_artifacts`, `checkpoint_artifacts`)
//!    - Projections: Derived markdown / reports (.planning/*, REPORT.md), never authoritative operational truth.
//! 3. Deterministic SHA-256 content hashing.
//! 4. Immutability protection and versioning.
//! 5. Verification evidence vs Artifact distinction.
//! 6. Restart/recovery durability and partial write failure safety.
//! 7. Full production-path execution flow.

use chrono::Utc;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::ids::{ArtifactId, CheckId, MissionId, TaskId, WorkflowRunId, WorkflowStepRunId};
use m31a::persistence::artifacts::{
    ArtifactError, ArtifactProvenance, ArtifactService, ArtifactStatus, ArtifactStore,
    FsArtifactStore, compute_artifact_hash,
};
use m31a::persistence::sqlite::initialize_database;
use m31a::state::task::TaskResult;
use m31a::verification::gate::EvidenceCompletionGate;
use m31a::verification::types::{CheckTier, VerificationCheck};
use m31a::workflow::genesis::project::ProjectCharter;

/// Helper to set up an isolated test fixture with SQLite DB and FsArtifactStore.
async fn setup_test_fixture() -> (
    tempfile::TempDir,
    SqlitePool,
    Arc<FsArtifactStore>,
    Arc<ArtifactService>,
) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".m31a").join("m31a.db");
    let artifacts_dir = dir.path().join(".m31a").join("artifacts");

    tokio::fs::create_dir_all(&artifacts_dir).await.unwrap();

    let pool = initialize_database(&db_path).await.unwrap();
    let store = Arc::new(FsArtifactStore::new(artifacts_dir));
    let service = Arc::new(ArtifactService::new(store.clone(), pool.clone()));

    (dir, pool, store, service)
}

// =============================================================================
// TEST A: Create and Register Artifact
// =============================================================================
#[tokio::test]
async fn test_a_create_register_artifact() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let payload = b"Hello, M31A canonical artifact lifecycle!";
    let provenance = ArtifactProvenance {
        producer_role: Some("test_runner".to_string()),
        ..Default::default()
    };

    let record = service
        .create_and_store("greeting.txt", payload, "txt", provenance)
        .await
        .expect("create_and_store should succeed");

    assert_eq!(record.name, "greeting.txt");
    assert_eq!(record.extension, "txt");
    assert_eq!(record.size_bytes, payload.len() as u64);
    assert_eq!(record.status, ArtifactStatus::Valid);
    assert_eq!(record.version, 1);

    // Retrieve payload bytes
    let retrieved = service
        .retrieve_payload(record.id, "txt")
        .await
        .expect("retrieve_payload should succeed");
    assert_eq!(retrieved, payload);

    // Retrieve metadata
    let meta = service
        .get_metadata(record.id)
        .await
        .unwrap()
        .expect("metadata should exist");
    assert_eq!(meta.id, record.id);
    assert_eq!(meta.content_hash, record.content_hash);
}

// =============================================================================
// TEST B: Deterministic Content Hash
// =============================================================================
#[tokio::test]
async fn test_b_deterministic_content_hash() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let data = b"Deterministic payload bytes for hashing verification.";
    let expected_hash = compute_artifact_hash(data);

    let rec1 = service
        .create_and_store(
            "doc1.txt",
            data,
            "txt",
            ArtifactProvenance::for_mission(MissionId::new()),
        )
        .await
        .unwrap();

    assert_eq!(rec1.content_hash, expected_hash);

    // Moving or reading elsewhere yields exact same hash
    let hash_recalculated = compute_artifact_hash(data);
    assert_eq!(rec1.content_hash, hash_recalculated);
}

// =============================================================================
// TEST C: Duplicate Content Identity
// =============================================================================
#[tokio::test]
async fn test_c_duplicate_content_identity() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let payload = b"Shared invariant binary payload across multiple documents";
    let hash = compute_artifact_hash(payload);

    let rec1 = service
        .create_and_store("first.bin", payload, "bin", ArtifactProvenance::default())
        .await
        .unwrap();

    let rec2 = service
        .create_and_store("second.bin", payload, "bin", ArtifactProvenance::default())
        .await
        .unwrap();

    assert_ne!(rec1.id, rec2.id); // distinct logical artifact identities
    assert_eq!(rec1.content_hash, rec2.content_hash); // identical content hash
    assert_eq!(rec1.content_hash, hash);

    // Lookup by hash finds the latest recorded artifact
    let found = service
        .get_metadata_by_hash(&hash)
        .await
        .unwrap()
        .expect("should find artifact by hash");
    assert_eq!(found.content_hash, hash);
}

// =============================================================================
// TEST D: Provenance Linkage
// =============================================================================
#[tokio::test]
async fn test_d_provenance_linkage() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let parent_id = ArtifactId::new();
    let check_id = CheckId::new();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let wf_id = WorkflowRunId::new();
    let step_id = WorkflowStepRunId::new();

    let provenance = ArtifactProvenance {
        mission_id: Some(mission_id),
        task_id: Some(task_id),
        workflow_run_id: Some(wf_id),
        step_run_id: Some(step_id),
        producer_role: Some("DiscoverySpecialist".to_string()),
        prompt_id: Some("socratic_charter_prompt".to_string()),
        prompt_version: Some(3),
        model: Some("claude-3-7-sonnet".to_string()),
        parent_artifact_ids: vec![parent_id],
        verification_check_ids: vec![check_id],
    };

    let record = service
        .create_and_store(
            "provenance_spec.json",
            b"{\"provenance\": true}",
            "json",
            provenance.clone(),
        )
        .await
        .unwrap();

    let meta = service.get_metadata(record.id).await.unwrap().unwrap();
    assert_eq!(meta.provenance.mission_id, Some(mission_id));
    assert_eq!(meta.provenance.task_id, Some(task_id));
    assert_eq!(meta.provenance.workflow_run_id, Some(wf_id));
    assert_eq!(meta.provenance.step_run_id, Some(step_id));
    assert_eq!(
        meta.provenance.producer_role.as_deref(),
        Some("DiscoverySpecialist")
    );
    assert_eq!(
        meta.provenance.prompt_id.as_deref(),
        Some("socratic_charter_prompt")
    );
    assert_eq!(meta.provenance.prompt_version, Some(3));
    assert_eq!(meta.provenance.model.as_deref(), Some("claude-3-7-sonnet"));
    assert_eq!(meta.provenance.parent_artifact_ids, vec![parent_id]);
}

// =============================================================================
// TEST E: Artifact Validation & Integrity Audit
// =============================================================================
#[tokio::test]
async fn test_e_artifact_validation() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let payload = b"Data that will pass validation";
    let record = service
        .create_and_store("valid.txt", payload, "txt", ArtifactProvenance::default())
        .await
        .unwrap();

    let audit = service
        .verify_integrity(record.id, "txt")
        .await
        .expect("verify_integrity should succeed");

    assert!(audit.is_valid);
    assert!(audit.payload_exists);
    assert!(audit.metadata_exists);
    assert_eq!(audit.expected_hash, record.content_hash);
    assert_eq!(audit.actual_hash, Some(record.content_hash));
    assert!(audit.error_detail.is_none());
}

// =============================================================================
// TEST F: Immutable Artifact Protection
// =============================================================================
#[tokio::test]
async fn test_f_immutable_artifact_protection() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let artifact_id = ArtifactId::new();
    let original_payload = b"Original immutable artifact payload";
    let prov = ArtifactProvenance::default();

    // 1. Initial write
    let rec = service
        .create_and_store_with_id(
            artifact_id,
            "immutable.txt",
            original_payload,
            "txt",
            prov.clone(),
        )
        .await
        .unwrap();
    assert_eq!(rec.id, artifact_id);

    // 2. Idempotent identical write succeeds
    let rec_same = service
        .create_and_store_with_id(
            artifact_id,
            "immutable.txt",
            original_payload,
            "txt",
            prov.clone(),
        )
        .await
        .unwrap();
    assert_eq!(rec_same.id, artifact_id);

    // 3. Mutation attempt with DIFFERENT content must be rejected
    let mutated_payload = b"HACKED / MUTATED PAYLOAD";
    let err = service
        .create_and_store_with_id(
            artifact_id,
            "immutable.txt",
            mutated_payload,
            "txt",
            prov.clone(),
        )
        .await
        .unwrap_err();

    match err {
        ArtifactError::ImmutableViolation(id) => assert_eq!(id, artifact_id),
        other => panic!("expected ImmutableViolation, got {:?}", other),
    }

    // 4. Stored payload remains original
    let current_bytes = service.retrieve_payload(artifact_id, "txt").await.unwrap();
    assert_eq!(current_bytes, original_payload);
}

// =============================================================================
// TEST G: Artifact Versioning
// =============================================================================
#[tokio::test]
async fn test_g_artifact_versioning() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let v1 = service
        .create_and_store(
            "spec.md",
            b"# Version 1 Specification",
            "md",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();
    assert_eq!(v1.version, 1);

    let v2 = service
        .create_version(
            v1.id,
            "spec.md",
            b"# Version 2 Specification (Revised)",
            "md",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();

    assert_eq!(v2.version, 2);
    assert_ne!(v1.id, v2.id);
    assert_ne!(v1.content_hash, v2.content_hash);
    assert!(v2.provenance.parent_artifact_ids.contains(&v1.id));

    // Both versions exist independently in storage
    let v1_bytes = service.retrieve_payload(v1.id, "md").await.unwrap();
    let v2_bytes = service.retrieve_payload(v2.id, "md").await.unwrap();
    assert_eq!(v1_bytes, b"# Version 1 Specification");
    assert_eq!(v2_bytes, b"# Version 2 Specification (Revised)");
}

// =============================================================================
// TEST H: Retrieve After Restart
// =============================================================================
#[tokio::test]
async fn test_h_retrieve_after_restart() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".m31a").join("m31a.db");
    let artifacts_dir = dir.path().join(".m31a").join("artifacts");
    tokio::fs::create_dir_all(&artifacts_dir).await.unwrap();

    let artifact_id = ArtifactId::new();
    let payload = b"Durable payload surviving process termination";

    // Session 1: Create and persist artifact
    {
        let pool = initialize_database(&db_path).await.unwrap();
        let store = Arc::new(FsArtifactStore::new(&artifacts_dir));
        let service = ArtifactService::new(store, pool.clone());

        service
            .create_and_store_with_id(
                artifact_id,
                "restart_test.dat",
                payload,
                "dat",
                ArtifactProvenance::default(),
            )
            .await
            .unwrap();

        pool.close().await;
    }

    // Session 2: Reload after restart
    {
        let pool = initialize_database(&db_path).await.unwrap();
        let store = Arc::new(FsArtifactStore::new(&artifacts_dir));
        let service = ArtifactService::new(store, pool);

        let metadata = service
            .get_metadata(artifact_id)
            .await
            .unwrap()
            .expect("metadata must persist across process restarts");
        assert_eq!(metadata.id, artifact_id);

        let retrieved = service
            .retrieve_payload(artifact_id, "dat")
            .await
            .expect("payload must be retrievable after restart");
        assert_eq!(retrieved, payload);

        let audit = service.verify_integrity(artifact_id, "dat").await.unwrap();
        assert!(audit.is_valid);
    }
}

// =============================================================================
// TEST I: Missing Payload Detection
// =============================================================================
#[tokio::test]
async fn test_i_missing_payload_detection() {
    let (_dir, _pool, store, service) = setup_test_fixture().await;

    let record = service
        .create_and_store(
            "missing.log",
            b"Log contents to be deleted",
            "log",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();

    // Physically delete file from disk to simulate payload loss
    let path = store.path_for(record.id, "log");
    tokio::fs::remove_file(&path).await.unwrap();

    // Verify integrity detects missing payload
    let audit = service.verify_integrity(record.id, "log").await.unwrap();
    assert!(!audit.is_valid);
    assert!(!audit.payload_exists);
    assert!(audit.metadata_exists);
    assert!(audit.error_detail.unwrap().contains("Payload file missing"));

    // Attempt to retrieve payload fails cleanly
    let err = service
        .retrieve_payload(record.id, "log")
        .await
        .unwrap_err();
    match err {
        ArtifactError::PayloadNotFound(id, ext) => {
            assert_eq!(id, record.id);
            assert_eq!(ext, "log");
        }
        other => panic!("expected PayloadNotFound, got {:?}", other),
    }
}

// =============================================================================
// TEST J: Hash Mismatch Detection (Tamper Detection)
// =============================================================================
#[tokio::test]
async fn test_j_hash_mismatch_detection() {
    let (_dir, _pool, store, service) = setup_test_fixture().await;

    let record = service
        .create_and_store(
            "integrity_target.txt",
            b"Untampered genuine content",
            "txt",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();

    // Directly tamper with the payload file on disk
    let path = store.path_for(record.id, "txt");
    tokio::fs::write(&path, b"TAMPERED CORRUPTED CONTENT")
        .await
        .unwrap();

    // Verify integrity detects hash mismatch
    let audit = service.verify_integrity(record.id, "txt").await.unwrap();
    assert!(!audit.is_valid);
    assert!(audit.payload_exists);
    assert!(audit.metadata_exists);
    assert_ne!(audit.expected_hash, audit.actual_hash.clone().unwrap());

    // retrieve_payload must refuse to return corrupted payload
    let err = service
        .retrieve_payload(record.id, "txt")
        .await
        .unwrap_err();
    match err {
        ArtifactError::HashMismatch {
            artifact_id,
            expected,
            actual,
        } => {
            assert_eq!(artifact_id, record.id);
            assert_eq!(expected, record.content_hash);
            assert_eq!(actual, audit.actual_hash.unwrap());
        }
        other => panic!("expected HashMismatch, got {:?}", other),
    }
}

// =============================================================================
// TEST K: Workflow Artifact Integration
// =============================================================================
#[tokio::test]
async fn test_k_workflow_artifact_integration() {
    let (_dir, pool, _store, service) = setup_test_fixture().await;

    let wf_run_id = WorkflowRunId::new();
    let step_run_id = WorkflowStepRunId::new();

    // Ensure parent workflow run and step run exist in SQLite
    let now_str = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO workflow_runs (id, definition_id, workspace_root, status, started_at, updated_at)
        VALUES (?, 'genesis', '/workspace', 'running', ?, ?)
        "#,
    )
    .bind(wf_run_id.as_bytes().as_slice())
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO workflow_step_runs (id, workflow_run_id, step_key, status, started_at)
        VALUES (?, ?, 'discovery', 'running', ?)
        "#,
    )
    .bind(step_run_id.as_bytes().as_slice())
    .bind(wf_run_id.as_bytes().as_slice())
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Create workflow artifact via ArtifactService
    let prov = ArtifactProvenance::for_workflow_step(wf_run_id, step_run_id, "discovery_analyst");
    let rec = service
        .create_and_store("DISCOVERY.md", b"# Discovered Facts", "md", prov)
        .await
        .unwrap();

    // Verify row was inserted into workflow_artifacts table automatically
    let row = sqlx::query(
        "SELECT id, name, content_hash, version, status FROM workflow_artifacts WHERE id = ?",
    )
    .bind(rec.id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();

    use sqlx::Row;
    let name: String = row.get("name");
    let content_hash: String = row.get("content_hash");
    let status_str: String = row.get("status");

    assert_eq!(name, "DISCOVERY.md");
    assert_eq!(content_hash, rec.content_hash);
    assert_eq!(status_str, "valid");

    // List by workflow run
    let list = service.list_by_workflow_run(wf_run_id).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, rec.id);
}

// =============================================================================
// TEST L: Execution Artifact Integration (Task Outputs)
// =============================================================================
#[tokio::test]
async fn test_l_execution_artifact_integration() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let prov = ArtifactProvenance::for_task(mission_id, task_id, "coding_agent");
    let rec = service
        .create_and_store(
            "compiler_output.log",
            b"Compilation finished with 0 errors",
            "log",
            prov,
        )
        .await
        .unwrap();

    // Task execution records output artifact
    let mut task_result = TaskResult::new("Task completed successfully");
    task_result.output_artifacts.push(rec.id);

    assert_eq!(task_result.output_artifacts, vec![rec.id]);

    // Query artifacts by task
    let task_artifacts = service.list_by_task(task_id).await.unwrap();
    assert_eq!(task_artifacts.len(), 1);
    assert_eq!(task_artifacts[0].id, rec.id);
}

// =============================================================================
// TEST M: Verification Evidence Association
// =============================================================================
#[tokio::test]
async fn test_m_verification_evidence_association() {
    let (dir, pool, store, service) = setup_test_fixture().await;

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // Create mission and task in DB for foreign key constraints
    let now_str = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Test mission', 'active', ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Test task', 'pending', ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Create evidence artifact containing test runner output
    let evidence_bytes = b"cargo test output: 562 passed; 0 failed";
    let evidence_art = service
        .create_and_store(
            "test_evidence.log",
            evidence_bytes,
            "log",
            ArtifactProvenance::for_task(mission_id, task_id, "test_runner"),
        )
        .await
        .unwrap();

    // 2. Construct VerificationCheck linking the evidence artifact
    let check = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::Tests,
        "cargo test",
        "all",
        Some(evidence_art.id),
        "All tests passed",
        compute_artifact_hash(evidence_bytes),
    );

    // Save check in DB
    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized,
            evidence_artifact_id, summary, snapshot_hash, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(check.check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(check.tier.as_u8() as i64)
    .bind(check.status.as_str())
    .bind(&check.command_or_tool)
    .bind(&check.inputs_normalized)
    .bind(evidence_art.id.as_bytes().as_slice())
    .bind(&check.summary)
    .bind(&check.snapshot_hash)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // 3. EvidenceCompletionGate confirms evidence artifact presence in store
    let gate = EvidenceCompletionGate::new(pool.clone(), store.clone(), dir.path().to_path_buf());
    let decision = gate
        .evaluate_task_completion(mission_id, task_id, &check.snapshot_hash)
        .await
        .expect("gate evaluation should succeed");
    assert!(
        decision.is_satisfied,
        "VerificationCheck with valid evidence artifact must pass gate evaluation"
    );
}

// =============================================================================
// TEST N: Project Knowledge Projection
// =============================================================================
#[tokio::test]
async fn test_n_project_knowledge_projection() {
    let (dir, _pool, _store, service) = setup_test_fixture().await;

    // Build typed domain charter
    let mut charter = ProjectCharter::new("Project Alpha", "Build a high-performance database");
    charter.boundaries.in_scope.push("Rust crate".to_string());
    charter.boundaries.non_goals.push("NodeJS".to_string());

    let markdown = charter.to_markdown();
    let prov = ArtifactProvenance {
        producer_role: Some("ProjectSynthesizer".to_string()),
        ..Default::default()
    };

    // Store authoritative payload in ArtifactService
    let rec = service
        .create_and_store("PROJECT.md", markdown.as_bytes(), "md", prov)
        .await
        .unwrap();

    // Project to human-readable .planning/ directory
    let projection_path = dir.path().join(".planning").join("PROJECT.md");
    let projected_file = service
        .project(rec.id, "md", &projection_path)
        .await
        .expect("projection must succeed");

    assert_eq!(projected_file, projection_path);
    assert!(projected_file.exists());

    let read_back = tokio::fs::read_to_string(&projected_file).await.unwrap();
    assert_eq!(read_back, markdown);
}

// =============================================================================
// TEST O: Projection Failure Handling
// =============================================================================
#[tokio::test]
async fn test_o_projection_failure_handling() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let rec = service
        .create_and_store(
            "sample.txt",
            b"Safe content",
            "txt",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();

    // Attempt to write projection to an invalid path (e.g. invalid filename or root)
    #[cfg(unix)]
    let invalid_path = PathBuf::from("/dev/null/forbidden_subdir/projection.txt");
    #[cfg(not(unix))]
    let invalid_path = PathBuf::from("Z:\\invalid\\nonexistent\\path.txt");

    let err = service
        .project(rec.id, "txt", &invalid_path)
        .await
        .unwrap_err();
    match err {
        ArtifactError::ProjectionError(msg) => {
            assert!(!msg.is_empty());
        }
        other => panic!("expected ProjectionError, got {:?}", other),
    }

    // Original artifact remains intact and validated
    let audit = service.verify_integrity(rec.id, "txt").await.unwrap();
    assert!(audit.is_valid);
}

// =============================================================================
// TEST P: Incomplete/Partial Artifact Recovery & Staging Safety
// =============================================================================
#[tokio::test]
async fn test_p_incomplete_partial_artifact_recovery() {
    let (_dir, _pool, store, service) = setup_test_fixture().await;

    let artifact_id = ArtifactId::new();
    let tmp_staging_path = store
        .base_dir()
        .join(format!(".tmp_{}_staging.txt", artifact_id));

    // Simulate an interrupted write that left a staging file behind
    tokio::fs::write(&tmp_staging_path, b"Partial incomplete bytes")
        .await
        .unwrap();

    // ArtifactService should NOT recognise the uncommitted temporary file
    let audit = service.verify_integrity(artifact_id, "txt").await.unwrap();
    assert!(!audit.is_valid);
    assert!(!audit.metadata_exists);

    // Clean up temporary file safely
    let _ = tokio::fs::remove_file(tmp_staging_path).await;
}

// =============================================================================
// TEST Q: Multiple Consumers See Same Artifact Identity
// =============================================================================
#[tokio::test]
async fn test_q_multiple_consumers_see_the_same_artifact_identity() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    let payload = b"Universal artifact consumed by verifier, reporter, and TUI";
    let record = service
        .create_and_store(
            "report_payload.json",
            payload,
            "json",
            ArtifactProvenance::default(),
        )
        .await
        .unwrap();

    // Consumer 1: By ArtifactId
    let c1_meta = service.get_metadata(record.id).await.unwrap().unwrap();
    let c1_bytes = service.retrieve_payload(record.id, "json").await.unwrap();

    // Consumer 2: By Content Hash
    let c2_meta = service
        .get_metadata_by_hash(&record.content_hash)
        .await
        .unwrap()
        .unwrap();
    let c2_bytes = service.retrieve_payload(c2_meta.id, "json").await.unwrap();

    // Consumer 3: By Integrity Check
    let c3_audit = service.verify_integrity(record.id, "json").await.unwrap();

    assert_eq!(c1_meta.id, record.id);
    assert_eq!(c2_meta.id, record.id);
    assert_eq!(c1_bytes, payload);
    assert_eq!(c2_bytes, payload);
    assert_eq!(c3_audit.expected_hash, record.content_hash);
    assert_eq!(c3_audit.actual_hash, Some(record.content_hash));
}

// =============================================================================
// TEST R: No Competing Artifact Authority Reachable in Production
// =============================================================================
#[tokio::test]
async fn test_r_no_competing_artifact_authority() {
    let (_dir, _pool, _store, service) = setup_test_fixture().await;

    // Verify capability provider wraps the canonical service and store
    let provider = m31a::capability::providers::fs_artifacts::FsArtifactStoreProvider::from_service(
        service.clone(),
    );

    use m31a::capability::traits::artifacts::ArtifactStoreService;
    let artifact_id = ArtifactId::new().to_string();
    let path = provider
        .store_artifact(&artifact_id, b"Capability provider payload", "txt")
        .await
        .expect("provider store should succeed");

    assert!(path.exists());

    // Both provider and canonical service observe identical payload
    let prov_loaded = provider.load_artifact(&artifact_id, "txt").await.unwrap();
    let parsed_id = ArtifactId::from_str(&artifact_id).unwrap();
    let serv_loaded = service.retrieve_payload(parsed_id, "txt").await.unwrap();

    assert_eq!(prov_loaded, serv_loaded);

    // Metadata is recorded in canonical SQLite ledger
    let meta = service.get_metadata(parsed_id).await.unwrap().unwrap();
    assert_eq!(meta.id, parsed_id);
    assert_eq!(meta.content_hash, compute_artifact_hash(&prov_loaded));
}

// =============================================================================
// PRODUCTION-PATH TEST: Complete Real Artifact Flow
// =============================================================================
#[tokio::test]
async fn test_production_path_artifact_flow() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path().to_path_buf();
    let st_root = ws_root.join(".m31a");
    let db_path = st_root.join("m31a.db");
    let artifacts_dir = st_root.join("artifacts");

    tokio::fs::create_dir_all(&artifacts_dir).await.unwrap();

    let pool = initialize_database(&db_path).await.unwrap();
    let store = Arc::new(FsArtifactStore::new(&artifacts_dir));
    let service = Arc::new(ArtifactService::new(store.clone(), pool.clone()));

    // 1. Step output produced
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let wf_id = WorkflowRunId::new();
    let step_id = WorkflowStepRunId::new();
    let now_str = Utc::now().to_rfc3339();

    // Insert database prerequisites
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Production flow', 'active', ?, ?)")
        .bind(mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Step task', 'pending', ?, ?)")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO workflow_runs (id, definition_id, workspace_root, status, started_at, updated_at) VALUES (?, 'flow', '/ws', 'running', ?, ?)")
        .bind(wf_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO workflow_step_runs (id, workflow_run_id, step_key, status, started_at) VALUES (?, ?, 'compile', 'running', ?)")
        .bind(step_id.as_bytes().as_slice())
        .bind(wf_id.as_bytes().as_slice())
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

    // 2. Real output payload
    let compiler_log = b"rustc compilation: 0 warnings, 0 errors, binary created successfully";

    // 3. Stored via ArtifactService with hash and provenance
    let prov = ArtifactProvenance {
        mission_id: Some(mission_id),
        task_id: Some(task_id),
        workflow_run_id: Some(wf_id),
        step_run_id: Some(step_id),
        producer_role: Some("CompilerStep".to_string()),
        ..Default::default()
    };
    let record = service
        .create_and_store("compile.log", compiler_log, "log", prov)
        .await
        .unwrap();

    // 4. Verification evidence association
    let check = VerificationCheck::passed(
        mission_id,
        task_id,
        CheckTier::Compiler,
        "cargo build",
        "bin",
        Some(record.id),
        "Compilation succeeded without warnings",
        compute_artifact_hash(compiler_log),
    );

    sqlx::query(
        r#"
        INSERT INTO verification_checks (
            id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized,
            evidence_artifact_id, summary, snapshot_hash, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(check.check_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(check.tier.as_u8() as i64)
    .bind(check.status.as_str())
    .bind(&check.command_or_tool)
    .bind(&check.inputs_normalized)
    .bind(record.id.as_bytes().as_slice())
    .bind(&check.summary)
    .bind(&check.snapshot_hash)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // 5. Verification Gate Evaluation
    let gate = EvidenceCompletionGate::new(pool.clone(), store.clone(), ws_root.clone());
    let decision = gate
        .evaluate_task_completion(mission_id, task_id, &check.snapshot_hash)
        .await
        .expect("gate evaluation should succeed");
    assert!(decision.is_satisfied);

    // 6. Projection to human-readable path
    let proj_path = ws_root.join(".planning").join("COMPILE_REPORT.md");
    let projected = service.project(record.id, "log", &proj_path).await.unwrap();
    assert!(projected.exists());

    // 7. Restart simulation: drop pool and service
    pool.close().await;
    drop(service);
    drop(store);

    // 8. Reload after restart
    let pool2 = initialize_database(&db_path).await.unwrap();
    let store2 = Arc::new(FsArtifactStore::new(&artifacts_dir));
    let service2 = ArtifactService::new(store2, pool2);

    let reloaded = service2.retrieve_payload(record.id, "log").await.unwrap();
    assert_eq!(reloaded, compiler_log);

    let audit2 = service2.verify_integrity(record.id, "log").await.unwrap();
    assert!(audit2.is_valid);
}
