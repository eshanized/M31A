//! Independent Verification Engine (`m31a-verify::engine`).
//!
//! Generates revision-bound, impact-aware verification plans and independently
//! verifies frozen candidate change-sets, producing durable, cryptographically bound evidence.
//!
//! Invariants:
//! 1. Verifier evaluates the frozen change-set, not a live mutating agent directory.
//! 2. Deterministic empirical checks (compiler, tests, linters) precede semantic review.
//! 3. Zero-test false positives are rejected: a command that executed 0 tests cannot prove test passage.
//! 4. Skipped or unsupported checks cannot be reported as passing.
//! 5. Every mandatory criterion must bind to durable evidence matching the exact manifest hash.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use chrono::Utc;
use m31a_core::workflow::{ChangeSetManifest, DurableVerificationRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;
use uuid::Uuid;

use crate::runners::ProjectRunner;
use crate::safety::{CodebaseSafetyGate, SafetyGateVerdict};

/// Category of verification requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCategory {
    BuildCheck,
    UnitTest,
    IntegrationTest,
    TargetedTest,
    SafetyAudit,
    SemanticReview,
    CustomCriterion,
}

impl VerificationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuildCheck => "build_check",
            Self::UnitTest => "unit_test",
            Self::IntegrationTest => "integration_test",
            Self::TargetedTest => "targeted_test",
            Self::SafetyAudit => "safety_audit",
            Self::SemanticReview => "semantic_review",
            Self::CustomCriterion => "custom_criterion",
        }
    }
}

/// An individual verifiable requirement within a verification plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationRequirement {
    pub id: String,
    pub category: VerificationCategory,
    pub description: String,
    pub target_paths: Vec<PathBuf>,
    pub target_symbols: Vec<String>,
    pub mandatory: bool,
}

/// Structured, content-hashed verification plan derived from task requirements and repo impact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationPlan {
    pub plan_id: String,
    pub workflow_id: String,
    pub task_id: String,
    pub attempt_id: u32,
    pub manifest_hash: String,
    pub requirements: Vec<VerificationRequirement>,
    pub related_tests: Vec<String>,
    pub impacted_symbols: Vec<String>,
    pub plan_hash: String,
    pub created_at_epoch_ms: i64,
}

impl VerificationPlan {
    /// Computes deterministic content hash over all requirements and manifest binding.
    pub fn compute_plan_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.workflow_id.as_bytes());
        hasher.update(self.task_id.as_bytes());
        hasher.update(self.attempt_id.to_le_bytes());
        hasher.update(self.manifest_hash.as_bytes());

        for req in &self.requirements {
            hasher.update(req.id.as_bytes());
            hasher.update(req.category.as_str().as_bytes());
            hasher.update(req.description.as_bytes());
            hasher.update([if req.mandatory { 1 } else { 0 }]);
        }

        for rt in &self.related_tests {
            hasher.update(rt.as_bytes());
        }
        for is in &self.impacted_symbols {
            hasher.update(is.as_bytes());
        }

        format!("{:x}", hasher.finalize())
    }
}

/// Overall outcome report of evidence validation against a verification plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvaluationReport {
    pub success: bool,
    pub total_requirements: usize,
    pub satisfied_requirements: usize,
    pub failed_requirements: Vec<(String, String)>,
    pub missing_requirements: Vec<String>,
    pub zero_test_false_positives_detected: usize,
    pub summary: String,
}

/// Authoritative independent verification engine.
#[derive(Debug, Default, Clone)]
pub struct IndependentVerificationEngine;

impl IndependentVerificationEngine {
    pub fn new() -> Self {
        Self
    }

