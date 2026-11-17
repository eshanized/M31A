//! Specialized Maintenance Engineering Playbooks & Strategies (`m31a-maintenance::planner::strategies`).
//!
//! Enforces:
//! 1. Infrastructure failures are never classified as code defects; no application code is modified for runner outages.
//! 2. Regression tests are planned and verified before implementation fixes.
//! 3. Major dependency migrations require explicit approval and risk controls.
//! 4. Unreproducible bugs are marked `NeedsInformation`; no hallucinated fixes.
//! 5. Documentation updates are grounded in real AST symbols.

use std::path::{Path, PathBuf};
use anyhow::Result;

use crate::domain::{
    AutonomyTier, MaintenanceCase, MaintenanceCategory, MaintenancePlan,
    MaintenanceRiskLevel, PlanVerificationRequirement, RootCauseHypothesis, SignalSeverity,
};

/// Classification of CI or build failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiFailureClassification {
    SourceCompilation,
    TestRegression,
    LintFormatting,
    DependencyResolution,
    BuildConfig,
    CiConfig,
    RunnerInfrastructure,
    ExternalService,
    FlakyTest,
    Unknown,
}

impl CiFailureClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SourceCompilation => "source_compilation_failure",
            Self::TestRegression => "test_regression",
            Self::LintFormatting => "lint_formatting_failure",
            Self::DependencyResolution => "dependency_resolution_failure",
            Self::BuildConfig => "build_configuration_failure",
            Self::CiConfig => "ci_configuration_failure",
            Self::RunnerInfrastructure => "runner_infrastructure_outage",
            Self::ExternalService => "external_service_outage",
            Self::FlakyTest => "flaky_test",
            Self::Unknown => "unknown_failure",
        }
    }

    pub fn is_infrastructure(&self) -> bool {
        matches!(self, Self::RunnerInfrastructure | Self::ExternalService)
    }
}

/// Context provided to a maintenance strategy for planning.
#[derive(Debug, Clone)]
pub struct StrategyPlanContext<'a> {
    pub workspace_root: &'a Path,
    pub baseline_sha: &'a str,
    pub repo_id: &'a str,
}

/// Maintenance strategy abstraction.
pub trait MaintenanceStrategy: Send + Sync {
    fn category(&self) -> MaintenanceCategory;
    fn can_handle(&self, case: &MaintenanceCase) -> bool;
    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan>;
}

// -------------------------------------------------------------------------------------------------
// 1. BROKEN CI STRATEGY
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct BrokenCiStrategy;

impl BrokenCiStrategy {
    /// Classifies failure text or logs to avoid touching application code on runner outages.
    pub fn classify_failure(raw_output: &str) -> CiFailureClassification {
        let lower = raw_output.to_lowercase();
        if lower.contains("runner failed")
            || lower.contains("lost communication with runner")
            || lower.contains("no space left on device")
            || lower.contains("out of memory: kill process")
            || lower.contains("system error: connection reset by peer")
        {
            CiFailureClassification::RunnerInfrastructure
        } else if lower.contains("502 bad gateway")
            || lower.contains("503 service unavailable")
            || lower.contains("rate limit exceeded")
            || lower.contains("timeout connecting to registry")
        {
            CiFailureClassification::ExternalService
        } else if lower.contains("could not compile")
            || lower.contains("error[e")
            || lower.contains("syntaxerror")
            || lower.contains("compilation error")
        {
            CiFailureClassification::SourceCompilation
        } else if lower.contains("test result: failed")
            || lower.contains("assertion failed")
            || lower.contains("failed test")
            || lower.contains("failures:")
        {
            CiFailureClassification::TestRegression
        } else if lower.contains("clippy::")
            || lower.contains("eslint")
            || lower.contains("formatting error")
            || lower.contains("rustfmt")
        {
            CiFailureClassification::LintFormatting
        } else {
            CiFailureClassification::Unknown
        }
    }
}

impl MaintenanceStrategy for BrokenCiStrategy {
    fn category(&self) -> MaintenanceCategory {
        MaintenanceCategory::BrokenCi
    }

