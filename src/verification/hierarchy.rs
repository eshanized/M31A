//! 7-Tier Verification Hierarchy Engine (VER-01, D-01).
//!
//! Evaluates the verification hierarchy as a gated dependency graph:
//! Tier 1 (Deterministic) -> Tier 2 (Compiler) -> Tier 3 (Tests) / Tier 4 (Static Analysis)
//! -> Tier 5 (Diff/Invariants) -> Tier 6 (Independent Review) -> Tier 7 (Model Diagnosis).
//!
//! Enforces fail-fast gating: failure in an earlier tier (e.g. Tier 2 Compiler) immediately
//! blocks downstream dependent tiers (Tiers 3, 4, 6) from executing, eliminating token and time waste.

use std::path::Path;
use std::sync::Arc;

use crate::ids::{MissionId, TaskId};
use crate::verification::runners::{
    CompilerRunner, DeterministicRunner, DiffInvariantsRunner, StaticAnalysisRunner, TestRunner,
    VerificationRunner,
};
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

pub struct VerificationHierarchyEngine {
    pub tier1_deterministic: Arc<dyn VerificationRunner>,
    pub tier2_compiler: Arc<dyn VerificationRunner>,
    pub tier3_tests: Arc<dyn VerificationRunner>,
    pub tier4_static_analysis: Arc<dyn VerificationRunner>,
    pub tier5_diff_invariants: Arc<dyn VerificationRunner>,
    pub tier6_reviewer: Option<Arc<dyn VerificationRunner>>,
}

impl Default for VerificationHierarchyEngine {
    fn default() -> Self {
        Self::production()
    }
}

impl VerificationHierarchyEngine {
    pub fn production() -> Self {
        Self {
            tier1_deterministic: Arc::new(DeterministicRunner::new()),
            tier2_compiler: Arc::new(CompilerRunner::new()),
            tier3_tests: Arc::new(TestRunner::new()),
            tier4_static_analysis: Arc::new(StaticAnalysisRunner::new()),
            tier5_diff_invariants: Arc::new(DiffInvariantsRunner::new()),
            tier6_reviewer: None,
        }
    }

    /// Construct verification hierarchy tailored for a workspace using ProjectAdapter.
    pub fn for_workspace(
        workspace_root: &Path,
        verification_config: Option<&crate::config::WorkspaceVerificationConfig>,
        project_type_override: Option<&str>,
    ) -> Self {
        let adapter = crate::verification::adapter::ProjectAdapter::detect(
            workspace_root,
            verification_config,
            project_type_override,
        );

        Self {
            tier1_deterministic: Arc::new(DeterministicRunner::with_manifest_and_source(
                adapter.manifest_file,
                adapter.source_dir,
            )),
            tier2_compiler: Arc::new(CompilerRunner::with_command(adapter.compiler_command)),
            tier3_tests: Arc::new(TestRunner::new().with_command(adapter.test_command)),
            tier4_static_analysis: Arc::new(StaticAnalysisRunner::with_command(
                adapter.linter_command,
            )),
            tier5_diff_invariants: Arc::new(DiffInvariantsRunner::new()),
            tier6_reviewer: None,
        }
    }

    /// Construct verification hierarchy tailored for a workspace from authoritative ResolvedConfiguration.
    pub fn for_workspace_with_config(
        workspace_root: &Path,
        config: &crate::config::ResolvedConfiguration,
    ) -> Self {
        Self::for_workspace(
            workspace_root,
            config.app_config.workspace.verification.as_ref(),
            config.app_config.workspace.project_type.as_deref(),
        )
    }

    pub fn new(
        tier1: Arc<dyn VerificationRunner>,
        tier2: Arc<dyn VerificationRunner>,
        tier3: Arc<dyn VerificationRunner>,
        tier4: Arc<dyn VerificationRunner>,
        tier5: Arc<dyn VerificationRunner>,
        tier6: Option<Arc<dyn VerificationRunner>>,
    ) -> Self {
        Self {
            tier1_deterministic: tier1,
            tier2_compiler: tier2,
            tier3_tests: tier3,
            tier4_static_analysis: tier4,
            tier5_diff_invariants: tier5,
            tier6_reviewer: tier6,
        }
    }