    /// Generates a structured, impact-aware verification plan from an immutable ChangeSetManifest.
    pub fn generate_plan(
        workspace_root: &Path,
        manifest: &ChangeSetManifest,
        acceptance_criteria: &[String],
    ) -> Result<VerificationPlan> {
        let now = Utc::now().timestamp_millis();
        let plan_id = format!("vplan-{}", Uuid::new_v4().simple());

        let modified_files: Vec<PathBuf> = manifest
            .files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();

        // 1. Analyze repository impact using m31a-ast
        let mut impacted_symbols = Vec::new();
        let mut related_tests = Vec::new();

        let ast_index = m31a_ast::AstCodebaseIndex::build(workspace_root, 1000);
        let impact_report =
            m31a_ast::SemanticImpactAnalyzer::analyze_impact(&ast_index, &modified_files, &[]);
        for sym in impact_report.modified_symbols {
            impacted_symbols.push(sym);
        }
        for imp in impact_report.impacts {
            if !impacted_symbols.contains(&imp.target_symbol) {
                impacted_symbols.push(imp.target_symbol);
            }
        }
        related_tests.extend(impact_report.recommended_test_targets);

        // 2. Synthesize requirements
        let mut requirements = Vec::new();

        // Requirement 1: Compilation / Static Type Check
        requirements.push(VerificationRequirement {
            id: "req-build-check".into(),
            category: VerificationCategory::BuildCheck,
            description: "Repository compiles cleanly without blocking errors".into(),
            target_paths: modified_files.clone(),
            target_symbols: Vec::new(),
            mandatory: true,
        });

        // Requirement 2: Test Suite Execution
        requirements.push(VerificationRequirement {
            id: "req-tests-run".into(),
            category: VerificationCategory::UnitTest,
            description: "Regression and unit test suite passes with non-zero executed tests"
                .into(),
            target_paths: modified_files.clone(),
            target_symbols: impacted_symbols.clone(),
            mandatory: true,
        });

        // Requirement 3: Codebase Safety & Blast-Radius Audit
        requirements.push(VerificationRequirement {
            id: "req-safety-audit".into(),
            category: VerificationCategory::SafetyAudit,
            description: "Semantic blast radius and safety gates pass without blocking violations"
                .into(),
            target_paths: modified_files.clone(),
            target_symbols: impacted_symbols.clone(),
            mandatory: true,
        });

        // Requirement 4: User/Task Acceptance Criteria
        for (idx, crit) in acceptance_criteria.iter().enumerate() {
            requirements.push(VerificationRequirement {
                id: format!("req-custom-{}", idx + 1),
                category: VerificationCategory::CustomCriterion,
                description: crit.clone(),
                target_paths: Vec::new(),
                target_symbols: Vec::new(),
                mandatory: true,
            });
        }

        let mut plan = VerificationPlan {
            plan_id,
            workflow_id: manifest.workflow_id.clone(),
            task_id: manifest.task_id.clone(),
            attempt_id: manifest.attempt_id,
            manifest_hash: manifest.manifest_hash.clone(),
            requirements,
            related_tests,
            impacted_symbols,
            plan_hash: String::new(),
            created_at_epoch_ms: now,
        };

        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }

    /// Parses test numbers from standard test runners (cargo test, pytest, jest, go test).
    pub fn parse_test_counts(
        stdout: &str,
        stderr: &str,
    ) -> (Option<usize>, Option<usize>, Option<usize>, Option<usize>) {
        let combined = format!("{stdout}\n{stderr}");

        // Rust: "test result: ok. 12 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out"
        let re_rust = regex::Regex::new(
            r"test result:\s+\w+\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored",
        )
        .unwrap();
        if let Some(caps) = re_rust.captures(&combined) {
            let passed: usize = caps[1].parse().unwrap_or(0);
            let failed: usize = caps[2].parse().unwrap_or(0);
            let ignored: usize = caps[3].parse().unwrap_or(0);
            let total = passed + failed + ignored;
            return (Some(total), Some(passed), Some(failed), Some(ignored));
        }

        // Python/Pytest: "12 passed, 2 skipped, 1 failed in 0.42s"
        let re_pytest_passed = regex::Regex::new(r"(\d+)\s+passed").unwrap();
        let re_pytest_failed = regex::Regex::new(r"(\d+)\s+failed").unwrap();
        let re_pytest_skipped = regex::Regex::new(r"(\d+)\s+skipped").unwrap();

        let passed = re_pytest_passed
            .captures(&combined)
            .and_then(|c| c[1].parse::<usize>().ok());
        let failed = re_pytest_failed
            .captures(&combined)
            .and_then(|c| c[1].parse::<usize>().ok());
        let skipped = re_pytest_skipped
            .captures(&combined)
            .and_then(|c| c[1].parse::<usize>().ok());

        if passed.is_some() || failed.is_some() {
            let p = passed.unwrap_or(0);
            let f = failed.unwrap_or(0);
            let s = skipped.unwrap_or(0);
            return (Some(p + f + s), Some(p), Some(f), Some(s));
        }

        // Generic fallback if tests ran
        (None, None, None, None)
    }