    fn can_handle(&self, case: &MaintenanceCase) -> bool {
        case.category == MaintenanceCategory::BrokenCi
    }

    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan> {
        let classification = Self::classify_failure(&case.problem_statement);

        // Invariant: Do not modify application code on infrastructure failure!
        if classification.is_infrastructure() {
            let mut plan = MaintenancePlan::new(
                &case.id,
                ctx.repo_id,
                ctx.baseline_sha,
                MaintenanceCategory::BrokenCi,
                classification.as_str(),
                "infra_cooldown_retry",
                MaintenanceRiskLevel::Low,
                AutonomyTier::Tier0Observe,
                false,
            );
            plan.hypotheses.push(RootCauseHypothesis {
                id: "hyp_infra_1".to_string(),
                description: format!(
                    "Failure is attributed to transient infrastructure outage ({}). No code patch required.",
                    classification.as_str()
                ),
                confidence: 0.95,
                evidence_refs: case.evidence_refs.clone(),
                verified: true,
            });
            return Ok(plan);
        }

        // Code defect remediation plan
        let mut plan = MaintenancePlan::new(
            &case.id,
            ctx.repo_id,
            ctx.baseline_sha,
            MaintenanceCategory::BrokenCi,
            classification.as_str(),
            "minimal_source_correction",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );

        plan.hypotheses.push(RootCauseHypothesis {
            id: "hyp_ci_1".to_string(),
            description: format!("Deterministic failure classified as {}", classification.as_str()),
            confidence: 0.85,
            evidence_refs: case.evidence_refs.clone(),
            verified: false,
        });

        // Verification requirements
        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_check".to_string(),
            category: "build_check".to_string(),
            description: "Compiler and workspace build check must succeed with 0 diagnostics".to_string(),
            target_paths: case.affected_files.clone(),
            mandatory: true,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_tests".to_string(),
            category: "unit_test".to_string(),
            description: "Failing test suite must pass with 0 failures and non-zero executed tests".to_string(),
            target_paths: case.affected_files.clone(),
            mandatory: true,
        });

        plan.expected_files = case.affected_files.clone();
        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }
}

// -------------------------------------------------------------------------------------------------
// 2. DEPENDENCY UPDATE STRATEGY
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct DependencyUpdateStrategy;

impl MaintenanceStrategy for DependencyUpdateStrategy {
    fn category(&self) -> MaintenanceCategory {
        MaintenanceCategory::DependencyUpdate
    }

    fn can_handle(&self, case: &MaintenanceCase) -> bool {
        case.category == MaintenanceCategory::DependencyUpdate
    }

    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan> {
        let is_major = case.problem_statement.contains("major") || case.title.contains("major");
        let (risk, autonomy_tier, requires_approval, strategy_name) = if is_major {
            (
                MaintenanceRiskLevel::High,
                AutonomyTier::Tier3RestrictedHighImpact,
                true,
                "major_version_migration",
            )
        } else {
            (
                MaintenanceRiskLevel::Low,
                AutonomyTier::Tier2GuardedRemediation,
                false,
                "semver_compatible_bump",
            )
        };

        let mut plan = MaintenancePlan::new(
            &case.id,
            ctx.repo_id,
            ctx.baseline_sha,
            MaintenanceCategory::DependencyUpdate,
            if is_major { "major_upgrade" } else { "patch_minor_upgrade" },
            strategy_name,
            risk,
            autonomy_tier,
            requires_approval,
        );

        plan.hypotheses.push(RootCauseHypothesis {
            id: "hyp_dep_1".to_string(),
            description: format!("Dependency update requested with risk {:?}", risk),
            confidence: 0.9,
            evidence_refs: case.evidence_refs.clone(),
            verified: true,
        });

        // Verification requirements
        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_lockfile".to_string(),
            category: "build_check".to_string(),
            description: "Manifest and lockfile must resolve consistently without conflicts".to_string(),
            target_paths: vec![PathBuf::from("Cargo.lock"), PathBuf::from("Cargo.toml")],
            mandatory: true,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_test_suite".to_string(),
            category: "unit_test".to_string(),
            description: "Targeted and dependent package tests must pass against updated dependency".to_string(),
            target_paths: Vec::new(),
            mandatory: true,
        });

        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }
}

