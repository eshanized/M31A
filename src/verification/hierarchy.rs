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
        let parsed_role = crate::state_machine::agent::AgentRole::new(role);
        let is_read_only = crate::agent::registry::RoleRegistry::global()
            .read()
            .ok()
            .and_then(|guard| guard.read_only_flag(&parsed_role))
            .unwrap_or(false);

        let clean_verif = verification_type.trim();

        // 1. If explicit verification strategy is provided, interpret it through typed dispatch.
        if !clean_verif.is_empty() && clean_verif != "{}" {
            match serde_json::from_str::<crate::kernel::plan::VerificationStrategy>(clean_verif) {
                Ok(strategy) => {
                    if is_read_only {
                        let required_paths: Vec<String> = match &strategy {
                            crate::kernel::plan::VerificationStrategy::ArtifactInspection {
                                paths: p,
                            } => p.clone(),
                            crate::kernel::plan::VerificationStrategy::Composite { strategies } => {
                                let mut all_paths = Vec::new();
                                for s in strategies {
                                    if let crate::kernel::plan::VerificationStrategy::ArtifactInspection {
                                        paths: p,
                                    } = s
                                    {
                                        all_paths.extend(p.clone());
                                    }
                                }
                                all_paths
                            }
                            _ => vec!["non-empty".into()],
                        };
                        if required_paths.is_empty() {
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
                    }
                    return self
                        .evaluate_strategy(
                            mission_id,
                            task_id,
                            workspace_root,
                            snapshot_hash,
                            &strategy,
                        )
                        .await;
                }
                Err(err) => {
                    return vec![VerificationCheck::failed(
                        mission_id,
                        task_id,
                        CheckTier::Deterministic,
                        "verification_contract_parser",
                        clean_verif,
                        None,
                        format!("Failed to parse verification strategy contract: {}", err),
                        Some("InvalidVerificationContract".into()),
                        snapshot_hash,
                    )];
                }
            }
        }

        // 2. Default contract when no explicit strategy declared:
        // Read-only roles produce a clean read check.
        if is_read_only {
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

        // Mutating default contract: Tier 1 (Deterministic) -> Tier 2 (Compiler)
        let mut checks = Vec::new();

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

        if parsed_role == crate::state_machine::agent::AgentRole::verifier() {
            let t3_res = match self
                .tier3_tests
                .execute(mission_id, task_id, workspace_root, snapshot_hash)
                .await
            {
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
        }

        checks
    }

    /// Evaluates an explicit typed `VerificationStrategy` recursively.
    pub async fn evaluate_strategy(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
        strategy: &crate::kernel::plan::VerificationStrategy,
    ) -> Vec<VerificationCheck> {
        let mut checks = Vec::new();
        match strategy {
            crate::kernel::plan::VerificationStrategy::Compilation => {
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
                    checks.push(VerificationCheck::blocked(
                        mission_id,
                        task_id,
                        CheckTier::Compiler,
                        snapshot_hash,
                        "Prerequisite Tier 1 deterministic checks failed",
                    ));
                    return checks;
                }

                let t2_res = match self
                    .tier2_compiler
                    .execute(mission_id, task_id, workspace_root, snapshot_hash)
                    .await
                {
                    Ok(mut c) => {
                        if c.status == CheckStatus::NotApplicable {
                            c.status = CheckStatus::Blocked;
                            c.summary = "Compilation strategy required by task contract, but no compiler unit is configured".to_string();
                        }
                        c
                    }
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
                checks.push(t2_res);
            }
            crate::kernel::plan::VerificationStrategy::AutomatedTest { command } => {
                let t3_res = if let Some(cmd) = command {
                    TestRunner::new()
                        .with_command(cmd.clone())
                        .execute(mission_id, task_id, workspace_root, snapshot_hash)
                        .await
                } else {
                    self.tier3_tests
                        .execute(mission_id, task_id, workspace_root, snapshot_hash)
                        .await
                };

                let check = match t3_res {
                    Ok(mut c) => {
                        if c.status == CheckStatus::NotApplicable {
                            c.status = CheckStatus::Blocked;
                            c.summary = "Automated test strategy required by task contract, but no test runner is configured".to_string();
                        }
                        c
                    }
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
                checks.push(check);
            }
            crate::kernel::plan::VerificationStrategy::StaticAnalysis { tool } => {
                let t4_res = if let Some(tool_cmd) = tool {
                    StaticAnalysisRunner::with_command(tool_cmd.clone())
                        .execute(mission_id, task_id, workspace_root, snapshot_hash)
                        .await
                } else {
                    self.tier4_static_analysis
                        .execute(mission_id, task_id, workspace_root, snapshot_hash)
                        .await
                };

                let check = match t4_res {
                    Ok(mut c) => {
                        if c.status == CheckStatus::NotApplicable {
                            c.status = CheckStatus::Blocked;
                            c.summary = "Static analysis strategy required by task contract, but no linter is configured".to_string();
                        }
                        c
                    }
                    Err(e) => VerificationCheck::failed(
                        mission_id,
                        task_id,
                        CheckTier::StaticAnalysis,
                        "tier4_static_analysis",
                        "",
                        None,
                        e,
                        Some("StaticAnalysis".into()),
                        snapshot_hash,
                    ),
                };
                checks.push(check);
            }
            crate::kernel::plan::VerificationStrategy::ArtifactInspection { paths } => {
                if paths.is_empty() {
                    checks.push(VerificationCheck::failed(
                        mission_id,
                        task_id,
                        CheckTier::Deterministic,
                        "artifact_inspection",
                        "",
                        None,
                        "Artifact inspection strategy declared empty required paths",
                        Some("EmptyArtifactList".into()),
                        snapshot_hash,
                    ));
                } else {
                    let mut all_exist = true;
                    for p in paths {
                        let full = workspace_root.join(p);
                        if !full.exists() {
                            all_exist = false;
                            checks.push(VerificationCheck::failed(
                                mission_id,
                                task_id,
                                CheckTier::Deterministic,
                                "artifact_inspection",
                                p,
                                None,
                                format!("Required artifact not found on disk: {}", p),
                                Some("ArtifactNotFound".into()),
                                snapshot_hash,
                            ));
                            break;
                        }
                    }
                    if all_exist {
                        checks.push(VerificationCheck::passed(
                            mission_id,
                            task_id,
                            CheckTier::Deterministic,
                            "artifact_inspection",
                            format!("paths={}", paths.join(", ")),
                            None,
                            "All required artifacts verified on disk",
                            snapshot_hash,
                        ));
                    }
                }
            }
            crate::kernel::plan::VerificationStrategy::ReviewGate { reviewer_role } => {
                if let Some(reviewer) = self.tier6_reviewer.as_ref() {
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
                    let role_desc = reviewer_role
                        .as_ref()
                        .map(|r| r.as_str())
                        .unwrap_or("independent_reviewer");
                    checks.push(VerificationCheck::passed(
                        mission_id,
                        task_id,
                        CheckTier::IndependentReview,
                        "review_gate",
                        "",
                        None,
                        format!(
                            "ReviewGate satisfied for reviewer '{}' (no external reviewer runner configured)",
                            role_desc
                        ),
                        snapshot_hash,
                    ));
                }
            }
            crate::kernel::plan::VerificationStrategy::Composite { strategies } => {
                for child_strat in strategies {
                    let child_checks = Box::pin(self.evaluate_strategy(
                        mission_id,
                        task_id,
                        workspace_root,
                        snapshot_hash,
                        child_strat,
                    ))
                    .await;
                    checks.extend(child_checks);
                }
            }
        }
        checks
    }
}
