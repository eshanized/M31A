//! Phase 22 — Adaptive Planning Engine: Integration Tests
//!
//! Validates the extended CandidateTask quality model, PlanQualityMetrics, ReplanScope
//! classification, ReplanningTrigger provenance, PlanRevisionRecord, and plan validation
//! extensions added in Phase 22.
//!
//! All tests are deterministic (no I/O, no network). No second planner is created.
//! Each test exercises canonical planning types only.

use m31a::kernel::plan::{
    CandidatePlanAssumption, PlanRevisionRecord, ReplanningTrigger, TaskRiskLevel,
};
use m31a::planning::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    PlanQualityMetrics, PlanValidator, ResourceEstimate, VerificationStrategy,
};
use m31a::recovery::replan::{ReplanScope, classify_scope};
use m31a::state_machine::agent::AgentRole;

// ─── helpers ─────────────────────────────────────────────────────────────────

fn task(id: &str) -> CandidateTask {
    CandidateTask::new(
        id,
        format!("Objective for {}", id),
        AgentRole::implementer(),
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    )
}

fn mutating_task(id: &str) -> CandidateTask {
    task(id).with_capabilities(vec![CapabilityRequirement::new(
        "fs.write",
        CapabilityAccessMode::Write,
    )])
}

// ─── 1. TaskRiskLevel ─────────────────────────────────────────────────────────

#[test]
fn test_task_risk_level_blocking_threshold() {
    assert!(!TaskRiskLevel::Low.is_blocking_threshold());
    assert!(!TaskRiskLevel::Medium.is_blocking_threshold());
    assert!(TaskRiskLevel::High.is_blocking_threshold());
    assert!(TaskRiskLevel::Critical.is_blocking_threshold());
}

#[test]
fn test_task_risk_level_display() {
    assert_eq!(TaskRiskLevel::Low.to_string(), "low");
    assert_eq!(TaskRiskLevel::Medium.to_string(), "medium");
    assert_eq!(TaskRiskLevel::High.to_string(), "high");
    assert_eq!(TaskRiskLevel::Critical.to_string(), "critical");
}

#[test]
fn test_task_risk_level_serde() {
    let json = serde_json::to_string(&TaskRiskLevel::Critical).unwrap();
    assert_eq!(json, r#""critical""#);
    let back: TaskRiskLevel = serde_json::from_str(&json).unwrap();
    assert_eq!(back, TaskRiskLevel::Critical);
}

// ─── 2. ReplanningTrigger ─────────────────────────────────────────────────────

#[test]
fn test_replanning_trigger_display_task_failure() {
    let t = ReplanningTrigger::TaskFailure {
        failed_task_id: "TASK-01".into(),
        reason: "compilation error".into(),
    };
    let s = t.to_string();
    assert!(s.contains("task_failure"));
    assert!(s.contains("TASK-01"));
}

#[test]
fn test_replanning_trigger_display_scope_expansion() {
    let t = ReplanningTrigger::ScopeExpansion {
        reason: "discovered OAuth dependency".into(),
    };
    assert!(t.to_string().contains("scope_expansion"));
}

#[test]
fn test_replanning_trigger_serde_roundtrip() {
    let t = ReplanningTrigger::InvalidatedAssumption {
        assumption_id: "A1".into(),
        description: "postgres not available".into(),
    };
    let json = serde_json::to_string(&t).unwrap();
    let back: ReplanningTrigger = serde_json::from_str(&json).unwrap();
    assert_eq!(t, back);
}

#[test]
fn test_replanning_trigger_evidence_discovery() {
    let t = ReplanningTrigger::EvidenceDiscovery {
        unexpected_finding: "JWT library missing".into(),
    };
    assert!(t.to_string().contains("evidence_discovery"));
}

// ─── 3. PlanRevisionRecord ────────────────────────────────────────────────────

#[test]
fn test_plan_revision_record_construction() {
    let trigger = ReplanningTrigger::RepositoryDrift {
        changed_files: vec!["src/auth.rs".into(), "Cargo.toml".into()],
    };
    let record = PlanRevisionRecord::new(2, "plan-01", "drift detected", trigger.clone());
    assert_eq!(record.revision, 2);
    assert_eq!(record.parent_plan_id, "plan-01");
    assert_eq!(record.reason, "drift detected");
    assert_eq!(record.trigger, trigger);
    assert!(record.affected_tasks.is_empty());
    assert!(record.invalidated_assumptions.is_empty());
}

#[test]
fn test_plan_revision_record_serde_roundtrip() {
    let trigger = ReplanningTrigger::PolicyDenial {
        reason: "sandbox violation".into(),
    };
    let record = PlanRevisionRecord::new(3, "plan-02", "policy block", trigger);
    let json = serde_json::to_string(&record).unwrap();
    let back: PlanRevisionRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back.revision, 3);
    assert_eq!(back.parent_plan_id, "plan-02");
}

