//! Phase 12 Completion Reports, Two-Phase Sealing & Evidence Linking Verification Suite (RPT-01, RPT-02, RPT-03, D-13).

use chrono::Utc;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::ids::{AgentId, ArtifactId, CheckId, CheckpointId, EventId, MissionId, TaskId};
use m31a::persistence::artifacts::FsArtifactStore;
use m31a::persistence::sqlite::repositories::report::SqliteReportRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::report::evidence::{EvidenceLinker, EvidenceUri};
use m31a::report::generator::ReportGenerator;
use m31a::report::json::JsonReportProjector;
use m31a::report::markdown::MarkdownReportProjector;
use m31a::report::model::{
    AgentReportItem, ArtifactReportItem, CompletionReport, CompletionStatus, FailureReportItem,
    FileChangeReportItem, GitReportState, ModelReportItem, PlanReportSummary, ReportCandidate,
    RequirementReportItem, ReviewReportItem, TaskReportItem, VerificationReportItem,
};
use m31a::state::completion::{CompletionContext, CompletionGate, CompletionGateError};
use m31a::telemetry::redactor::SecretRedactor;

#[tokio::test]
async fn test_completion_report_generation() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_report_gen.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let artifact_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&artifact_dir));
    let report_repo = Arc::new(SqliteReportRepository::new(pool.clone()));
    let redactor = Arc::new(SecretRedactor::new());

    let generator = ReportGenerator::new(
        pool.clone(),
        artifact_store.clone(),
        report_repo.clone(),
        redactor.clone(),
    );

    let mission_id = MissionId::new();
    let now = Utc::now().to_rfc3339();

    // Insert dummy mission, tasks, agents in SQLite
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Demonstrate autonomous pipeline execution")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let task1_id = TaskId::new();
    let task2_id = TaskId::new();
    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task1_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("Initialize repository structure")
        .bind("succeeded")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task2_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("Verify unit test pass rates")
        .bind("succeeded")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    let agent_id = AgentId::new();
    sqlx::query("INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("LeadArchitect")
        .bind("idle")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

    // Insert a model telemetry span to populate model stats
    sqlx::query(
        r#"
        INSERT INTO telemetry_spans (
            span_id, trace_id, parent_span_id, mission_id, task_id, agent_id,
            name, kind, start_time_us, end_time_us, duration_us, status,
            error_message, attributes_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind("span-model-1")
    .bind("trace-1")
    .bind(None::<String>)
    .bind(mission_id.to_string())
    .bind(task1_id.to_string())
    .bind(agent_id.to_string())
    .bind("llm_invoke")
    .bind("model")
    .bind(1000i64)
    .bind(2000i64)
    .bind(1000i64)
    .bind("ok")
    .bind(None::<String>)
    .bind(r#"{"provider": "anthropic", "model": "claude-3-5-sonnet", "tokens": 4200, "cost_usd": 0.035}"#)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Build candidate report (Phase 1)
    let candidate = generator.build_candidate(&mission_id).await.unwrap();

    assert_eq!(candidate.mission_id, mission_id);
    assert_eq!(
        candidate.objective,
        "Demonstrate autonomous pipeline execution"
    );
    assert_eq!(candidate.tasks_executed.len(), 2);
    assert_eq!(candidate.agents_used.len(), 1);
    assert_eq!(candidate.models_used.len(), 1);
    assert_eq!(candidate.models_used[0].provider, "anthropic");
    assert_eq!(candidate.models_used[0].total_tokens, 4200);
}

#[tokio::test]
async fn test_two_phase_candidate_sealing() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_two_phase.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let artifact_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&artifact_dir));
    let report_repo = Arc::new(SqliteReportRepository::new(pool.clone()));
    let redactor = Arc::new(SecretRedactor::new());

    let generator = ReportGenerator::new(
        pool.clone(),
        artifact_store.clone(),
        report_repo.clone(),
        redactor.clone(),
    );

    let mission_id = MissionId::new();
    let candidate = ReportCandidate {
        schema_version: 1,
        mission_id,
        objective: "Mission for two-phase sealing test".to_string(),
        requirements_summary: vec![RequirementReportItem {
            id: "RPT-01".to_string(),
            description: "Two-phase sealing without circular deadlock".to_string(),
            status: "Satisfied".to_string(),
            evidence_locator: Some(EvidenceLinker::check(CheckId::new())),
        }],
        plan_summary: PlanReportSummary {
            plan_id: "plan-two-phase".to_string(),
            total_tasks: 1,
            completed_tasks: 1,
            failed_tasks: 0,
            duration_seconds: 45,
        },
        tasks_executed: vec![TaskReportItem {
            task_id: TaskId::new(),
            title: "Phase 1 task".to_string(),
            status: "Succeeded".to_string(),
            agent_id: Some(AgentId::new()),
            duration_seconds: 45,
            evidence_locator: None,
        }],
        agents_used: vec![],
        models_used: vec![],
        files_changed: vec![],
        git_state: GitReportState {
            base_commit: "base123".to_string(),
            final_commit: "final123".to_string(),
            branch: "master".to_string(),
            commit_trailers: vec!["Signed-off-by: M31A".to_string()],
            clean_worktree: true,
        },
        verifications: vec![],
        review_verdicts: vec![],
        failures_encountered: vec![],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![],
        provisional_status: Some(CompletionStatus::Succeeded),
        candidate_timestamp: Utc::now(),
        duration_seconds: 45,
    };

    // Step 1: Candidate is built & stored.
    let candidate_ready = generator
        .build_candidate_from_candidate(candidate)
        .await
        .unwrap();

    // Step 2: Controller sets CompletionContext.final_report_persisted = true.
    let mut completion_context = CompletionContext::all_satisfied();
    completion_context.final_report_persisted = true;

    // Evaluate gate (D-13: circular dependency broken)
    let gate_result = CompletionGate::evaluate(&completion_context);
    assert!(
        gate_result.is_ok(),
        "CompletionGate passes with candidate persisted"
    );

    // Step 3: Seal into final CompletionReport
    let sealed_report = generator
        .seal_report(candidate_ready, &gate_result)
        .await
        .unwrap();

    assert_eq!(sealed_report.final_status, CompletionStatus::Succeeded);

    // Verify metadata persisted to SQLite
    let metadata_opt = report_repo.get_report_metadata(&mission_id).await.unwrap();
    assert!(
        metadata_opt.is_some(),
        "Metadata row exists in completion_reports table"
    );
    let metadata = metadata_opt.unwrap();
    assert_eq!(metadata.mission_id, mission_id);
    assert_eq!(metadata.status, "succeeded");
    assert_eq!(metadata.tasks_succeeded, 1);
    assert_eq!(metadata.tasks_failed, 0);

    // Verify that gate failure produces Failed or Blocked status
    let failed_gate = Err(vec![CompletionGateError::UnresolvedTasks]);
    let failed_candidate = ReportCandidate {
        schema_version: 1,
        mission_id: MissionId::new(),
        objective: "Failing mission".to_string(),
        requirements_summary: vec![],
        plan_summary: PlanReportSummary {
            plan_id: "p2".to_string(),
            total_tasks: 1,
            completed_tasks: 0,
            failed_tasks: 1,
            duration_seconds: 10,
        },
        tasks_executed: vec![],
        agents_used: vec![],
        models_used: vec![],
        files_changed: vec![],
        git_state: GitReportState {
            base_commit: "a".to_string(),
            final_commit: "b".to_string(),
            branch: "main".to_string(),
            commit_trailers: vec![],
            clean_worktree: true,
        },
        verifications: vec![],
        review_verdicts: vec![],
        failures_encountered: vec![],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![],
        provisional_status: None,
        candidate_timestamp: Utc::now(),
        duration_seconds: 10,
    };
    let sealed_failed = generator
        .seal_report(failed_candidate, &failed_gate)
        .await
        .unwrap();
    assert_eq!(sealed_failed.final_status, CompletionStatus::Failed);

    let blocked_gate = Err(vec![CompletionGateError::FatalPolicyState]);
    let blocked_candidate = ReportCandidate {
        schema_version: 1,
        mission_id: MissionId::new(),
        objective: "Blocked mission".to_string(),
        requirements_summary: vec![],
        plan_summary: PlanReportSummary {
            plan_id: "p3".to_string(),
            total_tasks: 1,
            completed_tasks: 0,
            failed_tasks: 0,
            duration_seconds: 10,
        },
        tasks_executed: vec![],
        agents_used: vec![],
        models_used: vec![],
        files_changed: vec![],
        git_state: GitReportState {
            base_commit: "a".to_string(),
            final_commit: "b".to_string(),
            branch: "main".to_string(),
            commit_trailers: vec![],
            clean_worktree: true,
        },
        verifications: vec![],
        review_verdicts: vec![],
        failures_encountered: vec![],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![],
        provisional_status: None,
        candidate_timestamp: Utc::now(),
        duration_seconds: 10,
    };
    let sealed_blocked = generator
        .seal_report(blocked_candidate, &blocked_gate)
        .await
        .unwrap();
    assert_eq!(sealed_blocked.final_status, CompletionStatus::Blocked);
}