// -------------------------------------------------------------------------------------------------
// 3. ISSUE BUG FIX STRATEGY
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct IssueBugFixStrategy;

impl MaintenanceStrategy for IssueBugFixStrategy {
    fn category(&self) -> MaintenanceCategory {
        MaintenanceCategory::IssueBug
    }

    fn can_handle(&self, case: &MaintenanceCase) -> bool {
        case.category == MaintenanceCategory::IssueBug
    }

    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan> {
        let has_repro_hints = case.problem_statement.contains("reproduce")
            || case.problem_statement.contains("steps")
            || case.problem_statement.contains("error:")
            || case.problem_statement.contains("panic");

        // If bug is unreproducible or lacks critical details, do not hallucinate fixes!
        if !has_repro_hints {
            let mut plan = MaintenancePlan::new(
                &case.id,
                ctx.repo_id,
                ctx.baseline_sha,
                MaintenanceCategory::IssueBug,
                "unreproducible_bug",
                "request_clarification",
                MaintenanceRiskLevel::Low,
                AutonomyTier::Tier0Observe,
                false,
            );
            plan.hypotheses.push(RootCauseHypothesis {
                id: "hyp_missing_info".to_string(),
                description: "Report lacks reproducible steps, error logs, or environment details. Human clarification required."
                    .to_string(),
                confidence: 0.95,
                evidence_refs: case.evidence_refs.clone(),
                verified: true,
            });
            return Ok(plan);
        }

        // Reproducible bug fix with regression-test-first workflow
        let mut plan = MaintenancePlan::new(
            &case.id,
            ctx.repo_id,
            ctx.baseline_sha,
            MaintenanceCategory::IssueBug,
            "reproducible_defect",
            "regression_test_driven_fix",
            MaintenanceRiskLevel::Medium,
            AutonomyTier::Tier1Prepare,
            false,
        );

        plan.hypotheses.push(RootCauseHypothesis {
            id: "hyp_bug_1".to_string(),
            description: "Identified functional regression in application logic".to_string(),
            confidence: 0.8,
            evidence_refs: case.evidence_refs.clone(),
            verified: false,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_regression_test".to_string(),
            category: "unit_test".to_string(),
            description: "Newly synthesized regression test must fail on baseline and pass after fix".to_string(),
            target_paths: Vec::new(),
            mandatory: true,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_suite".to_string(),
            category: "unit_test".to_string(),
            description: "Existing test suite must pass without regressions".to_string(),
            target_paths: Vec::new(),
            mandatory: true,
        });

        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }
}

// -------------------------------------------------------------------------------------------------
// 4. DOCUMENTATION DRIFT STRATEGY
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct DocDriftStrategy;

impl MaintenanceStrategy for DocDriftStrategy {
    fn category(&self) -> MaintenanceCategory {
        MaintenanceCategory::DocDrift
    }

    fn can_handle(&self, case: &MaintenanceCase) -> bool {
        case.category == MaintenanceCategory::DocDrift
    }

    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan> {
        let mut plan = MaintenancePlan::new(
            &case.id,
            ctx.repo_id,
            ctx.baseline_sha,
            MaintenanceCategory::DocDrift,
            "documentation_drift",
            "prose_doc_synchronization",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier1Prepare,
            false,
        );

        plan.hypotheses.push(RootCauseHypothesis {
            id: "hyp_doc_1".to_string(),
            description: "Documentation references stale symbols or missing public signatures".to_string(),
            confidence: 0.9,
            evidence_refs: case.evidence_refs.clone(),
            verified: true,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_doc_sync".to_string(),
            category: "custom_criterion".to_string(),
            description: "All referenced code blocks, command arguments, and API symbols must resolve against real AST".to_string(),
            target_paths: vec![PathBuf::from("README.md"), PathBuf::from("docs/")],
            mandatory: true,
        });

        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }
}