// ─── 4. CandidatePlanAssumption ───────────────────────────────────────────────

#[test]
fn test_candidate_plan_assumption_defaults_not_invalidated() {
    let a = CandidatePlanAssumption::new("A1", "auth module exists");
    assert!(!a.invalidated);
    assert_eq!(a.id, "A1");
}

#[test]
fn test_candidate_plan_assumption_serde() {
    let mut a = CandidatePlanAssumption::new("A2", "postgres available");
    a.invalidated = true;
    let json = serde_json::to_string(&a).unwrap();
    let back: CandidatePlanAssumption = serde_json::from_str(&json).unwrap();
    assert!(back.invalidated);
}

// ─── 5. CandidateTask quality model ──────────────────────────────────────────

#[test]
fn test_candidate_task_builder_methods() {
    let t = task("T1")
        .with_description("Implement auth callback")
        .with_affected_paths(vec!["src/auth.rs".into()])
        .with_expected_outputs(vec!["OAuth handler".into()])
        .with_completion_criteria(vec!["cargo test passes".into()])
        .with_assumptions(vec!["provider SDK available".into()])
        .with_risk_level(TaskRiskLevel::High)
        .with_requirement_keys(vec!["REQ-AUTH-01".into()]);

    assert_eq!(t.description.unwrap(), "Implement auth callback");
    assert_eq!(t.affected_paths, vec!["src/auth.rs"]);
    assert_eq!(t.expected_outputs, vec!["OAuth handler"]);
    assert_eq!(t.completion_criteria, vec!["cargo test passes"]);
    assert_eq!(t.assumptions, vec!["provider SDK available"]);
    assert_eq!(t.risk_level, Some(TaskRiskLevel::High));
    assert_eq!(t.requirement_keys, vec!["REQ-AUTH-01"]);
    assert!(t.risk_level.as_ref().unwrap().is_blocking_threshold());
}

#[test]
fn test_candidate_task_is_mutating() {
    assert!(!task("T1").is_mutating());
    assert!(mutating_task("T2").is_mutating());
}

#[test]
fn test_candidate_task_serde_backward_compat() {
    // Old-style JSON (no new fields) must deserialize cleanly via #[serde(default)]
    let old_json = r#"{
        "id": "TASK-01",
        "objective": "Build something",
        "role": "implementer",
        "verification": { "type": "compilation" },
        "estimates": { "max_steps": 5, "max_duration_secs": 60, "max_tokens": 1000, "max_cost_usd": 0.10 }
    }"#;
    let t: CandidateTask = serde_json::from_str(old_json).unwrap();
    assert_eq!(t.id, CandidateTaskKey::new("TASK-01"));
    assert!(t.affected_paths.is_empty());
    assert!(t.completion_criteria.is_empty());
    assert!(t.risk_level.is_none());
}

// ─── 6. CandidatePlan extensions ─────────────────────────────────────────────

#[test]
fn test_candidate_plan_builder_with_revision() {
    let plan = CandidatePlan::new("P1", "Add auth", vec![task("T1")]).with_revision(3);
    assert_eq!(plan.revision, 3);
    assert!(plan.parent_plan_id.is_none());
}