#[tokio::test]
async fn test_durable_evidence_links() {
    let art_id = ArtifactId::new();
    let chk_id = CheckId::new();
    let ckp_id = CheckpointId::new();
    let evt_id = EventId::new();

    // Verify formatting
    let art_uri_str = EvidenceLinker::artifact(art_id);
    assert_eq!(art_uri_str, format!("artifact://{art_id}"));

    let chk_uri_str = EvidenceLinker::check(chk_id);
    assert_eq!(chk_uri_str, format!("check://{chk_id}"));

    let cmt_uri_str = EvidenceLinker::commit("0123456789abcdef");
    assert_eq!(cmt_uri_str, "commit://0123456789abcdef");

    let ckp_uri_str = EvidenceLinker::checkpoint(ckp_id);
    assert_eq!(ckp_uri_str, format!("checkpoint://{ckp_id}"));

    let pol_uri_str = EvidenceLinker::policy_grant("grant_rule_42");
    assert_eq!(pol_uri_str, "policy://grant_rule_42");

    let evt_uri_str = EvidenceLinker::event(evt_id);
    assert_eq!(evt_uri_str, format!("event://{evt_id}"));

    // Verify parsing & validation
    assert!(EvidenceLinker::is_valid(&art_uri_str));
    assert!(EvidenceLinker::is_valid(&chk_uri_str));
    assert!(EvidenceLinker::is_valid(&cmt_uri_str));
    assert!(EvidenceLinker::is_valid(&ckp_uri_str));
    assert!(EvidenceLinker::is_valid(&pol_uri_str));
    assert!(EvidenceLinker::is_valid(&evt_uri_str));

    let parsed_art = EvidenceLinker::validate(&art_uri_str).unwrap();
    assert_eq!(parsed_art, EvidenceUri::Artifact(art_id));

    let parsed_chk = EvidenceLinker::validate(&chk_uri_str).unwrap();
    assert_eq!(parsed_chk, EvidenceUri::Check(chk_id));

    // Reject invalid schemes & broken IDs
    assert!(!EvidenceLinker::is_valid("http://example.com"));
    assert!(!EvidenceLinker::is_valid("artifact://not-a-uuid"));
    assert!(!EvidenceLinker::is_valid("random_string_no_scheme"));

    // Total evidence count in report
    let report = CompletionReport {
        schema_version: 1,
        mission_id: MissionId::new(),
        objective: "Evidence count check".to_string(),
        requirements_summary: vec![RequirementReportItem {
            id: "R1".to_string(),
            description: "Req 1".to_string(),
            status: "Passed".to_string(),
            evidence_locator: Some(art_uri_str.clone()),
        }],
        plan_summary: PlanReportSummary {
            plan_id: "p1".to_string(),
            total_tasks: 1,
            completed_tasks: 1,
            failed_tasks: 0,
            duration_seconds: 1,
        },
        tasks_executed: vec![TaskReportItem {
            task_id: TaskId::new(),
            title: "Task 1".to_string(),
            status: "Succeeded".to_string(),
            agent_id: None,
            duration_seconds: 1,
            evidence_locator: Some(chk_uri_str.clone()),
        }],
        agents_used: vec![],
        models_used: vec![],
        files_changed: vec![FileChangeReportItem {
            path: "src/lib.rs".to_string(),
            lines_added: 5,
            lines_removed: 1,
            diff_artifact_locator: Some(art_uri_str.clone()),
        }],
        git_state: GitReportState {
            base_commit: "a".to_string(),
            final_commit: "b".to_string(),
            branch: "master".to_string(),
            commit_trailers: vec![],
            clean_worktree: true,
        },
        verifications: vec![VerificationReportItem {
            tier: 1,
            check_id: "chk1".to_string(),
            status: "passed".to_string(),
            evidence_locator: Some(chk_uri_str.clone()),
        }],
        review_verdicts: vec![],
        failures_encountered: vec![],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![ArtifactReportItem {
            id: art_id,
            name: "diff.patch".to_string(),
            path: "artifacts/diff.patch".to_string(),
            size_bytes: 1024,
            sha256: "abc...".to_string(),
        }],
        final_status: CompletionStatus::Succeeded,
        created_at: Utc::now(),
        duration_seconds: 1,
    };

    assert_eq!(report.total_evidence_count(), 5);
}