// -------------------------------------------------------------------------------------------------
// 5. SECURITY ADVISORY STRATEGY
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct SecurityAdvisoryStrategy;

impl MaintenanceStrategy for SecurityAdvisoryStrategy {
    fn category(&self) -> MaintenanceCategory {
        MaintenanceCategory::SecurityAdvisory
    }

    fn can_handle(&self, case: &MaintenanceCase) -> bool {
        case.category == MaintenanceCategory::SecurityAdvisory
    }

    fn create_plan(&self, case: &MaintenanceCase, ctx: &StrategyPlanContext) -> Result<MaintenancePlan> {
        let is_critical = case.severity == SignalSeverity::Critical;
        let mut plan = MaintenancePlan::new(
            &case.id,
            ctx.repo_id,
            ctx.baseline_sha,
            MaintenanceCategory::SecurityAdvisory,
            "vulnerability_remediation",
            "isolated_security_patch",
            if is_critical {
                MaintenanceRiskLevel::Critical
            } else {
                MaintenanceRiskLevel::High
            },
            AutonomyTier::Tier3RestrictedHighImpact,
            true, // Always require approval for security vulnerability deliverables
        );

        plan.hypotheses.push(RootCauseHypothesis {
            id: "hyp_sec_1".to_string(),
            description: "Preserving advisory provenance and isolating remediation without public disclosure leakage".to_string(),
            confidence: 0.95,
            evidence_refs: case.evidence_refs.clone(),
            verified: true,
        });

        plan.verification_requirements.push(PlanVerificationRequirement {
            id: "req_security_audit".to_string(),
            category: "safety_audit".to_string(),
            description: "Safety gate audit must pass and verify affected vulnerability is mitigated".to_string(),
            target_paths: Vec::new(),
            mandatory: true,
        });

        plan.plan_hash = plan.compute_plan_hash();
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broken_ci_infra_outage_does_not_patch_code() {
        let strategy = BrokenCiStrategy;
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            "Runner failed during test run",
            "Runner failed: Lost communication with runner. System error: connection reset by peer",
            "ci_runner_fail",
            SignalSeverity::High,
        );

        let ctx = StrategyPlanContext {
            workspace_root: Path::new("/dummy"),
            baseline_sha: "sha_xyz",
            repo_id: "repo_1",
        };

        let plan = strategy.create_plan(&case, &ctx).unwrap();
        assert_eq!(plan.selected_strategy, "infra_cooldown_retry");
        assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier0Observe);
        assert!(plan.expected_files.is_empty());
    }

    #[test]
    fn test_unreproducible_bug_requests_clarification() {
        let strategy = IssueBugFixStrategy;
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::IssueBug,
            "Vague bug report",
            "Application doesn't work for me, please fix it ASAP",
            "issue_vague",
            SignalSeverity::Medium,
        );

        let ctx = StrategyPlanContext {
            workspace_root: Path::new("/dummy"),
            baseline_sha: "sha_xyz",
            repo_id: "repo_1",
        };

        let plan = strategy.create_plan(&case, &ctx).unwrap();
        assert_eq!(plan.selected_strategy, "request_clarification");
        assert_eq!(plan.problem_classification, "unreproducible_bug");
    }

    #[test]
    fn test_major_dependency_upgrade_requires_approval() {
        let strategy = DependencyUpdateStrategy;
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::DependencyUpdate,
            "Major upgrade for tokio",
            "Bump tokio from 1.0 to 2.0 (major version breaking change)",
            "dep_tokio_major",
            SignalSeverity::High,
        );

        let ctx = StrategyPlanContext {
            workspace_root: Path::new("/dummy"),
            baseline_sha: "sha_xyz",
            repo_id: "repo_1",
        };

        let plan = strategy.create_plan(&case, &ctx).unwrap();
        assert!(plan.requires_approval);
        assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier3RestrictedHighImpact);
    }
}