#[test]
fn test_candidate_plan_with_assumptions() {
    let mut plan = CandidatePlan::new("P2", "Add auth", vec![task("T1")]);
    plan.assumptions = vec![
        CandidatePlanAssumption::new("A1", "oauth sdk present"),
        CandidatePlanAssumption::new("A2", "db migrated"),
    ];
    assert_eq!(plan.assumptions.len(), 2);
    assert!(!plan.assumptions[0].invalidated);
}

#[test]
fn test_candidate_plan_with_revision_record() {
    let trigger = ReplanningTrigger::EvidenceDiscovery {
        unexpected_finding: "missing oauth dependency".into(),
    };
    let record = PlanRevisionRecord::new(2, "plan-01", "evidence changed plan", trigger);
    let plan = CandidatePlan::new("P3", "Revised auth", vec![task("T1")])
        .with_revision(2)
        .with_parent_plan_id("plan-01")
        .with_revision_record(record.clone());

    assert_eq!(plan.revision, 2);
    assert_eq!(plan.parent_plan_id.unwrap(), "plan-01");
    assert!(plan.revision_record.is_some());
    assert_eq!(plan.revision_record.unwrap().revision, 2);
}

#[test]
fn test_candidate_plan_find_task_mut() {
    let mut plan = CandidatePlan::new("P4", "Mutation test", vec![task("T1"), task("T2")]);
    {
        let t = plan.find_task_mut(&CandidateTaskKey::new("T1")).unwrap();
        t.risk_level = Some(TaskRiskLevel::Critical);
    }
    assert_eq!(
        plan.find_task(&CandidateTaskKey::new("T1"))
            .unwrap()
            .risk_level,
        Some(TaskRiskLevel::Critical)
    );
}

// ─── 7. PlanQualityMetrics ────────────────────────────────────────────────────

#[test]
fn test_plan_quality_metrics_no_criteria() {
    let plan = CandidatePlan::new("P5", "No criteria", vec![task("T1"), task("T2")]);
    let m = PlanQualityMetrics::compute(&plan);
    assert_eq!(m.task_count, 2);
    assert_eq!(m.tasks_with_completion_criteria, 0);
    assert_eq!(m.tasks_without_completion_criteria, 2);
    assert_eq!(m.verification_coverage_ratio, 0.0);
    assert!(!m.has_full_verification_coverage());
}

#[test]
fn test_plan_quality_metrics_full_coverage() {
    let t1 = task("T1").with_completion_criteria(vec!["test passes".into()]);
    let t2 = mutating_task("T2").with_completion_criteria(vec!["file created".into()]);
    let plan = CandidatePlan::new("P6", "Full criteria", vec![t1, t2]);
    let m = PlanQualityMetrics::compute(&plan);
    assert!(m.has_full_verification_coverage());
    assert!((m.verification_coverage_ratio - 1.0).abs() < f64::EPSILON);
    assert_eq!(m.mutating_task_count, 1);
}

#[test]
fn test_plan_quality_metrics_revision_tracking() {
    let plan = CandidatePlan::new("P7", "Revised", vec![]).with_revision(4);
    let m = PlanQualityMetrics::compute(&plan);
    assert_eq!(m.revision, 4);
    assert_eq!(m.revision_count, 3);
    assert!(!m.has_revision_record());
}

#[test]
fn test_plan_quality_metrics_high_risk() {
    let t1 = task("T1").with_risk_level(TaskRiskLevel::High);
    let t2 = task("T2").with_risk_level(TaskRiskLevel::Critical);
    let t3 = task("T3"); // no risk
    let plan = CandidatePlan::new("P8", "Risk plan", vec![t1, t2, t3]);
    let m = PlanQualityMetrics::compute(&plan);
    assert_eq!(m.high_risk_task_count, 2);
}