#[tokio::test]
async fn test_secret_redaction_in_reports() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_redaction.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let artifact_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&artifact_dir));
    let report_repo = Arc::new(SqliteReportRepository::new(pool.clone()));
    let redactor = Arc::new(SecretRedactor::new());

    // Register a specific exact-match secret
    redactor.register_secret("confidential_enterprise_token_xyz999");

    let generator = ReportGenerator::new(
        pool.clone(),
        artifact_store.clone(),
        report_repo.clone(),
        redactor.clone(),
    );

    let mission_id = MissionId::new();
    let candidate = ReportCandidate {
        schema_version: 1,
        mission_id,
        objective: "Deploy with secret confidential_enterprise_token_xyz999".to_string(),
        requirements_summary: vec![RequirementReportItem {
            id: "SEC-TEST".to_string(),
            description: "Test AWS key: AKIAIOSFODNN7EXAMPLE".to_string(),
            status: "Satisfied".to_string(),
            evidence_locator: None,
        }],
        plan_summary: PlanReportSummary {
            plan_id: "plan-sec".to_string(),
            total_tasks: 1,
            completed_tasks: 1,
            failed_tasks: 0,
            duration_seconds: 10,
        },
        tasks_executed: vec![TaskReportItem {
            task_id: TaskId::new(),
            title: "Execute query with Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.superSecretSignature".to_string(),
            status: "Succeeded".to_string(),
            agent_id: None,
            duration_seconds: 10,
            evidence_locator: None,
        }],
        agents_used: vec![],
        models_used: vec![],
        files_changed: vec![],
        git_state: GitReportState {
            base_commit: "a".to_string(),
            final_commit: "b".to_string(),
            branch: "master".to_string(),
            commit_trailers: vec![],
            clean_worktree: true,
        },
        verifications: vec![],
        review_verdicts: vec![],
        failures_encountered: vec![FailureReportItem {
            classification: "AuthError".to_string(),
            root_cause: "Token sk-ant-api03-abcdefghijklmnop12345 expired".to_string(),
            recovered: true,
        }],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![],
        provisional_status: Some(CompletionStatus::Succeeded),
        candidate_timestamp: Utc::now(),
        duration_seconds: 10,
    };

    let processed_candidate = generator
        .build_candidate_from_candidate(candidate)
        .await
        .unwrap();

    let gate_res = Ok(());
    let sealed = generator
        .seal_report(processed_candidate, &gate_res)
        .await
        .unwrap();

    // Verify all secrets are sanitized in the sealed struct
    assert!(
        !sealed
            .objective
            .contains("confidential_enterprise_token_xyz999")
    );
    assert!(sealed.objective.contains("[REDACTED:EXACT_SECRET]"));

    assert!(
        !sealed.requirements_summary[0]
            .description
            .contains("AKIAIOSFODNN7EXAMPLE")
    );
    assert!(
        sealed.requirements_summary[0]
            .description
            .contains("[REDACTED:AWS_KEY]")
    );

    assert!(
        !sealed.tasks_executed[0]
            .title
            .contains("superSecretSignature")
    );
    assert!(
        sealed.tasks_executed[0]
            .title
            .contains("[REDACTED:BEARER_TOKEN]")
    );

    assert!(
        !sealed.failures_encountered[0]
            .root_cause
            .contains("sk-ant-api03")
    );
    assert!(
        sealed.failures_encountered[0]
            .root_cause
            .contains("[REDACTED:API_TOKEN]")
    );

    // Verify Markdown projection is sanitized
    let md = MarkdownReportProjector::render_report(&sealed);
    assert!(!md.contains("confidential_enterprise_token_xyz999"));
    assert!(!md.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!md.contains("superSecretSignature"));

    // Verify JSON projection is sanitized
    let json = JsonReportProjector::render_report(&sealed).unwrap();
    assert!(!json.contains("confidential_enterprise_token_xyz999"));
    assert!(!json.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!json.contains("superSecretSignature"));
}