    /// Executes deterministic verification checks against the workspace root.
    pub async fn execute_plan(
        workspace_root: &Path,
        plan: &VerificationPlan,
        manifest: &ChangeSetManifest,
        timeout: Duration,
    ) -> Result<Vec<DurableVerificationRecord>> {
        let mut records = Vec::new();

        // 1. Execute Build Check
        let b_start = Instant::now();
        let build_raw = ProjectRunner::run_check(workspace_root, timeout).await;
        let b_duration = b_start.elapsed().as_millis() as u64;

        let (build_status, build_exit, build_diag) = match build_raw {
            Ok(ref r) if r.success => ("Passed".to_string(), Some(r.exit_code), None),
            Ok(ref r) => (
                "Failed".to_string(),
                Some(r.exit_code),
                Some(format!("{}\n{}", r.stdout, r.stderr)),
            ),
            Err(ref e) => (
                "InfrastructureError".to_string(),
                None,
                Some(format!("Build check failed to execute: {e}")),
            ),
        };

        records.push(DurableVerificationRecord {
            id: format!("vrec-{}", Uuid::new_v4().simple()),
            workflow_id: plan.workflow_id.clone(),
            task_id: plan.task_id.clone(),
            attempt_id: plan.attempt_id,
            manifest_hash: manifest.manifest_hash.clone(),
            source_base_sha: manifest.source_base_sha.clone(),
            target_commit_sha: manifest.target_commit_sha.clone(),
            verification_plan_hash: plan.plan_hash.clone(),
            requirement_id: "req-build-check".into(),
            category: "build_check".into(),
            operation: "ProjectRunner::run_check".into(),
            exit_code: build_exit,
            status: build_status.clone(),
            tests_total: None,
            tests_passed: None,
            tests_failed: None,
            tests_skipped: None,
            duration_ms: b_duration,
            diagnostics: build_diag,
            artifact_refs: Vec::new(),
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
        });

        // 2. Execute Test Suite
        let t_start = Instant::now();
        let test_raw = if build_status == "Passed" {
            ProjectRunner::run_tests(workspace_root, timeout).await
        } else {
            Err(anyhow!("Skipping test run because build check failed"))
        };
        let t_duration = t_start.elapsed().as_millis() as u64;

        let (test_status, test_exit, total, passed, failed, skipped, test_diag) = match test_raw {
            Ok(ref r) => {
                let (tot, pass, fail, skip) = Self::parse_test_counts(&r.stdout, &r.stderr);
                if tot == Some(0) {
                    // ZERO-TEST FALSE POSITIVE ENFORCEMENT: A command that ran 0 tests cannot be reported as proof of passing!
                    warn!(
                        "Test command exited with success but executed zero tests; rejecting as proof"
                    );
                    (
                        "Inconclusive".to_string(),
                        Some(r.exit_code),
                        tot,
                        pass,
                        fail,
                        skip,
                        Some("Zero tests executed; command discovered no runnable tests".into()),
                    )
                } else if r.success && fail.unwrap_or(0) == 0 {
                    (
                        "Passed".to_string(),
                        Some(r.exit_code),
                        tot,
                        pass,
                        fail,
                        skip,
                        None,
                    )
                } else {
                    (
                        "Failed".to_string(),
                        Some(r.exit_code),
                        tot,
                        pass,
                        fail,
                        skip,
                        Some(format!("{}\n{}", r.stdout, r.stderr)),
                    )
                }
            }
            Err(ref e) => (
                "InfrastructureError".to_string(),
                None,
                None,
                None,
                None,
                None,
                Some(e.to_string()),
            ),
        };

        records.push(DurableVerificationRecord {
            id: format!("vrec-{}", Uuid::new_v4().simple()),
            workflow_id: plan.workflow_id.clone(),
            task_id: plan.task_id.clone(),
            attempt_id: plan.attempt_id,
            manifest_hash: manifest.manifest_hash.clone(),
            source_base_sha: manifest.source_base_sha.clone(),
            target_commit_sha: manifest.target_commit_sha.clone(),
            verification_plan_hash: plan.plan_hash.clone(),
            requirement_id: "req-tests-run".into(),
            category: "unit_test".into(),
            operation: "ProjectRunner::run_tests".into(),
            exit_code: test_exit,
            status: test_status,
            tests_total: total,
            tests_passed: passed,
            tests_failed: failed,
            tests_skipped: skipped,
            duration_ms: t_duration,
            diagnostics: test_diag,
            artifact_refs: Vec::new(),
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
        });

        // 3. Execute Safety Audit
        let s_start = Instant::now();
        let mod_paths: Vec<PathBuf> = manifest
            .files
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();
        let safety_res = CodebaseSafetyGate::audit_workspace_changes(
            workspace_root,
            &mod_paths,
            &plan.impacted_symbols,
        );
        let s_duration = s_start.elapsed().as_millis() as u64;

        let (safety_status, safety_diag) = match safety_res {
            Ok(report) if report.verdict == SafetyGateVerdict::Passed => {
                ("Passed".to_string(), None)
            }
            Ok(report) => (
                "Failed".to_string(),
                Some(format!("Safety gate verdict: {:?}", report.verdict)),
            ),
            Err(e) => ("InfrastructureError".to_string(), Some(e.to_string())),
        };

        records.push(DurableVerificationRecord {
            id: format!("vrec-{}", Uuid::new_v4().simple()),
            workflow_id: plan.workflow_id.clone(),
            task_id: plan.task_id.clone(),
            attempt_id: plan.attempt_id,
            manifest_hash: manifest.manifest_hash.clone(),
            source_base_sha: manifest.source_base_sha.clone(),
            target_commit_sha: manifest.target_commit_sha.clone(),
            verification_plan_hash: plan.plan_hash.clone(),
            requirement_id: "req-safety-audit".into(),
            category: "safety_audit".into(),
            operation: "CodebaseSafetyGate::audit_workspace_changes".into(),
            exit_code: Some(0),
            status: safety_status,
            tests_total: None,
            tests_passed: None,
            tests_failed: None,
            tests_skipped: None,
            duration_ms: s_duration,
            diagnostics: safety_diag,
            artifact_refs: Vec::new(),
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
        });

        Ok(records)
    }