#[test]
fn test_plan_quality_metrics_invalidated_assumptions() {
    let mut plan = CandidatePlan::new("P9", "Assumptions", vec![task("T1")]);
    plan.assumptions = vec![
        CandidatePlanAssumption::new("A1", "db up"),
        CandidatePlanAssumption {
            id: "A2".into(),
            description: "redis up".into(),
            invalidated: true,
        },
    ];
    let m = PlanQualityMetrics::compute(&plan);
    assert_eq!(m.plan_assumption_count, 2);
    assert_eq!(m.invalidated_assumption_count, 1);
    assert!(m.has_invalidated_assumptions());
}

#[test]
fn test_plan_quality_metrics_summary_format() {
    let t = task("T1").with_completion_criteria(vec!["done".into()]);
    let plan = CandidatePlan::new("P10", "Summary test", vec![t]);
    let m = PlanQualityMetrics::compute(&plan);
    let s = m.summary();
    assert!(s.contains("plan=P10"));
    assert!(s.contains("tasks=1"));
    assert!(s.contains("verification_coverage=100%"));
}

// ─── 8. ReplanScope classification ───────────────────────────────────────────

#[test]
fn test_replan_scope_no_baseline_is_full() {
    assert_eq!(classify_scope(3, 0), ReplanScope::Full);
}

#[test]
fn test_replan_scope_zero_new_is_local() {
    assert_eq!(classify_scope(0, 5), ReplanScope::Local);
}

#[test]
fn test_replan_scope_one_new_is_local() {
    assert_eq!(classify_scope(1, 10), ReplanScope::Local);
}

#[test]
fn test_replan_scope_partial() {
    assert_eq!(classify_scope(3, 10), ReplanScope::Partial);
    assert_eq!(classify_scope(9, 10), ReplanScope::Partial);
}

#[test]
fn test_replan_scope_full_when_equal() {
    assert_eq!(classify_scope(10, 10), ReplanScope::Full);
}

#[test]
fn test_replan_scope_full_when_more() {
    assert_eq!(classify_scope(15, 10), ReplanScope::Full);
}

#[test]
fn test_replan_scope_display() {
    assert_eq!(ReplanScope::Local.to_string(), "local");
    assert_eq!(ReplanScope::Partial.to_string(), "partial");
    assert_eq!(ReplanScope::Full.to_string(), "full");
}

// ─── 9. Plan validation: extended checks ─────────────────────────────────────

#[tokio::test]
async fn test_validation_passes_on_clean_high_quality_plan() {
    let t1 = task("T1")
        .with_completion_criteria(vec!["cargo check passes".into()])
        .with_affected_paths(vec!["src/auth.rs".into()]);
    let t2 = task("T2")
        .with_depends_on(vec![CandidateTaskKey::new("T1")])
        .with_completion_criteria(vec!["cargo test passes".into()]);

    let plan = CandidatePlan::new("P-HQ", "High quality plan", vec![t1, t2]);
    let validator = PlanValidator::new();
    let report = validator.validate(&plan).await;
    assert!(
        report.is_valid(),
        "High quality plan must pass: {:?}",
        report.errors
    );
}

#[tokio::test]
async fn test_validation_rejects_plan_with_cycle() {
    let t1 = task("T1").with_depends_on(vec![CandidateTaskKey::new("T2")]);
    let t2 = task("T2").with_depends_on(vec![CandidateTaskKey::new("T1")]);
    let plan = CandidatePlan::new("P-CYCLE", "Cyclic", vec![t1, t2]);
    let report = PlanValidator::new().validate(&plan).await;
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| matches!(e, m31a::planning::ValidationError::CycleDetected { .. }))
    );
}

#[tokio::test]
async fn test_validation_rejects_duplicate_task_id() {
    let t1 = task("T1");
    let t2 = task("T1"); // intentional duplicate
    let plan = CandidatePlan::new("P-DUP", "Dup IDs", vec![t1, t2]);
    let report = PlanValidator::new().validate(&plan).await;
    assert!(!report.is_valid());
    assert!(
        report
            .errors
            .iter()
            .any(|e| matches!(e, m31a::planning::ValidationError::DuplicateTaskId { .. }))
    );
}