#[tokio::test]
async fn test_dual_projection_json_schema_validation() {
    let report = CompletionReport {
        schema_version: 1,
        mission_id: MissionId::new(),
        objective: "Dual projection validation".to_string(),
        requirements_summary: vec![RequirementReportItem {
            id: "RPT-02".to_string(),
            description: "Dual projections validated against schema".to_string(),
            status: "Satisfied".to_string(),
            evidence_locator: Some("artifact://018f0000-0000-7000-8000-000000000001".to_string()),
        }],
        plan_summary: PlanReportSummary {
            plan_id: "plan-dual-projection".to_string(),
            total_tasks: 2,
            completed_tasks: 2,
            failed_tasks: 0,
            duration_seconds: 180,
        },
        tasks_executed: vec![
            TaskReportItem {
                task_id: TaskId::new(),
                title: "Generate Markdown".to_string(),
                status: "Succeeded".to_string(),
                agent_id: Some(AgentId::new()),
                duration_seconds: 90,
                evidence_locator: None,
            },
            TaskReportItem {
                task_id: TaskId::new(),
                title: "Generate JSON Schema".to_string(),
                status: "Succeeded".to_string(),
                agent_id: Some(AgentId::new()),
                duration_seconds: 90,
                evidence_locator: None,
            },
        ],
        agents_used: vec![AgentReportItem {
            agent_id: AgentId::new(),
            role: "Projector".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            steps_count: 5,
            total_tokens: 12000,
        }],
        models_used: vec![ModelReportItem {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            prompt_tokens: 8000,
            completion_tokens: 4000,
            total_tokens: 12000,
            estimated_cost_usd: 0.084,
        }],
        files_changed: vec![FileChangeReportItem {
            path: "src/report/markdown.rs".to_string(),
            lines_added: 120,
            lines_removed: 5,
            diff_artifact_locator: None,
        }],
        git_state: GitReportState {
            base_commit: "c0ffee1".to_string(),
            final_commit: "c0ffee2".to_string(),
            branch: "master".to_string(),
            commit_trailers: vec!["Signed-off-by: M31A".to_string()],
            clean_worktree: true,
        },
        verifications: vec![VerificationReportItem {
            tier: 2,
            check_id: "check-clippy".to_string(),
            status: "passed".to_string(),
            evidence_locator: None,
        }],
        review_verdicts: vec![ReviewReportItem {
            reviewer_role: "SecurityArchitect".to_string(),
            verdict: "Approved".to_string(),
            comments: "Schema and redaction validated".to_string(),
            timestamp: Utc::now(),
        }],
        failures_encountered: vec![],
        retries_and_replans: vec![],
        policy_escalations: vec![],
        artifacts_produced: vec![ArtifactReportItem {
            id: ArtifactId::new(),
            name: "REPORT.md".to_string(),
            path: "artifacts/REPORT.md".to_string(),
            size_bytes: 3500,
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        }],
        final_status: CompletionStatus::Succeeded,
        created_at: Utc::now(),
        duration_seconds: 180,
    };

    // 1. Validate Markdown projection
    let md = MarkdownReportProjector::render_report(&report);
    assert!(md.starts_with("---\n"));
    assert!(md.contains("mission_id:"));
    assert!(md.contains("status: \"succeeded\""));
    assert!(md.contains("## Executive Summary"));
    assert!(md.contains("## Requirements & Verification Matrix"));
    assert!(md.contains("## Plan & Task Execution Log"));
    assert!(md.contains("## Agents & Models Utilized"));
    assert!(md.contains("## Code Changes & Git Attribution"));
    assert!(md.contains("## Verification & Review Verdicts"));
    assert!(md.contains("## Failure & Recovery History"));
    assert!(md.contains("## Policy & Governance Escalations"));
    assert!(md.contains("## Artifact Index"));

    // 2. Validate JSON projection and JSON Schema
    let json_str = JsonReportProjector::render_report(&report).unwrap();
    let validated =
        JsonReportProjector::validate_json(&json_str).expect("Valid JSON matching model");
    assert_eq!(validated.mission_id, report.mission_id);
    assert_eq!(validated.final_status, CompletionStatus::Succeeded);

    let schema_val = JsonReportProjector::generate_schema();
    assert!(schema_val.is_object());
    let schema_json = JsonReportProjector::generate_schema_json().unwrap();
    assert!(schema_json.contains("CompletionReport"));
}