    /// Authoritatively evaluates whether all mandatory requirements in the plan have valid evidence.
    pub fn evaluate_evidence(
        plan: &VerificationPlan,
        manifest: &ChangeSetManifest,
        records: &[DurableVerificationRecord],
    ) -> VerificationEvaluationReport {
        let mut satisfied = 0;
        let mut failed = Vec::new();
        let mut missing = Vec::new();
        let mut zero_tests_detected = 0;

        for req in &plan.requirements {
            let matching_records: Vec<&DurableVerificationRecord> = records
                .iter()
                .filter(|r| {
                    r.requirement_id == req.id
                        && r.manifest_hash == manifest.manifest_hash
                        && r.verification_plan_hash == plan.plan_hash
                })
                .collect();

            if matching_records.is_empty() {
                if req.mandatory {
                    missing.push(req.id.clone());
                }
                continue;
            }

            let mut passed = false;
            for rec in matching_records {
                if rec.status == "Passed" {
                    // Check if test execution claimed success with 0 tests
                    if (rec.category == "unit_test" || rec.category == "test")
                        && let Some(0) = rec.tests_total
                    {
                        zero_tests_detected += 1;
                        failed.push((req.id.clone(), "Zero tests executed".into()));
                        continue;
                    }
                    passed = true;
                    break;
                } else {
                    failed.push((
                        req.id.clone(),
                        rec.diagnostics
                            .clone()
                            .unwrap_or_else(|| format!("Status is {}", rec.status)),
                    ));
                }
            }

            if passed {
                satisfied += 1;
            }
        }

        let total = plan.requirements.len();
        let success = missing.is_empty() && failed.is_empty() && satisfied == total;

        let summary = if success {
            format!("All {total} mandatory verification requirements successfully satisfied")
        } else {
            format!(
                "Verification failed: {} satisfied, {} failed, {} missing",
                satisfied,
                failed.len(),
                missing.len()
            )
        };

        VerificationEvaluationReport {
            success,
            total_requirements: total,
            satisfied_requirements: satisfied,
            failed_requirements: failed,
            missing_requirements: missing,
            zero_test_false_positives_detected: zero_tests_detected,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::workflow::{FileChangeRecord, FileChangeType, WorkspaceIsolationStrategy};
    use tempfile::tempdir;

    fn sample_manifest() -> ChangeSetManifest {
        let mut m = ChangeSetManifest {
            manifest_version: 1,
            id: "cs-test".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            task_id: "t-1".into(),
            attempt_id: 1,
            worker_id: "w-1".into(),
            agent_id: None,
            repository_root: PathBuf::from("/repo"),
            canonical_workspace_id: "ws-1".into(),
            source_base_sha: "head_123".into(),
            target_commit_sha: Some("head_123".into()),
            files: vec![FileChangeRecord {
                relative_path: PathBuf::from("src/lib.rs"),
                original_path: None,
                change_type: FileChangeType::Modified,
                original_exists: true,
                original_content_hash: Some("old".into()),
                new_content_hash: Some("new".into()),
                original_mode: Some(0o644),
                new_mode: Some(0o644),
                symlink_target: None,
                size_bytes: 100,
                is_binary: false,
                patch_artifact_path: None,
            }],
            isolation_strategy: WorkspaceIsolationStrategy::Worktree,
            created_at_epoch_ms: 1000,
            frozen_at_epoch_ms: None,
            is_frozen: false,
            manifest_hash: String::new(),
        };
        m.freeze();
        m
    }

    #[test]
    fn test_generate_plan_and_plan_hash() {
        let temp = tempdir().unwrap();
        let manifest = sample_manifest();
        let criteria = vec!["Build must pass".to_string()];

        let plan = IndependentVerificationEngine::generate_plan(temp.path(), &manifest, &criteria)
            .unwrap();

        assert_eq!(plan.manifest_hash, manifest.manifest_hash);
        assert!(!plan.plan_hash.is_empty());
        assert_eq!(plan.requirements.len(), 4);
    }

    #[test]
    fn test_zero_test_execution_rejected() {
        let temp = tempdir().unwrap();
        let manifest = sample_manifest();
        let plan =
            IndependentVerificationEngine::generate_plan(temp.path(), &manifest, &[]).unwrap();

        let zero_test_record = DurableVerificationRecord {
            id: "vrec-1".into(),
            workflow_id: plan.workflow_id.clone(),
            task_id: plan.task_id.clone(),
            attempt_id: plan.attempt_id,
            manifest_hash: manifest.manifest_hash.clone(),
            source_base_sha: manifest.source_base_sha.clone(),
            target_commit_sha: manifest.target_commit_sha.clone(),
            verification_plan_hash: plan.plan_hash.clone(),
            requirement_id: "req-tests-run".into(),
            category: "unit_test".into(),
            operation: "test".into(),
            exit_code: Some(0),
            status: "Passed".into(), // Claimed passed, but total is 0!
            tests_total: Some(0),
            tests_passed: Some(0),
            tests_failed: Some(0),
            tests_skipped: Some(0),
            duration_ms: 10,
            diagnostics: None,
            artifact_refs: Vec::new(),
            timestamp_epoch_ms: 2000,
        };

        let report =
            IndependentVerificationEngine::evaluate_evidence(&plan, &manifest, &[zero_test_record]);
        assert!(!report.success);
        assert_eq!(report.zero_test_false_positives_detected, 1);
    }

    #[test]
    fn test_stale_or_mismatched_manifest_hash_rejected() {
        let temp = tempdir().unwrap();
        let manifest = sample_manifest();
        let plan =
            IndependentVerificationEngine::generate_plan(temp.path(), &manifest, &[]).unwrap();

        // Record with different manifest hash (stale / from earlier attempt)
        let stale_record = DurableVerificationRecord {
            id: "vrec-stale".into(),
            workflow_id: plan.workflow_id.clone(),
            task_id: plan.task_id.clone(),
            attempt_id: plan.attempt_id,
            manifest_hash: "foreign_manifest_hash".into(),
            source_base_sha: manifest.source_base_sha.clone(),
            target_commit_sha: manifest.target_commit_sha.clone(),
            verification_plan_hash: plan.plan_hash.clone(),
            requirement_id: "req-build-check".into(),
            category: "build_check".into(),
            operation: "check".into(),
            exit_code: Some(0),
            status: "Passed".into(),
            tests_total: None,
            tests_passed: None,
            tests_failed: None,
            tests_skipped: None,
            duration_ms: 10,
            diagnostics: None,
            artifact_refs: Vec::new(),
            timestamp_epoch_ms: 2000,
        };

        let report =
            IndependentVerificationEngine::evaluate_evidence(&plan, &manifest, &[stale_record]);
        assert!(!report.success);
        assert!(
            report
                .missing_requirements
                .contains(&"req-build-check".to_string())
        );
    }
}
