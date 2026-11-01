//! `m31a-verify`: Spec-driven verification, plan checking, and test execution engine.
//!
//! Implements 8-dimension pre-execution plan analysis, adversarial goal-backward verification,
//! automated test suite execution, and targeted self-healing prompt generation.

pub mod benchmark;
pub mod deployment;
pub mod diagnostics;
pub mod goal_backward;
pub mod iac;
pub mod integrity;
pub mod nyquist;
pub mod plan_checker;
pub mod regression;
pub mod root_cause;
pub mod runners;
pub mod safety;
pub mod scope;
pub mod self_heal;
pub mod verification_timeline;
pub mod visual_tdd;

pub use benchmark::{
    AutonomousEngineeringBenchmark, AutonomousEngineeringBenchmarkReport, BenchmarkMetric,
    BenchmarkReport, FlamegraphGenerator, PerformanceProfiler, PerformanceVerdict, ProfileReport,
    SamplingProfiler, StackSample, TaskEvaluationInput,
};
pub use deployment::{
    DeploymentReadinessGate, DeploymentReadinessReport, DeploymentVerdict, GateCheck,
};
pub use diagnostics::{Diagnostic, DiagnosticParser, DiagnosticSeverity};
pub use goal_backward::{
    ArtifactCheck, AutomatedTestCheck, GoalBackwardReport, GoalBackwardVerifier, TruthStatus,
    TruthVerification, VerificationOutcome,
};
pub use iac::{IacAuditReport, IacIssue, IacSeverity, IacValidator};
pub use integrity::{
    IntegrityVerdict, MutationResult, MutationStatus, TestFlaw, TestFlawKind, TestIntegrityAuditor,
    TestIntegrityReport,
};
pub use nyquist::{
    NYQUIST_SENTINEL_PREFIX, NyquistCheckStatus, NyquistEngine, NyquistItemResult, NyquistReport,
    NyquistRequirement, NyquistVerdict,
};
pub use plan_checker::{
    IssueSeverity, PlanCheckReport, PlanChecker, PlanIssue, PlanModel, TaskModel,
};
pub use regression::{
    RegressionAuditReport, RegressionAuditStatus, RegressionGuard, RegressionTestSynthesizer,
};
pub use root_cause::{
    CausalDiagnosticGraph, CausalDiagnosticNode, PatchClassification, PatchIntegrityReport,
    RootCauseAnalyzer, SuperficialPatchKind,
};
pub use runners::{ProjectKind, ProjectRunner, RawRunResult};
pub use scope::{EvaluationScope, ScopeCommit, ScopeOptions, ScopeSource, ScopeStatus, ScopeUnit};
pub use self_heal::{SelfHealingGenerator, VerificationReport};
pub use verification_timeline::{
    AgentCompletionReport, AgentPlan, CompletionOutcome, CriterionEvidence, EvidenceLedger,
    EvidenceRecord, EvidenceType, PlanCriterion, VerificationStatus, VerificationTimeline,
};

pub use safety::{CodebaseSafetyGate, SafetyAuditReport, SafetyGateVerdict};
pub use visual_tdd::{
    AssertionOutcome, UiActionStep, UiAssertion, VisualTddReport, VisualTddRunner,
    VisualTddScenario,
};

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub struct AutonomousVerifier;

impl AutonomousVerifier {
    /// Runs compiler / fast typecheck and returns verification report with self-healing prompt if failed.
    pub async fn verify_check(cwd: &Path, timeout: Duration) -> Result<VerificationReport> {
        let raw = ProjectRunner::run_check(cwd, timeout).await?;
        let combined = format!("{}\n{}", raw.stdout, raw.stderr);
        let diags = DiagnosticParser::parse(&combined);
        Ok(SelfHealingGenerator::evaluate(raw, diags))
    }

    /// Runs test suite and returns verification report with self-healing prompt if failed.
    pub async fn verify_tests(cwd: &Path, timeout: Duration) -> Result<VerificationReport> {
        let raw = ProjectRunner::run_tests(cwd, timeout).await?;
        let combined = format!("{}\n{}", raw.stdout, raw.stderr);
        let diags = DiagnosticParser::parse(&combined);
        Ok(SelfHealingGenerator::evaluate(raw, diags))
    }

    /// Audits codebase safety, data-flow taints, and semantic blast radius.
    pub fn verify_safety(
        workspace_root: &Path,
        modified_files: &[PathBuf],
        candidate_symbols: &[String],
    ) -> Result<SafetyAuditReport> {
        CodebaseSafetyGate::audit_workspace_changes(
            workspace_root,
            modified_files,
            candidate_symbols,
        )
    }

    /// Executes an autonomous Visual TDD scenario against a web frontend.
    pub async fn verify_visual_state(
        scenario: &VisualTddScenario,
        session_opt: Option<Arc<m31a_browser::BrowserSession>>,
        artifact_dir: Option<&Path>,
    ) -> Result<VisualTddReport> {
        VisualTddRunner::run_scenario(scenario, session_opt, artifact_dir).await
    }
}