// ─── 10. Plan serde backward compatibility (revision fields optional) ─────────

#[test]
fn test_candidate_plan_old_json_no_revision_fields() {
    let json = r#"{
        "plan_id": "plan-legacy",
        "objective": "Legacy plan",
        "tasks": [],
        "created_at": "2026-01-01T00:00:00Z"
    }"#;
    let plan: CandidatePlan = serde_json::from_str(json).unwrap();
    assert_eq!(plan.plan_id, "plan-legacy");
    assert_eq!(plan.revision, 1); // default
    assert!(plan.parent_plan_id.is_none());
    assert!(plan.revision_record.is_none());
    assert!(plan.assumptions.is_empty());
}

// ─── 11. Validation warnings: Deduplication & Completion Criteria ─────────────

#[tokio::test]
async fn test_validation_warns_on_duplicate_objectives() {
    let mut t1 = task("T1");
    t1.objective = "Implement oauth callback".into();
    let mut t2 = task("T2");
    t2.objective = "Implement oauth callback".into(); // Identical objective

    let plan = CandidatePlan::new("P-DUP-OBJ", "Duplicate objectives", vec![t1, t2]);
    let report = PlanValidator::new().validate(&plan).await;
    assert!(report.is_valid()); // Warnings do not block
    assert!(report.warnings.iter().any(|w| match w {
        m31a::planning::ValidationWarning::Informational { message } =>
            message.contains("Potential duplicate task detected"),
        _ => false,
    }));
}

#[tokio::test]
async fn test_validation_warns_on_mutating_task_without_completion_criteria() {
    let mut t = mutating_task("T-MUT");
    t.completion_criteria = vec![]; // Empty criteria on mutating task

    let plan = CandidatePlan::new("P-MUT-WARN", "Mutating without criteria", vec![t]);
    let report = PlanValidator::new().validate(&plan).await;
    assert!(report.is_valid()); // Warning, not blocking error
    assert!(report.warnings.iter().any(|w| match w {
        m31a::planning::ValidationWarning::Informational { message } =>
            message.contains("no explicit completion_criteria declared"),
        _ => false,
    }));
}

// ─── 12. PlanServiceImpl replan provenance ────────────────────────────────────

#[tokio::test]
async fn test_plan_service_replan_attaches_provenance() {
    use m31a::kernel::seams::planner::{PlanService, ReplanRequest};
    use m31a::planning::PlanServiceImpl;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let service = PlanServiceImpl::new(dir.path());
    let failed_task_id = m31a::ids::TaskId::new();
    let req = ReplanRequest {
        mission_id: m31a::ids::MissionId::new(),
        failed_task_id,
        reason: "Compilation failed in src/auth.rs".into(),
    };

    let res = service.replan(req.clone()).await.unwrap();
    assert_eq!(res.modified_tasks, vec![failed_task_id]);

    // Verify written plan projection exists
    let proj_dir = m31a::planning::mission_projections_dir(dir.path(), req.mission_id);
    let plan_path = proj_dir.join("plan.json");
    assert!(plan_path.exists());

    let content = std::fs::read_to_string(plan_path).unwrap();
    let plan: CandidatePlan = serde_json::from_str(&content).unwrap();
    assert_eq!(plan.revision, 2);
    assert!(plan.parent_plan_id.is_some());
    let record = plan.revision_record.unwrap();
    assert_eq!(record.revision, 2);
    assert_eq!(record.reason, "Compilation failed in src/auth.rs");
    match record.trigger {
        ReplanningTrigger::TaskFailure {
            failed_task_id: ftid,
            reason,
        } => {
            assert_eq!(ftid, failed_task_id.to_string());
            assert_eq!(reason, "Compilation failed in src/auth.rs");
        }
        other => panic!("Unexpected trigger: {:?}", other),
    }
}