    /// Evaluates the 7-tier verification hierarchy enforcing strict fail-fast gating (D-01).
    pub async fn execute_hierarchy(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
        mandate_reviewer: bool,
    ) -> Vec<VerificationCheck> {
        let mut checks = Vec::new();

        // Tier 1: Deterministic Checks
        let t1_res = match self
            .tier1_deterministic
            .execute(mission_id, task_id, workspace_root, snapshot_hash)
            .await
        {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "tier1_runner",
                "",
                None,
                e,
                Some("Environment".into()),
                snapshot_hash,
            ),
        };
        let t1_passed = t1_res.status == CheckStatus::Passed;
        checks.push(t1_res);

        if !t1_passed {
            // Tier 1 failure blocks all downstream tiers (D-01)
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::Compiler,
                snapshot_hash,
                "Prerequisite Tier 1 deterministic checks failed",
            ));
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::Tests,
                snapshot_hash,
                "Prerequisite Tier 1 deterministic checks failed",
            ));
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::StaticAnalysis,
                snapshot_hash,
                "Prerequisite Tier 1 deterministic checks failed",
            ));
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::DiffInvariants,
                snapshot_hash,
                "Prerequisite Tier 1 deterministic checks failed",
            ));
            if mandate_reviewer {
                checks.push(VerificationCheck::blocked(
                    mission_id,
                    task_id,
                    CheckTier::IndependentReview,
                    snapshot_hash,
                    "Prerequisite Tier 1 deterministic checks failed",
                ));
            }
            return checks;
        }

        // Tier 2: Compiler & Type Checker
        let t2_res = match self
            .tier2_compiler
            .execute(mission_id, task_id, workspace_root, snapshot_hash)
            .await
        {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Compiler,
                "tier2_compiler",
                "",
                None,
                e,
                Some("Compilation".into()),
                snapshot_hash,
            ),
        };
        let t2_passed = t2_res.status == CheckStatus::Passed;
        checks.push(t2_res);

        if !t2_passed {
            // Tier 2 compiler failure blocks tests, static analysis, diff invariants, and model review (D-01)
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::Tests,
                snapshot_hash,
                "Compiler diagnostics failed",
            ));
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::StaticAnalysis,
                snapshot_hash,
                "Compiler diagnostics failed",
            ));
            checks.push(VerificationCheck::blocked(
                mission_id,
                task_id,
                CheckTier::DiffInvariants,
                snapshot_hash,
                "Compiler diagnostics failed",
            ));
            if mandate_reviewer {
                checks.push(VerificationCheck::blocked(
                    mission_id,
                    task_id,
                    CheckTier::IndependentReview,
                    snapshot_hash,
                    "Compiler diagnostics failed",
                ));
            }
            return checks;
        }

        // Tier 3: Tests and Tier 4: Static Analysis run concurrently
        let (t3_res_raw, t4_res_raw) = tokio::join!(
            self.tier3_tests
                .execute(mission_id, task_id, workspace_root, snapshot_hash),
            self.tier4_static_analysis
                .execute(mission_id, task_id, workspace_root, snapshot_hash)
        );

        let t3_res = match t3_res_raw {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Tests,
                "tier3_tests",
                "",
                None,
                e,
                Some("Test".into()),
                snapshot_hash,
            ),
        };
        let t3_passed = t3_res.status == CheckStatus::Passed;
        checks.push(t3_res);

        let t4_res = match t4_res_raw {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::StaticAnalysis,
                "tier4_static_analysis",
                "",
                None,
                e,
                Some("Compilation".into()),
                snapshot_hash,
            ),
        };
        checks.push(t4_res);

        // Tier 5: Diff & Invariants
        let t5_res = match self
            .tier5_diff_invariants
            .execute(mission_id, task_id, workspace_root, snapshot_hash)
            .await
        {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::DiffInvariants,
                "tier5_diff",
                "",
                None,
                e,
                Some("RepositoryState".into()),
                snapshot_hash,
            ),
        };
        checks.push(t5_res);

        // Tier 6: Independent Model Review (Risk- & Gate-Driven)
        if mandate_reviewer {
            if !t3_passed {
                // If tests failed, block independent review to save model tokens (D-01)
                checks.push(VerificationCheck::blocked(
                    mission_id,
                    task_id,
                    CheckTier::IndependentReview,
                    snapshot_hash,
                    "Prerequisite automated test suites failed",
                ));
            } else if let Some(ref reviewer) = self.tier6_reviewer {
                let t6_res = match reviewer
                    .execute(mission_id, task_id, workspace_root, snapshot_hash)
                    .await
                {
                    Ok(c) => c,
                    Err(e) => VerificationCheck::failed(
                        mission_id,
                        task_id,
                        CheckTier::IndependentReview,
                        "tier6_reviewer",
                        "",
                        None,
                        e,
                        Some("Model".into()),
                        snapshot_hash,
                    ),
                };
                checks.push(t6_res);
            } else {
                checks.push(VerificationCheck::skipped(
                    mission_id,
                    task_id,
                    CheckTier::IndependentReview,
                    snapshot_hash,
                    "Reviewer mandated but no reviewer runner configured",
                ));
            }
        } else {
            checks.push(VerificationCheck::skipped(
                mission_id,
                task_id,
                CheckTier::IndependentReview,
                snapshot_hash,
                "Independent review not mandated for this task scope",
            ));
        }

        checks
    }

    /// Evaluates task-scoped verification checks based on the task role and contract.
    pub async fn execute_hierarchy_for_task(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
        role: &str,
        verification_type: &str,
    ) -> Vec<VerificationCheck> {
        // 1. Read-only roles produce a clean read check. Eligibility comes
        // from the role registry; the string comparison is a defensive
        // fallback for ids that predate registration, never a second table.
        //
        // Exception: if the verification type explicitly declares required
        // artifact paths, those must be enforced even for read-only roles.
        // A pure evidence-gathering read-only step has empty paths and passes
        // the shortcut. A step whose quality_gate.required_artifacts is
        // non-empty (compiled into a non-empty ArtifactInspection) must
        // produce those files regardless of read-only status.
        let parsed_role = crate::state_machine::agent::AgentRole::new(role);
        let is_read_only = crate::agent::registry::RoleRegistry::global()
            .read()
            .ok()
            .and_then(|guard| guard.read_only_flag(&parsed_role))
            .unwrap_or(false);

        // Parse the verification strategy early — needed by both read-only
        // shortcut logic and the mutating-role artifact check below.
        let parsed_strategy =
            serde_json::from_str::<crate::kernel::plan::VerificationStrategy>(verification_type)
                .ok();

        // Collect required paths from the strategy (ArtifactInspection or Composite).
        let required_paths: Vec<String> = {
            let mut paths = Vec::new();
            if let Some(ref strategy) = parsed_strategy {
                match strategy {
                    crate::kernel::plan::VerificationStrategy::ArtifactInspection { paths: p } => {
                        paths.extend(p.clone())
                    }
                    crate::kernel::plan::VerificationStrategy::Composite { strategies } => {
                        for s in strategies {
                            if let crate::kernel::plan::VerificationStrategy::ArtifactInspection {
                                paths: p,
                            } = s
                            {
                                paths.extend(p.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            paths
        };

        if is_read_only && required_paths.is_empty() {
            return vec![VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "read_only_task_gate",
                "",
                None,
                "Read-only task completed without mutating workspace",
                snapshot_hash,
            )];
        }

        // 2. Enforce required artifact paths (applies to mutating roles and
        //    read-only roles whose quality gate explicitly declares paths).
        for path in &required_paths {
            let full_path = workspace_root.join(path);
            if !full_path.exists() {
                return vec![VerificationCheck::failed(
                    mission_id,
                    task_id,
                    CheckTier::Deterministic,
                    "artifact_inspection",
                    path,
                    None,
                    format!("Required artifact not found on disk: {}", path),
                    Some("ArtifactNotFound".into()),
                    snapshot_hash,
                )];
            }
        }

        // If the task is read-only and all required paths exist, read-only
        // tasks do not perform workspace compilation.
        if is_read_only {
            return vec![VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "read_only_task_gate",
                "",
                None,
                "Read-only task artifacts verified on disk",
                snapshot_hash,
            )];
        }

        // Determine if this task strategy requires build/compilation tiers
        // (Tier 1 & Tier 2). Non-compilation strategies (such as pure
        // ArtifactInspection or ReviewGate) verify their artifacts or gates
        // without running cargo manifest / compiler checks on the workspace.
        let requires_compilation = match &parsed_strategy {
            Some(crate::kernel::plan::VerificationStrategy::ArtifactInspection { .. }) => false,
            Some(crate::kernel::plan::VerificationStrategy::ReviewGate { .. }) => false,
            Some(crate::kernel::plan::VerificationStrategy::Composite { strategies }) => {
                strategies.iter().any(|s| {
                    matches!(
                        s,
                        crate::kernel::plan::VerificationStrategy::Compilation
                            | crate::kernel::plan::VerificationStrategy::AutomatedTest { .. }
                            | crate::kernel::plan::VerificationStrategy::StaticAnalysis { .. }
                    )
                })
            }
            _ => true,
        };

        if !requires_compilation {
            return vec![VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "artifact_inspection",
                "",
                None,
                "All required artifacts verified on disk",
                snapshot_hash,
            )];
        }

        let mut checks = Vec::new();

        // Tier 1: Deterministic Checks
        let t1_res = match self
            .tier1_deterministic
            .execute(mission_id, task_id, workspace_root, snapshot_hash)
            .await
        {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Deterministic,
                "tier1_runner",
                "",
                None,
                e,
                Some("Environment".into()),
                snapshot_hash,
            ),
        };
        let t1_passed = t1_res.status == CheckStatus::Passed;
        checks.push(t1_res);
        if !t1_passed {
            return checks;
        }

        // Tier 2: Compiler & Type Checker
        let t2_res = match self
            .tier2_compiler
            .execute(mission_id, task_id, workspace_root, snapshot_hash)
            .await
        {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Compiler,
                "tier2_compiler",
                "",
                None,
                e,
                Some("Compilation".into()),
                snapshot_hash,
            ),
        };
        let t2_passed = t2_res.status == CheckStatus::Passed;
        checks.push(t2_res);
        if !t2_passed {
            return checks;
        }

        let requires_tests = parsed_role == crate::state_machine::agent::AgentRole::verifier()
            || verification_type.contains("\"test\"")
            || verification_type.contains("\"type\":\"test");
        // A task-declared test command overrides the runner default:
        // the strategy's evidence is the command's actual output,
        // executed workspace-scoped with the runner's timeouts.
        // No shell is involved (direct exec); privilege is task-equivalent.
        let test_command_override: Option<String> =
            serde_json::from_str::<crate::kernel::plan::VerificationStrategy>(verification_type)
                .ok()
                .and_then(|strategy| match strategy {
                    crate::kernel::plan::VerificationStrategy::AutomatedTest { command } => command,
                    _ => None,
                });
        let requires_tests = requires_tests || test_command_override.is_some();
        if !requires_tests {
            return checks;
        }

        // Tier 3: Tests. A task-declared command runs through a
        // command-overridden runner; otherwise the configured default runs.
        // Both execute workspace-scoped with runner timeouts and produce
        // evidence-backed checks (never assumed success).
        let t3_result: Result<VerificationCheck, String> =
            if let Some(ref cmd) = test_command_override {
                TestRunner::new()
                    .with_command(cmd.clone())
                    .execute(mission_id, task_id, workspace_root, snapshot_hash)
                    .await
            } else {
                self.tier3_tests
                    .execute(mission_id, task_id, workspace_root, snapshot_hash)
                    .await
            };
        let t3_res = match t3_result {
            Ok(c) => c,
            Err(e) => VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Tests,
                "tier3_tests",
                "",
                None,
                e,
                Some("Test".into()),
                snapshot_hash,
            ),
        };
        checks.push(t3_res);

        checks
    }
}
